use std::collections::VecDeque;
use std::convert::TryInto;

use log::{debug, error, trace, warn};
use x11rb::{
    connection::Connection,
    protocol::{
        randr::ConnectionExt as _,
        xinput::DeviceUse,
        xproto::{ConnectionExt as _, GetKeyboardMappingReply, GetModifierMappingReply, Screen},
        xtest::ConnectionExt as _,
    },
    rust_connection::{ConnectError, ConnectionError, DefaultStream, ReplyError, RustConnection},
    wrapper::ConnectionExt as _,
};

use super::keymap::{Bind, KeyMap, Keysym, NO_SYMBOL};
use crate::{
    keycodes::Modifier, Axis, Button, Coordinate, Direction, InputError, InputResult, Key,
    Keyboard, Mouse, NewConError,
};

type CompositorConnection = RustConnection<DefaultStream>;

pub type Keycode = u8;

pub struct Con {
    connection: CompositorConnection,
    screen: Screen,
    keymap: KeyMap<Keycode>,
    modifiers: Vec<Keycode>,
    delay: u32, // milliseconds
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

        Ok(Con {
            connection,
            screen,
            keymap,
            modifiers,
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

            if syms.iter().all(|&s| s == NO_SYMBOL.raw()) {
                unused_keycodes.push_back(kc);
            }
        }
        debug!("unused keycodes: {unused_keycodes:?}");
        unused_keycodes
    }

    /// Find the keycodes that must be used for the modifiers
    fn find_modifier_keycodes(
        connection: &CompositorConnection,
    ) -> Result<Vec<Keycode>, ReplyError> {
        let modifier_reply = connection.get_modifier_mapping()?.reply()?;
        let keycodes_per_modifier = modifier_reply.keycodes_per_modifier() as usize;
        let GetModifierMappingReply {
            keycodes: modifiers,
            ..
        } = modifier_reply;
        trace!("keycodes per modifier: {keycodes_per_modifier:?}");
        trace!("the keycodes associated with the modifiers are:\n{modifiers:?}");

        debug!("modifier mapping:");
        let mut modifier_keycodes = vec![0; 8];
        'mods: for (mod_no, mod_keycode) in modifier_keycodes.iter_mut().enumerate().take(8) {
            let start = mod_no * keycodes_per_modifier;
            for &keycode in &modifiers[start..start + keycodes_per_modifier] {
                if keycode != 0 {
                    // Found one keycode that can be used for this modifier
                    debug!("mod_no: {mod_no} -> {keycode}");
                    *mod_keycode = keycode;
                    continue 'mods;
                }
            }
            warn!("modifier_no: {mod_no} is unmapped");
        }

        Ok(modifier_keycodes)
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
        for &keycode in self.keymap.additionally_mapped.values() {
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

impl Keyboard for Con {
    fn fast_text(&mut self, _text: &str) -> InputResult<Option<()>> {
        warn!("fast text entry is not yet implemented with x11rb");
        // TODO: Add fast method
        // xdotools can do it, so it is possible
        Ok(None)
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        // Check if the key is a modifier
        let keycode: u16 = match Modifier::try_from(key) {
            // If it is a modifier, the already mapped keycode must be used
            Ok(modifier) => {
                debug!("it is a modifier: {modifier:?}");
                self.modifiers[modifier.no()].into()
            }
            // All regular keys might have to get mapped
            _ => self.keymap.key_to_keycode(&self.connection, key)?.into(),
        };

        self.raw(keycode, direction)
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        let Ok(keycode) = keycode.try_into() else {
            return Err(InputError::InvalidInput(
                "Keycode was too large. It has to fit in u8 on X11",
            ));
        };
        let time = self.keymap.pending_delays();
        let root = self.screen.root;
        let root_x = 0;
        let root_y = 0;
        let deviceid = self.device_id(DeviceUse::IS_X_KEYBOARD)?;

        debug!(
            "xtest_fake_input with keycode {}, deviceid {}, delay {}",
            keycode, deviceid, time
        );
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
                    InputError::Simulate("error when using xtest_fake_input with x11rb: {e:?}")
                })?;
            trace!("press");
        }

        // TODO: Check if we need to update the delays again
        // self.keymap.update_delays(keycode);
        // let time = self.keymap.pending_delays();

        if direction == Direction::Release || direction == Direction::Click {
            self.connection
                .xtest_fake_input(
                    x11rb::protocol::xproto::KEY_RELEASE_EVENT,
                    keycode,
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
            trace!("released");
        }

        self.connection.sync()
            .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after the keyboard mapping was changed: {e:?}")
            })?;

        // Let the keymap know that the key was held/no longer held
        // This is important to avoid unmapping held keys
        self.keymap.key(keycode, direction);

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

    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        let mut length = length;
        let button = if length < 0 {
            length = -length;
            match axis {
                Axis::Horizontal => Button::ScrollLeft,
                Axis::Vertical => Button::ScrollUp,
            }
        } else {
            match axis {
                Axis::Horizontal => Button::ScrollRight,
                Axis::Vertical => Button::ScrollDown,
            }
        };
        for _ in 0..length {
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
