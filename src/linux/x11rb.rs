use std::convert::TryInto;
use std::ffi::CString;
use std::time::Duration;

use log::{debug, error, trace};
use x11rb::{
    connection::Connection,
    errors::{ConnectError, ReplyError},
    protocol::{
        Event,
        randr::ConnectionExt as _,
        xkb::{self as xkb_ext, ID},
        xproto::{ConnectionExt as _, Mapping, Screen},
        xtest::ConnectionExt as _,
    },
    wrapper::ConnectionExt as _,
    xcb_ffi::XCBConnection,
};
use xkbcommon::xkb;

use super::xkb::Keymap;
use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse, NewConError,
};

type CompositorConnection = XCBConnection;

/// How long a dynamically mapped keycode must be idle before it is reused. A
/// client may still have earlier key events for that keycode in its queue and
/// would translate them with the new binding, so we give it time to catch up.
const MIN_REUSE_DELAY: Duration = Duration::from_millis(150);

/// XTEST's `deviceid` for the core device. The value 0 is the protocol's
/// "core device" sentinel; the server ignores the field for core event types
/// (`PickKeyboard`/`PickPointer` choose the core device), so it is always safe
/// to use.
const XTEST_CORE_DEVICE: u8 = 0;

pub struct Con {
    connection: CompositorConnection,
    screen: Screen,
    keymap: Keymap,
    keysyms_per_keycode: u8,
}

impl From<ConnectError> for NewConError {
    fn from(error: ConnectError) -> Self {
        error!("{error:?}");
        Self::EstablishCon("failed to establish the connection")
    }
}
impl From<ReplyError> for NewConError {
    fn from(error: ReplyError) -> Self {
        error!("{error:?}");
        Self::Reply
    }
}
impl Con {
    /// Tries to establish a new X11 connection using the specified parameters
    ///
    /// # Arguments
    ///
    /// * `dpy_name` - If no `dpy_name` is provided, the value from $DISPLAY is
    ///   used
    ///
    /// # Errors
    /// TODO
    pub fn new(dpy_name: Option<&str>) -> Result<Con, NewConError> {
        debug!("using x11rb");
        let dpy_name = dpy_name
            .map(CString::new)
            .transpose()
            .map_err(|_| NewConError::EstablishCon("the display name contains a NUL byte"))?;
        let (connection, screen_idx) = XCBConnection::connect(dpy_name.as_deref())?;
        let setup = connection.setup();
        let screen = setup.roots[screen_idx].clone();

        Self::setup_xkb(&connection)?;
        let (keymap, keysyms_per_keycode) = Self::fetch_keymap(&connection)
            .map_err(|()| NewConError::EstablishCon("failed to fetch the X11 keymap"))?;

        Ok(Con {
            connection,
            screen,
            keymap,
            keysyms_per_keycode,
        })
    }

    /// Enable the XKB extension, which `xkbcommon-x11` needs before it can
    /// fetch the keymap from the server.
    fn setup_xkb(connection: &CompositorConnection) -> Result<(), NewConError> {
        let mut major = 0;
        let mut minor = 0;
        let mut base_event = 0;
        let mut base_error = 0;
        let supported = xkb::x11::setup_xkb_extension(
            connection,
            xkb::x11::MIN_MAJOR_XKB_VERSION,
            xkb::x11::MIN_MINOR_XKB_VERSION,
            xkb::x11::SetupXkbExtensionFlags::NoFlags,
            &mut major,
            &mut minor,
            &mut base_event,
            &mut base_error,
        );
        if !supported {
            error!("the X server does not support the XKB extension");
            return Err(NewConError::EstablishCon(
                "the X server does not support the XKB extension",
            ));
        }
        // The server picks the compatible version; 1.0 is the minimum the
        // library needs, and every X server since X11R6.1 supports it.
        debug!(
            "XKB extension negotiated: {major}.{minor} (event base {base_event}, error base {base_error})"
        );

        // `xkbcommon-x11` only sends `XkbUseExtension`, which marks us as an
        // XKB client. Servers then stop sending the core `MappingNotify` to
        // clients that did not select the corresponding map parts, so select
        // them here. Otherwise `drain_events` would never notice an external
        // keymap change.
        let map_parts = xkb_ext::MapPart::KEY_SYMS | xkb_ext::MapPart::MODIFIER_MAP;
        xkb_ext::select_events(
            connection,
            u16::from(ID::USE_CORE_KBD),
            xkb_ext::EventType::default(),
            xkb_ext::EventType::MAP_NOTIFY,
            map_parts,
            map_parts,
            &xkb_ext::SelectEventsAux::default(),
        )
        .map_err(|e| {
            error!("unable to select XKB map events: {e:?}");
            NewConError::EstablishCon("unable to select XKB map events")
        })?
        .check()
        .map_err(|e| {
            error!("unable to select XKB map events: {e:?}");
            NewConError::EstablishCon("unable to select XKB map events")
        })?;

        Ok(())
    }

