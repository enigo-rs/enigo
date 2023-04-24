use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;

use x11rb::protocol::{
    randr::ConnectionExt as _,
    xinput::DeviceUse,
    xproto::{ConnectionExt as _, GetKeyboardMappingReply, Screen},
    xtest::ConnectionExt as _,
};
use x11rb::rust_connection::{DefaultStream, RustConnection};
use x11rb::{connection::Connection, wrapper::ConnectionExt as _};

use xkbcommon::xkb::{keysym_from_name, keysyms, KEY_NoSymbol, Keysym, KEYSYM_NO_FLAGS};

use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};

/// Default delay between chunks of keys that are sent to the X11 server
const DEFAULT_DELAY: u32 = 12;

#[derive(Debug)]
pub enum X11Error {
    MappingFailed(Keysym),
    Format(std::io::Error),
}

impl std::fmt::Display for X11Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            X11Error::MappingFailed(e) => write!(f, "Allocation failed: {e}"),
            X11Error::Format(e) => write!(f, "Format: {e}"),
        }
    }
}

impl From<std::io::Error> for X11Error {
    fn from(e: std::io::Error) -> Self {
        X11Error::Format(e)
    }
}

pub struct EnigoX11 {
    connection: RustConnection<DefaultStream>,
    delay: u32,
    screen: Screen,
    charmap: HashMap<Keysym, u8>,
    unused_keycodes: VecDeque<u8>,
    held: Vec<Key>,                               // Currently held keys
    last_keys: Vec<u8>,                           // Last pressed keycodes
    last_event_before_delays: std::time::Instant, // Time of the last event
    pending_delays: u32,
}

impl Default for EnigoX11 {
    fn default() -> Self {
        Self::new(DEFAULT_DELAY)
    }
}

impl EnigoX11 {
    pub fn new(delay: u32) -> EnigoX11 {
        let (connection, screen_idx) = x11rb::connect(None).unwrap();
        let delay = delay / 1000;
        let setup = connection.setup();
        let screen = setup.roots[screen_idx].clone();
        let min_keycode = setup.min_keycode;
        let max_keycode = setup.max_keycode;
        let charmap = HashMap::new();
        let unused_keycodes = Self::find_unused_keycodes(&connection, min_keycode, max_keycode);
        // Check if a mapping is possible
        assert!(
            !(unused_keycodes.is_empty()),
            "There was no space to map any keycodes"
        );
        let held = Vec::new();
        let last_keys = vec![];
        let last_event_before_delays = std::time::Instant::now();
        let pending_delays = 0;
        EnigoX11 {
            connection,
            delay,
            screen,
            charmap,
            unused_keycodes,
            held,
            last_keys,
            last_event_before_delays,
            pending_delays,
        }
    }

    /// Get the delay per keypress.
    /// Default value is 12 ms.
    /// This is Linux-specific.
    #[must_use]
    pub fn delay(&self) -> u32 {
        self.delay
    }
    /// Set the delay in ms per keypress.
    /// This is Linux-specific.
    pub fn set_delay(&mut self, delay: u32) {
        self.delay = delay / 1000;
    }

    fn find_unused_keycodes(
        connection: &RustConnection<DefaultStream>,
        keycode_min: u8,
        keycode_max: u8,
    ) -> VecDeque<u8> {
        let mut unused_keycodes: VecDeque<u8> =
            VecDeque::with_capacity((keycode_max - keycode_min) as usize);

        let GetKeyboardMappingReply {
            keysyms_per_keycode,
            keysyms,
            ..
        } = connection
            .get_keyboard_mapping(keycode_min, keycode_max - keycode_min)
            .unwrap()
            .reply()
            .unwrap();

        // Split the mapping into the chunks of keysyms that are mapped to each keycode
        let keysyms = keysyms.chunks(keysyms_per_keycode as usize);
        for (syms, kc) in keysyms.zip(keycode_min..=keycode_max) {
            // Check if the keycode is unused
            if syms.iter().all(|&s| s == KEY_NoSymbol) {
                unused_keycodes.push_back(kc);
            }
        }
        unused_keycodes
    }

    fn get_keycode(&mut self, keysym: Keysym) -> Result<u8, X11Error> {
        if let Some(keycode) = self.charmap.get(&keysym) {
            // The keysym is already mapped and cached in the charmap
            Ok(*keycode)
        } else {
            // The keysym needs to get mapped to an unused keycode
            self.map_sym(keysym) // Always map the keycode if it has not yet
                                 // been mapped, so it is layer agnostic
        }
    }

