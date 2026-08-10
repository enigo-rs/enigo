use std::collections::VecDeque;
use std::convert::TryInto;

use log::{debug, error, trace, warn};
use x11rb::{
    connection::Connection,
    protocol::{
        randr::ConnectionExt as _,
        xinput::DeviceUse,
        xkb::{self, ConnectionExt as _},
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
    min_keycode: Keycode,
    /// XKB key types, indexed by key type index
    key_types: Vec<xkb::KeyType>,
    /// XKB key type indices per group for every keycode, indexed by
    /// `keycode - min_keycode`
    key_type_indices: Vec<[u8; 4]>,
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

        // Get the XKB key types so the modifiers needed to reach a keysym level
        // can be derived from the keymap instead of being hardcoded
        let (key_types, key_type_indices) = Self::get_key_types(&connection)?;

        Ok(Con {
            connection,
            screen,
            keymap,
            modifiers,
            min_keycode,
            key_types,
            key_type_indices,
        })
    }

    /// Get the XKB key types and the key type assigned to every keycode.
    ///
    /// A key type describes, for each keysym level, the modifier combination
    /// that selects it. Together with the key type assigned to each key, this
    /// is the keymap's own description of which modifiers switch to which
    /// level, so it can be used instead of assuming fixed modifiers.
    fn get_key_types(
        connection: &CompositorConnection,
    ) -> Result<(Vec<xkb::KeyType>, Vec<[u8; 4]>), NewConError> {
        // The XKB extension has to be initialized before any of its requests
        // can be used
        connection.xkb_use_extension(1, 0)?.reply()?;

        let reply = connection
            .xkb_get_map(
                xkb::ID::USE_CORE_KBD.into(),
                xkb::MapPart::KEY_TYPES | xkb::MapPart::KEY_SYMS, // return these fully
                xkb::MapPart::from(0u16),                         // nothing partially
                0,                                                // first_type
                0,                                                // n_types
                0,                                                // first_key_sym
                0,                                                // n_key_syms
                0,                                                // first_key_action
                0,                                                // n_key_actions
                0,                                                // first_key_behavior
                0,                                                // n_key_behaviors
                xkb::VMod::from(0u16),
                0, // first_key_explicit
                0, // n_key_explicit
                0, // first_mod_map_key
                0, // n_mod_map_keys
                0, // first_v_mod_map_key
                0, // n_v_mod_map_keys
            )?
            .reply()?;

        let key_types = reply.map.types_rtrn.unwrap_or_default();
        let key_type_indices = reply
            .map
            .syms_rtrn
            .unwrap_or_default()
            .iter()
            .map(|sym_map| sym_map.kt_index)
            .collect();

        Ok((key_types, key_type_indices))
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
    /// Return the modifier keycodes needed to reach `level` for `keycode`.
    ///
    /// The level-to-modifier relationship is read from the XKB key types
    /// instead of being hardcoded, because it is not guaranteed to be the same
    /// for every keymap (e.g. `AltGr` is not always mapped to the same
    /// modifier).
    ///
    /// Each key type lists, for every level, the real modifier mask that
    /// selects it. We resolve the key type assigned to the key's base group,
    /// look up the mask for the requested level and translate that mask into
    /// the keycodes of the corresponding modifiers.
    fn modifier_keycodes_for_level(&self, keycode: Keycode, level: u8) -> Vec<Keycode> {
        // The base level never needs a modifier
        if level == 0 {
            return vec![];
        }

        let Some(mods_mask) = self.level_modifier_mask(keycode, level) else {
            warn!("no key type modifier mapping found for keycode {keycode} at level {level}");
            return vec![];
        };

        // Translate the real modifier mask into the keycodes of those
        // modifiers. Bit i of the mask corresponds to self.modifiers[i]
        // (0: Shift, 1: Lock, 2: Control, 3-7: Mod1-Mod5).
        let mut mod_keycodes = vec![];
        for (i, keycodes) in self.modifiers.iter().enumerate() {
            if mods_mask & (1u16 << i) == 0 {
                continue;
            }
            if let Some(&kc) = keycodes.first() {
                mod_keycodes.push(kc);
            } else {
                warn!("modifier no {i} selects level {level} but has no keycode mapped");
            }
        }
        mod_keycodes
    }

    /// Look up the real modifier mask that selects `level` for `keycode` in the
    /// XKB key types.
    fn level_modifier_mask(&self, keycode: Keycode, level: u8) -> Option<u16> {
        let index = usize::from(keycode.checked_sub(self.min_keycode)?);
        let kt_index = self.key_type_indices.get(index)?;
        // Use the base group (group 1). Reaching a level in a higher group
        // would additionally require a group-switch modifier, which the levels
        // resolved here do not use.
        let key_type = self.key_types.get(usize::from(kt_index[0]))?;

        key_type
            .map
            .iter()
            .find(|entry| entry.active && entry.level == level)
            .map(|entry| u16::from(entry.mods_mask))
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

        let mod_keycodes = self.modifier_keycodes_for_level(keycode, level);

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
