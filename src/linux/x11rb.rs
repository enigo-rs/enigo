use std::convert::TryInto;
use std::ffi::CString;

use log::{debug, error, trace, warn};

use x11rb::{
    connection::{Connection, RequestConnection},
    protocol::{
        randr::ConnectionExt as _,
        xinput::DeviceUse,
        xkb::{ConnectionExt as _, EventType, ID, MapPart, SelectEventsAux, X11_EXTENSION_NAME},
        xproto::{ConnectionExt as _, Screen},
        xtest::ConnectionExt as _,
    },
    rust_connection::{ConnectError, ConnectionError, ReplyError},
    wrapper::ConnectionExt as _,
    xcb_ffi::XCBConnection,
};

use xkbcommon::xkb as xkbc;

use super::{keymap::Keysym, keymap2::Keymap2};
use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse, NewConError,
};

pub type Keycode = u8;

pub struct Con {
    connection: XCBConnection,
    screen: Screen,
    keymap: Keymap2,
    additionally_mapped: Vec<Keycode>,
    held_keycodes: Vec<Keycode>,                  // cannot get unmapped
    last_keys: Vec<Keycode>,                      // last pressed keycodes milliseconds
    last_event_before_delays: std::time::Instant, // time of the last event
    delay: u32,                                   // milliseconds
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
    /// * `delay` - Minimum delay in milliseconds between key presses in order
    ///   to properly enter all chars
    /// * `dpy_name` - If no `dpy_name` is provided, the value from $DISPLAY is
    ///   used
    ///
    /// # Errors
    /// TODO
    pub fn new(dpy_name: Option<&str>, delay: u32) -> Result<Con, NewConError> {
        debug!("using x11rb");

        let dpy_name = dpy_name
            .map(|name| {
                CString::new(name).map_err(|_| {
                    NewConError::EstablishCon("the display name contained a null byte")
                })
            })
            .transpose()?;

        let (connection, screen_idx) = XCBConnection::connect(dpy_name.as_deref())?;

        connection.prefetch_extension_information(X11_EXTENSION_NAME)?;
        let setup = connection.setup();

        let xkb = connection.xkb_use_extension(1, 0)?;
        let xkb = xkb.reply()?;
        assert!(
            xkb.supported,
            "This program requires the X11 server to support the XKB extension"
        );

        let screen = setup.roots[screen_idx].clone();

        // Ask the X11 server to send us XKB events.
        // TODO: No idea what to pick here. I guess this is asking unnecessarily for too
        // much?
        let events =
            EventType::NEW_KEYBOARD_NOTIFY | EventType::MAP_NOTIFY | EventType::STATE_NOTIFY;
        // TODO: No idea what to pick here. I guess this is asking unnecessarily for too
        // much?
        let map_parts = MapPart::KEY_TYPES
            | MapPart::KEY_SYMS
            | MapPart::MODIFIER_MAP
            | MapPart::EXPLICIT_COMPONENTS
            | MapPart::KEY_ACTIONS
            | MapPart::KEY_BEHAVIORS
            | MapPart::VIRTUAL_MODS
            | MapPart::VIRTUAL_MOD_MAP;
        connection.xkb_select_events(
            ID::USE_CORE_KBD.into(),
            0u8.into(),
            events,
            map_parts,
            map_parts,
            &SelectEventsAux::new(),
        )?;

        let keymap = {
            // Set up xkbcommon state and get the current keymap.
            let context = xkbc::Context::new(xkbc::CONTEXT_NO_FLAGS);
            let device_id = xkbc::x11::get_core_keyboard_device_id(&connection);
            if device_id < 0 {
                return Err(NewConError::EstablishCon("getting the device id failed"));
            };
            let keymap = xkbc::x11::keymap_new_from_device(
                &context,
                &connection,
                device_id,
                xkbc::KEYMAP_COMPILE_NO_FLAGS,
            );

            let format = xkbcommon::xkb::KEYMAP_FORMAT_TEXT_V1;
            let original_keymap = keymap.get_as_string(format);
            let state = xkbc::x11::state_new_from_device(&keymap, &connection, device_id);

            Keymap2::new(context, original_keymap, keymap, state)
                .map_err(|_| NewConError::EstablishCon("unable to create keymap"))?
        };

        let last_event_before_delays = std::time::Instant::now();

        Ok(Con {
            connection,
            screen,
            keymap,
            additionally_mapped: vec![],
            held_keycodes: vec![],
            delay,
            last_event_before_delays,
            last_keys: Vec::with_capacity(64),
        })
    }

    /// Get the delay per keypress in milliseconds
    #[must_use]
    pub fn delay(&self) -> u32 {
        self.delay
    }

    /// Set the delay in milliseconds per keypress
    pub fn set_delay(&mut self, delay: u32) {
        self.delay = delay;
    }

