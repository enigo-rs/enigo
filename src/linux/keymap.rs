use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::fmt::Display;
use std::io::{Seek, SeekFrom, Write};

use xkbcommon::xkb::keysym_get_name;

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

    // Try to enter the key
    #[allow(clippy::unnecessary_wraps)]
    pub fn key_to_keycode<C: Bind<Keycode>>(&mut self, c: &C, key: Key) -> Option<Keycode> {
        let keycode = if let Key::Raw(kc) = key {
            let kcz: usize = kc.try_into().unwrap();
            kcz.try_into().unwrap()
        } else {
            let sym = Keysym::try_from(key).unwrap();
            if let Some(&keycode) = self.keymap.get(&sym) {
                // The keysym is already mapped and cached in the keymap
                Ok(keycode)
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
    pub fn unmap<C: Bind<Keycode>>(&mut self, c: &C, keysym: Keysym, keycode: Keycode) {
        c.bind_key(keycode, NO_SYMBOL);
        self.needs_regeneration = true;
        self.unused_keycodes.push_back(keycode);
        self.keymap.remove(&keysym);
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
                .unwrap_or(u32::MAX);
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
            for (sym, keycode) in mapped_keys {
                self.unmap(c, sym, keycode);
            }
            return true;
        }
        false
    }

    /// Regenerate the keymap if there were any changes
    /// and write the new keymap to a temporary file
    ///
    /// If there was the need to regenerate the keymap, the size of the keymap
    /// is returned
    #[cfg(feature = "wayland")]
    pub fn regenerate(&mut self) -> Result<Option<u32>, std::io::Error> {
        use super::{KEYMAP_BEGINNING, KEYMAP_END};

        // Don't do anything if there were no changes
        if !self.needs_regeneration {
            return Ok(None);
        }

        // Create a file to store the layout
        if self.file.is_none() {
            let mut temp_file = tempfile::tempfile()?;
            temp_file.write_all(KEYMAP_BEGINNING)?;
            self.file = Some(temp_file);
        }

        let keymap_file = self
            .file
            .as_mut()
            .expect("There was no file to write to. This should not be possible!");
        // Move the virtual cursor of the file to the end of the part of the keymap that
        // is always the same so we only overwrite the parts that can change.
        keymap_file.seek(SeekFrom::Start(KEYMAP_BEGINNING.len() as u64))?;
        for (&keysym, &keycode) in &self.keymap {
            write!(
                keymap_file,
                "
            key <I{}> {{ [ {} ] }}; // \\n",
                keycode,
                keysym_get_name(keysym)
            )?;
        }
        keymap_file.write_all(KEYMAP_END)?;
        // Truncate the file at the current cursor position in order to cut off any old
        // data in case the keymap was smaller than the old one
        let keymap_len = keymap_file.stream_position()?;
        keymap_file.set_len(keymap_len)?;
        self.needs_regeneration = false;
        match keymap_len.try_into() {
            Ok(v) => Ok(Some(v)),
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "the length of the new keymap exceeds the u32::MAX",
            )),
        }
    }

    /// Tells the keymap that a modifier was pressed
    /// Updates the internal state of the modifiers and returns the new bitflag
    /// representing the state of the modifiers
    pub fn enter_modifier(
        &mut self,
        modifier: ModifierBitflag,
        direction: Direction,
    ) -> ModifierBitflag {
        match direction {
            Direction::Press => {
                self.modifiers |= modifier;
                self.modifiers
            }
            Direction::Release => {
                self.modifiers &= !modifier;
                self.modifiers
            }
            Direction::Click => self.modifiers,
        }
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
