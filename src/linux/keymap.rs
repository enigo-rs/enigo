use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;

use log::{debug, trace};
pub(super) use xkeysym::{KeyCode, Keysym};

use crate::{Direction, InputError, InputResult, Key};

const DEFAULT_DELAY: u32 = 12;
pub type Keycode = u8;

#[derive(Debug)]
pub(super) struct KeyMapMapping {
    pub(super) additionally_mapped: HashMap<Keysym, Keycode>,
    keycode_min: Keycode,
    keycode_max: Keycode,
    keysyms_per_keycode: u8,
    keysyms: Vec<u32>,
    unused_keycodes: VecDeque<Keycode>,
}

#[derive(Debug)]
struct KeyMapState {}

#[derive(Debug)]
pub struct KeyMap {
    pub(super) keymap_mapping: KeyMapMapping,
    keymap_state: KeyMapState,
    delay: u32,
}

impl KeyMap {
    /// Create a new `KeyMap`
    pub fn new(
        keycode_min: Keycode,
        keycode_max: Keycode,
        unused_keycodes: VecDeque<Keycode>,
        keysyms_per_keycode: u8,
        keysyms: Vec<u32>,
    ) -> Self {
        let capacity: usize = (keycode_max - keycode_min) as usize;
        let capacity = capacity + 1;
        let keymap = HashMap::with_capacity(capacity);

        let keymap_state = KeyMapState {};

        let keymap_mapping = KeyMapMapping {
            additionally_mapped: keymap,
            keycode_min,
            keycode_max,
            keysyms_per_keycode,
            keysyms,
            unused_keycodes,
        };

        let delay = DEFAULT_DELAY;

        Self {
            keymap_mapping,
            keymap_state,
            delay,
        }
    }

    fn keysym_to_keycode(&self, keysym: Keysym) -> Option<Keycode> {
        let keycode_min = self.keymap_mapping.keycode_min;
        let keycode_max = self.keymap_mapping.keycode_max;

        // TODO: Change this range to 0..self.keysyms_per_keycode once we find out how
        // to detect the level and switch it
        for j in 0..1 {
            for i in keycode_min..=keycode_max {
                let min_keycode: u32 = keycode_min.into();
                let keycode = KeyCode::from(i);
                let min_keycode = KeyCode::from(min_keycode);
                if let Some(ks) = xkeysym::keysym(
                    keycode,
                    j,
                    min_keycode,
                    self.keymap_mapping.keysyms_per_keycode,
                    &self.keymap_mapping.keysyms,
                ) {
                    if ks == keysym {
                        trace!("found keysym in row {i}, col {j}");
                        return Some(i);
                    }
                }
            }
        }
        None
    }

    // Try to enter the key
    #[allow(clippy::unnecessary_wraps)]
    pub fn key_to_keycode<C: Bind>(&mut self, c: &C, key: Key) -> InputResult<Keycode> {
        let sym = Keysym::from(key);

        if let Some(keycode) = self.keysym_to_keycode(sym) {
            return Ok(keycode);
        }

        let keycode = {
            if let Some(&keycode) = self.keymap_mapping.additionally_mapped.get(&sym) {
                // The keysym is already mapped and cached in the keymap
                keycode
            } else {
                // Unmap keysyms if there are no unused keycodes
                self.make_room(c)?;
                // The keysym needs to get mapped to an unused keycode.
                // Always map the keycode if it has not yet been mapped, so it is layer agnostic
                self.map(c, sym)?
            }
        };

        // self.update_delays(keycode);
        Ok(keycode)
    }

    /// Add the Keysym to the keymap
    ///
    /// This does not apply the changes
    pub fn map<C: Bind>(&mut self, c: &C, keysym: Keysym) -> InputResult<Keycode> {
        match self.keymap_mapping.unused_keycodes.pop_front() {
            // A keycode is unused so a mapping is possible
            Some(unused_keycode) => {
                trace!("trying to map keycode {unused_keycode} to keysym {keysym:?}");
                if c.bind_key(unused_keycode, keysym).is_err() {
                    return Err(InputError::Mapping(format!("{keysym:?}")));
                }
                self.keymap_mapping
                    .additionally_mapped
                    .insert(keysym, unused_keycode);
                debug!("mapped keycode {unused_keycode} to keysym {keysym:?}");
                Ok(unused_keycode)
            }
            // All keycodes are being used. A mapping is not possible
            None => Err(InputError::Mapping(format!("{keysym:?}"))),
        }
    }

    /// Remove the Keysym from the keymap
    ///
    /// This does not apply the changes
    pub fn unmap<C: Bind>(&mut self, c: &C, keysym: Keysym, keycode: Keycode) -> InputResult<()> {
        trace!("trying to unmap keysym {keysym:?}");
        if c.bind_key(keycode, Keysym::NoSymbol).is_err() {
            return Err(InputError::Unmapping(format!("{keysym:?}")));
        }
        self.keymap_mapping.unused_keycodes.push_back(keycode);
        self.keymap_mapping.additionally_mapped.remove(&keysym);
        debug!("unmapped keysym {keysym:?}");
        Ok(())
    }

    /// Check if there are still unused keycodes available. If there aren't,
    /// make some room by freeing the already mapped keycodes.
    /// Returns true, if keys were unmapped and the keymap needs to be
    /// regenerated
    fn make_room<C: Bind>(&mut self, c: &C) -> InputResult<()> {
        // Unmap all keys, if all keycodes are already being used
        if self.keymap_mapping.unused_keycodes.is_empty() {
            let mapped_keys = self.keymap_mapping.additionally_mapped.clone();
            let held_keycodes = self.keymap_state.held_keycodes.clone();
            let mut made_room = false;

            for (&sym, &keycode) in mapped_keys
                .iter()
                .filter(|(_, keycode)| !held_keycodes.contains(keycode))
            {
                self.unmap(c, sym, keycode)?;
                made_room = true;
            }
            if made_room {
                return Ok(());
            }
            return Err(InputError::Unmapping("all keys that were mapped are also currently held. no way to make room for new mappings".to_string()));
        }
        Ok(())
    }

    pub fn key(&mut self, keycode: Keycode, direction: Direction) {
        match direction {
            Direction::Press => {
                debug!("added the key {keycode} to the held keycodes");
                self.keymap_state.held_keycodes.push(keycode);
            }
            Direction::Release => {
                debug!("removed the key {keycode} from the held keycodes");
                self.keymap_state.held_keycodes.retain(|&k| k != keycode);
            }
            Direction::Click => (),
        }
    }
}

pub trait Bind {
    // Map the keysym to the given keycode
    // Only use keycodes that are not used, otherwise the existing mapping is
    // overwritten
    // If the keycode is mapped to the NoSymbol keysym, the key is unbound and can
    // get used again later
    fn bind_key(&self, _: Keycode, _: Keysym) -> Result<(), ()> {
        Ok(()) // No need to do anything
    }
}

impl Bind for () {}
