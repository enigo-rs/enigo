use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::fmt::Display;

use xkbcommon::xkb::{keysym_from_name, KEYSYM_NO_FLAGS};

use super::{ConnectionError, Keysym, NO_SYMBOL};
use crate::{Direction, Key};

const DEFAULT_DELAY: u32 = 12;

pub type ModifierBitflag = u32; // TODO: Maybe create a proper type for this

#[derive(Debug)]
pub struct KeyMap<Keycode> {
    pub(super) keymap: HashMap<Keysym, Keycode>,
    unused_keycodes: VecDeque<Keycode>,
    needs_regeneration: bool,
    pub(super) file: Option<std::fs::File>, // Temporary file that contains the keymap
    modifiers: ModifierBitflag,             // State of the modifiers
    last_keys: Vec<Keycode>,                // Last pressed keycodes
    delay: u32,                             // milliseconds
    pub(super) last_event_before_delays: std::time::Instant, // Time of the last event
    pub(super) pending_delays: u32,
}

// TODO: Check if the bounds can be simplified
impl<
        Keycode: std::ops::Sub
            + PartialEq
            + Copy
            + Clone
            + Display
            + TryInto<usize>
            + std::convert::TryFrom<usize>,
    > KeyMap<Keycode>
where
    <Keycode as TryInto<usize>>::Error: std::fmt::Debug,
    <Keycode as TryFrom<usize>>::Error: std::fmt::Debug,
{
    /// Create a new `KeyMap`
    pub fn new(
        keycode_min: Keycode,
        keycode_max: Keycode,
        unused_keycodes: VecDeque<Keycode>,
    ) -> Self {
        let capacity: usize = keycode_max.try_into().unwrap() - keycode_min.try_into().unwrap();
        let capacity = capacity + 1;
        let delay = DEFAULT_DELAY;
        let keymap = HashMap::with_capacity(capacity);
        let file = None;
        let needs_regeneration = true;
        let modifiers = 0;
        let last_keys = vec![];
        let last_event_before_delays = std::time::Instant::now();
        let pending_delays = 0;
        Self {
            keymap,
            unused_keycodes,
            needs_regeneration,
            file,
            modifiers,
            last_keys,
            delay,
            last_event_before_delays,
            pending_delays,
        }
    }

    /// Converts a Key to a Keysym
    #[allow(clippy::too_many_lines)]
    pub fn key_to_keysym(key: Key) -> Keysym {
        match key {
            Key::Layout(c) => match c {
                '\n' => Keysym::Return,
                '\t' => Keysym::Tab,
                _ => {
                    // TODO: Replace with Keysym.from_char(ch: char)
                    let hex: u32 = c.into();
                    let name = format!("U{hex:x}");
                    keysym_from_name(&name, KEYSYM_NO_FLAGS)
                }
            },
            Key::Raw(k) => {
                // Raw keycodes cannot be converted to keysyms
                panic!("Attempted to convert raw keycode {k} to keysym");
            }
            Key::Alt | Key::Option => Keysym::Alt_L,
            Key::Backspace => Keysym::BackSpace,
            Key::Begin => Keysym::Begin,
            Key::Break => Keysym::Break,
            Key::Cancel => Keysym::Cancel,
            Key::CapsLock => Keysym::Caps_Lock,
            Key::Clear => Keysym::Clear,
            Key::Control | Key::LControl => Keysym::Control_L,
            Key::Delete => Keysym::Delete,
            Key::DownArrow => Keysym::Down,
            Key::End => Keysym::End,
            Key::Escape => Keysym::Escape,
            Key::Execute => Keysym::Execute,
            Key::F1 => Keysym::F1,
            Key::F2 => Keysym::F2,
            Key::F3 => Keysym::F3,
            Key::F4 => Keysym::F4,
            Key::F5 => Keysym::F5,
            Key::F6 => Keysym::F6,
            Key::F7 => Keysym::F7,
            Key::F8 => Keysym::F8,
            Key::F9 => Keysym::F9,
            Key::F10 => Keysym::F10,
            Key::F11 => Keysym::F11,
            Key::F12 => Keysym::F12,
            Key::F13 => Keysym::F13,
            Key::F14 => Keysym::F14,
            Key::F15 => Keysym::F15,
            Key::F16 => Keysym::F16,
            Key::F17 => Keysym::F17,
            Key::F18 => Keysym::F18,
            Key::F19 => Keysym::F19,
            Key::F20 => Keysym::F20,
            Key::F21 => Keysym::F21,
            Key::F22 => Keysym::F22,
            Key::F23 => Keysym::F23,
            Key::F24 => Keysym::F24,
            Key::F25 => Keysym::F25,
            Key::F26 => Keysym::F26,
            Key::F27 => Keysym::F27,
            Key::F28 => Keysym::F28,
            Key::F29 => Keysym::F29,
            Key::F30 => Keysym::F30,
            Key::F31 => Keysym::F31,
            Key::F32 => Keysym::F32,
            Key::F33 => Keysym::F33,
            Key::F34 => Keysym::F34,
            Key::F35 => Keysym::F35,
            Key::Find => Keysym::Find,
            Key::Hangul => Keysym::Hangul,
            Key::Hanja => Keysym::Hangul_Hanja,
            Key::Help => Keysym::Help,
            Key::Home => Keysym::Home,
            Key::Insert => Keysym::Insert,
            Key::Kanji => Keysym::Kanji,
            Key::LeftArrow => Keysym::Left,
            Key::Linefeed => Keysym::Linefeed,
            Key::LMenu => Keysym::Menu,
            Key::ModeChange => Keysym::Mode_switch,
            Key::MediaNextTrack => Keysym::XF86_AudioNext,
            Key::MediaPlayPause => Keysym::XF86_AudioPlay,
            Key::MediaPrevTrack => Keysym::XF86_AudioPrev,
            Key::MediaStop => Keysym::XF86_AudioStop,
            Key::Numlock => Keysym::Num_Lock,
            Key::PageDown => Keysym::Page_Down,
            Key::PageUp => Keysym::Page_Up,
            Key::Pause => Keysym::Pause,
            Key::Print => Keysym::Print,
            Key::RControl => Keysym::Control_R,
            Key::Redo => Keysym::Redo,
            Key::Return => Keysym::Return,
            Key::RightArrow => Keysym::Right,
            Key::RShift => Keysym::Shift_R,
            Key::ScrollLock => Keysym::Scroll_Lock,
            Key::Select => Keysym::Select,
            Key::ScriptSwitch => Keysym::script_switch,
            Key::Shift | Key::LShift => Keysym::Shift_L,
            Key::ShiftLock => Keysym::Shift_Lock,
            Key::Space => Keysym::space,
            Key::SysReq => Keysym::Sys_Req,
            Key::Tab => Keysym::Tab,
            Key::Undo => Keysym::Undo,
            Key::UpArrow => Keysym::Up,
            Key::VolumeDown => Keysym::XF86_AudioLowerVolume,
            Key::VolumeUp => Keysym::XF86_AudioRaiseVolume,
            Key::VolumeMute => Keysym::XF86_AudioMute,
            Key::Command | Key::Super | Key::Windows | Key::Meta => Keysym::Super_L,
        }
    }

    // Try to enter the key
    #[allow(clippy::unnecessary_wraps)]
    pub fn key_to_keycode<C: Bind<Keycode>>(&mut self, c: &C, key: Key) -> Option<Keycode> {
        let keycode = if let Key::Raw(kc) = key {
            let kcz: usize = kc.try_into().unwrap();
            kcz.try_into().unwrap()
        } else {
            let sym = KeyMap::<Keycode>::key_to_keysym(key);
            if let Some(keycode) = self.keymap.get(&sym) {
                // The keysym is already mapped and cached in the keymap
                Ok(*keycode)
            } else {
                // The keysym needs to get mapped to an unused keycode.
                // Always map the keycode if it has not yet been mapped, so it is layer agnostic
                self.map(c, sym)
            }
            .unwrap()
        };

        Some(keycode)
    }

    /// Add the Keysym to the keymap
    ///
    /// This does not apply the changes
    pub fn map<C: Bind<Keycode>>(
        &mut self,
        c: &C,
        keysym: Keysym,
    ) -> Result<Keycode, ConnectionError> {
        match self.unused_keycodes.pop_front() {
            // A keycode is unused so a mapping is possible
            Some(unused_keycode) => {
                c.bind_key(unused_keycode, keysym);
                self.needs_regeneration = true;
                self.keymap.insert(keysym, unused_keycode);
                Ok(unused_keycode)
            }
            // All keycodes are being used. A mapping is not possible
            None => Err(ConnectionError::MappingFailed(keysym)),
        }
    }

    /// Remove the Keysym from the keymap
    ///
    /// This does not apply the changes
    pub fn unmap<C: Bind<Keycode>>(&mut self, c: &C, keysym: Keysym) {
        if let Some(&keycode) = self.keymap.get(&keysym) {
            c.bind_key(keycode, NO_SYMBOL);
            self.unused_keycodes.push_back(keycode);
            self.keymap.remove(&keysym);
        }
    }

    // Update the delay
    // TODO: A delay of 1 ms in all cases seems to work on my machine. Maybe
    // this is not needed?
    // TODO: Only needed for x11rb
    pub fn update_delays(&mut self, keycode: Keycode) {
        // Check if a delay is needed
        // A delay is required, if one of the keycodes was recently entered and there
        // was no delay between it

        // e.g. A quick rabbit
        // Chunk 1: 'A quick' # Add a delay before the second space
        // Chunk 2: ' rab'     # Add a delay before the second 'b'
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

    /// Check if there are still unused keycodes available. If there aren't,
    /// make some room by freeing the already mapped keycodes.
    /// Returns true, if keys were unmapped and the keymap needs to be
    /// regenerated
    pub fn make_room<C: Bind<Keycode>>(&mut self, c: &C) -> bool {
        // Unmap all keys, if all keycodes are already being used
        // TODO: Don't unmap the keycodes if they will be needed next
        // TODO: Don't unmap held keys!
        if self.unused_keycodes.is_empty() {
            let mapped_keys = self.keymap.clone();
            for &sym in mapped_keys.keys() {
                self.unmap(c, sym);
            }
            return true;
        }
        false
    }
}

pub trait Bind<Keycode> {
    // Map the keysym to the given keycode
    // Only use keycodes that are not used, otherwise the existing mapping is
    // overwritten
    // If the keycode is mapped to the NoSymbol keysym, the key is unbinded and can
    // get used again later
    fn bind_key(&self, keycode: Keycode, keysym: Keysym);
}

impl<Keycode> Bind<Keycode> for () {
    fn bind_key(&self, _: Keycode, _: Keysym) {
        // No need to do anything
    }
}