    fn key_to_keysym(key: Key) -> Keysym {
        match key {
            Key::Layout(c) => match c {
                '\n' => keysyms::KEY_Return,
                '\t' => keysyms::KEY_Tab,
                _ => {
                    let hex: u32 = c.into();
                    let name = format!("U{hex:x}");
                    keysym_from_name(&name, KEYSYM_NO_FLAGS)
                }
            },
            Key::Raw(k) => {
                // Raw keycodes cannot be converted to keysyms
                panic!("Attempted to convert raw keycode {k} to keysym");
            }
            Key::Alt | Key::Option => keysyms::KEY_Alt_L,
            Key::Backspace => keysyms::KEY_BackSpace,
            Key::Begin => keysyms::KEY_Begin,
            Key::Break => keysyms::KEY_Break,
            Key::Cancel => keysyms::KEY_Cancel,
            Key::CapsLock => keysyms::KEY_Caps_Lock,
            Key::Clear => keysyms::KEY_Clear,
            Key::Control | Key::LControl => keysyms::KEY_Control_L,
            Key::Delete => keysyms::KEY_Delete,
            Key::DownArrow => keysyms::KEY_Down,
            Key::End => keysyms::KEY_End,
            Key::Escape => keysyms::KEY_Escape,
            Key::Execute => keysyms::KEY_Execute,
            Key::F1 => keysyms::KEY_F1,
            Key::F2 => keysyms::KEY_F2,
            Key::F3 => keysyms::KEY_F3,
            Key::F4 => keysyms::KEY_F4,
            Key::F5 => keysyms::KEY_F5,
            Key::F6 => keysyms::KEY_F6,
            Key::F7 => keysyms::KEY_F7,
            Key::F8 => keysyms::KEY_F8,
            Key::F9 => keysyms::KEY_F9,
            Key::F10 => keysyms::KEY_F10,
            Key::F11 => keysyms::KEY_F11,
            Key::F12 => keysyms::KEY_F12,
            Key::F13 => keysyms::KEY_F13,
            Key::F14 => keysyms::KEY_F14,
            Key::F15 => keysyms::KEY_F15,
            Key::F16 => keysyms::KEY_F16,
            Key::F17 => keysyms::KEY_F17,
            Key::F18 => keysyms::KEY_F18,
            Key::F19 => keysyms::KEY_F19,
            Key::F20 => keysyms::KEY_F20,
            Key::F21 => keysyms::KEY_F21,
            Key::F22 => keysyms::KEY_F22,
            Key::F23 => keysyms::KEY_F23,
            Key::F24 => keysyms::KEY_F24,
            Key::F25 => keysyms::KEY_F25,
            Key::F26 => keysyms::KEY_F26,
            Key::F27 => keysyms::KEY_F27,
            Key::F28 => keysyms::KEY_F28,
            Key::F29 => keysyms::KEY_F29,
            Key::F30 => keysyms::KEY_F30,
            Key::F31 => keysyms::KEY_F31,
            Key::F32 => keysyms::KEY_F32,
            Key::F33 => keysyms::KEY_F33,
            Key::F34 => keysyms::KEY_F34,
            Key::F35 => keysyms::KEY_F35,
            Key::Find => keysyms::KEY_Find,
            Key::Hangul => keysyms::KEY_Hangul,
            Key::Hanja => keysyms::KEY_Hangul_Hanja,
            Key::Help => keysyms::KEY_Help,
            Key::Home => keysyms::KEY_Home,
            Key::Insert => keysyms::KEY_Insert,
            Key::Kanji => keysyms::KEY_Kanji,
            Key::LeftArrow => keysyms::KEY_Left,
            Key::Linefeed => keysyms::KEY_Linefeed,
            Key::LMenu => keysyms::KEY_Menu,
            Key::ModeChange => keysyms::KEY_Mode_switch,
            Key::Numlock => keysyms::KEY_Num_Lock,
            Key::PageDown => keysyms::KEY_Page_Down,
            Key::PageUp => keysyms::KEY_Page_Up,
            Key::Pause => keysyms::KEY_Pause,
            Key::Print => keysyms::KEY_Print,
            Key::RControl => keysyms::KEY_Control_R,
            Key::Redo => keysyms::KEY_Redo,
            Key::Return => keysyms::KEY_Return,
            Key::RightArrow => keysyms::KEY_Right,
            Key::RShift => keysyms::KEY_Shift_R,
            Key::ScrollLock => keysyms::KEY_Scroll_Lock,
            Key::Select => keysyms::KEY_Select,
            Key::ScriptSwitch => keysyms::KEY_script_switch,
            Key::Shift | Key::LShift => keysyms::KEY_Shift_L,
            Key::ShiftLock => keysyms::KEY_Shift_Lock,
            Key::Space => keysyms::KEY_space,
            Key::SysReq => keysyms::KEY_Sys_Req,
            Key::Tab => keysyms::KEY_Tab,
            Key::Undo => keysyms::KEY_Undo,
            Key::UpArrow => keysyms::KEY_Up,
            Key::Command | Key::Super | Key::Windows | Key::Meta => keysyms::KEY_Super_L,
        }
    }

