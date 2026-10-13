use std::collections::VecDeque;
use std::convert::TryInto;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use log::{debug, error, trace, warn};
use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        randr::ConnectionExt as _,
        xinput::DeviceUse,
        xproto::{
            AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt as _, EventMask,
            GetKeyboardMappingReply, GetModifierMappingReply, InputFocus, Screen, Window,
        },
        xtest::ConnectionExt as _,
    },
    rust_connection::{ConnectError, ConnectionError, DefaultStream, ReplyError, RustConnection},
    wrapper::ConnectionExt as _,
};

use super::keymap::{Bind, FocusSync, KeyMap, Keysym};
use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse, NewConError,
};

type CompositorConnection = RustConnection<DefaultStream>;

pub type Keycode = u8;

const PING_TIMEOUT: Duration = Duration::from_secs(1);

static PINGS: AtomicU32 = AtomicU32::new(1);

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        WM_PROTOCOLS,
        _NET_WM_PING,
    }
}

pub struct Con {
    connection: CompositorConnection,
    screen: Screen,
    atoms: Atoms,
    keymap: KeyMap<Keycode>,
    modifiers: [Vec<Keycode>; 8],
}

/// Binds keysyms and syncs with the clients that got key events
struct Binder<'a> {
    connection: &'a CompositorConnection,
    root: Window,
    atoms: &'a Atoms,
}

/// Selects `SUBSTRUCTURE_NOTIFY` on the root window until dropped, to get the
/// answers to `_NET_WM_PING`
struct RootEvents<'a> {
    connection: &'a CompositorConnection,
    root: Window,
}

impl From<ConnectionError> for NewConError {
    fn from(error: ConnectionError) -> Self {
        // This should only be possible when trying to get the modifier map
        error!("{error:?}");
        Self::EstablishCon("failed to get the modifier map")
    }
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
        let (connection, screen_idx) = x11rb::connect(dpy_name)?;
        let setup = connection.setup();
        let screen = setup.roots[screen_idx].clone();
        let min_keycode = setup.min_keycode;
        let max_keycode = setup.max_keycode;
        let (keysyms_per_keycode, keysyms) =
            Self::get_keyboard_mapping(&connection, min_keycode, max_keycode)?; // Check if a mapping is possible
        let unused_keycodes =
            Self::unused_keycodes(min_keycode, max_keycode, keysyms_per_keycode, &keysyms); // Check if a mapping is possible

        if unused_keycodes.is_empty() {
            return Err(NewConError::NoEmptyKeycodes);
        }
        let keymap = KeyMap::new(
            min_keycode,
            max_keycode,
            unused_keycodes,
            keysyms_per_keycode,
            keysyms,
        );

        // Get the keycodes of the modifiers
        let modifiers = Self::find_modifier_keycodes(&connection)?;
        let atoms = Atoms::new(&connection)?.reply()?;

