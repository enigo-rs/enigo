use std::convert::TryInto;
use std::error::Error;
use std::fmt::{self, Formatter};
use std::{collections::VecDeque, fmt::Display};

use x11rb::{
    connection::Connection,
    protocol::{
        randr::ConnectionExt as _,
        xinput::DeviceUse,
        xproto::{ConnectionExt as _, GetKeyboardMappingReply, Screen},
        xtest::ConnectionExt as _,
    },
    rust_connection::{ConnectError, ConnectionError, DefaultStream, ReplyError, RustConnection},
    wrapper::ConnectionExt as _,
};

use super::{
    keymap::{Bind, KeyMap},
    Keysym, NO_SYMBOL,
};
use crate::{
    Axis, Coordinate, Direction, InputError, InputResult, Key, KeyboardControllableNext,
    MouseButton, MouseControllableNext, NewConError,
};

type CompositorConnection = RustConnection<DefaultStream>;

/// Default delay between chunks of keys that are sent to the X11 server in
/// milliseconds
const DEFAULT_DELAY: u32 = 12;

pub type Keycode = u8;

#[allow(clippy::module_name_repetitions)]
pub struct Con {
    connection: CompositorConnection,
    screen: Screen,
    keymap: KeyMap<Keycode>,
    delay: u32, // milliseconds
}

impl From<ConnectError> for NewConError {
    fn from(error: ConnectError) -> Self {
        println!("{error:?}");
        // TODO: Describe why exactly it failed
        Self::EstablishCon("failed to establish the connection")
    }
}
impl From<ReplyError> for NewConError {
    fn from(error: ReplyError) -> Self {
        println!("{error:?}");
        Self::Reply
    }
}
impl Con {
    /// Tries to establish a new X11 connection using the specified parameters
    ///
    /// `delay`: Minimum delay in milliseconds between keypresses in order to
    /// properly enter all chars
    ///
    /// `dpy_name`: If no `dpy_name` is provided, the value from $DISPLAY is
    /// used.
    ///
    /// # Errors
    /// TODO
    pub fn new(dpy_name: Option<&str>, delay: u32) -> Result<Con, NewConError> {
        let (connection, screen_idx) = x11rb::connect(dpy_name)?;
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

    /// Tries to establish a new X11 connection using default parameters
    ///
    /// # Errors
    /// TODO
    pub fn try_default() -> Result<Self, NewConError> {
        let dyp_name = None;
        let delay = DEFAULT_DELAY;
        Self::new(dyp_name, delay)
    }

    /// Get the delay per keypress in milliseconds.
    /// Default value is 12 ms.
    /// This is Linux-specific.
    #[must_use]
    pub fn delay(&self) -> u32 {
        self.delay
    }
    /// Set the delay in milliseconds per keypress.
    /// This is Linux-specific.
    pub fn set_delay(&mut self, delay: u32) {
        self.delay = delay;
    }

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
        for (syms, kc) in keysyms.zip(keycode_min..=keycode_max) {
            // Check if the keycode is unused
            if syms.iter().all(|&s| s == NO_SYMBOL.raw()) {
                unused_keycodes.push_back(kc);
            }
        }
        Ok(unused_keycodes)
    }
}

impl Drop for Con {
    fn drop(&mut self) {
        // Map all previously mapped keycodes to the NoSymbol keysym to revert all
        // changes
        for &keycode in self.keymap.keymap.values() {
            self.connection.bind_key(keycode, NO_SYMBOL);
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
            .map_err(|e| ())?;
        self.sync().map_err(|e| ())
    }
}

impl KeyboardControllableNext for Con {
    fn fast_text_entry(&mut self, _text: &str) -> InputResult<Option<()>> {
        // TODO: Add fast method
        // xdotools can do it, so it is possible
        Ok(None)
    }
    /// Try to enter the key
    fn enter_key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        self.keymap.make_room(&())?;
        let keycode = self.keymap.key_to_keycode(&self.connection, key).unwrap();
        self.keymap.update_delays(keycode);
        // Send the events to the compositor
        let detail = keycode;
        let time = self.keymap.pending_delays;
        let root = self.screen.root;
        let root_x = 0;
        let root_y = 0;
        let deviceid = x11rb::protocol::xinput::list_input_devices(&self.connection)
            .unwrap()
            .reply()
            .unwrap()
            .devices
            .iter()
            .find(|d| d.device_use == DeviceUse::IS_X_KEYBOARD)
            .map(|d| d.device_id)
            .unwrap();

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
                .unwrap();
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
                .unwrap();
        }
        self.connection.sync().unwrap();
        self.keymap.last_event_before_delays = std::time::Instant::now();
        Ok(())
    }
}

impl MouseControllableNext for Con {
    // Sends a button event to the X11 server via `XTest` extension
    fn send_mouse_button_event(&mut self, button: MouseButton, direction: Direction, delay: u32) {
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
        let mut time = delay;
        let root = self.screen.root;
        let root_x = 0;
        let root_y = 0;
        let deviceid = x11rb::protocol::xinput::list_input_devices(&self.connection)
            .unwrap()
            .reply()
            .unwrap()
            .devices
            .iter()
            .find(|d| d.device_use == DeviceUse::IS_X_POINTER)
            .map(|d| d.device_id)
            .unwrap();

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
                .unwrap();
        }
        if direction == Direction::Release || direction == Direction::Click {
            // Add a delay for the release part of a click
            // TODO: Maybe calculate here if a delay is needed as well
            if direction == Direction::Click {
                time = DEFAULT_DELAY;
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
                .unwrap();
        }
        self.connection.sync().unwrap();
    }

    // Sends a motion notify event to the X11 server via `XTest` extension
    // TODO: Check if using x11rb::protocol::xproto::warp_pointer would be better
    fn send_motion_notify_event(&mut self, x: i32, y: i32, coordinate: Coordinate) {
        let type_ = x11rb::protocol::xproto::MOTION_NOTIFY_EVENT;
        // TRUE -> relative coordinates
        // FALSE -> absolute coordinates
        let detail = match coordinate {
            Coordinate::Relative => 1,
            Coordinate::Absolute => 0,
        };
        let time = x11rb::CURRENT_TIME;
        let root = x11rb::NONE; //  the root window of the screen the pointer is currently on
        let root_x = x.try_into().unwrap();
        let root_y = y.try_into().unwrap();
        let deviceid = x11rb::protocol::xinput::list_input_devices(&self.connection)
            .unwrap()
            .reply()
            .unwrap()
            .devices
            .iter()
            .find(|d| d.device_use == DeviceUse::IS_X_POINTER)
            .map(|d| d.device_id)
            .unwrap();
        self.connection
            .xtest_fake_input(type_, detail, time, root, root_x, root_y, deviceid)
            .unwrap();
        self.connection.sync().unwrap();
    }

    fn mouse_scroll_event(&mut self, length: i32, axis: Axis) {
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
            self.send_mouse_button_event(button, Direction::Click, self.delay);
        }
    }

    fn main_display(&self) -> (i32, i32) {
        let main_display = self
            .connection
            .randr_get_screen_resources(self.screen.root)
            .unwrap()
            .reply()
            .unwrap()
            .modes[0];

        (main_display.width as i32, main_display.height as i32)
    }

    fn mouse_loc(&self) -> (i32, i32) {
        let reply = self
            .connection
            .query_pointer(self.screen.root)
            .unwrap()
            .reply()
            .unwrap();
        (reply.root_x as i32, reply.root_y as i32)
    }
}