    /// Fetch the real XKB keymap from the server. It contains the layout
    /// groups, key types and compatibility map that the core protocol cannot
    /// represent.
    fn fetch_keymap(connection: &CompositorConnection) -> Result<(Keymap, u8), ()> {
        let keysyms_per_keycode = Self::keysyms_per_keycode(connection).map_err(|e| {
            error!("unable to read the keyboard mapping: {e:?}");
        })?;
        let device_id = xkb::x11::get_core_keyboard_device_id(connection);
        // XKB predates XInput2, so a device ID has to fit in 7 bits.
        if !(0..=127).contains(&device_id) {
            error!("no usable core keyboard device (XKB device id {device_id})");
            return Err(());
        }
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::x11::keymap_new_from_device(
            &context,
            connection,
            device_id,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        );
        Ok((
            Keymap::from_keymap(context, keymap, MIN_REUSE_DELAY),
            keysyms_per_keycode,
        ))
    }

    /// The width of the server's core keysym map. A dynamically changed keycode
    /// is pushed with this many columns so the map width does not shrink.
    fn keysyms_per_keycode(connection: &CompositorConnection) -> Result<u8, ReplyError> {
        let setup = connection.setup();
        let reply = connection
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)?
            .reply()?;
        Ok(reply.keysyms_per_keycode)
    }

    /// Apply a single dynamically changed keycode to the X server. Only the
    /// keycode that `map_key` just changed is pushed, so pending key events for
    /// unrelated keycodes keep their original mapping.
    ///
    /// A dynamic mapping writes `keysym` to every level, so every X11 core
    /// column gets the same keysym; there is nothing to read back from the
    /// keymap. Because all columns are identical, the server detects a
    /// replicated mapping and collapses the key to a single group that applies
    /// to every layout and level, so the key works regardless of the active
    /// group (`XkbGetEffectiveGroup` forces a one-group key to group 1).
    fn push_keycode(&mut self, keycode: u16, keysym: xkb::Keysym) -> InputResult<()> {
        let Ok(keycode) = keycode.try_into() else {
            return Err(InputError::InvalidInput(
                "Keycode was too large. It has to fit in u8 on X11",
            ));
        };
        let keysyms = vec![keysym.raw(); self.keysyms_per_keycode as usize];

        self.connection
            .change_keyboard_mapping(1, keycode, self.keysyms_per_keycode, &keysyms)
            .map_err(|e| {
                error!("error when changing the keyboard mapping with x11rb: {e:?}");
                InputError::Simulate("error when changing the keyboard mapping with x11rb")
            })?;
        self.connection.sync().map_err(|e| {
            error!("error when syncing with X server after changing the keyboard mapping: {e:?}");
            InputError::Simulate(
                "error when syncing with X server after changing the keyboard mapping",
            )
        })?;

        // Discard the `MappingNotify` our own request produced so it is not
        // mistaken for an external keymap change.
        self.drain_events(Some(keycode))?;
        Ok(())
    }

    /// Rebuild the keymap from the server after an external `MappingNotify`.
    /// Our dynamic mappings are dropped: the external change already replaced
    /// them, and the fresh keymap has no `additionally_mapped` bookkeeping.
    fn rebuild_keymap(&mut self) -> InputResult<()> {
        let (keymap, keysyms_per_keycode) = Self::fetch_keymap(&self.connection)
            .map_err(|()| InputError::Simulate("failed to fetch the refreshed X11 keymap"))?;
        self.keysyms_per_keycode = keysyms_per_keycode;
        self.keymap.replace_with(keymap);
        Ok(())
    }

    /// Drain pending events, handling `MappingNotify`. A keyboard mapping
    /// notify for `own_keycode` is our own request and is discarded; every
    /// other keyboard or modifier mapping change triggers a rebuild.
    ///
    /// Rebuilds are coalesced into one per drain: the server sends one
    /// `MappingNotify` per request, so a burst of changes (for example a
    /// remapping daemon) would otherwise fetch the whole keymap once per
    /// event. A notify is only sent after the server applied the change, so a
    /// single fetch after the queue is empty reflects the final state.
    fn drain_events(&mut self, own_keycode: Option<u8>) -> InputResult<()> {
        let mut keymap_changed = false;
        while let Some(event) = self.connection.poll_for_event().map_err(|e| {
            error!("unable to poll for X11 events: {e:?}");
            InputError::Simulate("unable to poll for X11 events")
        })? {
            let Event::MappingNotify(mapping) = event else {
                continue;
            };
            if mapping.request == Mapping::KEYBOARD
                && own_keycode
                    .is_some_and(|keycode| mapping.first_keycode == keycode && mapping.count == 1)
            {
                continue;
            }
            match mapping.request {
                Mapping::KEYBOARD | Mapping::MODIFIER => keymap_changed = true,
                _ => {}
            }
        }
        if keymap_changed {
            self.rebuild_keymap()?;
        }
        Ok(())
    }

    /// Mirror the server's keyboard state into the shared keymap before a
    /// lookup. The server is the source of truth: it also reflects modifier
    /// and layout changes made by the physical keyboard or by other clients,
    /// which the client-side `update_key` tracking cannot observe. Without
    /// this, lookups would use the wrong shift level after such a change.
    fn sync_state(&mut self) -> InputResult<()> {
        let reply = xkb_ext::get_state(&self.connection, u16::from(ID::USE_CORE_KBD))
            .map_err(|e| {
                error!("unable to query the XKB state: {e:?}");
                InputError::Simulate("unable to query the XKB state")
            })?
            .reply()
            .map_err(|e| {
                error!("unable to read the XKB state reply: {e:?}");
                InputError::Simulate("unable to read the XKB state reply")
            })?;
        // Only the effective group is needed for lookups; mirroring it as the
        // base layout is enough and avoids the signed base/latched group
        // fields.
        self.keymap.update_state(
            u32::from(reply.base_mods),
            u32::from(reply.latched_mods),
            u32::from(reply.locked_mods),
            u32::from(reply.group),
            0,
            0,
        );
        Ok(())
    }
}

