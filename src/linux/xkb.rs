use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};
#[cfg(feature = "wayland")]
use std::{
    fs::File,
    io::{Read as _, Seek as _, SeekFrom},
    os::fd::OwnedFd,
};

#[cfg(feature = "wayland")]
use log::error;
use log::{debug, trace, warn};
#[cfg(any(feature = "wayland", test))]
use xkbcommon::xkb::KeymapFormat;
use xkbcommon::xkb::{
    self, Context, KEYMAP_COMPILE_NO_FLAGS, KEYMAP_FORMAT_TEXT_V1, KeyDirection, Keycode, Keysym,
    LayoutMask, ModMask, STATE_LAYOUT_DEPRESSED, STATE_LAYOUT_EFFECTIVE, STATE_LAYOUT_LATCHED,
    STATE_LAYOUT_LOCKED, STATE_MODS_DEPRESSED, STATE_MODS_LATCHED, STATE_MODS_LOCKED, State,
};

use super::MAX_KEYCODE;
#[cfg(feature = "wayland")]
use super::MAX_KEYMAP_SIZE;
use crate::{InputError, InputResult, Key};

/// A single source of truth for the XKB keymap and keyboard state.
///
/// It keeps the original keymap that the compositor sent, tracks the current
/// modifier/layout state with `xkb::State`, and remembers the keysyms that were
/// dynamically added to the keymap (with the keycode they were mapped to and
/// when that keycode was last used).
pub struct Keymap {
    context: Context,
    /// The compositor keymap serialized to the canonical `TEXT_V1` form. We
    /// regenerate the active keymap from this string whenever we add or remove
    /// a mapping, so we never have to parse the original keymap by hand.
    source: String,
    /// The name of each free keycode that is declared in `source`, keyed by raw
    /// keycode. Used to reference an existing keycode when adding a mapping
    /// without redeclaring it.
    names: HashMap<u32, String>,
    active: xkb::Keymap,
    state: State,
    /// Keycodes that produce at least one symbol under some layout and level.
    /// Lookups iterate this list instead of the whole declared keycode range.
    symbol_keycodes: Vec<Keycode>,
    /// Keysyms that were not present in the compositor keymap and were added
    /// dynamically, with the keycode that now produces them and its last use.
    additionally_mapped: HashMap<Keysym, MappedKey>,
    /// Keycodes that are currently held down. A held keycode must never be
    /// unmapped while it is still held, otherwise releasing it would be
    /// impossible.
    pressed_keys: HashSet<Keycode>,
    /// Keycodes that still have no symbols and have not been used for a dynamic
    /// mapping, in ascending order. Popping the front yields the lowest free
    /// keycode.
    free_keycodes: VecDeque<Keycode>,
    /// The XKB type and level count to use for dynamically added keys. The
    /// keysym is written to every level of that type so it is produced no
    /// matter which modifier combination is active.
    fill: (&'static str, usize),
    /// How long a dynamically mapped keycode must be idle before it may be
    /// reused. X11 sets a conservative value because the client that received
    /// the earlier key events may still translate them with the old binding;
    /// Wayland leaves it at zero because the compositor owns translation.
    min_reuse_delay: Duration,
}

/// A dynamically added mapping: the keycode that now produces a keysym, and
/// when that keycode was last used to send a key event. Keeping both together
/// means a timestamp can never outlive its mapping.
#[derive(Clone)]
struct MappedKey {
    keycode: Keycode,
    /// `None` until the keycode has been used. Decides which mapping to reuse
    /// when no free keycode is left.
    last_used: Option<Instant>,
}

// SAFETY: `xkb::Context`, `xkb::Keymap` and `xkb::State` wrap non-atomic raw
// pointers and are therefore not auto-`Send`. `Keymap` remains the sole owner
// of these pointers for its lifetime (it is not `Clone` and never exposes the
// raw pointers), so moving it to another thread is safe.
unsafe impl Send for Keymap {}

impl Keymap {
    /// Create a keymap from a keymap string, configuring how long a dynamically
    /// mapped keycode must be idle before it may be reused. Wayland passes
    /// [`Duration::ZERO`] because the compositor owns translation; X11 uses
    /// [`Self::from_keymap`] with a conservative delay.
    #[cfg(any(feature = "wayland", test))]
    pub fn from_string(
        context: Context,
        mut string: String,
        format: KeymapFormat,
        min_reuse_delay: Duration,
    ) -> Option<Self> {
        while string.ends_with('\0') {
            string.pop();
        }
        let keymap =
            xkb::Keymap::new_from_string(&context, string, format, KEYMAP_COMPILE_NO_FLAGS)?;
        Some(Self::new(context, keymap, min_reuse_delay))
    }

    /// Build a [`Keymap`] from an already-compiled keymap, for example one
    /// fetched from the X server through the XKB extension.
    #[cfg(feature = "x11rb")]
    pub fn from_keymap(context: Context, keymap: xkb::Keymap, min_reuse_delay: Duration) -> Self {
        Self::new(context, keymap, min_reuse_delay)
    }

    /// Create a keymap from the file descriptor the Wayland compositor sent.
    #[cfg(feature = "wayland")]
    pub fn from_fd(
        context: Context,
        format: KeymapFormat,
        fd: OwnedFd,
        size: u32,
    ) -> Result<Self, ()> {
        let size = size as usize;
        if size == 0 || size > MAX_KEYMAP_SIZE {
            error!("keymap size {size} is outside 1..={MAX_KEYMAP_SIZE}; resetting the keymap");
            return Err(());
        }

        let mut keymap_file = File::from(fd);
        // The compositor hands us a fresh file descriptor, but seek to the
        // start anyway to be robust against a descriptor that was already read.
        keymap_file.seek(SeekFrom::Start(0)).map_err(|e| {
            error!("unable to seek to the start of the keymap file: {e}");
        })?;

        let mut string = String::new();
        // Never read more than the size the compositor announced, even if the
        // descriptor itself is larger.
        keymap_file
            .take(size as u64)
            .read_to_string(&mut string)
            .map_err(|e| {
                error!("unable to read the keymap file: {e}");
            })?;

        Self::from_string(context, string, format, Duration::ZERO).ok_or_else(|| {
            error!("unable to compile the keymap with xkbcommon");
        })
    }

    /// Compile the default `us` layout. Used as a fallback when the compositor
    /// never sent a keymap (for example headless Sway without a physical
    /// keyboard).
    #[cfg(feature = "wayland")]
    pub fn default() -> Result<Self, ()> {
        let context = Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap =
            xkb::Keymap::new_from_names(&context, "", "", "us", "", None, KEYMAP_COMPILE_NO_FLAGS)
                .ok_or_else(|| {
                    error!("unable to compile the default keymap");
                })?;
        Ok(Self::new(context, keymap, Duration::ZERO))
    }

    fn new(context: Context, keymap: xkb::Keymap, min_reuse_delay: Duration) -> Self {
        let min = keymap.min_keycode().raw();
        let max = keymap.max_keycode().raw();
        debug!("keymap declares keycodes {min}..={max}");
        if max > MAX_KEYCODE {
            warn!(
                "keymap declares maximum keycode {max}, above the usable {MAX_KEYCODE}; ignoring higher keycodes"
            );
        }

        let source = keymap.get_as_string(KEYMAP_FORMAT_TEXT_V1);
        let state = State::new(&keymap);
        let keycodes = Self::scan_keycodes(&keymap);
        // Only free keycodes can ever be chosen for a mapping, so only their
        // names matter.
        let names = keycodes
            .free
            .iter()
            .filter_map(|&keycode| {
                keymap
                    .key_get_name(keycode)
                    .map(|name| (keycode.raw(), name.to_owned()))
            })
            .collect();
        Self {
            context,
            source,
            names,
            active: keymap,
            state,
            symbol_keycodes: keycodes.symbols,
            additionally_mapped: HashMap::new(),
            pressed_keys: HashSet::new(),
            free_keycodes: keycodes.free,
            fill: keycodes.fill,
            min_reuse_delay,
        }
    }

    /// Update the keymap from a new file descriptor, preserving the current
    /// modifier/layout state and pressed keys.
    #[cfg(feature = "wayland")]
    pub fn update(&mut self, format: KeymapFormat, fd: OwnedFd, size: u32) -> Result<(), ()> {
        let new = Self::from_fd(self.context.clone(), format, fd, size)?;
        self.replace_with(new);
        Ok(())
    }

    /// Replace this keymap with `new`, preserving the current modifier/layout
    /// state and pressed keys. Dynamic mappings (and the LRU/free-keycode
    /// bookkeeping) are dropped; the caller must build `new` with the desired
    /// reuse delay.
    pub fn replace_with(&mut self, mut new: Keymap) {
        Self::restore_state(&mut new.state, &self.state, &self.pressed_keys);
        new.pressed_keys = std::mem::take(&mut self.pressed_keys);
        *self = new;
    }

    /// Copy the modifier/layout state and held keys from `from` into `state`.
    ///
    /// xkbcommon separates two models: `update_key` (the server/event model,
    /// which runs XKB actions through filters) and `update_mask` (the
    /// client/snapshot model). There is no API to clone one state into another
    /// keymap, so both are used here, in this order:
    ///
    /// 1. Replay `pressed` first. A later `update_key(Up)` can only clear a
    ///    modifier through the filter created by its matching `Down`; without
    ///    the replay, releasing a held key would leave the modifier set.
    /// 2. Write the serialized components afterwards. The replay has already
    ///    applied each held key's effect once, so the masks must overwrite
    ///    them; applying the masks first would make relative actions (such as a
    ///    group delta) count twice.
    ///
    /// xkbcommon does not recommend mixing the two entry points because they
    /// keep separate bookkeeping (filters and per-modifier key counts), so the
    /// result is approximate: a held keycode whose XKB action changed in the
    /// new keymap cannot be released consistently, and latched state that
    /// belongs to an already-released key has no filter behind it. The next
    /// server snapshot (`XkbGetState` on X11, a `wl_keyboard.modifiers` event
    /// on Wayland) corrects the components again.
    fn restore_state(state: &mut State, from: &State, pressed: &HashSet<Keycode>) {
        for &keycode in pressed {
            state.update_key(keycode, KeyDirection::Down);
        }
        state.update_mask(
            from.serialize_mods(STATE_MODS_DEPRESSED),
            from.serialize_mods(STATE_MODS_LATCHED),
            from.serialize_mods(STATE_MODS_LOCKED),
            from.serialize_layout(STATE_LAYOUT_DEPRESSED),
            from.serialize_layout(STATE_LAYOUT_LATCHED),
            from.serialize_layout(STATE_LAYOUT_LOCKED),
        );
    }

    /// Update the state with a pressed/released keycode. Returns the new
    /// modifier and layout masks if they changed, otherwise `None`.
    pub fn update_key(
        &mut self,
        keycode: Keycode,
        direction: KeyDirection,
    ) -> Option<(ModMask, ModMask, ModMask, LayoutMask)> {
        let old = self.masks();
        match direction {
            KeyDirection::Up => {
                self.pressed_keys.remove(&keycode);
            }
            KeyDirection::Down => {
                self.pressed_keys.insert(keycode);
            }
        }
        self.state.update_key(keycode, direction);
        let new = self.masks();
        (old != new).then_some(new)
    }

    /// The current depressed, latched and locked modifiers and the effective
    /// layout.
    pub(super) fn masks(&self) -> (ModMask, ModMask, ModMask, LayoutMask) {
        (
            self.state.serialize_mods(STATE_MODS_DEPRESSED),
            self.state.serialize_mods(STATE_MODS_LATCHED),
            self.state.serialize_mods(STATE_MODS_LOCKED),
            self.state.serialize_layout(STATE_LAYOUT_EFFECTIVE),
        )
    }