        Ok(Con {
            connection,
            screen,
            atoms,
            keymap,
            modifiers,
        })
    }

    /// Find keycodes that have not yet been mapped any keysyms
    fn get_keyboard_mapping(
        connection: &CompositorConnection,
        keycode_min: Keycode,
        keycode_max: Keycode,
    ) -> Result<(u8, Vec<u32>), ReplyError> {
        let GetKeyboardMappingReply {
            keysyms_per_keycode,
            keysyms,
            ..
        } = connection
            .get_keyboard_mapping(keycode_min, keycode_max - keycode_min + 1)?
            .reply()?;

        //let keysyms = keysyms.into_iter().map(|s| Keysym::from(s)).collect();
        Ok((keysyms_per_keycode, keysyms))
    }

    fn unused_keycodes(
        keycode_min: Keycode,
        keycode_max: Keycode,
        keysyms_per_keycode: u8,
        keysyms: &[u32],
    ) -> VecDeque<Keycode> {
        let mut unused_keycodes: VecDeque<Keycode> =
            VecDeque::with_capacity((keycode_max - keycode_min) as usize);

        // Split the mapping into the chunks of keysyms that are mapped to each keycode
        trace!("initial keymap:");
        let keysyms = keysyms.chunks(keysyms_per_keycode as usize);
        for (syms, kc) in keysyms.zip(keycode_min..=keycode_max) {
            // Check if the keycode is unused
            if log::log_enabled!(log::Level::Trace) {
                let syms_name: Vec<Keysym> = syms.iter().map(|&s| Keysym::from(s)).collect();
                trace!("{kc}:  {syms_name:?}");
            }

            // Never use keycode 8
            // Keycode 8 is special: when converted to evdev keycodes,
            // 8 is subtracted, resulting in 0. This typically leads to no effect
            // when simulating input because keycode 0 corresponds to NoSymbol,
            // meaning it has no assigned key mapping.
            if syms.iter().all(|&s| s == Keysym::NoSymbol.raw()) && kc != 8 {
                unused_keycodes.push_back(kc);
            }
        }
        debug!("unused keycodes: {unused_keycodes:?}");
        unused_keycodes
    }

    /// Find the keycodes that must be used for the modifiers
    fn find_modifier_keycodes(
        connection: &CompositorConnection,
    ) -> Result<[Vec<Keycode>; 8], ReplyError> {
        let modifier_reply = connection.get_modifier_mapping()?.reply()?;
        let keycodes_per_modifier = modifier_reply.keycodes_per_modifier() as usize;
        let GetModifierMappingReply {
            keycodes: modifiers,
            ..
        } = modifier_reply;

        let mut modifiers_array: [Vec<Keycode>; 8] = Default::default(); // Initialize with empty vectors
        let modifier_mapping = modifiers.chunks(keycodes_per_modifier);
        if modifier_mapping.len() > 8 {
            error!(
                "the associated keycodes of {} modifiers were returned! Only 8 were expected",
                modifier_mapping.len()
            );
            return Err(ReplyError::ConnectionError(ConnectionError::UnknownError));
        }
        for (mod_no, mod_keycodes) in modifier_mapping.enumerate() {
            let keycodes: Vec<_> = mod_keycodes.iter().copied().filter(|&kc| kc != 0).collect();
            if keycodes.is_empty() {
                warn!("modifier_no: {mod_no} is unmapped");
            }
            modifiers_array[mod_no] = keycodes;
        }
        debug!("the keycodes associated with the modifiers are:\n{modifiers_array:?}");

        Ok(modifiers_array)
    }

    /// Unmap the keysyms that were mapped to unused keycodes
    pub fn unmap_everything(&mut self) -> InputResult<()> {
        let (binder, keymap) = self.split();
        keymap.unmap_everything(&binder)
    }

    fn split(&mut self) -> (Binder<'_>, &mut KeyMap<Keycode>) {
        let Con {
            connection,
            screen,
            atoms,
            keymap,
            ..
        } = self;
        let binder = Binder {
            connection,
            root: screen.root,
            atoms,
        };
        (binder, keymap)
    }

    /// Wait until the X server handled all requests sent so far and get the
    /// window with the keyboard focus
    fn input_focus(&self) -> InputResult<Window> {
        self.connection
            .get_input_focus()
            .map_err(ReplyError::from)
            .and_then(x11rb::cookie::Cookie::reply)
            .map(|reply| reply.focus)
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate(
                    "error when syncing with X server using x11rb after xtest_fake_input was called",
                )
            })
    }

    // Get the device id of the first device that is found which has the same usage
    // as the input parameter
    fn device_id(&self, usage: DeviceUse) -> InputResult<u8> {
        x11rb::protocol::xinput::list_input_devices(&self.connection)
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when listing input devices with x11rb")
            })?
            .reply()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error with the reply from listing input devices with x11rb")
            })?
            .devices
            .iter()
            .find(|d| d.device_use == usage)
            .map_or_else(
                || {
                    Err(InputError::Simulate(
                        "error with the reply from listing input devices with x11rb",
                    ))
                },
                |d| Ok(d.device_id),
            )
    }
}

impl Drop for Con {
    fn drop(&mut self) {
        // Map all previously mapped keycodes to the NoSymbol keysym to revert all
        // changes
        debug!("x11rb connection was dropped");
        if let Err(e) = self.unmap_everything() {
            error!("unable to unmap the keycodes. {e:?}");
        }
    }
}

impl Bind<Keycode> for Binder<'_> {
    fn bind_key(&self, keycode: Keycode, keysym: Keysym) -> Result<(), ()> {
        // A list of two keycodes has to be mapped, otherwise the map is not what would
        // be expected If we would try to map only one keysym, we would get a
        // map that is tolower(keysym), toupper(keysym), tolower(keysym),
        // toupper(keysym), tolower(keysym), toupper(keysym), 0, 0, 0, 0, ...
        // https://stackoverflow.com/a/44334103
        self.connection
            .change_keyboard_mapping(1, keycode, 2, &[keysym.raw(), keysym.raw()])
            .map_err(|e| error!("error when changing the keyboard mapping with x11rb: {e:?}"))?;
        self.connection.sync().map_err(|e| error!("error when syncing with X server using x11rb after the keyboard mapping was changed: {e:?}"))
    }

    fn sync(&self, window: Window) -> FocusSync {
        self.ping(window).unwrap_or_else(|e| {
            warn!("unable to ping the client of window {window}: {e:?}");
            FocusSync::Unavailable
        })
    }
}