impl Drop for Con {
    fn drop(&mut self) {
        debug!("x11rb connection was dropped");
        // Unmap only the keycodes we dynamically mapped, leaving every other
        // keycode untouched. Those keycodes were free (all-NoSymbol) before we
        // mapped them, so restoring them means sending NoSymbol for every
        // column.
        let no_symbols = vec![0u32; self.keysyms_per_keycode as usize];
        for keycode in self.keymap.mapped_keycodes().collect::<Vec<_>>() {
            let Ok(keycode) = u8::try_from(keycode.raw()) else {
                error!(
                    "mapped keycode {} exceeds u8; skipping restore",
                    keycode.raw()
                );
                continue;
            };
            if let Err(e) = self.connection.change_keyboard_mapping(
                1,
                keycode,
                self.keysyms_per_keycode,
                &no_symbols,
            ) {
                error!("unable to unmap keycode {keycode} on drop: {e:?}");
            }
        }
        let _ = self.connection.sync();
    }
}

impl Keyboard for Con {
    fn fast_text(&mut self, _text: &str) -> InputResult<Option<()>> {
        Ok(None)
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        // Handle any pending external keymap changes and mirror the current
        // physical modifier/layout state before looking up the key.
        self.drain_events(None)?;
        self.sync_state()?;

        let keycode = if let Some(keycode) = self.keymap.key_to_keycode(key) {
            keycode
        } else {
            debug!("keycode for key {key:?} was not found; adding it to the keymap");
            let keycode = self.keymap.map_key(key)?;
            self.push_keycode(keycode, xkb::Keysym::from(key))?;
            keycode
        };
        self.raw(keycode, direction)
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        let Ok(keycode) = u8::try_from(keycode) else {
            return Err(InputError::InvalidInput(
                "Keycode was too large. It has to fit in u8 on X11",
            ));
        };
        let xkb_keycode = xkb::Keycode::new(u32::from(keycode));
        let time = x11rb::CURRENT_TIME; // CURRENT_TIME == 0
        let root = self.screen.root;
        let root_x = 0;
        let root_y = 0;
        let deviceid = XTEST_CORE_DEVICE;

        debug!("xtest_fake_input with keycode {keycode}, time {time}");

        // Mirror each event only after the server processed it, so the state
        // and the held-key set follow what actually happened. A click is a
        // press followed by a release; toggles (CapsLock, group switches, ...)
        // change the state on the press and keep it after the release.
        if direction == Direction::Press || direction == Direction::Click {
            self.connection
                .xtest_fake_input(
                    x11rb::protocol::xproto::KEY_PRESS_EVENT,
                    keycode,
                    time,
                    root,
                    root_x,
                    root_y,
                    deviceid,
                )
                .map_err(|e| {
                    error!("{e}");
                    InputError::Simulate("error when using xtest_fake_input with x11rb")
                })?;
            trace!("press");

            self.connection.sync().map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after xtest_fake_input was called")
            })?;