    /// Replace the modifier and layout state.
    ///
    /// Wayland feeds this from `wl_keyboard.modifiers`; X11 from the
    /// `XkbGetState` reply, which also reflects changes made by the physical
    /// keyboard or by other clients.
    #[cfg(any(feature = "wayland", feature = "x11rb"))]
    pub fn update_state(
        &mut self,
        depressed_mods: ModMask,
        latched_mods: ModMask,
        locked_mods: ModMask,
        depressed_layout: u32,
        latched_layout: u32,
        locked_layout: u32,
    ) {
        self.state.update_mask(
            depressed_mods,
            latched_mods,
            locked_mods,
            depressed_layout,
            latched_layout,
            locked_layout,
        );
    }

    /// Find a keycode that currently produces `key`.
    ///
    /// The lookup uses [`State::key_get_syms`], so the active layout and any
    /// modifiers that are currently held are taken into account. This matters:
    /// the caller presses the returned keycode on its own, so it has to already
    /// produce `key` under the current conditions. A key that yields several
    /// symbols at the current level is ambiguous, so it only counts when it
    /// yields exactly `key`. If no keycode does (for example because the keysym
    /// only exists at a level that needs a modifier we are not holding),
    /// [`Self::map_key`] is used instead, which adds the keysym at level 0 of a
    /// free keycode.
    pub fn key_to_keycode(&self, key: Key) -> Option<u16> {
        let keysym = Keysym::from(key);

        if let Some(mapped) = self.additionally_mapped.get(&keysym) {
            return u16::try_from(mapped.keycode.raw()).ok();
        }

        self.symbol_keycodes
            .iter()
            .copied()
            .find(|&keycode| self.state.key_get_syms(keycode) == [keysym])
            .and_then(|keycode| u16::try_from(keycode.raw()).ok())
    }

    /// Add `key` to the keymap so it can be entered. Returns the keycode that
    /// now produces it.
    ///
    /// This regenerates the keymap from the original compositor keymap with the
    /// new mapping appended, then recompiles it with xkbcommon. This is far
    /// simpler and more robust than parsing the keymap text ourselves.
    pub fn map_key(&mut self, key: Key) -> InputResult<u16> {
        let keysym = Keysym::from(key);
        if let Some(mapped) = self.additionally_mapped.get(&keysym) {
            return u16::try_from(mapped.keycode.raw())
                .map_err(|_| InputError::Mapping("mapped keycode exceeds u16".to_string()));
        }

        let sym_name = xkb::keysym_get_name(keysym);
        if sym_name.is_empty() {
            return Err(InputError::Mapping(format!("{keysym:?} has no name")));
        }

        // Choose a keycode (possibly reusing one) and build the resulting
        // mapping set without mutating `self`, so a failed recompile leaves the
        // keymap unchanged.
        let (keycode, reused) = self.choose_keycode()?;
        let keycode_u16 = u16::try_from(keycode.raw())
            .map_err(|_| InputError::Mapping("mapped keycode exceeds u16".to_string()))?;

        let next = {
            let mut next = self.additionally_mapped.clone();
            if reused {
                next.retain(|_, mapped| mapped.keycode != keycode);
            }
            next.insert(
                keysym,
                MappedKey {
                    keycode,
                    last_used: None,
                },
            );
            next
        };

        let (new_active, new_state) = self.rebuild(self.keymap_string_for(&next)?)?;

        // Guard against silently generating a keymap that lost the insertion
        // (for example if a section header was not found). Verify it in the
        // layout that is active right now, because that is where the next key
        // event will be sent; xkbcommon replicates a `Group1`-only key into the
        // other layouts, so this also covers them. `NoSymbol` (for example
        // `Key::Other(0)`) has no symbols to look up and is a harmless no-op,
        // so it is exempt from the check.
        let active_layout = new_state.serialize_layout(STATE_LAYOUT_EFFECTIVE);
        if keysym != Keysym::NoSymbol
            && new_active.key_get_syms_by_level(keycode, active_layout, 0) != [keysym]
        {
            return Err(InputError::Mapping(format!(
                "could not add {key:?} to the keymap"
            )));
        }

        // Commit only after the new keymap compiled and verified.
        self.active = new_active;
        self.state = new_state;
        self.additionally_mapped = next;
        if !reused {
            let popped = self.free_keycodes.pop_front();
            debug_assert_eq!(popped, Some(keycode));
        }
        Ok(keycode_u16)
    }

    /// The active keymap serialized to `TEXT_V1`. Used to send the keymap to
    /// the Wayland compositor.
    #[cfg(any(feature = "wayland", test))]
    pub fn keymap_string(&self) -> InputResult<String> {
        self.keymap_string_for(&self.additionally_mapped)
    }

    /// Build a keymap string equal to `source` plus the given dynamic mappings.
    fn keymap_string_for(&self, mappings: &HashMap<Keysym, MappedKey>) -> InputResult<String> {
        let mut keymap_string = self.source.clone();
        for (&keysym, mapped) in mappings {
            let sym_name = xkb::keysym_get_name(keysym);
            keymap_string = self.insert_mapping(keymap_string, mapped.keycode, &sym_name)?;
        }
        Ok(keymap_string)
    }

    /// The keycodes that currently carry a dynamically added mapping. These are
    /// the only keycodes a backend ever needs to push or restore; the original
    /// keycodes are never touched.
    #[cfg(feature = "x11rb")]
    pub fn mapped_keycodes(&self) -> impl Iterator<Item = Keycode> + '_ {
        self.additionally_mapped
            .values()
            .map(|mapped| mapped.keycode)
    }

    /// Record that `keycode` was used to send a key event. Only dynamically
    /// mapped keycodes are tracked; everything else is ignored.
    pub fn record_use(&mut self, keycode: Keycode) {
        if let Some(mapped) = self
            .additionally_mapped
            .values_mut()
            .find(|mapped| mapped.keycode == keycode)
        {
            mapped.last_used = Some(Instant::now());
        }
    }

    /// Choose a keycode for a new mapping. Returns the keycode and whether it
    /// is being reused (an existing mapping is evicted to make room). Does not
    /// mutate `self`.
    ///
    /// Keycodes are limited to `u16::MAX` because that is what the public
    /// `raw` API accepts. Keycode 8 is reserved (it has no evdev counterpart)
    /// and is never used, even when a keymap declares it.
    fn choose_keycode(&self) -> InputResult<(Keycode, bool)> {
        if let Some(&keycode) = self.free_keycodes.front() {
            return Ok((keycode, false));
        }

        // No free keycode left; reuse the least-recently-used mapping whose
        // keycode is not held. A keycode with no recorded use is treated as
        // older than any used one; ties are broken by the lowest keycode for
        // determinism.
        let evicted = self
            .additionally_mapped
            .values()
            .filter(|mapped| !self.pressed_keys.contains(&mapped.keycode))
            .min_by(|a, b| {
                a.last_used
                    .cmp(&b.last_used)
                    .then_with(|| a.keycode.cmp(&b.keycode))
            })
            .ok_or_else(|| {
                InputError::Mapping(
                    "no free keycode and every mapped keycode is currently held".to_string(),
                )
            })?;

        // The client may still be translating earlier key events for this
        // keycode with the old binding; wait until it has been idle long
        // enough.
        if let Some(last_used) = evicted.last_used {
            let deadline = last_used + self.min_reuse_delay;
            let now = Instant::now();
            if now < deadline {
                std::thread::sleep(deadline - now);
            }
        }

        Ok((evicted.keycode, true))
    }

    /// Insert a key declaration into the `xkb_symbols` section of
    /// `keymap_string`, declaring the keycode first if needed.
    ///
    /// The keysym is written to every level of an explicit XKB type so it is
    /// produced regardless of the active modifier combination, instead of
    /// relying on the server's fallback behaviour for unmatched modifiers.
    ///
    /// `keymap_string` must be in the canonical form produced by
    /// `xkb::Keymap::get_as_string`, which we control in this module. This is
    /// why we can rely on the section layout instead of parsing the keymap.
    fn insert_mapping(
        &self,
        mut keymap_string: String,
        keycode: Keycode,
        sym_name: &str,
    ) -> InputResult<String> {
        let identifier = if let Some(name) = self.names.get(&keycode.raw()) {
            name.clone()
        } else {
            let identifier = format!("I{}", keycode.raw());
            let declaration = format!("\t<{identifier}> = {};\n", keycode.raw());
            keymap_string =
                Self::insert_after_section_header(keymap_string, "xkb_keycodes ", &declaration)?;
            identifier
        };

        let (type_name, levels) = self.fill;
        let syms = std::iter::repeat_n(sym_name, levels)
            .collect::<Vec<_>>()
            .join(", ");
        let key = format!(
            "\tkey <{identifier}> {{\ttype[Group1]=\"{type_name}\",\tsymbols[Group1]= [ {syms} ] }};\n"
        );
        Self::insert_after_section_header(keymap_string, "xkb_symbols ", &key)
    }

    /// Recompile the keymap from a new keymap string, preserving the current
    /// modifier/layout state and pressed keys. Returns the new keymap and state
    /// without mutating `self`, so the caller can commit only after verifying
    /// the result.
    fn rebuild(&self, keymap_string: String) -> InputResult<(xkb::Keymap, State)> {
        let new_keymap = xkb::Keymap::new_from_string(
            &self.context,
            keymap_string,
            KEYMAP_FORMAT_TEXT_V1,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| {
            trace!("unable to recompile the modified keymap");
            InputError::Mapping("unable to recompile the modified keymap".to_string())
        })?;

        let mut new_state = State::new(&new_keymap);
        Self::restore_state(&mut new_state, &self.state, &self.pressed_keys);
        Ok((new_keymap, new_state))
    }
}

/// The keycodes classified from the original keymap.
struct Keycodes {
    /// Keycodes that produce at least one symbol under some layout and level.
    symbols: Vec<Keycode>,
    /// Keycodes with no symbols, in ascending order, free for remapping.
    free: VecDeque<Keycode>,
    /// The type and level count needed to cover every level in the keymap.
    fill: (&'static str, usize),
}

impl Keymap {
    /// Pick the smallest standard XKB type that can hold `levels` shift levels
    /// and return its name together with the number of levels it exposes.
    pub(super) fn levels_type(levels: u32) -> (&'static str, usize) {
        match levels {
            0 | 1 => ("ONE_LEVEL", 1),
            2 => ("TWO_LEVEL", 2),
            3 | 4 => ("FOUR_LEVEL", 4),
            _ => ("EIGHT_LEVEL", 8),
        }
    }

    /// Scan the usable keycode range once, classifying each keycode and picking
    /// the type/level count for remapped keys. The range is clamped to
    /// [`MAX_KEYCODE`] so a malformed keymap cannot make the scan unbounded.
    fn scan_keycodes(keymap: &xkb::Keymap) -> Keycodes {
        let min = keymap.min_keycode().raw();
        let max = keymap.max_keycode().raw().min(MAX_KEYCODE);
        let mut symbols = Vec::new();
        let mut free = VecDeque::new();
        let mut levels = 1;
        for raw in min..=max {
            let keycode = Keycode::new(raw);
            let mut has_symbol = false;
            for layout in 0..keymap.num_layouts_for_key(keycode) {
                let num_levels = keymap.num_levels_for_key(keycode, layout);
                levels = levels.max(num_levels);
                for level in 0..num_levels {
                    if !keymap
                        .key_get_syms_by_level(keycode, layout, level)
                        .is_empty()
                    {
                        has_symbol = true;
                    }
                }
            }
            if has_symbol {
                symbols.push(keycode);
            } else if raw >= 9 {
                // Keycode 8 is reserved (evdev 0) and must never be allocated.
                free.push_back(keycode);
            }
        }
        Keycodes {
            symbols,
            free,
            fill: Self::levels_type(levels),
        }
    }