    fn map_sym(&mut self, keysym: Keysym) -> Result<u8, X11Error> {
        match self.unused_keycodes.pop_front() {
            // A keycode is unused so a mapping is possible
            Some(unused_keycode) => {
                println!("Need to bind:");
                println!("keysym:{keysym}");
                println!("keycode:{unused_keycode}");

                self.bind_key(unused_keycode, keysym);
                self.charmap.insert(keysym, unused_keycode);
                Ok(unused_keycode)
            }
            // All keycodes are being used. A mapping is not possible
            None => Err(X11Error::MappingFailed(keysym)),
        }
    }

    // Map the the given keycode to the NoSymbol keysym so it can get reused
    fn unmap_sym(&mut self, keysym: Keysym) {
        if let Some(&keycode) = self.charmap.get(&keysym) {
            self.bind_key(keycode, KEY_NoSymbol);
            self.unused_keycodes.push_back(keycode);
            self.charmap.remove(&keysym);
        }
    }

    // Map the keysym to the given keycode
    // Only use keycodes that are not used, otherwise the existing mapping is
    // overwritten
    // If the keycode is mapped to the NoSymbol keysym, the key is unbinded and can
    // get used again later
    fn bind_key(&self, keycode: u8, keysym: Keysym) {
        // A list of two keycodes has to be mapped, otherwise the map is not what would
        // be expected If we would try to map only one keysym, we would get a
        // map that is tolower(keysym), toupper(keysym), tolower(keysym),
        // toupper(keysym), tolower(keysym), toupper(keysym), 0, 0, 0, 0, ...
        // https://stackoverflow.com/a/44334103
        self.connection
            .change_keyboard_mapping(1, keycode, 2, &[keysym, keysym])
            .unwrap();
        self.connection.sync().unwrap();
    }

    // Update the delay
    // TODO: A delay of 1 ms in all cases seems to work on my machine. Maybe this is
    // not needed?
    fn update_delays(&mut self, keycode: u8) {
        // Check if a delay is needed
        // A delay is required, if one of the keycodes was recently entered and there
        // was no delay between it

        // e.g. A quick rabbit
        // Chunk 1: 'A quick' # Add a delay before the second space
        // Chunk 2: 'rab'     # Add a delay before the second 'b'
        // Chunk 3: 'bit'     # Enter the remaining chars

        if self.last_keys.contains(&keycode) {
            let elapsed_ms = self
                .last_event_before_delays
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap();
            self.pending_delays = self.delay.saturating_sub(elapsed_ms);
            self.last_keys.clear();
        } else {
            self.pending_delays = 1;
        }
        self.last_keys.push(keycode);
    }

    // Sends a key event to the X11 server via XTest extension
    fn send_key_event(&mut self, keycode: u8, press: bool) {
        let type_ = if press {
            x11rb::protocol::xproto::KEY_PRESS_EVENT
        } else {
            x11rb::protocol::xproto::KEY_RELEASE_EVENT
        };
        let detail = keycode;
        let time = self.pending_delays;
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

        self.connection
            .xtest_fake_input(type_, detail, time, root, root_x, root_y, deviceid)
            .unwrap();
        self.connection.sync().unwrap();
        self.last_event_before_delays = std::time::Instant::now();
    }

