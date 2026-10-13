use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::fmt::Display;
use std::hash::Hash;
use std::time::{Duration, Instant};

use log::{debug, trace};
use x11rb::protocol::xproto::Window;
pub(super) use xkeysym::{KeyCode, Keysym};

use crate::{Direction, InputError, InputResult, Key};

/// How long a keycode must go unused before it can get rebound, if the client
/// cannot confirm it read the key events. Clients that lag behind look up
/// earlier key events under the binding at the time they read them.
const REBIND_IDLE: Duration = Duration::from_millis(150);

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
    last_used: HashMap<Keycode, Vec<(Window, Instant)>>, // last use per focus window
    // the client of the window read all key events sent before the instant
    synced: Option<(Window, Instant)>,
}

#[derive(Debug)]
pub struct KeyMap<Keycode> {
    pub(super) keymap_mapping: KeyMapMapping<Keycode>,
    keymap_state: KeyMapState<Keycode>,
}

impl<Keycode> KeyMap<Keycode>
where
    Keycode: Copy + Clone + Eq + Hash + Display,
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
            last_used: HashMap::new(),
            synced: None,
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
        self.keymap_state.last_used.remove(&keycode);
        self.keymap_mapping.unused_keycodes.push_back(keycode);
        self.keymap_mapping.additionally_mapped.remove(&keysym);
        debug!("unmapped keysym {keysym:?}");
        Ok(())
    }

    /// Unmap all keysyms that were mapped, once the client read the key
    /// events sent with them
    pub fn unmap_everything<C: Bind<Keycode>>(&mut self, c: &C) -> InputResult<()> {
        let mapped_keys = self.keymap_mapping.additionally_mapped.clone();
        let mut uses: Vec<(Window, Instant)> = mapped_keys
            .values()
            .filter_map(|keycode| self.keymap_state.last_used.get(keycode))
            .flatten()
            .copied()
            .collect();
        uses.sort_unstable_by(|a, b| b.cmp(a));
        uses.dedup_by_key(|&mut (window, _)| window);
        for (window, used) in uses {
            self.wait_read(c, window, used);
        }
        for (sym, keycode) in mapped_keys {
            self.unmap(c, sym, keycode)?;
        }
        Ok(())
    }

    /// Check if there are still unused keycodes available. If there aren't,
    /// make room by unmapping the least recently used keysym that is not
    /// held, once the client read the key events sent with it
    fn make_room<C: Bind<Keycode>>(&mut self, c: &C) -> InputResult<()> {
        if !self.keymap_mapping.unused_keycodes.is_empty() {
            return Ok(());
        }
        let Some((sym, keycode)) = self
            .keymap_mapping
            .additionally_mapped
            .iter()
            .filter(|(_, keycode)| !self.keymap_state.held_keycodes.contains(keycode))
            .min_by_key(|(_, keycode)| {
                self.keymap_state
                    .last_used
                    .get(keycode)
                    .and_then(|uses| uses.iter().map(|&(_, used)| used).max())
            })
            .map(|(&sym, &keycode)| (sym, keycode))
        else {
            return Err(InputError::Unmapping("all keys that were mapped are also currently held. no way to make room for new mappings".to_string()));
        };
        let uses = self
            .keymap_state
            .last_used
            .get(&keycode)
            .cloned()
            .unwrap_or_default();
        for (window, used) in uses {
            self.wait_read(c, window, used);
        }
        self.unmap(c, sym, keycode)
    }

    /// Wait until the client of `window` read the key events sent to it up
    /// to `last_used`
    fn wait_read<C: Bind<Keycode>>(&mut self, c: &C, window: Window, last_used: Instant) {
        if self
            .keymap_state
            .synced
            .is_some_and(|(synced_window, synced)| synced_window == window && last_used < synced)
        {
            return;
        }
        let sent = Instant::now();
        match c.sync(window) {
            FocusSync::CaughtUp => self.keymap_state.synced = Some((window, sent)),
            FocusSync::Unavailable | FocusSync::TimedOut => sleep_until(last_used + REBIND_IDLE),
        }
    }

    /// Record a key event sent to the `focus` window
    pub fn key(&mut self, keycode: Keycode, direction: Direction, focus: Window) {
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
        if !self
            .keymap_mapping
            .additionally_mapped
            .values()
            .any(|&mapped| mapped == keycode)
        {
            return;
        }
        let now = Instant::now();
        let history = self.keymap_state.last_used.entry(keycode).or_default();
        match history.iter_mut().find(|(window, _)| *window == focus) {
            Some((_, used)) => *used = now,
            None => history.push((focus, now)),
        }
    }
}