    /// Insert `insertion` directly after the opening brace of the section whose
    /// header starts with `header` (for example `xkb_symbols `).
    fn insert_after_section_header(
        mut keymap_string: String,
        header: &str,
        insertion: &str,
    ) -> InputResult<String> {
        let section = header.trim_end();
        let needle = format!("\n{header}");
        let Some(section_start) = keymap_string.find(&needle) else {
            return Err(InputError::Mapping(format!(
                "the keymap has no {section} section"
            )));
        };
        let Some(brace) = keymap_string[section_start..]
            .find('{')
            .map(|i| section_start + i)
        else {
            return Err(InputError::Mapping(format!(
                "the {section} section has no opening brace"
            )));
        };
        keymap_string.insert_str(brace + 1, &format!("\n{insertion}"));
        Ok(keymap_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> Context {
        Context::new(xkb::CONTEXT_NO_FLAGS)
    }

    fn compile(string: &str) -> xkb::Keymap {
        xkb::Keymap::new_from_string(
            &context(),
            string.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("keymap should compile")
    }

    fn keymap_from_name(layout: &str) -> Keymap {
        let context = context();
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            layout,
            "",
            None,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("named layout should compile");
        let source = keymap.get_as_string(KEYMAP_FORMAT_TEXT_V1);
        Keymap::from_string(context, source, KEYMAP_FORMAT_TEXT_V1, Duration::ZERO)
            .expect("Keymap::from_string should accept canonical output")
    }

    /// Build a `us`-layout keymap without going through the Wayland-only
    /// `Keymap::default`, so the shared lookup/remap code is tested for the
    /// X11 backend as well.
    fn test_keymap() -> Keymap {
        keymap_from_name("us")
    }

    fn mod_bit(keymap: &Keymap, name: &str) -> u32 {
        let index = keymap.active.mod_get_index(name);
        assert_ne!(index, xkb::MOD_INVALID, "modifier {name} is missing");
        1u32 << index
    }

    fn shift_keycode(keymap: &Keymap) -> u32 {
        u32::from(
            keymap
                .key_to_keycode(Key::Shift)
                .expect("shift should be mapped"),
        )
    }

    /// When a dynamically mapped `keycode` was last used, or `None` if it was
    /// never used or is not dynamically mapped.
    fn last_use_time(keymap: &Keymap, keycode: Keycode) -> Option<Instant> {
        keymap
            .additionally_mapped
            .values()
            .find(|mapped| mapped.keycode == keycode)
            .and_then(|mapped| mapped.last_used)
    }

    /// Pretend that a dynamically mapped `keycode` was last used at `at`.
    fn set_last_use_time(keymap: &mut Keymap, keycode: Keycode, at: Instant) {
        let mapped = keymap
            .additionally_mapped
            .values_mut()
            .find(|mapped| mapped.keycode == keycode)
            .expect("keycode is dynamically mapped");
        mapped.last_used = Some(at);
    }

    /// A keymap that maps only `a`..`f` onto keycodes 9..=15 and leaves the
    /// keycodes 16..=20 inside the configured range undeclared. This exercises
    /// the "declare an undeclared keycode" path in `insert_mapping`.
    const SPARSE: &str = r#"xkb_keymap {
        xkb_keycodes "sparse" { minimum = 9; maximum = 20;
            <I9> = 9; <I10> = 10; <I11> = 11; <I12> = 12;
            <I13> = 13; <I14> = 14; <I15> = 15; <I20> = 20;
        };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "sparse" {
            key <I9>  { [ Escape ] };
            key <I10> { [ a ] };
            key <I11> { [ b ] };
            key <I12> { [ c ] };
            key <I13> { [ d ] };
            key <I14> { [ e ] };
            key <I15> { [ f ] };
        };
    };"#;

    fn sparse_keymap() -> Keymap {
        Keymap::from_string(
            context(),
            SPARSE.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("sparse keymap should compile")
    }

    /// Only one keysym and two free keycodes (10, 11), so a third mapping has
    /// to evict an existing one.
    const TINY: &str = r#"xkb_keymap {
        xkb_keycodes "tiny" { minimum = 9; maximum = 11;
            <I9> = 9; <I10> = 10; <I11> = 11;
        };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "tiny" { key <I9> { [ Escape ] }; };
    };"#;

    fn tiny_keymap() -> Keymap {
        Keymap::from_string(
            context(),
            TINY.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("tiny keymap should compile")
    }

    /// A keymap used to replace the active keymap via `update`. It maps
    /// `a`..`f` to keycodes 9..=15 (none of the `us` keycodes) and `adiaeresis`
    /// onto keycode 11 at both levels.
    #[cfg(feature = "wayland")]
    const UPDATE_TARGET: &str = r#"xkb_keymap {
        xkb_keycodes "target" { minimum = 9; maximum = 12;
            <I9> = 9; <I10> = 10; <I11> = 11; <I12> = 12;
        };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "target" {
            key <I9> { [ Escape ] };
            key <I11> { [ adiaeresis, Adiaeresis ] };
        };
    };"#;

    #[cfg(feature = "wayland")]
    fn temp_keymap_fd(string: &str) -> (KeymapFormat, std::os::fd::OwnedFd, u32) {
        use std::io::Write as _;
        let mut file = tempfile::tempfile().expect("tempfile");
        file.write_all(string.as_bytes()).expect("write keymap");
        let size = u32::try_from(string.len()).expect("keymap fits in u32");
        (KEYMAP_FORMAT_TEXT_V1, file.into(), size)
    }

    // ------------------------------------------------------------------
    // Construction and serialization
    // ------------------------------------------------------------------

    #[test]
    fn from_string_accepts_canonical_output() {
        let keymap = test_keymap();
        assert_ne!(keymap.keymap_string().expect("keymap string"), "");
        // The source round-trips through xkbcommon.
        compile(&keymap.keymap_string().expect("keymap string"));
    }

    #[test]
    fn from_string_trims_trailing_nuls() {
        let canonical = test_keymap().keymap_string().expect("keymap string") + "\0\0\0";
        let keymap =
            Keymap::from_string(context(), canonical, KEYMAP_FORMAT_TEXT_V1, Duration::ZERO);
        assert!(keymap.is_some(), "trailing NUL bytes must be ignored");
    }

    #[test]
    fn from_string_rejects_invalid_input() {
        let keymap = Keymap::from_string(
            context(),
            "not a keymap".to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        );
        assert!(keymap.is_none());
    }

    #[test]
    fn keymap_string_is_recompilable() {
        let keymap = test_keymap();
        let recompiled = compile(&keymap.keymap_string().expect("keymap string"));
        assert_eq!(
            recompiled.min_keycode().raw(),
            keymap.active.min_keycode().raw()
        );
    }

    #[cfg(feature = "wayland")]
    #[test]
    fn default_builds_a_usable_us_layout() {
        let keymap = Keymap::default().expect("default keymap should compile");
        assert!(keymap.key_to_keycode(Key::Unicode('a')).is_some());
        assert!(keymap.key_to_keycode(Key::Return).is_some());
        assert!(keymap.key_to_keycode(Key::Begin).is_none());
    }

    #[cfg(feature = "wayland")]
    #[test]
    fn from_fd_rejects_empty_and_oversized_keymaps() {
        let (format, fd, _) = temp_keymap_fd(SPARSE);
        assert!(Keymap::from_fd(context(), format, fd, 0).is_err());

        let (format, fd, _) = temp_keymap_fd(SPARSE);
        let too_large = u32::try_from(MAX_KEYMAP_SIZE).unwrap() + 1;
        assert!(Keymap::from_fd(context(), format, fd, too_large).is_err());
    }

    #[cfg(feature = "wayland")]
    #[test]
    fn from_fd_ignores_bytes_past_the_announced_size() {
        use std::io::Write as _;
        let mut file = tempfile::tempfile().expect("tempfile");
        file.write_all(SPARSE.as_bytes()).expect("write");
        // Garbage that must not be parsed.
        file.write_all(b"\nTHIS IS NOT VALID XKB\n").expect("write");
        let size = u32::try_from(SPARSE.len()).expect("fits");
        let fd: std::os::fd::OwnedFd = file.into();
        let keymap = Keymap::from_fd(context(), KEYMAP_FORMAT_TEXT_V1, fd, size);
        assert!(keymap.is_ok(), "only `size` bytes must be read");
    }

    #[cfg(feature = "wayland")]
    #[test]
    fn update_replaces_the_keymap_and_preserves_modifiers() {
        let mut keymap = test_keymap();
        // Hold shift before the keymap is replaced.
        let shift = shift_keycode(&keymap);
        keymap.update_key(Keycode::new(shift), KeyDirection::Down);

        let (format, fd, size) = temp_keymap_fd(UPDATE_TARGET);
        keymap.update(format, fd, size).expect("update");

        // The new keymap no longer knows the us `a` keycode.
        assert!(keymap.key_to_keycode(Key::Unicode('a')).is_none());
        // Shift was preserved, so `a` is now the uppercase level of keycode 11.
        assert_eq!(keymap.key_to_keycode(Key::Unicode('Ä')), Some(11));
        assert!(keymap.key_to_keycode(Key::Unicode('ä')).is_none());
    }

    // ------------------------------------------------------------------
    // Lookup
    // ------------------------------------------------------------------

    #[test]
    fn lookup_finds_level_zero_keys() {
        let keymap = test_keymap();
        for key in [
            Key::Unicode('a'),
            Key::Unicode('z'),
            Key::Unicode('1'),
            Key::Unicode(' '),
            Key::Return,
            Key::Escape,
            Key::Space,
        ] {
            assert!(
                keymap.key_to_keycode(key).is_some(),
                "expected {key:?} to be found"
            );
        }
    }

    #[test]
    fn lookup_returns_none_for_unmapped_keysym() {
        let keymap = test_keymap();
        assert!(keymap.key_to_keycode(Key::Begin).is_none());
    }

    #[test]
    fn lookup_matches_key_other() {
        let keymap = test_keymap();
        let from_char = keymap.key_to_keycode(Key::Unicode('a'));
        // `Other` carries a raw keysym; 0x61 is the keysym for 'a'.
        assert_eq!(keymap.key_to_keycode(Key::Other(0x61)), from_char);
    }

    #[test]
    fn lookup_respects_held_modifiers() {
        let mut keymap = test_keymap();
        let shift = shift_keycode(&keymap);
        let lowercase = keymap.key_to_keycode(Key::Unicode('a')).expect("a");
        keymap.update_key(Keycode::new(shift), KeyDirection::Down);

        // With Shift held, the native keycode of `a` produces `A`, so the
        // shifted character must not need a remap.
        assert_eq!(
            keymap.key_to_keycode(Key::Unicode('A')),
            Some(lowercase),
            "the shifted character must resolve to the native keycode"
        );
        assert!(keymap.key_to_keycode(Key::Unicode('a')).is_none());

        keymap.update_key(Keycode::new(shift), KeyDirection::Up);
        assert!(keymap.key_to_keycode(Key::Unicode('a')).is_some());
        assert!(keymap.key_to_keycode(Key::Unicode('A')).is_none());
    }

    #[test]
    fn lookup_respects_locked_modifier() {
        let mut keymap = test_keymap();
        let caps = u32::from(
            keymap
                .key_to_keycode(Key::CapsLock)
                .expect("caps lock is mapped"),
        );
        // A caps-lock press toggles the Lock modifier and releasing it keeps
        // it.
        keymap.update_key(Keycode::new(caps), KeyDirection::Down);
        keymap.update_key(Keycode::new(caps), KeyDirection::Up);

        assert!(keymap.key_to_keycode(Key::Unicode('A')).is_some());
        assert!(keymap.key_to_keycode(Key::Unicode('a')).is_none());
    }

    #[test]
    fn right_modifiers_use_their_own_keycodes() {
        // #391: the right modifier keys must resolve to their own keycode, not
        // to the left one, and must update the same modifier state.
        let mut keymap = test_keymap();
        let shift_mask = mod_bit(&keymap, "Shift");

        for (left_key, right_key) in [(Key::Shift, Key::RShift), (Key::Control, Key::RControl)] {
            let left = keymap.key_to_keycode(left_key).expect("left modifier");
            let right = keymap.key_to_keycode(right_key).expect("right modifier");
            assert_ne!(
                left, right,
                "{left_key:?} and {right_key:?} must not share a keycode"
            );
        }

        let right_shift = keymap.key_to_keycode(Key::RShift).expect("right shift");
        assert!(
            keymap
                .update_key(Keycode::new(u32::from(right_shift)), KeyDirection::Down)
                .is_some()
        );
        assert_eq!(keymap.masks().0, shift_mask);
    }

    #[test]
    fn lookup_after_mapping_returns_the_mapped_keycode() {
        let mut keymap = test_keymap();
        let keycode = keymap.map_key(Key::Begin).expect("map");
        assert_eq!(keymap.key_to_keycode(Key::Begin), Some(keycode));
    }

    // ------------------------------------------------------------------
    // Mapping
    // ------------------------------------------------------------------

    #[test]
    fn map_key_returns_a_plausible_keycode() {
        let mut keymap = test_keymap();
        let min = keymap.active.min_keycode().raw();
        let max = keymap.active.max_keycode().raw();
        for key in [Key::Begin, Key::Unicode('❤'), Key::Unicode('❄')] {
            let keycode = u32::from(keymap.map_key(key).expect("map"));
            assert!((min..=max).contains(&keycode), "{keycode} outside keymap");
            // Keycode 8 is special (evdev 0) and must never be used.
            assert_ne!(keycode, 8);
        }
    }

    #[test]
    fn map_key_is_idempotent() {
        let mut keymap = test_keymap();
        let first = keymap.map_key(Key::Begin).expect("map");
        let second = keymap.map_key(Key::Begin).expect("map");
        assert_eq!(first, second);
    }

    #[test]
    fn map_key_handles_unicode_keysyms() {
        let mut keymap = test_keymap();
        for key in [
            Key::Unicode('❤'),
            Key::Unicode('中'),
            Key::Unicode('\u{1D54A}'), // 𝕊, outside the BMP
            Key::Unicode('\u{0301}'),  // combining acute accent
        ] {
            let keycode = keymap.map_key(key).expect("map");
            assert_eq!(keymap.key_to_keycode(key), Some(keycode));

            let compiled = compile(&keymap.keymap_string().expect("keymap string"));
            let state = State::new(&compiled);
            assert!(
                state
                    .key_get_syms(Keycode::new(u32::from(keycode)))
                    .contains(&Keysym::from(key)),
                "compiled keymap should produce {key:?}"
            );
        }
    }

    #[test]
    fn map_key_places_the_keysym_at_level_zero() {
        let mut keymap = test_keymap();
        // '@' lives on level 1 (Shift+2) of the us layout.
        let keycode = keymap.map_key(Key::Unicode('@')).expect("map");

        let compiled = compile(&keymap.keymap_string().expect("keymap string"));
        let state = State::new(&compiled);
        // With no modifiers the mapped keycode must produce '@' at level 0.
        assert!(
            state
                .key_get_syms(Keycode::new(u32::from(keycode)))
                .contains(&Keysym::from(Key::Unicode('@')))
        );
    }

    #[test]
    fn mapped_key_uses_an_explicit_multi_level_type() {
        let mut keymap = test_keymap();
        keymap.map_key(Key::Unicode('@')).expect("map");
        let string = keymap.keymap_string().expect("keymap string");
        assert!(string.contains("type[Group1]="));
    }

    #[test]
    fn mapped_keysym_is_produced_at_every_level() {
        // Remapped keys must produce the keysym at every level, so that no
        // modifier combination can change or hide it. This is explicit (an
        // explicit XKB type) rather than relying on a fallback.
        let mut keymap = test_keymap();
        let key = Key::Unicode('@');
        let keycode = keymap.map_key(key).expect("map");
        let compiled = compile(&keymap.keymap_string().expect("keymap string"));

        let level_count = compiled.num_levels_for_key(Keycode::new(u32::from(keycode)), 0);
        assert!(
            level_count >= 4,
            "expected a multi-level type, got {level_count}"
        );
        for level in 0..level_count {
            assert!(
                compiled
                    .key_get_syms_by_level(Keycode::new(u32::from(keycode)), 0, level)
                    .contains(&Keysym::from(key)),
                "level {level} should produce {key:?}"
            );
        }

        // And under real modifier masks, including Shift, Level3 and Level5.
        let shift_bit = mod_bit(&keymap, "Shift");
        let altgr_bit = mod_bit(&keymap, "Mod5");
        let level5_bit = mod_bit(&keymap, "Mod3");
        for mask in [
            0,
            shift_bit,
            altgr_bit,
            level5_bit,
            shift_bit | altgr_bit,
            shift_bit | altgr_bit | level5_bit,
        ] {
            let mut state = State::new(&compiled);
            state.update_mask(mask, 0, 0, 0, 0, 0);
            assert!(
                state
                    .key_get_syms(Keycode::new(u32::from(keycode)))
                    .contains(&Keysym::from(key)),
                "mapped keycode must produce {key:?} under mask {mask:#x}"
            );
        }
    }

    #[test]
    fn mapping_a_no_symbol_is_a_harmless_noop() {
        let mut keymap = test_keymap();
        assert!(keymap.map_key(Key::Other(0)).is_ok());
        // `NoSymbol` has no symbol to look up, but the keycode is remembered.
        assert!(keymap.key_to_keycode(Key::Other(0)).is_some());
    }

    #[test]
    fn multiple_mappings_do_not_lose_keycode_declarations() {
        let mut keymap = test_keymap();
        let first = keymap.map_key(Key::Unicode('❄')).expect("map");
        let second = keymap.map_key(Key::Unicode('☃')).expect("map");
        assert_ne!(first, second);
        assert_eq!(keymap.key_to_keycode(Key::Unicode('❄')), Some(first));
        assert_eq!(keymap.key_to_keycode(Key::Unicode('☃')), Some(second));

        let compiled = compile(&keymap.keymap_string().expect("keymap string"));
        let state = State::new(&compiled);
        assert!(
            state
                .key_get_syms(Keycode::new(u32::from(first)))
                .contains(&Keysym::from(Key::Unicode('❄')))
        );
        assert!(
            state
                .key_get_syms(Keycode::new(u32::from(second)))
                .contains(&Keysym::from(Key::Unicode('☃')))
        );
    }

    #[test]
    fn map_key_declares_undeclared_keycodes() {
        // The sparse keymap has undeclared keycodes within its configured
        // range; `insert_mapping` has to declare them before using them.
        let mut keymap = sparse_keymap();
        let keycode = keymap.map_key(Key::Unicode('❤')).expect("map");
        assert!((16..=20).contains(&keycode), "expected a gap keycode");

        let string = keymap.keymap_string().expect("keymap string");
        assert!(string.contains(&format!("<I{keycode}>")));
        let compiled = compile(&string);
        let state = State::new(&compiled);
        assert!(
            state
                .key_get_syms(Keycode::new(u32::from(keycode)))
                .contains(&Keysym::from(Key::Unicode('❤')))
        );
    }

    /// A keymap that declares a free keycode under a custom name, like a
    /// compositor keymap with physical key names (`<AE01>`, `<AD01>`, ...).
    const CUSTOM_NAME: &str = r#"xkb_keymap {
        xkb_keycodes "custom" { minimum = 9; maximum = 11;
            <I9> = 9; <FOO> = 10; <I11> = 11;
        };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "custom" { key <I9> { [ a ] }; };
    };"#;

    #[test]
    fn maps_onto_a_declared_keycode_with_a_custom_name() {
        // The free keycode 10 is declared as `<FOO>`, so the mapping has to
        // reference that name instead of declaring `<I10>`.
        let mut keymap = Keymap::from_string(
            context(),
            CUSTOM_NAME.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("custom keymap should compile");
        let keycode = keymap.map_key(Key::Unicode('❤')).expect("map");
        assert_eq!(keycode, 10, "the lowest free keycode should be chosen");

        let string = keymap.keymap_string().expect("keymap string");
        assert!(string.contains("key <FOO>"));
        let compiled = compile(&string);
        let state = State::new(&compiled);
        assert!(
            state
                .key_get_syms(Keycode::new(10))
                .contains(&Keysym::from(Key::Unicode('❤')))
        );
    }

    #[test]
    fn map_key_evicts_a_mapping_when_out_of_keycodes() {
        let mut keymap = tiny_keymap();
        let first = keymap.map_key(Key::Unicode('❄')).expect("map");
        let second = keymap.map_key(Key::Unicode('☃')).expect("map");
        assert_ne!(first, second);

        // Mark the first mapping held; eviction must prefer the unheld one.
        keymap.update_key(Keycode::new(u32::from(first)), KeyDirection::Down);

        let third = keymap
            .map_key(Key::Unicode('☂'))
            .expect("map with eviction");
        assert_eq!(third, second, "the unheld mapping should have been reused");
        assert_eq!(keymap.key_to_keycode(Key::Unicode('❄')), Some(first));
        assert_eq!(keymap.key_to_keycode(Key::Unicode('☂')), Some(third));
        assert!(keymap.key_to_keycode(Key::Unicode('☃')).is_none());
        assert_eq!(keymap.additionally_mapped.len(), 2);
    }

    #[test]
    fn map_key_fails_when_all_mappings_are_held() {
        let mut keymap = tiny_keymap();
        let first = keymap.map_key(Key::Unicode('❄')).expect("map");
        let second = keymap.map_key(Key::Unicode('☃')).expect("map");
        keymap.update_key(Keycode::new(u32::from(first)), KeyDirection::Down);
        keymap.update_key(Keycode::new(u32::from(second)), KeyDirection::Down);

        let result = keymap.map_key(Key::Unicode('☂'));
        assert!(matches!(result, Err(InputError::Mapping(_))));
    }

    // ------------------------------------------------------------------
    // State tracking
    // ------------------------------------------------------------------

    #[test]
    fn update_key_reports_only_modifier_changes() {
        let mut keymap = test_keymap();
        let shift = shift_keycode(&keymap);
        assert!(
            keymap
                .update_key(Keycode::new(shift), KeyDirection::Down)
                .is_some()
        );
        assert!(
            keymap
                .update_key(Keycode::new(shift), KeyDirection::Up)
                .is_some()
        );

        let a = u32::from(keymap.key_to_keycode(Key::Unicode('a')).unwrap());
        assert!(
            keymap
                .update_key(Keycode::new(a), KeyDirection::Down)
                .is_none()
        );
    }

    #[test]
    fn rebuild_keeps_held_modifiers() {
        let mut keymap = test_keymap();
        let shift = shift_keycode(&keymap);
        keymap.update_key(Keycode::new(shift), KeyDirection::Down);

        // A remap forces a rebuild of the keymap and its state.
        keymap.map_key(Key::Unicode('❤')).expect("map");

        // Shift must still be held afterwards.
        assert!(keymap.key_to_keycode(Key::Unicode('a')).is_none());
        assert!(keymap.key_to_keycode(Key::Unicode('A')).is_some());
    }

    #[test]
    fn update_state_affects_lookup() {
        let mut keymap = test_keymap();
        let shift = mod_bit(&keymap, "Shift");
        keymap.update_state(shift, 0, 0, 0, 0, 0);

        assert!(keymap.key_to_keycode(Key::Unicode('a')).is_none());
        assert!(keymap.key_to_keycode(Key::Unicode('A')).is_some());
    }

    // ------------------------------------------------------------------
    // Helpers and library assumptions
    // ------------------------------------------------------------------

    #[test]
    fn keymap_is_send() {
        // #442: `Enigo` is used from other threads, so the keymap must stay
        // `Send` even though it holds raw xkbcommon pointers.
        fn assert_send<T: Send>() {}
        assert_send::<Keymap>();
    }

    #[test]
    fn insert_after_section_header_targets_the_right_section() {
        let string = "xkb_keymap {\nxkb_keycodes \"x\" {\n  minimum = 9;\n};\nxkb_symbols \"x\" {\n  name[Group1]=\"x\";\n};\n};";
        let out =
            Keymap::insert_after_section_header(string.to_string(), "xkb_symbols ", "KEYLINE")
                .expect("the section should be found");
        assert!(out.contains("xkb_symbols \"x\" {\nKEYLINE"));
        assert!(!out.contains("xkb_keycodes \"x\" {\nKEYLINE"));
    }

    #[test]
    fn insert_after_section_header_fails_without_the_section() {
        let string = "no sections here";
        let error = Keymap::insert_after_section_header(string.to_string(), "xkb_symbols ", "X")
            .expect_err("a missing section must be an error");
        assert!(
            matches!(&error, InputError::Mapping(message) if message.contains("xkb_symbols")),
            "the error should name the missing section: {error:?}"
        );
    }

    #[test]
    fn insert_after_section_header_fails_without_an_opening_brace() {
        let string = "xkb_keymap {\nxkb_symbols \"x\"\n";
        assert!(
            Keymap::insert_after_section_header(string.to_string(), "xkb_symbols ", "X").is_err(),
            "a section without a body must be an error"
        );
    }

    #[test]
    fn canonical_get_as_string_has_the_sections_we_insert_into() {
        // `insert_after_section_header` relies on this exact layout.
        let keymap = test_keymap();
        let string = keymap.keymap_string().expect("keymap string");
        assert!(string.contains("\nxkb_keycodes "));
        assert!(string.contains("\nxkb_symbols "));
    }

    #[test]
    fn key_get_name_reports_declared_and_undeclared_keycodes() {
        // `insert_mapping` uses this to decide whether to declare a keycode.
        let keymap = sparse_keymap();
        assert!(keymap.active.key_get_name(Keycode::new(10)).is_some());
        assert!(keymap.active.key_get_name(Keycode::new(16)).is_none());
        // Keycode 8 is outside the legal range.
        assert!(keymap.active.key_get_name(Keycode::new(8)).is_none());
    }

    #[test]
    fn keysym_get_name_roundtrips_through_keysym_from_name() {
        for keysym in [
            Keysym::from(Key::Unicode('a')),
            Keysym::from(Key::Unicode('❤')),
            Keysym::new(0x1008_ff00), // a vendor keysym
        ] {
            let name = xkb::keysym_get_name(keysym);
            assert_ne!(name, "");
            assert_eq!(
                xkb::keysym_from_name(&name, xkb::KEYSYM_NO_FLAGS),
                keysym,
                "{name} should round-trip"
            );
        }
        // Garbage keysyms have no name; `build_keymap_string` relies on being
        // able to detect the empty string.
        assert_eq!(xkb::keysym_get_name(Keysym::new(0xdead_beef)), "");
    }

    #[test]
    fn empty_levels_are_reported_as_empty() {
        // `scan_keycodes` treats an empty slice as "no symbol" at that
        // level.
        let keymap = sparse_keymap();
        let keycode = Keycode::new(16); // undeclared but in range
        assert_eq!(keymap.active.key_get_syms_by_level(keycode, 0, 0), []);
    }

    /// A key whose level holds two keysyms. xkbcommon exposes this as a
    /// multi-symbol level (`key_get_syms` returns both, `key_get_one_sym`
    /// returns `NoSymbol`).
    const MULTI_SYM: &str = r#"xkb_keymap {
        xkb_keycodes "multi" { minimum = 9; maximum = 9; <I9> = 9; };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "multi" {
            key <I9> { type[Group1]="ONE_LEVEL", symbols[Group1]= [ { a, b } ] };
        };
    };"#;

    #[test]
    fn multi_symbol_levels_are_not_treated_as_a_match() {
        let keymap = Keymap::from_string(
            context(),
            MULTI_SYM.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("multi-sym keymap should compile");

        let syms = keymap.state.key_get_syms(Keycode::new(9));
        assert_eq!(syms.len(), 2);
        assert!(syms.contains(&Keysym::from(Key::Unicode('a'))));
        assert!(syms.contains(&Keysym::from(Key::Unicode('b'))));

        // The key is ambiguous, so it must not be used for either symbol.
        assert_eq!(keymap.key_to_keycode(Key::Unicode('a')), None);
        assert_eq!(keymap.key_to_keycode(Key::Unicode('b')), None);
    }

    #[test]
    fn no_symbol_does_not_match_a_key_without_symbols() {
        // A symbol-less key must not be treated as producing `NoSymbol`.
        let keymap = sparse_keymap();
        let free = Keycode::new(16);
        assert_eq!(keymap.state.key_get_syms(free), []);
        assert_eq!(keymap.key_to_keycode(Key::Other(0)), None);
    }

    #[test]
    fn min_keycode_never_includes_8() {
        let keymap = test_keymap();
        assert!(keymap.active.min_keycode().raw() > 8);
    }

    // ------------------------------------------------------------------
    // Type and level selection
    // ------------------------------------------------------------------

    #[test]
    fn levels_type_selects_the_smallest_standard_type() {
        // `map_key` derives the type for a remapped key from the highest level
        // count in the keymap. Pin the exact mapping so a future refactor
        // cannot silently pick a type that drops levels.
        assert_eq!(Keymap::levels_type(0), ("ONE_LEVEL", 1));
        assert_eq!(Keymap::levels_type(1), ("ONE_LEVEL", 1));
        assert_eq!(Keymap::levels_type(2), ("TWO_LEVEL", 2));
        assert_eq!(Keymap::levels_type(3), ("FOUR_LEVEL", 4));
        assert_eq!(Keymap::levels_type(4), ("FOUR_LEVEL", 4));
        assert_eq!(Keymap::levels_type(5), ("EIGHT_LEVEL", 8));
        assert_eq!(Keymap::levels_type(8), ("EIGHT_LEVEL", 8));
        // More levels than any standard type has still use EIGHT_LEVEL.
        assert_eq!(Keymap::levels_type(9), ("EIGHT_LEVEL", 8));
    }

    /// A keymap whose only key uses the default (single-level) type.
    const SINGLE_LEVEL: &str = r#"xkb_keymap {
        xkb_keycodes "single" { minimum = 9; maximum = 11;
            <I9> = 9; <I10> = 10; <I11> = 11;
        };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "single" { key <I9> { [ a ] }; };
    };"#;

    /// A keymap with a five-level key, so the largest type used by the keymap
    /// exceeds `FOUR_LEVEL`.
    const FIVE_LEVEL: &str = r#"xkb_keymap {
        xkb_keycodes "five" { minimum = 9; maximum = 11;
            <I9> = 9; <I10> = 10; <I11> = 11;
        };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "five" {
            key <I9> {
                type[Group1]="EIGHT_LEVEL",
                symbols[Group1]= [ a, b, c, d, e ]
            };
        };
    };"#;