    // Get the pending delay
    // TODO: A delay of 1 ms in all cases seems to work on my machine. Maybe
    // this is not needed?
    pub fn get_pending_delay(&mut self, keycode: Keycode) -> u32 {
        // Check if a delay is needed
        // A delay is required, if one of the keycodes was recently entered and there
        // was no delay between it

        // e.g. A quick rabbit
        // Chunk 1: 'A quick' # Add a delay before the second space
        // Chunk 2: ' rab'     # Add a delay before the second 'b'
        // Chunk 3: 'bit'     # Enter the remaining chars

        // In order to not grow the list of last pressed keys too big, we also clear it
        // when it gets longer than 64 items 64 was chosen arbitrarily
        let pending_delay = if self.last_keys.contains(&keycode) || self.last_keys.len() > 64 {
            let elapsed_ms = self
                .last_event_before_delays
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u32::MAX);
            trace!("delay needed");
            self.last_keys.clear();
            self.delay.saturating_sub(elapsed_ms)
        } else {
            trace!("no delay needed");
            1 // TODO: Try out 0 here. If 0 does not work, the other arm of the if statement should also get changed to have a minimum delay of 1
        };
        self.last_keys.push(keycode);
        pending_delay
    }

    // Get the device id of the first device that is found which has the same usage
    // as the input parameter
    // TODO: Should this get replaced with xkbc::x11::get_core_keyboard_device_id?
    fn device_id(&self, usage: DeviceUse) -> InputResult<u8> {
        x11rb::protocol::xinput::list_input_devices(&self.connection)
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when listing input devices with x11rb: {e:?}")
            })?
            .reply()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate(
                    "error with the reply from listing input devices with x11rb: {e:?}",
                )
            })?
            .devices
            .iter()
            .find(|d| d.device_use == usage)
            .map_or_else(
                || {
                    Err(InputError::Simulate(
                        "error with the reply from listing input devices with x11rb: {e:?}",
                    ))
                },
                |d| Ok(d.device_id),
            )
    }

    /// Press/Release a keycode
    ///
    /// # Errors
    /// TODO
    fn send_key_event(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        let Ok(keycode) = keycode.try_into() else {
            return Err(InputError::InvalidInput(
                "Keycode was too large. It has to fit in u8 on X11",
            ));
        };

        let time = self.get_pending_delay(keycode);
        let root = self.screen.root;
        let deviceid = self.device_id(DeviceUse::IS_X_KEYBOARD)?;
        let direction = match direction {
            Direction::Press => x11rb::protocol::xproto::KEY_PRESS_EVENT,
            Direction::Release => x11rb::protocol::xproto::KEY_RELEASE_EVENT,
            Direction::Click => {
                error!(
                    "Implementation error! This function must never be called with Direction::Click"
                );
                return Err(InputError::Simulate(
                    "error when using xtest_fake_input with x11rb",
                ));
            }
        };

        debug!("xtest_fake_input with keycode {keycode}, deviceid {deviceid}, delay {time}");
        self.connection
            .xtest_fake_input(direction, keycode, time, root, 0, 0, deviceid)
            .map_err(|e| {
                error!("error when using xtest_fake_input with x11rb:\n{e}");
                InputError::Simulate("error when using xtest_fake_input with x11rb")
            })?;

        self.last_event_before_delays = std::time::Instant::now();
        Ok(())
    }

    fn map_key(&mut self, key: Key) -> InputResult<u16> {
        let keysym = Keysym::from(key);
        let new_keycode = self.keymap.map_key(key, false)?;
        let new_keycode_u8 = new_keycode.try_into().unwrap(); // This is safe, because the previous function only returns a keycode <255

        // A list of two keycodes has to be mapped, otherwise the map is not what would
        // be expected. If we would try to map only one keysym, we would get a
        // map that is tolower(keysym), toupper(keysym), tolower(keysym),
        // toupper(keysym), tolower(keysym), toupper(keysym), 0, 0, 0, 0, ...
        // https://stackoverflow.com/a/44334103
        self.connection
            .change_keyboard_mapping(1, new_keycode_u8, 2, &[keysym.raw(), keysym.raw()])
            .map_err(|e| {
                error!("error when changing the keyboard mapping with x11rb: {e:?}");
                InputError::Mapping(
                    "error when changing the keyboard mapping with x11rb".to_string(),
                )
            })?;
        self.connection.sync().map_err(|e| {error!("error when syncing with X server using x11rb after the keyboard mapping was changed: {e:?}");InputError::Mapping("unable to sync with X11 server".to_string())})?;
        self.additionally_mapped.push(new_keycode_u8);
        Ok(new_keycode)
    }

    fn unmap_keycode(&mut self, keycode: Keycode) -> InputResult<()> {
        let Some((map_idx, _)) = self
            .additionally_mapped
            .iter()
            .enumerate()
            .find(|&(_, v)| *v == keycode)
        else {
            warn!("the keycode was not mapped");
            return Ok(());
        };
        let keysym = Keysym::NoSymbol;
        // A list of two keycodes has to be mapped, otherwise the map is not what would
        // be expected If we would try to map only one keysym, we would get a
        // map that is tolower(keysym), toupper(keysym), tolower(keysym),
        // toupper(keysym), tolower(keysym), toupper(keysym), 0, 0, 0, 0, ...
        // https://stackoverflow.com/a/44334103
        self.connection
            .change_keyboard_mapping(1, keycode, 2, &[keysym.raw(), keysym.raw()])
            .map_err(|e| {
                error!("error when changing the keyboard mapping with x11rb: {e:?}");
                InputError::Mapping(
                    "error when changing the keyboard mapping with x11rb".to_string(),
                )
            })?;
        self.connection.sync().map_err(|e| {error!("error when syncing with X server using x11rb after the keyboard mapping was changed: {e:?}");InputError::Mapping("unable to sync with X11 server".to_string())})?;
        self.additionally_mapped.swap_remove((map_idx).into());
        Ok(())
    }

    /// Unmap all the additional mappings
    fn unmap_everything(&mut self) -> InputResult<()> {
        let additionally_mapped = self.additionally_mapped.clone();
        let held_keycodes = self.held_keycodes.clone();
        for &keycode in additionally_mapped
            .iter()
            .filter(|keycode| !held_keycodes.contains(keycode))
        {
            self.unmap_keycode(keycode)?
        }
        Ok(())
    }
}