fn sleep_until(deadline: Instant) {
    let wait = deadline.saturating_duration_since(Instant::now());
    if !wait.is_zero() {
        debug!("waiting {wait:?} before rebinding a keycode");
        std::thread::sleep(wait);
    }
}

/// How the client of the window that got key events answered [`Bind::sync`]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusSync {
    /// It answered, so it read all key events sent to it before
    CaughtUp,
    /// No window of the client takes `_NET_WM_PING`
    Unavailable,
    /// It takes `_NET_WM_PING` but did not answer in time
    TimedOut,
}

/// Binds keysyms to keycodes, and finds out if a client read the key events
/// sent with a binding, so its keycode can get rebound
pub trait Bind<Keycode> {
    // Map the keysym to the given keycode
    // Only use keycodes that are not used, otherwise the existing mapping is
    // overwritten
    // If the keycode is mapped to the NoSymbol keysym, the key is unbound and can
    // get used again later
    fn bind_key(&self, _: Keycode, _: Keysym) -> Result<(), ()> {
        Ok(()) // No need to do anything
    }

    // Wait until the client of the window read all key events sent so far
    fn sync(&self, _: Window) -> FocusSync {
        FocusSync::Unavailable
    }
}

impl<Keycode> Bind<Keycode> for () {}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    const WINDOW: Window = 1;

    fn keymap() -> KeyMap<u8> {
        let keysyms = vec![0; (255 - 8 + 1) * 2];
        KeyMap::new(8, 255, VecDeque::from([200, 201]), 2, keysyms)
    }

    /// Answers every sync with `answer` and records the pinged windows
    struct Pings {
        answer: FocusSync,
        windows: RefCell<Vec<Window>>,
    }

    impl Pings {
        fn new(answer: FocusSync) -> Self {
            Self {
                answer,
                windows: RefCell::new(vec![]),
            }
        }
    }

    impl Bind<u8> for Pings {
        fn sync(&self, window: Window) -> FocusSync {
            self.windows.borrow_mut().push(window);
            self.answer
        }
    }

    fn type_key(keymap: &mut KeyMap<u8>, c: char) -> u8 {
        type_key_into(keymap, &(), WINDOW, c)
    }

    fn type_key_into<C: Bind<u8>>(
        keymap: &mut KeyMap<u8>,
        binder: &C,
        focus: Window,
        c: char,
    ) -> u8 {
        let keycode = keymap.key_to_keycode(binder, Key::Unicode(c)).unwrap();
        keymap.key(keycode, Direction::Click, focus);
        keycode
    }

    #[test]
    fn rebinds_the_least_recently_used_keycode_once_idle() {
        let mut keymap = keymap();
        let start = Instant::now();
        assert_eq!(type_key(&mut keymap, 'ä'), 200);
        assert_eq!(type_key(&mut keymap, 'ö'), 201);
        assert_eq!(type_key(&mut keymap, 'ä'), 200);

        assert_eq!(type_key(&mut keymap, 'ü'), 201);
        assert!(start.elapsed() >= REBIND_IDLE);
        let mapped = &keymap.keymap_mapping.additionally_mapped;
        assert_eq!(mapped.get(&Keysym::from(Key::Unicode('ä'))), Some(&200));
        assert_eq!(mapped.get(&Keysym::from(Key::Unicode('ü'))), Some(&201));
        assert_eq!(mapped.len(), 2);
    }

    #[test]
    fn idle_keycodes_rebind_without_waiting() {
        let mut keymap = keymap();
        type_key(&mut keymap, 'ä');
        type_key(&mut keymap, 'ö');
        let idle = Instant::now().checked_sub(REBIND_IDLE).unwrap();
        keymap
            .keymap_state
            .last_used
            .insert(200, vec![(WINDOW, idle)]);
        keymap
            .keymap_state
            .last_used
            .insert(201, vec![(WINDOW, idle)]);

        let start = Instant::now();
        type_key(&mut keymap, 'ü');
        assert!(start.elapsed() < REBIND_IDLE);
    }

    #[test]
    fn a_synced_client_needs_no_wait_and_one_sync_per_round() {
        let mut keymap = keymap();
        let pings = Pings::new(FocusSync::CaughtUp);
        let start = Instant::now();
        type_key_into(&mut keymap, &pings, WINDOW, 'ä');
        type_key_into(&mut keymap, &pings, WINDOW, 'ö');

        assert_eq!(type_key_into(&mut keymap, &pings, WINDOW, 'ü'), 200);
        assert_eq!(type_key_into(&mut keymap, &pings, WINDOW, 'é'), 201);
        assert_eq!(pings.windows.borrow().len(), 1);
        assert_eq!(type_key_into(&mut keymap, &pings, WINDOW, 'è'), 200);
        assert_eq!(pings.windows.borrow().len(), 2);
        assert!(start.elapsed() < REBIND_IDLE);
    }

    #[test]
    fn a_timeout_does_not_advance_the_sync_point() {
        let mut keymap = keymap();
        let pings = Pings::new(FocusSync::TimedOut);
        let start = Instant::now();
        type_key_into(&mut keymap, &pings, WINDOW, 'ä');
        type_key_into(&mut keymap, &pings, WINDOW, 'ö');

        type_key_into(&mut keymap, &pings, WINDOW, 'ü');
        assert!(start.elapsed() >= REBIND_IDLE);
        assert_eq!(keymap.keymap_state.synced, None);
        type_key_into(&mut keymap, &pings, WINDOW, 'é');
        assert_eq!(*pings.windows.borrow(), [WINDOW, WINDOW]);
    }

    #[test]
    fn a_sync_point_covers_only_its_window() {
        let mut keymap = keymap();
        let pings = Pings::new(FocusSync::CaughtUp);
        type_key_into(&mut keymap, &pings, 1, 'ä');
        type_key_into(&mut keymap, &pings, 1, 'ö');

        type_key_into(&mut keymap, &pings, 2, 'ü');
        type_key_into(&mut keymap, &pings, 2, 'é');
        assert_eq!(*pings.windows.borrow(), [1]);
        type_key_into(&mut keymap, &pings, 2, 'è');
        assert_eq!(*pings.windows.borrow(), [1, 2]);
    }

    #[test]
    fn a_keycode_waits_for_every_window_it_was_sent_to() {
        let mut keymap = keymap();
        let pings = Pings::new(FocusSync::CaughtUp);
        type_key_into(&mut keymap, &pings, 1, 'ä');
        type_key_into(&mut keymap, &pings, 2, 'ä');
        type_key_into(&mut keymap, &pings, 2, 'ö');

        assert_eq!(type_key_into(&mut keymap, &pings, 2, 'ü'), 200);
        assert_eq!(*pings.windows.borrow(), [1, 2]);
    }

    #[test]
    fn held_keycodes_are_not_rebound() {
        let mut keymap = keymap();
        let held = keymap.key_to_keycode(&(), Key::Unicode('ä')).unwrap();
        keymap.key(held, Direction::Press, WINDOW);
        type_key(&mut keymap, 'ö');

        let held = keymap.key_to_keycode(&(), Key::Unicode('ü')).unwrap();
        assert_eq!(held, 201);
        keymap.key(held, Direction::Press, WINDOW);
        keymap.key_to_keycode(&(), Key::Unicode('é')).unwrap_err();
    }

    #[test]
    fn unmap_everything_waits_for_the_last_key() {
        let mut keymap = keymap();
        let start = Instant::now();
        type_key(&mut keymap, 'ä');
        type_key(&mut keymap, 'ö');

        keymap.unmap_everything(&()).unwrap();
        assert!(start.elapsed() >= REBIND_IDLE);
        assert!(keymap.keymap_mapping.additionally_mapped.is_empty());
        assert_eq!(keymap.keymap_mapping.unused_keycodes.len(), 2);
    }

    #[test]
    fn unmap_everything_syncs_each_window_once() {
        let mut keymap = keymap();
        let pings = Pings::new(FocusSync::TimedOut);
        type_key_into(&mut keymap, &pings, 1, 'ä');
        type_key_into(&mut keymap, &pings, 2, 'ö');
        type_key_into(&mut keymap, &pings, 1, 'ö');

        keymap.unmap_everything(&pings).unwrap();
        let mut windows = pings.windows.borrow().clone();
        windows.sort_unstable();
        assert_eq!(windows, [1, 2]);
    }
}
