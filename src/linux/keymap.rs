use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::fmt::Display;

use log::debug;
pub(super) use xkbcommon::xkb::Keysym;

use crate::{Direction, InputError, InputResult, Key};

#[cfg(feature = "wayland")]
pub(super) type ModifierBitflag = u32; // TODO: Maybe create a proper type for this

/// The "empty" keyboard symbol.
// TODO: Replace it with the NO_SYMBOL from xkbcommon, once it is available
// there
pub const NO_SYMBOL: Keysym = Keysym::new(0);
#[cfg(feature = "x11rb")]
const DEFAULT_DELAY: u32 = 12;

#[derive(Debug)]
pub struct KeyMap<Keycode> {
    pub(super) keymap: HashMap<Keysym, Keycode>,
    unused_keycodes: VecDeque<Keycode>,
    protected_keycodes: Vec<Keycode>, /* These keycodes cannot be unmapped, because they are
                                       * currently held */
    needs_regeneration: bool,
    #[cfg(feature = "wayland")]
    pub(super) file: Option<std::fs::File>, // Temporary file that contains the keymap
    #[cfg(feature = "wayland")]
    modifiers: ModifierBitflag, // State of the modifiers
    #[cfg(feature = "x11rb")]
    last_keys: Vec<Keycode>, // Last pressed keycodes
    #[cfg(feature = "x11rb")]
    delay: u32, // milliseconds
    #[cfg(feature = "x11rb")]
    pub(super) last_event_before_delays: std::time::Instant, // Time of the last event
    #[cfg(feature = "x11rb")]
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
        let keymap = HashMap::with_capacity(capacity);
        let held_keycodes = vec![];
        let needs_regeneration = true;
        #[cfg(feature = "wayland")]
        let file = None;
        #[cfg(feature = "wayland")]
        let modifiers = 0;
        #[cfg(feature = "x11rb")]
        let last_keys = vec![];
        #[cfg(feature = "x11rb")]
        let delay = DEFAULT_DELAY;
        #[cfg(feature = "x11rb")]
        let last_event_before_delays = std::time::Instant::now();
        #[cfg(feature = "x11rb")]
        let pending_delays = 0;
        Self {
            keymap,
            unused_keycodes,
            protected_keycodes: held_keycodes,
            needs_regeneration,
            #[cfg(feature = "wayland")]
            file,
            #[cfg(feature = "wayland")]
            modifiers,
            #[cfg(feature = "x11rb")]
            last_keys,
            #[cfg(feature = "x11rb")]
            delay,
            #[cfg(feature = "x11rb")]
            last_event_before_delays,
            #[cfg(feature = "x11rb")]
            pending_delays,
        }
    }

    // Try to enter the key
    #[allow(clippy::unnecessary_wraps)]
    pub fn key_to_keycode<C: Bind<Keycode>>(&mut self, c: &C, key: Key) -> InputResult<Keycode> {
        let keycode = match key {
            Key::Raw(kc) => {
                // TODO: Get rid of there weird try_intos and unwraps
                let kcz: usize = kc.try_into().unwrap();
                kcz.try_into().unwrap()
            }
            key => {
                // Unwrapping here is okay, because the fn only returns an error if it was a
                // Key::Raw and we test that before
                let sym = Keysym::try_from(key).unwrap();
                if let Some(&keycode) = self.keymap.get(&sym) {
                    // The keysym is already mapped and cached in the keymap
                    keycode
                } else {
                    // The keysym needs to get mapped to an unused keycode.
                    // Always map the keycode if it has not yet been mapped, so it is layer agnostic
                    self.map(c, sym)?
                }
            }
        };

        Ok(keycode)
    }

    /// Add the Keysym to the keymap
    ///
    /// This does not apply the changes
    pub fn map<C: Bind<Keycode>>(&mut self, c: &C, keysym: Keysym) -> InputResult<Keycode> {
        match self.unused_keycodes.pop_front() {
            // A keycode is unused so a mapping is possible
            Some(unused_keycode) => {
                debug!(
                    "trying to map keycode {} to keysym {:?}",
                    unused_keycode, keysym
                );
                if c.bind_key(unused_keycode, keysym).is_err() {
                    return Err(InputError::Mapping(format!("{keysym:?}")));
                };
                self.needs_regeneration = true;
                self.keymap.insert(keysym, unused_keycode);
                debug!(
                    "Succeeded to map keycode {} to keysym {:?}",
                    unused_keycode, keysym
                );
                Ok(unused_keycode)
            }
            // All keycodes are being used. A mapping is not possible
            None => Err(InputError::Mapping(format!("{keysym:?}"))),
        }
    }

    /// Remove the Keysym from the keymap
    ///
    /// This does not apply the changes
    pub fn unmap<C: Bind<Keycode>>(
        &mut self,
        c: &C,
        keysym: Keysym,
        keycode: Keycode,
    ) -> InputResult<()> {
        debug!("trying to unmap keysym {:?}", keysym);
        if c.bind_key(keycode, NO_SYMBOL).is_err() {
            return Err(InputError::Unmapping(format!("{keysym:?}")));
        };
        self.needs_regeneration = true;
        self.unused_keycodes.push_back(keycode);
        self.keymap.remove(&keysym);
        debug!("Succeeded to unmap keysym {:?}", keysym);
        Ok(())
    }

    // Update the delay
    // TODO: A delay of 1 ms in all cases seems to work on my machine. Maybe
    // this is not needed?
    // TODO: Only needed for x11rb
    #[cfg(feature = "x11rb")]
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
            debug!("delay needed");
            self.last_keys.clear();
        } else {
            debug!("no delay needed");
            self.pending_delays = 1;
        }
        self.last_keys.push(keycode);
    }

    /// Check if there are still unused keycodes available. If there aren't,
    /// make some room by freeing the already mapped keycodes.
    /// Returns true, if keys were unmapped and the keymap needs to be
    /// regenerated
    pub fn make_room<C: Bind<Keycode>>(&mut self, c: &C) -> InputResult<bool> {
        // Unmap all keys, if all keycodes are already being used
        // TODO: Don't unmap the keycodes if they will be needed next
        if self.unused_keycodes.is_empty() {
            let mapped_keys = self.keymap.clone();
            let held_keycodes = self.protected_keycodes.clone();
            let mut made_room = false;

            for (&sym, &keycode) in mapped_keys
                .iter()
                .filter(|(_, keycode)| !held_keycodes.contains(keycode))
            {
                self.unmap(c, sym, keycode)?;
                made_room = true;
            }
            if made_room {
                return Ok(true);
            } else {
                return Err(InputError::Unmapping("all keys that were mapped are also currently held. no way to make room for new mappings".to_string()));
            }
        }
        Ok(false)
    }

    /// Regenerate the keymap if there were any changes
    /// and write the new keymap to a temporary file
    ///
    /// If there was the need to regenerate the keymap, the size of the keymap
    /// is returned
    #[cfg(feature = "wayland")]
    pub fn regenerate(&mut self) -> Result<Option<u32>, std::io::Error> {
        use super::{KEYMAP_BEGINNING, KEYMAP_END};
        use std::io::{Seek, SeekFrom, Write};
        use xkbcommon::xkb::keysym_get_name;

        // Don't do anything if there were no changes
        if !self.needs_regeneration {
            debug!("keymap did not change and does not require regeneration");
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
            Ok(v) => {
                debug!("regenerated the keymap");
                Ok(Some(v))
            }
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "the length of the new keymap exceeds the u32::MAX",
            )),
        }
    }

    /// Tells the keymap that a modifier was pressed
    /// Updates the internal state of the modifiers and returns the new bitflag
    /// representing the state of the modifiers
    #[cfg(feature = "wayland")]
    pub fn enter_modifier(
        &mut self,
        modifier: ModifierBitflag,
        direction: crate::Direction,
    ) -> ModifierBitflag {
        match direction {
            crate::Direction::Press => {
                self.modifiers |= modifier;
                self.modifiers
            }
            crate::Direction::Release => {
                self.modifiers &= !modifier;
                self.modifiers
            }
            crate::Direction::Click => self.modifiers,
        }
    }

    pub fn enter_key(&mut self, keycode: Keycode, direction: Direction) {
        match direction {
            Direction::Press => {
                debug!("added the key {keycode} to the held keycodes");
                self.protected_keycodes.push(keycode);
            }
            Direction::Release => {
                debug!("removed the key {keycode} from the held keycodes");
                self.protected_keycodes.retain(|&k| k != keycode);
            }
            Direction::Click => (),
        }
    }
}

pub trait Bind<Keycode> {
    // Map the keysym to the given keycode
    // Only use keycodes that are not used, otherwise the existing mapping is
    // overwritten
    // If the keycode is mapped to the NoSymbol keysym, the key is unbinded and can
    // get used again later
    fn bind_key(&self, _: Keycode, _: Keysym) -> Result<(), ()> {
        Ok(()) // No need to do anything
    }
}

impl<Keycode> Bind<Keycode> for () {}