impl Binder<'_> {
    /// Send `_NET_WM_PING` to the client of `focus` and wait for the pong. A
    /// client answers it in order with its other events, so it has read all
    /// key events sent before once the pong arrives
    fn ping(&self, focus: Window) -> Result<FocusSync, ReplyError> {
        let Atoms {
            WM_PROTOCOLS: protocols,
            _NET_WM_PING: ping,
        } = *self.atoms;
        let Some(window) = self.ping_window(focus)? else {
            return Ok(FocusSync::Unavailable);
        };

        let stamp = PINGS.fetch_add(1, Ordering::Relaxed).max(1);
        let _events = RootEvents::select(self.connection, self.root)?;
        let event = ClientMessageEvent::new(32, window, protocols, [ping, stamp, window, 0, 0]);
        self.connection
            .send_event(false, window, EventMask::NO_EVENT, event)?;
        self.connection.flush()?;
        let deadline = Instant::now() + PING_TIMEOUT;
        while Instant::now() < deadline {
            match self.connection.poll_for_event()? {
                Some(Event::ClientMessage(event))
                    if event.type_ == protocols
                        && event.data.as_data32()[..3] == [ping, stamp, window] =>
                {
                    return Ok(FocusSync::CaughtUp);
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        warn!("window {window} did not answer a ping within {PING_TIMEOUT:?}");
        Ok(FocusSync::TimedOut)
    }

    /// Find the window that takes `_NET_WM_PING`: `focus` or its closest
    /// ancestor, if it belongs to the same client. The resource id bits
    /// outside `resource_id_mask` tell the client
    fn ping_window(&self, focus: Window) -> Result<Option<Window>, ReplyError> {
        let Atoms {
            WM_PROTOCOLS: protocols,
            _NET_WM_PING: ping,
        } = *self.atoms;
        if focus == x11rb::NONE || focus == u32::from(InputFocus::POINTER_ROOT) {
            return Ok(None);
        }
        let client = !self.connection.setup().resource_id_mask;
        let mut window = focus;
        while window != self.root && window & client == focus & client {
            let supported = self
                .connection
                .get_property(false, window, protocols, AtomEnum::ATOM, 0, 64)?
                .reply()?
                .value32()
                .is_some_and(|mut atoms| atoms.any(|atom| atom == ping));
            if supported {
                return Ok(Some(window));
            }
            window = self.connection.query_tree(window)?.reply()?.parent;
        }
        Ok(None)
    }
}

impl<'a> RootEvents<'a> {
    fn select(connection: &'a CompositorConnection, root: Window) -> Result<Self, ReplyError> {
        let listen = ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY);
        connection.change_window_attributes(root, &listen)?;
        Ok(Self { connection, root })
    }
}

impl Drop for RootEvents<'_> {
    fn drop(&mut self) {
        let quiet = ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT);
        let restored = self
            .connection
            .change_window_attributes(self.root, &quiet)
            .map_err(ReplyError::from)
            .and_then(|_| self.connection.sync());
        if let Err(e) = restored {
            error!("unable to stop listening to the root window: {e:?}");
            return;
        }
        while let Ok(Some(_)) = self.connection.poll_for_event() {}
    }
}

impl Keyboard for Con {
    fn fast_text(&mut self, _text: &str) -> InputResult<Option<()>> {
        Ok(None)
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        let (binder, keymap) = self.split();
        let keycode = keymap.key_to_keycode(&binder, key)?;

        if log::log_enabled!(log::Level::Debug) {
            for (mod_idx, mod_keycodes) in self.modifiers.iter().enumerate() {
                if mod_keycodes.contains(&keycode) {
                    debug!("the key is modifier no: {mod_idx}");
                }
            }
        }

        self.raw(keycode.into(), direction)
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        let Ok(keycode) = keycode.try_into() else {
            return Err(InputError::InvalidInput(
                "Keycode was too large. It has to fit in u8 on X11",
            ));
        };
        let time = x11rb::CURRENT_TIME; // CURRENT_TIME == 0
        let root = self.screen.root;
        let root_x = 0;
        let root_y = 0;
        let deviceid = self.device_id(DeviceUse::IS_X_KEYBOARD)?;
        let mut focus = x11rb::NONE;

        debug!("xtest_fake_input with keycode {keycode}, deviceid {deviceid}, time {time}");
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

            focus = self.input_focus()?;
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

            focus = self.input_focus()?;
        }

        // Let the keymap know that the key was held/no longer held
        // This is important to avoid unmapping held keys
        self.keymap.key(keycode, direction, focus);

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
        let deviceid = self.device_id(DeviceUse::IS_X_POINTER)?;

        debug!("xtest_fake_input with button {detail}, deviceid {deviceid}, time {time}");
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
        let deviceid = self.device_id(DeviceUse::IS_X_POINTER)?;

        debug!(
            "xtest_fake_input with coordinate {detail}, deviceid {deviceid}, x {root_x}, y {root_y}, time {time}"
        );

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
