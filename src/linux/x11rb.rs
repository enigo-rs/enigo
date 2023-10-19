use std::collections::VecDeque;
use std::convert::TryInto;

use log::{debug, error, warn};
use x11rb::{
    connection::Connection,
    protocol::{
        randr::ConnectionExt as _,
        xinput::DeviceUse,
        xproto::{ConnectionExt as _, GetKeyboardMappingReply, Screen},
        xtest::ConnectionExt as _,
    },
    rust_connection::{ConnectError, DefaultStream, ReplyError, RustConnection},
    wrapper::ConnectionExt as _,
};

use super::keymap::{Bind, KeyMap, Keysym, NO_SYMBOL};
use crate::{
    Axis, Coordinate, Direction, InputError, InputResult, Key, KeyboardControllableNext,
    MouseButton, MouseControllableNext, NewConError,
};

type CompositorConnection = RustConnection<DefaultStream>;

pub type Keycode = u8;

pub struct Con {
    connection: CompositorConnection,
    screen: Screen,
    keymap: KeyMap<Keycode>,
    delay: u32, // milliseconds
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
    /// * `delay` - Minimum delay in milliseconds between keypresses in order to
    ///   properly enter all chars
    /// * `dpy_name` - If no `dpy_name` is provided, the value from $DISPLAY is
    ///   used
    ///
    /// # Errors
    /// TODO
    pub fn new(dpy_name: &Option<String>, delay: u32) -> Result<Con, NewConError> {
        debug!("using x11rb");
        let (connection, screen_idx) = x11rb::connect(dpy_name.as_deref())?;
        let setup = connection.setup();
        let screen = setup.roots[screen_idx].clone();
        let min_keycode = setup.min_keycode;
        let max_keycode = setup.max_keycode;
        let unused_keycodes = Self::find_unused_keycodes(&connection, min_keycode, max_keycode)?; // Check if a mapping is possible

        if unused_keycodes.is_empty() {
            return Err(NewConError::NoEmptyKeycodes);
        }
        let keymap = KeyMap::new(min_keycode, max_keycode, unused_keycodes);

        Ok(Con {
            connection,
            screen,
            keymap,
            delay,
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

    /// Find keycodes that have not yet been mapped any keysyms
    fn find_unused_keycodes(
        connection: &CompositorConnection,
        keycode_min: Keycode,
        keycode_max: Keycode,
    ) -> Result<VecDeque<Keycode>, ReplyError> {
        let mut unused_keycodes: VecDeque<Keycode> =
            VecDeque::with_capacity((keycode_max - keycode_min) as usize);

        let GetKeyboardMappingReply {
            keysyms_per_keycode,
            keysyms,
            ..
        } = connection
            .get_keyboard_mapping(keycode_min, keycode_max - keycode_min)?
            .reply()?;

        // Split the mapping into the chunks of keysyms that are mapped to each keycode
        let keysyms = keysyms.chunks(keysyms_per_keycode as usize);
        debug!("unused keycodes:");
        for (syms, kc) in keysyms.zip(keycode_min..=keycode_max) {
            // Check if the keycode is unused
            if syms.iter().all(|&s| s == NO_SYMBOL.raw()) {
                debug!("{}", kc);
                unused_keycodes.push_back(kc);
            }
        }
        Ok(unused_keycodes)
    }

    // Get the device id of the first device that is found which has the same usage
    // as the input parameter
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
}

impl Drop for Con {
    fn drop(&mut self) {
        // Map all previously mapped keycodes to the NoSymbol keysym to revert all
        // changes
        debug!("x11rb connection was dropped");
        for &keycode in self.keymap.keymap.values() {
            match self.connection.bind_key(keycode, NO_SYMBOL) {
                Ok(()) => debug!("unmapped keycode {keycode:?}"),
                Err(e) => error!("unable to unmap keycode {keycode:?}. {e:?}"),
            };
        }
    }
}

impl Bind<Keycode> for CompositorConnection {
    fn bind_key(&self, keycode: Keycode, keysym: Keysym) -> Result<(), ()> {
        // A list of two keycodes has to be mapped, otherwise the map is not what would
        // be expected If we would try to map only one keysym, we would get a
        // map that is tolower(keysym), toupper(keysym), tolower(keysym),
        // toupper(keysym), tolower(keysym), toupper(keysym), 0, 0, 0, 0, ...
        // https://stackoverflow.com/a/44334103
        self.change_keyboard_mapping(1, keycode, 2, &[keysym.raw(), keysym.raw()])
            .map_err(|e| error!("error when changing the keyboard mapping with x11rb: {e:?}"))?;
        self.sync().map_err(|e| error!("error when syncing with X server using x11rb after the keyboard mapping was changed: {e:?}"))
    }
}

impl KeyboardControllableNext for Con {
    fn fast_text_entry(&mut self, _text: &str) -> InputResult<Option<()>> {
        warn!("fast text entry is not yet implemented with x11rb");
        // TODO: Add fast method
        // xdotools can do it, so it is possible
        Ok(None)
    }

    fn enter_key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        self.keymap.make_room(&())?;
        let keycode = self.keymap.key_to_keycode(&self.connection, key)?;
        self.keymap.update_delays(keycode);

        let detail = keycode;
        let time = self.keymap.pending_delays;
        let root = self.screen.root;
        let root_x = 0;
        let root_y = 0;
        let deviceid = self.device_id(DeviceUse::IS_X_KEYBOARD)?;

        debug!(
            "xtest_fake_input with keycode {}, deviceid {}, delay {}",
            detail, deviceid, time
        );
        if direction == Direction::Press || direction == Direction::Click {
            self.connection
                .xtest_fake_input(
                    x11rb::protocol::xproto::KEY_PRESS_EVENT,
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

        // TODO: Check if we need to update the delays again
        // self.keymap.update_delays(keycode);
        // let time = self.keymap.pending_delays;

        if direction == Direction::Release || direction == Direction::Click {
            self.connection
                .xtest_fake_input(
                    x11rb::protocol::xproto::KEY_RELEASE_EVENT,
                    detail,
                    time, // TODO: Check if there needs to be a delay here
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

        self.keymap.last_event_before_delays = std::time::Instant::now();

        // Let the keymap know that the key was held/no longer held
        // This is important to avoid unmapping held keys
        self.keymap.enter_key(keycode, direction);

        Ok(())
    }
}

impl MouseControllableNext for Con {
    fn send_mouse_button_event(
        &mut self,
        button: MouseButton,
        direction: Direction,
    ) -> InputResult<()> {
        let detail = match button {
            MouseButton::Left => 1,
            MouseButton::Middle => 2,
            MouseButton::Right => 3,
            MouseButton::ScrollUp => 4,
            MouseButton::ScrollDown => 5,
            MouseButton::ScrollLeft => 6,
            MouseButton::ScrollRight => 7,
            MouseButton::Back => 8,
            MouseButton::Forward => 9,
        };
        let time = self.delay;
        let root = self.screen.root;
        let root_x = 0;
        let root_y = 0;
        let deviceid = self.device_id(DeviceUse::IS_X_POINTER)?;

        debug!(
            "xtest_fake_input with button {}, deviceid {}, delay {}",
            detail, deviceid, time
        );
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

    fn send_motion_notify_event(
        &mut self,
        x: i32,
        y: i32,
        coordinate: Coordinate,
    ) -> InputResult<()> {
        let type_ = x11rb::protocol::xproto::MOTION_NOTIFY_EVENT;
        let detail = match coordinate {
            Coordinate::Relative => 1,
            Coordinate::Absolute => 0,
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
            "xtest_fake_input with coordinate {}, deviceid {}, x {}, y {}, delay {}",
            detail, deviceid, root_x, root_y, time
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

    fn mouse_scroll_event(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        let mut length = length;
        let button = if length < 0 {
            length = -length;
            match axis {
                Axis::Horizontal => MouseButton::ScrollLeft,
                Axis::Vertical => MouseButton::ScrollUp,
            }
        } else {
            match axis {
                Axis::Horizontal => MouseButton::ScrollRight,
                Axis::Vertical => MouseButton::ScrollDown,
            }
        };
        for _ in 0..length {
            self.send_mouse_button_event(button, Direction::Click)?;
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

    fn mouse_loc(&self) -> InputResult<(i32, i32)> {
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