impl Drop for Con {
    fn drop(&mut self) {
        // Map all previously mapped keycodes to the NoSymbol keysym to revert all
        // changes
        debug!("x11rb connection was dropped");
        let _ = self.unmap_everything();
        debug!("Original keymap was restored");
    }
}

impl Keyboard for Con {
    fn fast_text(&mut self, _text: &str) -> InputResult<Option<()>> {
        warn!("fast text entry is not possible on X11");
        Ok(None)
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        let keycode = if let Some(keycode) = self.keymap.key_to_keycode(key) {
            keycode
        } else {
            debug!("keycode for key {key:?} was not found");
            let mapping_res = self.map_key(key);
            let keycode = match mapping_res {
                Err(InputError::Mapping(_)) => {
                    // Unmap and retry
                    self.unmap_everything()?;
                    self.map_key(key)?
                }

                Ok(keycode) => keycode,
                _ => return Err(InputError::Mapping("unable to map the key".to_string())),
            };

            keycode
        };

        self.raw(keycode.into(), direction)
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        if direction == Direction::Press || direction == Direction::Click {
            self.keymap
                .update_key_state(xkbc::Keycode::new(keycode.into()), xkbc::KeyDirection::Down);
            self.send_key_event(keycode, Direction::Press)?;
        }

        // TODO: Check if we need to update the delays again
        // self.keymap.update_delays(keycode);

        if direction == Direction::Release || direction == Direction::Click {
            self.keymap
                .update_key_state(xkbc::Keycode::new(keycode.into()), xkbc::KeyDirection::Up);
            self.send_key_event(keycode, Direction::Release)?;
        }

        self.connection.sync()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after the keyboard mapping was changed: {e:?}")
            })?;

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
        let time = self.delay;
        let root = self.screen.root;
        let root_x = 0;
        let root_y = 0;
        let deviceid = self.device_id(DeviceUse::IS_X_POINTER)?;

        debug!("xtest_fake_input with button {detail}, deviceid {deviceid}, delay {time}");
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
                    InputError::Simulate("error when using xtest_fake_input with x11rb: {e:?}")
                })?;
        }
        if direction == Direction::Release || direction == Direction::Click {
            // Add a delay for the release part of a click
            // TODO: Maybe calculate here if a delay is needed as well
            if direction == Direction::Click {
                // time = DEFAULT_DELAY;
            }

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
                    InputError::Simulate("error when using xtest_fake_input with x11rb: {e:?}")
                })?;
        }
        self.connection.sync()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after the keyboard mapping was changed: {e:?}")
            })?;
        Ok(())
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        let type_ = x11rb::protocol::xproto::MOTION_NOTIFY_EVENT;
        let detail = match coordinate {
            Coordinate::Rel => 1,
            Coordinate::Abs => 0,
        };
        let time = x11rb::CURRENT_TIME;
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
            "xtest_fake_input with coordinate {detail}, deviceid {deviceid}, x {root_x}, y {root_y}, delay {time}"
        );

        self.connection
            .xtest_fake_input(type_, detail, time, root, root_x, root_y, deviceid) // TODO: Check if using x11rb::protocol::xproto::warp_pointer would be better
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when using xtest_fake_input with x11rb: {e:?}")
            })?;
        self.connection.sync()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after the keyboard mapping was changed: {e:?}")
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
                InputError::Simulate(
                    "error when requesting randr_get_screen_resources with x11rb: {e:?}",
                )
            })?
            .reply()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate(
                    "error with the reply of randr_get_screen_resources with x11rb: {e:?}",
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
                InputError::Simulate("error when requesting query_pointer with x11rb: {e:?}")
            })?
            .reply()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error with the reply of query_pointer with x11rb: {e:?}")
            })?;
        Ok((reply.root_x as i32, reply.root_y as i32))
    }
}
