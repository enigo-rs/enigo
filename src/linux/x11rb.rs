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

use super::keymap::{Bind, KeyMap, Keysym};
use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse, NewConError,
};

type CompositorConnection = RustConnection<DefaultStream>;

pub type Keycode = u8;

pub struct Con {
    connection: CompositorConnection,
    screen: Screen,
    keymap: KeyMap<Keycode>,
    modifiers: [Vec<Keycode>; 8],
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

        Ok(Con {
            connection,
            screen,
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
        for &keycode in self.keymap.keymap_mapping.additionally_mapped.values() {
            match self.connection.bind_key(keycode, Keysym::NoSymbol) {
                Ok(()) => debug!("unmapped keycode {keycode:?}"),
                Err(e) => error!("unable to unmap keycode {keycode:?}. {e:?}"),
            }
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

impl Con {
    /// Return the modifier keycodes needed for a given keysym level.
    ///
    /// X11 keymap columns map to groups and levels:
    /// - Column 0: Group 1, no modifier
    /// - Column 1: Group 1, Shift
    /// - Column 2: Group 1, Level3 (`AltGr/Mode_switch`) or Group 2
    /// - Column 3: Group 1, Level3 + Shift or Group 2 + Shift
    /// - Column 4+: higher levels, typically ISO_Level3_Shift-based
    ///
    /// On most Linux systems, `AltGr` is at Mod5 (modifier index 7).
    fn modifier_keycodes_for_level(&self, level: u8) -> Vec<Keycode> {
        match level {
            0 => vec![],
            1 => self.modifiers[0].first().copied().into_iter().collect(), // Shift
            2 | 4 => {
                // AltGr / ISO_Level3_Shift / Mode_switch - typically Mod5 (index 7)
                // Fall back to Mod3 (index 5) if Mod5 is empty
                if let Some(&kc) = self.modifiers[7].first() {
                    vec![kc]
                } else if let Some(&kc) = self.modifiers[5].first() {
                    vec![kc]
                } else {
                    warn!("no AltGr modifier found for keysym level {level}");
                    vec![]
                }
            }
            3 | 5 => {
                // AltGr + Shift
                let mut mods = vec![];
                if let Some(&kc) = self.modifiers[7].first() {
                    mods.push(kc);
                } else if let Some(&kc) = self.modifiers[5].first() {
                    mods.push(kc);
                }
                if let Some(&kc) = self.modifiers[0].first() {
                    mods.push(kc);
                }
                if mods.is_empty() {
                    warn!("no modifiers found for keysym level {level}");
                }
                mods
            }
            _ => {
                warn!("modifier for keysym level {level} not yet supported");
                vec![]
            }
        }
    }
}

impl Keyboard for Con {
    fn fast_text(&mut self, _text: &str) -> InputResult<Option<()>> {
        Ok(None)
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        let (keycode, level) = self.keymap.key_to_keycode(&self.connection, key)?;

        if log::log_enabled!(log::Level::Debug) {
            for (mod_idx, mod_keycodes) in self.modifiers.iter().enumerate() {
                if mod_keycodes.contains(&keycode) {
                    debug!("the key is modifier no: {mod_idx}");
                }
            }
        }

        let mod_keycodes = self.modifier_keycodes_for_level(level);

        if mod_keycodes.is_empty() {
            self.raw(keycode.into(), direction)
        } else {
            // Track which modifiers we actually need to press (skip already-held ones)
            let mods_to_press: Vec<Keycode> = mod_keycodes
                .iter()
                .copied()
                .filter(|kc| !self.keymap.is_keycode_held(kc))
                .collect();

            match direction {
                Direction::Click => {
                    for &mod_kc in &mods_to_press {
                        self.raw(mod_kc.into(), Direction::Press)?;
                    }
                    self.raw(keycode.into(), Direction::Click)?;
                    for &mod_kc in mods_to_press.iter().rev() {
                        self.raw(mod_kc.into(), Direction::Release)?;
                    }
                }
                Direction::Press => {
                    for &mod_kc in &mods_to_press {
                        self.raw(mod_kc.into(), Direction::Press)?;
                    }
                    self.raw(keycode.into(), Direction::Press)?;
                }
                Direction::Release => {
                    self.raw(keycode.into(), Direction::Release)?;
                    for &mod_kc in mods_to_press.iter().rev() {
                        self.raw(mod_kc.into(), Direction::Release)?;
                    }
                }
            }
            Ok(())
        }
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

            self.connection.sync() .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after xtest_fake_input was called")
            })?;
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

            self.connection.sync() .map_err(|e| {
                error!("{e}");
                InputError::Simulate("error when syncing with X server using x11rb after xtest_fake_input was called")
            })?;
        }

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