            let _ = self.keymap.update_key(xkb_keycode, xkb::KeyDirection::Down);
            self.keymap.record_use(xkb_keycode);
        }

        if direction == Direction::Release || direction == Direction::Click {
            self.connection
                .xtest_fake_input(
                    x11rb::protocol::xproto::KEY_RELEASE_EVENT,
                    keycode,
                    time,
                    root,
                    root_x,
                    root_y,
                    deviceid,
                )
                .map_err(|e| {
                    error!("{e}");
                    InputError::Simulate("error when using xtest_fake_input with x11rb")
                })?;
            trace!("released");

            self.connection.sync().map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after xtest_fake_input was called")
            })?;

            let _ = self.keymap.update_key(xkb_keycode, xkb::KeyDirection::Up);
        }

        Ok(())
    }
}

impl Mouse for Con {
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
        let detail = match button {
            Button::Left => 1,
            Button::Middle => 2,
            Button::Right => 3,
            Button::ScrollUp => 4,
            Button::ScrollDown => 5,
            Button::ScrollLeft => 6,
            Button::ScrollRight => 7,
            Button::Back => 8,
            Button::Forward => 9,
        };
        let time = x11rb::CURRENT_TIME; // CURRENT_TIME == 0
        let root = self.screen.root;
        let root_x = 0;
        let root_y = 0;
        let deviceid = XTEST_CORE_DEVICE;