    // Try to enter the char
    // If press is None, it is assumed that the char is pressed and released
    // If press is true, the key that enters the 'c' is pressed
    // Otherwise the corresponding key is released
    fn press_key(&mut self, key: Key, press: Option<bool>) {
        // Nothing to do
        if key == Key::Layout('\0') {
            return;
        }

        // Unmap all keys, if all keycodes are already being used
        // TODO: Don't unmap the keycodes if they will be needed next
        if self.unused_keycodes.is_empty() {
            let mapped_keys = self.charmap.clone();
            for &sym in mapped_keys.keys() {
                self.unmap_sym(sym);
            }
        }

        let (sym, keycode) = if let Key::Raw(kc) = key {
            (None, kc.try_into().unwrap())
        } else {
            let sym = Self::key_to_keysym(key);
            let keycode = self.get_keycode(sym).unwrap();
            (Some(sym), keycode)
        };

        match press {
            None => {
                self.update_delays(keycode);
                self.send_key_event(keycode, true);
                self.send_key_event(keycode, false);
            }
            Some(true) => {
                self.update_delays(keycode);
                self.send_key_event(keycode, true);
                self.held.push(key);
            }
            Some(false) => {
                // self.update_delays(keycode); TODO: Check if releases really don't need a
                // delay
                self.send_key_event(keycode, false);
                if let Some(s) = sym {
                    self.unmap_sym(s);
                }
                self.held.retain(|&k| k != key);
            }
        }
    }

    // Sends a button event to the X11 server via XTest extension
    fn press_mouse(&self, button: MouseButton, press: bool, delay: u32) {
        let type_ = if press {
            x11rb::protocol::xproto::BUTTON_PRESS_EVENT
        } else {
            x11rb::protocol::xproto::BUTTON_RELEASE_EVENT
        };
        let detail = mousebutton(button);
        let time = delay;
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
        self.connection
            .xtest_fake_input(type_, detail, time, root, root_x, root_y, deviceid)
            .unwrap();

        self.connection.sync().unwrap();
    }

    // Sends a motion notify event to the X11 server via XTest extension
    // TODO: Check if using x11rb::protocol::xproto::warp_pointer would be better
    fn move_mouse(&self, x: i32, y: i32, relative: bool) {
        let type_ = x11rb::protocol::xproto::MOTION_NOTIFY_EVENT;
        // TRUE -> relative coordinates
        // FALSE -> absolute coordinates
        let detail = u8::from(relative);
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
}

impl Drop for EnigoX11 {
    // Release the held keys before the XConnection is dropped
    fn drop(&mut self) {
        for c in &self.held.clone() {
            self.press_key(*c, Some(false));
        }
        for &keycode in self.charmap.values() {
            // Map the the given keycode
            // to the NoSymbol keysym so
            // it can get reused
            self.bind_key(keycode, KEY_NoSymbol);
        }
    }
}

impl KeyboardControllable for EnigoX11 {
    fn key_sequence(&mut self, string: &str) {
        for c in string.chars() {
            self.press_key(Key::Layout(c), None);
        }
    }

    fn key_down(&mut self, key: crate::Key) {
        self.press_key(key, Some(true));
    }

    fn key_up(&mut self, key: crate::Key) {
        self.press_key(key, Some(false));
    }

    fn key_click(&mut self, key: crate::Key) {
        self.press_key(key, Some(true));
        self.press_key(key, Some(false));
    }
}

impl MouseControllable for EnigoX11 {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        self.move_mouse(x, y, false);
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        self.move_mouse(x, y, true);
    }

    fn mouse_down(&mut self, button: MouseButton) {
        self.press_mouse(button, true, 1);
    }

    fn mouse_up(&mut self, button: MouseButton) {
        self.press_mouse(button, false, 1);
    }

    fn mouse_click(&mut self, button: MouseButton) {
        self.press_mouse(button, true, 1);
        self.press_mouse(button, false, DEFAULT_DELAY);
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        let mut length = length;
        let button = if length < 0 {
            MouseButton::ScrollLeft
        } else {
            MouseButton::ScrollRight
        };

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }
    fn mouse_scroll_y(&mut self, length: i32) {
        let mut length = length;
        let button = if length < 0 {
            MouseButton::ScrollUp
        } else {
            MouseButton::ScrollDown
        };

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }

    fn main_display_size(&self) -> (i32, i32) {
        let main_display = self
            .connection
            .randr_get_screen_resources(self.screen.root)
            .unwrap()
            .reply()
            .unwrap()
            .modes[0];

        (main_display.width as i32, main_display.height as i32)
    }

    fn mouse_location(&self) -> (i32, i32) {
        let reply = self
            .connection
            .query_pointer(self.screen.root)
            .unwrap()
            .reply()
            .unwrap();
        (reply.root_x as i32, reply.root_y as i32)
    }
}

fn mousebutton(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::ScrollUp => 4,
        MouseButton::ScrollDown => 5,
        MouseButton::ScrollLeft => 6,
        MouseButton::ScrollRight => 7,
        MouseButton::Back => 8,
        MouseButton::Forward => 9,
    }
}