    #[test]
    fn fill_uses_one_level_for_a_single_level_keymap() {
        assert_eq!(
            Keymap::scan_keycodes(&compile(SINGLE_LEVEL)).fill,
            ("ONE_LEVEL", 1)
        );

        let mut keymap = Keymap::from_string(
            context(),
            SINGLE_LEVEL.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("single-level keymap should compile");
        assert_eq!(keymap.fill, ("ONE_LEVEL", 1));

        let keycode = keymap.map_key(Key::Unicode('❤')).expect("map");
        let string = keymap.keymap_string().expect("keymap string");
        assert!(string.contains("type[Group1]=\"ONE_LEVEL\""));

        let compiled = compile(&string);
        let mapped = Keycode::new(u32::from(keycode));
        assert_eq!(compiled.num_levels_for_key(mapped, 0), 1);
        assert!(
            compiled
                .key_get_syms_by_level(mapped, 0, 0)
                .contains(&Keysym::from(Key::Unicode('❤')))
        );
    }

    #[test]
    fn fill_uses_eight_levels_when_the_keymap_exceeds_four_levels() {
        assert_eq!(
            Keymap::scan_keycodes(&compile(FIVE_LEVEL)).fill,
            ("EIGHT_LEVEL", 8)
        );

        let mut keymap = Keymap::from_string(
            context(),
            FIVE_LEVEL.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("five-level keymap should compile");
        assert_eq!(keymap.fill, ("EIGHT_LEVEL", 8));

        let keycode = keymap.map_key(Key::Unicode('❤')).expect("map");
        let string = keymap.keymap_string().expect("keymap string");
        assert!(string.contains("type[Group1]=\"EIGHT_LEVEL\""));

        let compiled = compile(&string);
        let mapped = Keycode::new(u32::from(keycode));
        assert_eq!(compiled.num_levels_for_key(mapped, 0), 8);
        for level in 0..8 {
            assert!(
                compiled
                    .key_get_syms_by_level(mapped, 0, level)
                    .contains(&Keysym::from(Key::Unicode('❤'))),
                "level {level} should produce the remapped keysym"
            );
        }
    }

    // ------------------------------------------------------------------
    // Serialization stability
    // ------------------------------------------------------------------

    #[test]
    fn keymap_string_is_stable_and_keeps_existing_mappings() {
        let mut keymap = test_keymap();
        // Without dynamic mappings the string is exactly the source keymap.
        assert_eq!(
            keymap.keymap_string().expect("keymap string"),
            keymap.source
        );
        assert_eq!(
            keymap.keymap_string().expect("keymap string"),
            keymap.keymap_string().expect("keymap string")
        );

        let first = keymap.map_key(Key::Unicode('❤')).expect("map");
        let once = keymap.keymap_string().expect("keymap string");
        // Repeated serialization is deterministic (the mapping set is stable).
        assert_eq!(once, keymap.keymap_string().expect("keymap string"));

        let second = keymap.map_key(Key::Unicode('❄')).expect("map");
        let both = keymap.keymap_string().expect("keymap string");
        let compiled = compile(&both);

        // Adding a second mapping must not drop the first one's full block.
        for (keycode, key) in [(first, Key::Unicode('❤')), (second, Key::Unicode('❄'))] {
            let mapped = Keycode::new(u32::from(keycode));
            let level_count = compiled.num_levels_for_key(mapped, 0);
            assert!(level_count > 1, "expected a multi-level type");
            for level in 0..level_count {
                assert!(
                    compiled
                        .key_get_syms_by_level(mapped, 0, level)
                        .contains(&Keysym::from(key)),
                    "{key:?} should still be produced at level {level}"
                );
            }
        }
    }

    #[test]
    fn a_modified_keymap_string_recompiles_into_an_equivalent_keymap() {
        let mut keymap = test_keymap();
        let keycode = keymap.map_key(Key::Unicode('❤')).expect("map");

        let restored = Keymap::from_string(
            context(),
            keymap.keymap_string().expect("keymap string"),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("modified keymap should recompile");

        // The mapped key is found in the restored keymap without remapping.
        assert_eq!(restored.key_to_keycode(Key::Unicode('❤')), Some(keycode));
        assert_eq!(restored.additionally_mapped.len(), 0);
    }

    // ------------------------------------------------------------------
    // Level selection for ordinary keys
    // ------------------------------------------------------------------

    #[test]
    fn level_fallbacks_follow_the_xkb_key_type() {
        // The us `a` key uses a two-level alphabetic type. Modifiers that the
        // type does not consume (AltGr/Mod5, Level5/Mod3) leave level 0
        // reachable; Shift selects level 1. `key_to_keycode` returns the same
        // keycode for both levels because it only resolves keycodes.
        let mut keymap = test_keymap();
        let shift_mask = mod_bit(&keymap, "Shift");
        let altgr_mask = mod_bit(&keymap, "Mod5");
        let level5_mask = mod_bit(&keymap, "Mod3");

        keymap.state.update_mask(0, 0, 0, 0, 0, 0);
        let lowercase = keymap
            .key_to_keycode(Key::Unicode('a'))
            .expect("a should be reachable");
        assert_eq!(keymap.key_to_keycode(Key::Unicode('A')), None);

        for mask in [altgr_mask, level5_mask, altgr_mask | level5_mask] {
            keymap.state.update_mask(mask, 0, 0, 0, 0, 0);
            assert_eq!(
                keymap.key_to_keycode(Key::Unicode('a')),
                Some(lowercase),
                "mask {mask:#x} should keep the level-0 symbol reachable"
            );
            assert_eq!(
                keymap.key_to_keycode(Key::Unicode('A')),
                None,
                "mask {mask:#x} must not select the shifted symbol"
            );
        }

        for mask in [
            shift_mask,
            shift_mask | altgr_mask,
            shift_mask | level5_mask,
        ] {
            keymap.state.update_mask(mask, 0, 0, 0, 0, 0);
            assert_eq!(
                keymap.key_to_keycode(Key::Unicode('A')),
                Some(lowercase),
                "mask {mask:#x} should select the shifted symbol"
            );
            assert_eq!(
                keymap.key_to_keycode(Key::Unicode('a')),
                None,
                "mask {mask:#x} must not expose the unshifted symbol"
            );
        }
    }

    #[test]
    fn an_altgr_character_is_resolved_at_its_level_or_mapped() {
        // #531: on `de`, `@` lives on the AltGr level of the `q` key. With
        // AltGr held it must resolve to that native keycode; without it there
        // is no keycode for `@`, so it is mapped instead of typing `q`.
        let mut keymap = keymap_from_name("de");
        let q = keymap.key_to_keycode(Key::Unicode('q')).expect("q in de");
        assert_eq!(keymap.key_to_keycode(Key::Unicode('@')), None);

        let altgr = mod_bit(&keymap, "Mod5");
        keymap.state.update_mask(altgr, 0, 0, 0, 0, 0);
        assert_eq!(
            keymap.key_to_keycode(Key::Unicode('@')),
            Some(q),
            "AltGr must select the native level-2 keycode"
        );

        keymap.state.update_mask(0, 0, 0, 0, 0, 0);
        let mapped = keymap.map_key(Key::Unicode('@')).expect("map");
        let compiled = compile(&keymap.keymap_string().expect("keymap string"));
        assert_eq!(
            compiled.key_get_syms_by_level(Keycode::new(u32::from(mapped)), 0, 0),
            [Keysym::from(Key::Unicode('@'))],
        );
    }

    #[test]
    fn lookup_respects_the_active_layout_group() {
        let context = context();
        let native = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us,de",
            "",
            None,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("us,de should compile");
        let source = native.get_as_string(KEYMAP_FORMAT_TEXT_V1);
        let mut keymap =
            Keymap::from_string(context, source, KEYMAP_FORMAT_TEXT_V1, Duration::ZERO)
                .expect("canonical us,de should compile");

        let us_y = keymap.key_to_keycode(Key::Unicode('y')).expect("y in us");
        let us_z = keymap.key_to_keycode(Key::Unicode('z')).expect("z in us");
        assert_ne!(us_y, us_z);
        assert_eq!(
            keymap
                .active
                .num_layouts_for_key(Keycode::new(u32::from(us_y))),
            2
        );

        // Switch to the second layout (`de`), where `y` and `z` swap keycodes.
        keymap.state.update_mask(0, 0, 0, 1, 0, 0);
        assert_eq!(
            keymap.state.serialize_layout(STATE_LAYOUT_EFFECTIVE),
            1,
            "the second layout should be active"
        );
        assert_eq!(keymap.key_to_keycode(Key::Unicode('y')), Some(us_z));
        assert_eq!(keymap.key_to_keycode(Key::Unicode('z')), Some(us_y));
    }

    #[test]
    fn a_custom_layout_decides_which_keycode_produces_a_character() {
        // #302: with colemak, `f` sits on the key QWERTY uses for `e`, so
        // resolving `f` to the QWERTY keycode would type the wrong character.
        let context = context();
        let native = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us",
            "colemak",
            None,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("us,colemak should compile");
        let source = native.get_as_string(KEYMAP_FORMAT_TEXT_V1);
        let keymap = Keymap::from_string(context, source, KEYMAP_FORMAT_TEXT_V1, Duration::ZERO)
            .expect("canonical us,colemak should compile");

        let qwerty = keymap_from_name("us");
        let qwerty_e = qwerty.key_to_keycode(Key::Unicode('e')).expect("e in us");
        let colemak_f = keymap
            .key_to_keycode(Key::Unicode('f'))
            .expect("f in colemak");
        assert_eq!(colemak_f, qwerty_e, "colemak puts `f` on the `e` key");
        assert_ne!(
            colemak_f,
            qwerty.key_to_keycode(Key::Unicode('f')).expect("f in us"),
        );

        let compiled = compile(&keymap.keymap_string().expect("keymap string"));
        assert_eq!(
            compiled.key_get_syms_by_level(Keycode::new(u32::from(colemak_f)), 0, 0),
            [Keysym::from(Key::Unicode('f'))],
        );
    }

    #[test]
    fn a_group_only_keysym_is_not_found_in_the_wrong_group() {
        // Cyrillic `я` exists only in the `ru` group. It must not resolve while
        // `us` is active, and must resolve once `ru` is active.
        let context = context();
        let native = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us,ru",
            "",
            None,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("us,ru should compile");
        let source = native.get_as_string(KEYMAP_FORMAT_TEXT_V1);
        let mut keymap =
            Keymap::from_string(context, source, KEYMAP_FORMAT_TEXT_V1, Duration::ZERO)
                .expect("canonical us,ru should compile");

        assert_eq!(keymap.key_to_keycode(Key::Unicode('я')), None);

        keymap.state.update_mask(0, 0, 0, 1, 0, 0);
        assert!(
            keymap.key_to_keycode(Key::Unicode('я')).is_some(),
            "я should be reachable in the ru group"
        );
    }

    #[test]
    fn a_secondary_layout_character_is_mapped_while_the_primary_layout_is_active() {
        // #541: `я` only exists in the `ru` group. While `us` is active there
        // is no keycode for it, so it must be mapped to a spare keycode instead
        // of switching the user's layout.
        let mut keymap = keymap_from_name("us,ru");
        assert_eq!(keymap.state.serialize_layout(STATE_LAYOUT_EFFECTIVE), 0);
        assert_eq!(
            keymap.key_to_keycode(Key::Unicode('я')),
            None,
            "я must not resolve while the us group is active"
        );

        let keycode = keymap.map_key(Key::Unicode('я')).expect("map");
        assert_eq!(
            keymap.state.serialize_layout(STATE_LAYOUT_EFFECTIVE),
            0,
            "mapping must not switch the active layout"
        );
        assert_eq!(keymap.key_to_keycode(Key::Unicode('я')), Some(keycode));

        // The recompiled keymap must produce `я` from that keycode in the
        // active group without any modifier held.
        let compiled = compile(&keymap.keymap_string().expect("keymap string"));
        assert_eq!(
            compiled.key_get_syms_by_level(Keycode::new(u32::from(keycode)), 0, 0),
            [Keysym::from(Key::Unicode('я'))],
        );

        // The native `us` keys are untouched.
        assert!(keymap.key_to_keycode(Key::Unicode('y')).is_some());
    }

    #[test]
    fn rebuild_preserves_the_active_layout_group() {
        let context = context();
        let native = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us,de",
            "",
            None,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("us,de should compile");
        let source = native.get_as_string(KEYMAP_FORMAT_TEXT_V1);
        let mut keymap =
            Keymap::from_string(context, source, KEYMAP_FORMAT_TEXT_V1, Duration::ZERO)
                .expect("canonical us,de should compile");

        keymap.state.update_mask(0, 0, 0, 1, 0, 0);
        let layout_one_y = keymap.key_to_keycode(Key::Unicode('y')).expect("y in de");

        // A remap forces a rebuild of the keymap and its state.
        keymap.map_key(Key::Unicode('❤')).expect("map");

        assert_eq!(keymap.state.serialize_layout(STATE_LAYOUT_EFFECTIVE), 1);
        assert_eq!(keymap.key_to_keycode(Key::Unicode('y')), Some(layout_one_y));
    }

    #[test]
    fn mapping_while_a_non_primary_layout_is_active_works_in_every_group() {
        // `us,de` has two layouts. Map a key while `de` (group 1) is active:
        // the mapping must be verified and usable in the active group, and
        // xkbcommon must still expose it in the other groups.
        let mut keymap = keymap_from_name("us,de");
        keymap.state.update_mask(0, 0, 0, 1, 0, 0);

        let keycode = keymap.map_key(Key::Unicode('❤')).expect("map");
        assert_eq!(
            keymap.state.serialize_layout(STATE_LAYOUT_EFFECTIVE),
            1,
            "mapping must not change the active layout"
        );
        assert_eq!(keymap.key_to_keycode(Key::Unicode('❤')), Some(keycode));

        let compiled = compile(&keymap.keymap_string().expect("keymap string"));
        let mapped = Keycode::new(u32::from(keycode));
        for group in 0..2 {
            assert_eq!(
                compiled.key_get_syms_by_level(mapped, group, 0),
                [Keysym::from(Key::Unicode('❤'))],
                "the mapped key must produce its keysym in group {group}"
            );
        }
    }

    #[cfg(feature = "x11rb")]
    #[test]
    fn update_state_replaces_modifiers_and_layout() {
        let mut keymap = keymap_from_name("us,de");
        assert_eq!(keymap.active.num_layouts(), 2);

        let shift = shift_keycode(&keymap);
        keymap.update_key(Keycode::new(shift), KeyDirection::Down);
        let y_in_us = keymap.key_to_keycode(Key::Unicode('Y')).expect("Y in us");

        // Mirror an `XkbGetState` reply: Shift depressed, second group active.
        let shift_mask = mod_bit(&keymap, "Shift");
        keymap.update_state(shift_mask, 0, 0, 1, 0, 0);
        assert_eq!(
            keymap.state.serialize_mods(STATE_MODS_DEPRESSED),
            shift_mask
        );
        assert_eq!(keymap.state.serialize_layout(STATE_LAYOUT_EFFECTIVE), 1);
        let y_in_de = keymap.key_to_keycode(Key::Unicode('Y')).expect("Y in de");
        assert_ne!(y_in_us, y_in_de, "`y` and `z` swap between the layouts");

        // A full replacement clears the previous Shift and applies Lock.
        let lock_mask = mod_bit(&keymap, "Lock");
        keymap.update_state(0, 0, lock_mask, 0, 0, 0);
        assert_eq!(keymap.state.serialize_mods(STATE_MODS_DEPRESSED), 0);
        assert_eq!(keymap.state.serialize_mods(STATE_MODS_LOCKED), lock_mask);
        assert_eq!(keymap.state.serialize_layout(STATE_LAYOUT_EFFECTIVE), 0);
        assert!(keymap.key_to_keycode(Key::Unicode('A')).is_some());
        assert_eq!(keymap.key_to_keycode(Key::Unicode('a')), None);
    }

    // ------------------------------------------------------------------
    // Keycode selection and the pressed-key set
    // ------------------------------------------------------------------

    #[test]
    fn map_key_prefers_the_lowest_free_keycode() {
        // SPARSE maps keycodes 9..=15; 16, 17 and 18 are the first free ones.
        let mut keymap = sparse_keymap();
        assert_eq!(keymap.map_key(Key::Unicode('❤')).expect("map"), 16);
        assert_eq!(keymap.map_key(Key::Unicode('❄')).expect("map"), 17);
        assert_eq!(keymap.map_key(Key::Unicode('☃')).expect("map"), 18);

        // And the chosen keycodes really produce their keysym.
        let compiled = compile(&keymap.keymap_string().expect("keymap string"));
        for (raw, key) in [
            (16, Key::Unicode('❤')),
            (17, Key::Unicode('❄')),
            (18, Key::Unicode('☃')),
        ] {
            assert!(
                compiled
                    .key_get_syms_by_level(Keycode::new(raw), 0, 0)
                    .contains(&Keysym::from(key))
            );
        }
    }

    #[test]
    fn free_keycodes_are_computed_from_the_base_keymap() {
        // Key 9 has a symbol, key 10 is declared but all-NoSymbol, and key 11
        // is undeclared. Only 10 and 11 are usable for remapping.
        let mut keymap = Keymap::from_string(
            context(),
            SYMBOL_AND_NOSYMBOL.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("keymap should compile");

        let free: Vec<_> = keymap.free_keycodes.iter().copied().collect();
        assert_eq!(free, [Keycode::new(10), Keycode::new(11)]);

        // The first remap therefore goes to keycode 10 (lowest free).
        assert_eq!(keymap.map_key(Key::Unicode('❤')).expect("map"), 10);
    }

    #[test]
    fn pressed_keys_are_tracked_for_non_modifier_keys() {
        let mut keymap = test_keymap();
        let a = u32::from(keymap.key_to_keycode(Key::Unicode('a')).expect("a"));
        let keycode = Keycode::new(a);

        // A normal key does not change any modifier or layout mask...
        assert_eq!(keymap.update_key(keycode, KeyDirection::Down), None);
        // ...but it is tracked so it cannot be unmapped while held.
        assert!(keymap.pressed_keys.contains(&keycode));

        assert_eq!(keymap.update_key(keycode, KeyDirection::Up), None);
        assert!(!keymap.pressed_keys.contains(&keycode));
    }

    #[test]
    fn update_key_returns_the_serialized_masks() {
        let mut keymap = test_keymap();
        let shift_code = shift_keycode(&keymap);
        let shift_mask = mod_bit(&keymap, "Shift");

        let (depressed, latched, locked, layout) = keymap
            .update_key(Keycode::new(shift_code), KeyDirection::Down)
            .expect("pressing shift changes the state");
        assert_eq!(depressed, shift_mask);
        assert_eq!(latched, 0);
        assert_eq!(locked, 0);
        assert_eq!(layout, 0);

        let (depressed, latched, locked, _layout) = keymap
            .update_key(Keycode::new(shift_code), KeyDirection::Up)
            .expect("releasing shift changes the state");
        assert_eq!(depressed, 0);
        assert_eq!(latched, 0);
        assert_eq!(locked, 0);
    }

    #[test]
    fn a_click_on_a_toggle_key_keeps_the_locked_state() {
        // A click is a press followed by a release. CapsLock sets the lock on
        // the press and the release does not clear it, so the state and the
        // resulting shift level must change.
        let mut keymap = test_keymap();
        let caps = Keycode::new(u32::from(
            keymap.key_to_keycode(Key::CapsLock).expect("CapsLock"),
        ));
        let a = keymap.key_to_keycode(Key::Unicode('a')).expect("a");
        assert_eq!(keymap.key_to_keycode(Key::Unicode('A')), None);

        let _ = keymap.update_key(caps, KeyDirection::Down);
        let _ = keymap.update_key(caps, KeyDirection::Up);

        assert_eq!(
            keymap.state.serialize_mods(STATE_MODS_LOCKED),
            mod_bit(&keymap, "Lock"),
            "CapsLock must stay locked after the release"
        );
        assert_eq!(keymap.key_to_keycode(Key::Unicode('A')), Some(a));
        assert_eq!(keymap.key_to_keycode(Key::Unicode('a')), None);
    }

    // ------------------------------------------------------------------
    // Keycode ranges
    // ------------------------------------------------------------------

    /// A keymap whose configured maximum keycode is above 255.
    const HIGH_KEYCODES: &str = r#"xkb_keymap {
        xkb_keycodes "high" { minimum = 1; maximum = 300;
            <I9> = 9; <I300> = 300;
        };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "high" {
            key <I9> { [ a ] };
            key <I300> { [ b ] };
        };
    };"#;

    #[test]
    fn from_string_accepts_keycodes_above_255() {
        let mut keymap = Keymap::from_string(
            context(),
            HIGH_KEYCODES.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("keymap with high keycodes should compile");

        // xkbcommon clamps the minimum up to 9 but keeps the large maximum.
        assert_eq!(keymap.active.min_keycode().raw(), 9);
        assert_eq!(keymap.active.max_keycode().raw(), 300);
        assert_eq!(keymap.key_to_keycode(Key::Unicode('a')), Some(9));
        assert_eq!(keymap.key_to_keycode(Key::Unicode('b')), Some(300));

        // Mapping must not panic or produce a keycode outside the keymap range.
        let keycode = keymap.map_key(Key::Unicode('❤')).expect("map");
        assert!((9..=300).contains(&keycode));
    }

    /// A keymap that declares keycode 8, which is not a valid XKB keycode.
    const MIN_EIGHT: &str = r#"xkb_keymap {
        xkb_keycodes "min8" { minimum = 8; maximum = 12;
            <I8> = 8; <I9> = 9; <I10> = 10; <I11> = 11; <I12> = 12;
        };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "min8" { key <I9> { [ a ] }; };
    };"#;

    #[test]
    fn keycode_8_is_never_used_even_if_the_keymap_mentions_it() {
        let mut keymap = Keymap::from_string(
            context(),
            MIN_EIGHT.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("keymap should compile");

        // Unlike a `minimum = 1` keymap (which xkbcommon clamps to 9), an
        // explicit `minimum = 8` keeps keycode 8 in the keymap.
        assert_eq!(keymap.active.min_keycode().raw(), 8);

        // Keycode 8 is reserved, so it must never be handed out even though
        // it has no symbol and would otherwise look free.
        for key in [
            Key::Unicode('b'),
            Key::Unicode('c'),
            Key::Unicode('d'),
            Key::Unicode('e'),
        ] {
            assert_ne!(keymap.map_key(key).expect("map"), 8);
        }
    }

    #[test]
    fn keycodes_above_the_public_limit_are_ignored() {
        // xkbcommon keeps a named keycode of 70000, which is above the `u16`
        // the public `raw` API accepts. The scans must stay clamped so they
        // remain bounded, and mapping must still work on a usable keycode.
        let string = r#"xkb_keymap {
            xkb_keycodes "huge" { minimum = 9; maximum = 70000;
                <I9> = 9; <I70000> = 70000;
            };
            xkb_types { include "complete" };
            xkb_compatibility { include "complete" };
            xkb_symbols "huge" { key <I9> { [ a ] }; };
        };"#;
        let mut keymap = Keymap::from_string(
            context(),
            string.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("huge keymap should compile");

        assert_eq!(keymap.active.max_keycode().raw(), 70000);
        // Only keycode 9 has a symbol; the keycode above the cap is skipped.
        assert_eq!(keymap.symbol_keycodes.len(), 1);
        assert_eq!(keymap.key_to_keycode(Key::Unicode('a')), Some(9));
        // The free list is clamped to the largest usable keycode.
        assert_eq!(
            keymap.free_keycodes.back().copied(),
            Some(Keycode::new(MAX_KEYCODE))
        );

        let mapped = keymap.map_key(Key::Unicode('❤')).expect("map");
        assert_eq!(mapped, 10, "the lowest usable free keycode is chosen");
    }

    // ------------------------------------------------------------------
    // Dynamic mappings across keymap updates
    // ------------------------------------------------------------------

    #[cfg(feature = "wayland")]
    #[test]
    fn update_discards_dynamic_mappings_and_keeps_held_keys() {
        let mut keymap = test_keymap();
        keymap.map_key(Key::Unicode('❤')).expect("map");
        assert_eq!(keymap.additionally_mapped.len(), 1);

        // Hold shift in the old keymap before replacing it.
        let shift = shift_keycode(&keymap);
        keymap.update_key(Keycode::new(shift), KeyDirection::Down);

        let (format, fd, size) = temp_keymap_fd(UPDATE_TARGET);
        keymap.update(format, fd, size).expect("update");

        // Dynamic mappings are tied to the old keymap and are discarded.
        assert_eq!(keymap.additionally_mapped.len(), 0);
        assert_eq!(keymap.key_to_keycode(Key::Unicode('❤')), None);

        // The modifier state survived, so the shifted level is active.
        assert_eq!(keymap.key_to_keycode(Key::Unicode('Ä')), Some(11));

        // The held keycode is still tracked and can be released.
        assert!(keymap.pressed_keys.contains(&Keycode::new(shift)));
        keymap.update_key(Keycode::new(shift), KeyDirection::Up);
    }

    /// A keymap with one real symbol, one declared all-NoSymbol key and one
    /// undeclared key inside the configured range.
    const SYMBOL_AND_NOSYMBOL: &str = r#"xkb_keymap {
        xkb_keycodes "ns" { minimum = 9; maximum = 11;
            <I9> = 9; <I10> = 10; <I11> = 11;
        };
        xkb_types { include "complete" };
        xkb_compatibility { include "complete" };
        xkb_symbols "ns" {
            key <I9> { [ a ] };
            key <I10> { [ NoSymbol ] };
        };
    };"#;

    #[test]
    fn a_mapped_key_is_usable_in_every_layout_group() {
        // Dynamically added keys only declare `Group1`. xkbcommon still exposes
        // them for every active layout instead of producing nothing, so a
        // remap is not broken by the user switching layouts.
        let context = context();
        let native = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us,de",
            "",
            None,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("us,de should compile");
        let source = native.get_as_string(KEYMAP_FORMAT_TEXT_V1);
        let mut keymap =
            Keymap::from_string(context, source, KEYMAP_FORMAT_TEXT_V1, Duration::ZERO)
                .expect("canonical us,de should compile");

        let keycode = keymap.map_key(Key::Unicode('❤')).expect("map");
        let expected_levels = u32::try_from(keymap.fill.1).expect("the level count fits in u32");
        let compiled = compile(&keymap.keymap_string().expect("keymap string"));
        let mapped = Keycode::new(u32::from(keycode));

        // The key is declared for one group only, but both groups resolve it.
        assert_eq!(compiled.num_layouts_for_key(mapped), 1);
        for group in 0..2 {
            assert_eq!(compiled.num_levels_for_key(mapped, group), expected_levels);
            assert!(
                compiled
                    .key_get_syms_by_level(mapped, group, 0)
                    .contains(&Keysym::from(Key::Unicode('❤'))),
                "the mapped key should be usable in layout group {group}"
            );
        }

        // And it still works when the second layout is active.
        let mut state = State::new(&compiled);
        state.update_mask(0, 0, 0, 1, 0, 0);
        assert!(
            state
                .key_get_syms(mapped)
                .contains(&Keysym::from(Key::Unicode('❤')))
        );
    }

    // ------------------------------------------------------------------
    // Reuse (least-recently-used eviction and the idle wait)
    // ------------------------------------------------------------------

    #[test]
    fn record_use_tracks_only_dynamic_keycodes() {
        let mut keymap = test_keymap();
        let mapped = Keycode::new(u32::from(keymap.map_key(Key::Unicode('❤')).expect("map")));
        let base = Keycode::new(u32::from(
            keymap
                .key_to_keycode(Key::Unicode('a'))
                .expect("a is mapped"),
        ));

        // A base keycode is not tracked.
        keymap.record_use(base);
        assert!(
            keymap
                .additionally_mapped
                .values()
                .all(|mapped| mapped.last_used.is_none())
        );

        // A dynamically mapped keycode is.
        keymap.record_use(mapped);
        assert!(last_use_time(&keymap, mapped).is_some());
    }

    #[test]
    fn map_key_reuses_the_least_recently_used_mapping() {
        let mut keymap = tiny_keymap();
        let first = keymap.map_key(Key::Unicode('❄')).expect("map"); // keycode 10
        let second = keymap.map_key(Key::Unicode('☃')).expect("map"); // keycode 11
        assert_eq!((first, second), (10, 11));

        // `first` (keycode 10) was used longer ago than `second` (keycode 11).
        let now = Instant::now();
        set_last_use_time(&mut keymap, Keycode::new(10), now);
        set_last_use_time(
            &mut keymap,
            Keycode::new(11),
            now + Duration::from_millis(1),
        );

        let third = keymap
            .map_key(Key::Unicode('☂'))
            .expect("map with eviction");
        assert_eq!(third, first, "the least recently used mapping is reused");
        assert_eq!(keymap.key_to_keycode(Key::Unicode('☂')), Some(third));
        assert_eq!(keymap.key_to_keycode(Key::Unicode('☃')), Some(second));
        assert!(keymap.key_to_keycode(Key::Unicode('❄')).is_none());
    }

    #[test]
    fn never_used_mappings_are_evicted_before_used_ones() {
        let mut keymap = tiny_keymap();
        let first = keymap.map_key(Key::Unicode('❄')).expect("map"); // keycode 10
        let second = keymap.map_key(Key::Unicode('☃')).expect("map"); // keycode 11
        assert_eq!((first, second), (10, 11));

        // Only `first` has a recorded use; `second` is treated as never used.
        set_last_use_time(&mut keymap, Keycode::new(10), Instant::now());

        let third = keymap
            .map_key(Key::Unicode('☂'))
            .expect("map with eviction");
        assert_eq!(third, second, "the never-used mapping is evicted first");
    }

    #[test]
    fn from_string_configures_the_idle_window() {
        let keymap = Keymap::from_string(
            context(),
            TINY.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::from_millis(123),
        )
        .expect("keymap should compile");
        assert_eq!(keymap.min_reuse_delay, Duration::from_millis(123));

        // No reuse delay is the normal configuration for string keymaps.
        let keymap = Keymap::from_string(
            context(),
            TINY.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::ZERO,
        )
        .expect("keymap should compile");
        assert_eq!(keymap.min_reuse_delay, Duration::ZERO);
    }

    #[test]
    fn reuse_waits_until_the_keycode_has_been_idle() {
        let mut keymap = Keymap::from_string(
            context(),
            TINY.to_string(),
            KEYMAP_FORMAT_TEXT_V1,
            Duration::from_millis(40),
        )
        .expect("tiny keymap should compile");
        let first = keymap.map_key(Key::Unicode('❄')).expect("map"); // keycode 10
        let second = keymap.map_key(Key::Unicode('☃')).expect("map"); // keycode 11
        assert_eq!((first, second), (10, 11));

        // Both were used just now; the tie is broken by keycode, so `first`
        // (keycode 10) is the LRU and must become idle before reuse.
        let start = Instant::now();
        set_last_use_time(&mut keymap, Keycode::new(10), start);
        set_last_use_time(&mut keymap, Keycode::new(11), start);

        let third = keymap
            .map_key(Key::Unicode('☂'))
            .expect("map with eviction");
        assert_eq!(third, first);
        assert!(
            start.elapsed() >= Duration::from_millis(40),
            "reuse should have waited for the idle delay"
        );
    }

    #[test]
    fn reuse_forgets_the_previous_use_time() {
        let mut keymap = tiny_keymap();
        let first = keymap.map_key(Key::Unicode('❄')).expect("map"); // keycode 10
        let second = keymap.map_key(Key::Unicode('☃')).expect("map"); // keycode 11
        assert_eq!((first, second), (10, 11));

        // `first` (keycode 10) is the least recently used mapping.
        let now = Instant::now();
        set_last_use_time(&mut keymap, Keycode::new(10), now);
        set_last_use_time(
            &mut keymap,
            Keycode::new(11),
            now + Duration::from_millis(1),
        );

        let third = keymap
            .map_key(Key::Unicode('☂'))
            .expect("map with eviction");
        assert_eq!(third, first);

        // The reused keycode now carries a new keysym that has never been
        // used, so it must not look recently used.
        assert_eq!(last_use_time(&keymap, Keycode::new(10)), None);
        assert!(last_use_time(&keymap, Keycode::new(11)).is_some());
    }

    #[test]
    fn reusing_an_undeclared_keycode_does_not_duplicate_its_declaration() {
        // SPARSE leaves keycodes 16..=19 undeclared but free, so each one is
        // declared when first mapped. Reusing one must replace its symbol
        // without adding a second `<I16> = 16;` line to the string.
        let mut keymap = sparse_keymap();
        for key in [
            Key::Unicode('❤'),
            Key::Unicode('❄'),
            Key::Unicode('☃'),
            Key::Unicode('☂'),
            Key::Unicode('★'),
        ] {
            keymap.map_key(key).expect("map");
        }

        let string = keymap.keymap_string().expect("keymap string");
        assert_eq!(string.matches("<I16> = 16;").count(), 1);
        compile(&string);
    }

    #[test]
    fn declared_keycodes_are_not_redeclared() {
        // TINY declares both free keycodes in its `xkb_keycodes` section, so a
        // mapping must reference the existing names instead of declaring them.
        let mut keymap = tiny_keymap();
        keymap.map_key(Key::Unicode('❄')).expect("map"); // keycode 10
        keymap.map_key(Key::Unicode('☃')).expect("map"); // keycode 11

        let string = keymap.keymap_string().expect("keymap string");
        // A redeclaration would use our compact format; the canonical one is
        // padded, so the compact form must be absent.
        assert!(!string.contains("\t<I10> = 10;\n"));
        assert!(!string.contains("\t<I11> = 11;\n"));
        assert!(string.contains("key <I10>"));
        assert!(string.contains("key <I11>"));
    }

    #[test]
    fn replace_with_preserves_state_and_drops_dynamic_mappings() {
        let mut keymap = test_keymap();
        keymap.map_key(Key::Unicode('❤')).expect("map");
        let shift = shift_keycode(&keymap);
        keymap.update_key(Keycode::new(shift), KeyDirection::Down);

        // Replace with a fresh `us` keymap. The dynamic mapping is dropped, but
        // the held shift modifier is preserved.
        keymap.replace_with(test_keymap());

        assert_eq!(keymap.additionally_mapped.len(), 0);
        assert!(keymap.key_to_keycode(Key::Unicode('❤')).is_none());
        assert!(keymap.key_to_keycode(Key::Unicode('a')).is_none());
        assert!(keymap.key_to_keycode(Key::Unicode('A')).is_some());
        assert!(keymap.pressed_keys.contains(&Keycode::new(shift)));

        // Releasing the held key must still clear Shift: `restore_state`
        // replayed the press so the release has a filter to consume.
        keymap.update_key(Keycode::new(shift), KeyDirection::Up);
        assert_eq!(keymap.masks().0, 0);
    }

    #[test]
    fn releasing_a_key_that_was_never_pressed_is_a_noop() {
        // `update_key(Up)` clears a modifier only through the filter created
        // by the matching `Down`. This is why `restore_state` replays the held
        // keys instead of restoring the components with `update_mask` alone.
        let mut keymap = test_keymap();
        let shift = shift_keycode(&keymap);
        let shift_mask = mod_bit(&keymap, "Shift");

        keymap.update_state(shift_mask, 0, 0, 0, 0, 0);
        keymap.update_key(Keycode::new(shift), KeyDirection::Up);

        assert_eq!(keymap.masks().0, shift_mask);
    }

    #[test]
    fn replace_with_replays_held_keys_before_applying_the_masks() {
        // A group action applies its delta when the key is replayed. The
        // serialized masks must be applied afterwards: applying them first
        // would add the delta a second time.
        let keymap_with_group_action = || {
            let source = r#"xkb_keymap {
                xkb_keycodes "group" { minimum = 9; maximum = 11; <I9> = 9; <I10> = 10; <I11> = 11; };
                xkb_types "group" { include "complete" };
                xkb_compatibility "group" { include "complete" };
                xkb_symbols "group" {
                    name[Group1]="first";
                    name[Group2]="second";
                    key <I9> { [ a, A ], [ b, B ] };
                    key <I10> { [ ISO_Next_Group ] };
                    key <I11> {
                        symbols[Group1] = [ x ],
                        actions[Group1] = [ SetGroup(group=+1) ]
                    };
                };
            };"#;
            Keymap::from_string(
                context(),
                source.to_string(),
                KEYMAP_FORMAT_TEXT_V1,
                Duration::ZERO,
            )
            .expect("group action keymap should compile")
        };
        let group_key = Keycode::new(11);

        let mut keymap = keymap_with_group_action();
        assert_eq!(keymap.active.num_layouts(), 2);
        keymap.update_key(group_key, KeyDirection::Down);
        assert_eq!(keymap.state.serialize_layout(STATE_LAYOUT_DEPRESSED), 1);

        keymap.replace_with(keymap_with_group_action());
        assert_eq!(
            keymap.state.serialize_layout(STATE_LAYOUT_DEPRESSED),
            1,
            "the replayed group delta must not be applied twice"
        );

        keymap.update_key(group_key, KeyDirection::Up);
        assert_eq!(keymap.state.serialize_layout(STATE_LAYOUT_DEPRESSED), 0);
    }
}