        debug!("xtest_fake_input with button {detail}, time {time}");
        if direction == Direction::Press || direction == Direction::Click {
            self.connection
                .xtest_fake_input(
                    x11rb::protocol::xproto::BUTTON_PRESS_EVENT,
                    detail,
                    time,
                    root,
                    root_x,
                    root_y,
                    deviceid,
                )
                .map_err(|e| {
                    error!("{e}");
                    InputError::Simulate("error when using xtest_fake_input with x11rb")
                })?;

            self.connection.sync()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after xtest_fake_input was called")
            })?;
        }
        if direction == Direction::Release || direction == Direction::Click {
            self.connection
                .xtest_fake_input(
                    x11rb::protocol::xproto::BUTTON_RELEASE_EVENT,
                    detail,
                    time,
                    root,
                    root_x,
                    root_y,
                    deviceid,
                )
                .map_err(|e| {
                    error!("{e}");
                    InputError::Simulate("error when using xtest_fake_input with x11rb")
                })?;

            self.connection.sync()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after xtest_fake_input was called")
            })?;
        }
        Ok(())
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        let type_ = x11rb::protocol::xproto::MOTION_NOTIFY_EVENT;
        let detail = match coordinate {
            Coordinate::Rel => 1,
            Coordinate::Abs => 0,
        };
        let time = x11rb::CURRENT_TIME; // CURRENT_TIME == 0
        let root = x11rb::NONE; //  the root window of the screen the pointer is currently on

        let Ok(root_x) = x.try_into() else {
            return Err(InputError::InvalidInput(
                "the coordinates cannot be negative and must fit in i16",
            ));
        };
        let Ok(root_y) = y.try_into() else {
            return Err(InputError::InvalidInput(
                "the coordinates cannot be negative and must fit in i16",
            ));
        };
        let deviceid = XTEST_CORE_DEVICE;

        debug!("xtest_fake_input with coordinate {detail}, x {root_x}, y {root_y}, time {time}");

        self.connection
            .xtest_fake_input(type_, detail, time, root, root_x, root_y, deviceid) // TODO: Check if using x11rb::protocol::xproto::warp_pointer would be better
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when using xtest_fake_input with x11rb")
            })?;
        self.connection.sync()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after the keyboard mapping was changed")
            })?;
        Ok(())
    }

    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        let button = match (length.is_positive(), axis) {
            (true, Axis::Vertical) => Button::ScrollDown,
            (false, Axis::Vertical) => Button::ScrollUp,
            (true, Axis::Horizontal) => Button::ScrollRight,
            (false, Axis::Horizontal) => Button::ScrollLeft,
        };

        for _ in 0..length.abs() {
            self.button(button, Direction::Click)?;
        }

        Ok(())
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        let main_display = self
            .connection
            .randr_get_screen_resources(self.screen.root)
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when requesting randr_get_screen_resources with x11rb")
            })?
            .reply()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate(
                    "error with the reply of randr_get_screen_resources with x11rb",
                )
            })?
            .modes[0];

        Ok((main_display.width as i32, main_display.height as i32))
    }

    fn location(&self) -> InputResult<(i32, i32)> {
        let reply = self
            .connection
            .query_pointer(self.screen.root)
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when requesting query_pointer with x11rb")
            })?
            .reply()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error with the reply of query_pointer with x11rb")
            })?;
        Ok((reply.root_x as i32, reply.root_y as i32))
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::ConnectionExt as _;
    use x11rb::wrapper::ConnectionExt as _;

    use super::{Con, Event, Mapping, XCBConnection};

    /// Regression test for the `MappingNotify` selection in `setup_xkb`.
    ///
    /// `xkbcommon-x11` only sends `XkbUseExtension`, which marks the
    /// connection as an XKB client. Servers then suppress the core
    /// `MappingNotify` unless the client selected the corresponding map parts.
    /// If that selection regresses, `drain_events` silently stops noticing
    /// external keymap changes.
    #[test]
    #[serial(input)]
    fn receives_mapping_notify_after_an_external_keymap_change() {
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }

        let con = Con::new(None).expect("x11 connection");
        let (other, _) = XCBConnection::connect(None).expect("second x11 connection");

        // A keycode in the middle of the core range, away from the keys the
        // other tests press.
        const KEYCODE: u8 = 200;

        let original = other
            .get_keyboard_mapping(KEYCODE, 1)
            .expect("request the original mapping")
            .reply()
            .expect("read the original mapping");
        let width = original.keysyms_per_keycode;

        // Drop anything setup left in the queue so the assertion below can only
        // be satisfied by the change we are about to make.
        while con.connection.poll_for_event().expect("poll").is_some() {}

        let keysyms = vec![0x1008_FFu32; width as usize];
        other
            .change_keyboard_mapping(1, KEYCODE, width, &keysyms)
            .expect("change the mapping");
        other.sync().expect("sync the other connection");
        con.connection.sync().expect("sync the enigo connection");

        let mut saw = false;
        while let Some(event) = con.connection.poll_for_event().expect("poll") {
            if let Event::MappingNotify(mapping) = event {
                let end = u16::from(mapping.first_keycode) + u16::from(mapping.count);
                if mapping.request == Mapping::KEYBOARD
                    && u16::from(mapping.first_keycode) <= u16::from(KEYCODE)
                    && u16::from(KEYCODE) < end
                {
                    saw = true;
                    break;
                }
            }
        }
        assert!(saw, "the connection did not receive MappingNotify");

        // Restore the keycode for the other live tests.
        other
            .change_keyboard_mapping(1, KEYCODE, width, &original.keysyms)
            .expect("restore the mapping");
        let _ = other.sync();
    }
}
