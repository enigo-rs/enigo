use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::fmt::Display;

use log::{debug, trace};
pub(super) use xkeysym::{KeyCode, Keysym};

use crate::{Direction, InputError, InputResult, Key};

#[derive(Debug)]
pub(super) struct KeyMapMapping<Keycode> {
    pub(super) additionally_mapped: HashMap<Keysym, Keycode>,
    keycode_min: Keycode,
    keycode_max: Keycode,
    keysyms_per_keycode: u8,
    keysyms: Vec<u32>,
    unused_keycodes: VecDeque<Keycode>,
}

#[derive(Debug)]
struct KeyMapState<Keycode> {
    held_keycodes: Vec<Keycode>, // cannot get unmapped
    needs_regeneration: bool,
}

#[derive(Debug)]
pub struct KeyMap<Keycode> {
    pub(super) keymap_mapping: KeyMapMapping<Keycode>,
    keymap_state: KeyMapState<Keycode>,
}

impl<Keycode> KeyMap<Keycode>
where
    Keycode: Copy + Clone + PartialEq + Display,
    Keycode: TryInto<usize> + TryFrom<usize>,
    <Keycode as TryInto<usize>>::Error: std::fmt::Debug,
    <Keycode as TryFrom<usize>>::Error: std::fmt::Debug,
{
    /// Create a new `KeyMap`
    pub fn new(
        keycode_min: Keycode,
        keycode_max: Keycode,
        unused_keycodes: VecDeque<Keycode>,
        keysyms_per_keycode: u8,
        keysyms: Vec<u32>,
    ) -> Self {
        let capacity: usize = keycode_max.try_into().unwrap() - keycode_min.try_into().unwrap();
        let capacity = capacity + 1;
        let keymap = HashMap::with_capacity(capacity);

        let keymap_state = KeyMapState {
            held_keycodes: vec![],
            needs_regeneration: true,
        };

        let keymap_mapping = KeyMapMapping {
            additionally_mapped: keymap,
            keycode_min,
            keycode_max,
            keysyms_per_keycode,
            keysyms,
            unused_keycodes,
        };

        Self {
            keymap_mapping,
            keymap_state,
        }
    }

    /// Replace the cached server keymap while retaining additional mappings
    /// that are still present.
    pub fn refresh(
        &mut self,
        unused_keycodes: VecDeque<Keycode>,
        keysyms_per_keycode: u8,
        keysyms: Vec<u32>,
    ) {
        self.keymap_mapping
            .additionally_mapped
            .retain(|&keysym, &mut keycode| {
                Self::mapping_contains(
                    self.keymap_mapping.keycode_min,
                    keysyms_per_keycode,
                    &keysyms,
                    keycode,
                    keysym,
                )
            });
        self.keymap_mapping.keysyms_per_keycode = keysyms_per_keycode;
        self.keymap_mapping.keysyms = keysyms;
        self.keymap_mapping.unused_keycodes = unused_keycodes;
    }

    fn mapping_contains(
        keycode_min: Keycode,
        keysyms_per_keycode: u8,
        keysyms: &[u32],
        keycode: Keycode,
        keysym: Keysym,
    ) -> bool {
        let keycode_min: usize = keycode_min.try_into().unwrap();
        let keycode: usize = keycode.try_into().unwrap();
        let keysyms_per_keycode = keysyms_per_keycode as usize;
        let start = (keycode - keycode_min) * keysyms_per_keycode;
        let end = start + keysyms_per_keycode;

        keysyms
            .get(start..end)
            .is_some_and(|mapping| mapping.contains(&keysym.raw()))
    }

    fn update_mapping(&mut self, keycode: Keycode, keysym: Keysym) {
        let keycode_min: usize = self.keymap_mapping.keycode_min.try_into().unwrap();
        let keycode: usize = keycode.try_into().unwrap();
        let keysyms_per_keycode = self.keymap_mapping.keysyms_per_keycode as usize;
        let start = (keycode - keycode_min) * keysyms_per_keycode;
        let end = start + keysyms_per_keycode;
        let Some(mapping) = self.keymap_mapping.keysyms.get_mut(start..end) else {
            return;
        };

        mapping.fill(Keysym::NoSymbol.raw());
        if keysym != Keysym::NoSymbol {
            mapping.iter_mut().take(2).for_each(|sym| {
                *sym = keysym.raw();
            });
        }
    }

    fn keysym_to_keycode(&self, keysym: Keysym) -> Option<Keycode> {
        let keycode_min: usize = self.keymap_mapping.keycode_min.try_into().unwrap();
        let keycode_max: usize = self.keymap_mapping.keycode_max.try_into().unwrap();

        // TODO: Change this range to 0..self.keysyms_per_keycode once we find out how
        // to detect the level and switch it
        for j in 0..1 {
            for i in keycode_min..=keycode_max {
                let i: u32 = i.try_into().unwrap();
                let min_keycode: u32 = keycode_min.try_into().unwrap();
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
                        let i: usize = i.try_into().unwrap();
                        let i: Keycode = i.try_into().unwrap();
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
    pub fn key_to_keycode<C: Bind<Keycode>>(&mut self, c: &C, key: Key) -> InputResult<Keycode> {
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

        Ok(keycode)
    }

    /// Add the Keysym to the keymap
    ///
    /// This does not apply the changes
    pub fn map<C: Bind<Keycode>>(&mut self, c: &C, keysym: Keysym) -> InputResult<Keycode> {
        match self.keymap_mapping.unused_keycodes.pop_front() {
            // A keycode is unused so a mapping is possible
            Some(unused_keycode) => {
                trace!("trying to map keycode {unused_keycode} to keysym {keysym:?}");
                if c.bind_key(unused_keycode, keysym).is_err() {
                    return Err(InputError::Mapping(format!("{keysym:?}")));
                }
                self.keymap_state.needs_regeneration = true;
                self.keymap_mapping
                    .additionally_mapped
                    .insert(keysym, unused_keycode);
                self.update_mapping(unused_keycode, keysym);
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
    pub fn unmap<C: Bind<Keycode>>(
        &mut self,
        c: &C,
        keysym: Keysym,
        keycode: Keycode,
    ) -> InputResult<()> {
        trace!("trying to unmap keysym {keysym:?}");
        if c.bind_key(keycode, Keysym::NoSymbol).is_err() {
            return Err(InputError::Unmapping(format!("{keysym:?}")));
        }
        self.keymap_state.needs_regeneration = true;
        self.keymap_mapping.unused_keycodes.push_back(keycode);
        self.keymap_mapping.additionally_mapped.remove(&keysym);
        self.update_mapping(keycode, Keysym::NoSymbol);
        debug!("unmapped keysym {keysym:?}");
        Ok(())
    }

    /// Check if there are still unused keycodes available. If there aren't,
    /// make some room by freeing the already mapped keycodes.
    /// Returns true, if keys were unmapped and the keymap needs to be
    /// regenerated
    fn make_room<C: Bind<Keycode>>(&mut self, c: &C) -> InputResult<()> {
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

pub trait Bind<Keycode> {
    // Map the keysym to the given keycode
    // Only use keycodes that are not used, otherwise the existing mapping is
    // overwritten
    // If the keycode is mapped to the NoSymbol keysym, the key is unbound and can
    // get used again later
    fn bind_key(&self, _: Keycode, _: Keysym) -> Result<(), ()> {
        Ok(()) // No need to do anything
    }
}

impl<Keycode> Bind<Keycode> for () {}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    struct Binder {
        mappings: RefCell<Vec<(u8, Keysym)>>,
    }

    impl Bind<u8> for Binder {
        fn bind_key(&self, keycode: u8, keysym: Keysym) -> Result<(), ()> {
            self.mappings.borrow_mut().push((keycode, keysym));
            Ok(())
        }
    }

    fn keymap() -> KeyMap<u8> {
        KeyMap::new(
            8,
            9,
            VecDeque::from([9]),
            2,
            vec![Keysym::from(Key::Unicode('a')).raw(), 0, 0, 0],
        )
    }

    #[test]
    fn updates_cached_mapping_when_mapping_and_unmapping() {
        let binder = Binder::default();
        let mut keymap = keymap();
        let keysym = Keysym::from(Key::Unicode('€'));

        let keycode = keymap.map(&binder, keysym).unwrap();
        assert_eq!(keymap.keysym_to_keycode(keysym), Some(keycode));

        keymap.unmap(&binder, keysym, keycode).unwrap();
        assert_eq!(keymap.keysym_to_keycode(keysym), None);
    }

    #[test]
    fn refresh_discards_additional_mappings_removed_by_server() {
        let binder = Binder::default();
        let mut keymap = keymap();
        let keysym = Keysym::from(Key::Unicode('€'));
        let keycode = keymap.map(&binder, keysym).unwrap();

        keymap.refresh(
            VecDeque::from([keycode]),
            2,
            vec![Keysym::from(Key::Unicode('a')).raw(), 0, 0, 0],
        );

        assert!(keymap.keymap_mapping.additionally_mapped.is_empty());
        assert_eq!(
            keymap.key_to_keycode(&binder, Key::Unicode('€')).unwrap(),
            keycode
        );
    }

    #[test]
    fn refresh_retains_additional_mappings_still_present_on_server() {
        let binder = Binder::default();
        let mut keymap = keymap();
        let keysym = Keysym::from(Key::Unicode('€'));
        let keycode = keymap.map(&binder, keysym).unwrap();

        keymap.refresh(
            VecDeque::new(),
            2,
            vec![
                Keysym::from(Key::Unicode('a')).raw(),
                0,
                keysym.raw(),
                keysym.raw(),
            ],
        );

        assert_eq!(
            keymap.keymap_mapping.additionally_mapped.get(&keysym),
            Some(&keycode)
        );
        assert_eq!(
            keymap.key_to_keycode(&binder, Key::Unicode('€')).unwrap(),
            keycode
        );
    }
}
