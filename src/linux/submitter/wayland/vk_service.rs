// Imports from other crates
use std::{
    collections::HashSet,
    convert::TryInto,
    io::{Seek, SeekFrom, Write},
    os::unix::io::IntoRawFd,
    sync::{Arc, Mutex},
    time::Instant,
};
use tempfile::tempfile;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Main, Proxy};
use zwp_virtual_keyboard::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use zwp_virtual_keyboard::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

// Imports from other modules
use crate::keyboard;

// Macro to avoid repeating code
// Unwraps the value or returns an error
// The method unwrap() would panic on Err values, which is not wanted when this macro is used
macro_rules! unwrap_or_return {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(err) => return Err(err),
        }
    };
}

#[derive(Debug, PartialEq, Clone)]
/// Error when submitting
pub enum SubmitError {
    /// Virtual keyboard proxy was dropped and is no longer alive
    NotAlive,
    /// The keycode was invalid
    InvalidKeycode,
}

#[derive(Debug, PartialEq, Clone)]
/// Enum to differentiate between a key press and a release
pub enum KeyMotion {
    Press = 1,
    Release = 0,
}

bitflags! {
    /// Maps the names of the modifiers to their bitcodes (From squeekboard)
    /// The codes stem from https://www.x.org/releases/current/doc/kbproto/xkbproto.html#Keyboard_State
    pub struct ModifiersBitflag: u32 {
        const NO_MODIFIERS = 0x0;
        const SHIFT = 0x1;
        const LOCK = 0x2;
        const CONTROL = 0x4;
        /// Alt
        const MOD1 = 0x8;
        const MOD2 = 0x10;
        const MOD3 = 0x20;
        /// Meta
        const MOD4 = 0x40;
        /// AltGr
        const MOD5 = 0x80;
    }
}

impl From<keyboard::Modifier> for ModifiersBitflag {
    // Converts the modifiers in their bitflags
    fn from(item: keyboard::Modifier) -> Self {
        match item {
            keyboard::Modifier::Shift => ModifiersBitflag::SHIFT,
            keyboard::Modifier::Lock => ModifiersBitflag::LOCK,
            keyboard::Modifier::Control => ModifiersBitflag::CONTROL,
            keyboard::Modifier::Alt => ModifiersBitflag::MOD1,
            keyboard::Modifier::Mod2 => ModifiersBitflag::MOD2,
            keyboard::Modifier::Mod3 => ModifiersBitflag::MOD3,
            keyboard::Modifier::Mod4 => ModifiersBitflag::MOD4,
            keyboard::Modifier::Mod5 => ModifiersBitflag::MOD5,
        }
    }
}

/// Service that makes submitting keycodes and modifiers easier
pub struct VKService {
    base_time: std::time::Instant,
    pressed_keys: HashSet<u32>,
    pressed_modifiers: ModifiersBitflag,
    virtual_keyboard: Proxy<ZwpVirtualKeyboardV1>,
}

impl Drop for VKService {
    /// If the VKService gets dropped, all keys and modifiers get released to avoid a key getting repeatedly pressed even after the progam ended
    fn drop(&mut self) {
        error!("VKService was dropped");
        if self.release_all_keys_and_modifiers().is_err() {
            error!(
                "Some keys or modifiers could not be released and are still registered as pressed!"
            );
        };
    }
}

impl VKService {
    // Make a new VKService that is wrapped to allow changing values from multiple threads
    // This is necessary because when a CTRL+C signal is received, the keys and modifiers need to get released
    pub fn new(seat: &WlSeat, vk_mgr: &Main<ZwpVirtualKeyboardManagerV1>) -> Arc<Mutex<VKService>> {
        // Set starting values
        let base_time = Instant::now();
        let pressed_keys = HashSet::new();
        let pressed_modifiers = ModifiersBitflag::NO_MODIFIERS;
        // Get the VirtualKeyboard object from its manager
        let virtual_keyboard = vk_mgr.create_virtual_keyboard(&seat);
        // Initalize the keyboard with a keymap
        VKService::init_virtual_keyboard(&virtual_keyboard);
        // Get the proxy from the main object
        let virtual_keyboard = virtual_keyboard.as_ref().clone();
        // Create the service
        let vk_service = VKService {
            base_time,
            pressed_keys,
            pressed_modifiers,
            virtual_keyboard,
        };
        info!("VKService created");
        // Wrap the service in Arc<Mutex<>>
        let vk_service = Arc::new(Mutex::new(vk_service));
        // Overwrite the default handler of the CTRL+C signal to release the keys and modifiers when it is received
        VKService::release_keys_on_ctrl_c(Arc::clone(&vk_service));
        vk_service
    }

    /// Initialize the virtual keyboard with a keymap
    /// It can not be used before it gets initialized
    fn init_virtual_keyboard(virtual_keyboard_main: &Main<ZwpVirtualKeyboardV1>) {
        // Get the keymap the keyboard is supposed to get initialized with
        let src = super::keymap::KEYMAP;
        let keymap_size = super::keymap::KEYMAP.len();
        let keymap_size_u32: u32 = keymap_size.try_into().unwrap(); // Convert it from usize to u32, panics if it is not possible
        let keymap_size_u64: u64 = keymap_size.try_into().unwrap(); // Convert it from usize to u64, panics if it is not possible
                                                                    // Create a temporary file
        let mut keymap_file = tempfile().expect("Unable to create tempfile");
        // Allocate the required space in the file first
        keymap_file.seek(SeekFrom::Start(keymap_size_u64)).unwrap();
        keymap_file.write_all(&[0]).unwrap();
        keymap_file.seek(SeekFrom::Start(0)).unwrap();
        // Memory map the file
        let mut data = unsafe {
            memmap2::MmapOptions::new()
                .map_mut(&keymap_file)
                .expect("Could not access data from memory mapped file")
        };
        // Write the keymap to it
        data[..src.len()].copy_from_slice(src.as_bytes());
        // Initialize the virtual keyboard with the keymap
        let keymap_raw_fd = keymap_file.into_raw_fd();
        virtual_keyboard_main.keymap(1, keymap_raw_fd, keymap_size_u32);
        info!("VKService initialized the keyboard");
    }

    /// Get the elapsed time between now and when the keyboard was initialized
    fn get_time(&self) -> u32 {
        let duration = self.base_time.elapsed();
        let time = duration.as_millis();
        time.try_into().unwrap()
    }

    /// Try to release all keys and modifiers
    pub fn release_all_keys_and_modifiers(&mut self) -> Result<(), SubmitError> {
        // Try to release all keys
        let result_key_release = self.release_all_keys();
        // Try to release all modifiers
        let result_modifiert_release = self.release_all_modifiers();
        // If both was successful, return
        if result_key_release.is_ok() && result_modifiert_release.is_ok() {
            Ok(())
        // If there was an NotAlive error, return it
        } else if result_key_release == Err(SubmitError::NotAlive)
            || result_modifiert_release == Err(SubmitError::NotAlive)
        {
            Err(SubmitError::NotAlive) // This error is more important because it will no longer be possible to send any keycodes
        } else {
            Err(SubmitError::InvalidKeycode)
        }
    }

    /// Try to release all keys
    pub fn release_all_keys(&mut self) -> Result<(), SubmitError> {
        // Get the pressed keys in a vector
        let pressed_keys: Vec<u32> = self.pressed_keys.iter().cloned().collect();
        // If there are no errors, return Ok(())
        let mut success = Ok(());
        // Try to release each keycode individually
        for keycode in pressed_keys {
            // If it could not get released, return an error
            if let Err(err) = self.send_keycode(keycode, KeyMotion::Release) {
                success = Err(err); // Previous errors are disregarded
                error!(
                    "Failed to release all keys. Keycode causing the error: {}",
                    keycode
                );
            }
            // If it was successfully released, remove that key from the pressed_keys HashMap
            else {
                self.pressed_keys.remove(&keycode);
            }
        }
        success
    }

    /// Try to press and then release the key
    pub fn press_release_key(&mut self, keycode: u32) -> Result<(), SubmitError> {
        // Try to press the key
        let press_result = self.send_key(keycode, KeyMotion::Press);
        // If it was successfully pressed, try to release the key
        if press_result.is_ok() {
            self.send_key(keycode, KeyMotion::Release)
        } else {
            press_result
        }
    }

    /// Try to toggle the key
    /// If it was pressed before, release it
    /// If it was released before, press it
    pub fn toggle_key(&mut self, keycode: u32) -> Result<(), SubmitError> {
        // Check if it is currently pressed, if it is..
        if self.pressed_keys.contains(&keycode) {
            // ..release it
            self.send_key(keycode, KeyMotion::Release)
        }
        // If it is currently released
        else {
            // ..press it
            self.send_key(keycode, KeyMotion::Press)
        }
    }

    // Try to send a key press or a key release
    pub fn send_key(&mut self, keycode: u32, keymotion: KeyMotion) -> Result<(), SubmitError> {
        // The check is not strictly necessary because when building the keyboard, only keys with valid keycodes can be created
        // It is still useful though to catch errors if the programmer trys calling the method with invalid keycodes or if the module is used as library
        if input_event_codes_hashmap::is_valid_input_code(&input_event_codes_hashmap::KEY, keycode)
        {
            self.send_keycode(keycode, keymotion)
        } else {
            error!("Keycode {} was invalid", keycode);
            Err(SubmitError::InvalidKeycode)
        }
    }

    // Tries to send a key press or a key release via the virtual_keyboard protocol without checking if the keycode is valid
    fn send_keycode(&mut self, keycode: u32, keymotion: KeyMotion) -> Result<(), SubmitError> {
        if self.virtual_keyboard.is_alive() {
            // Add or remove the keycode from the HashSet of pressed keys
            match keymotion {
                KeyMotion::Press => self.pressed_keys.insert(keycode),
                KeyMotion::Release => self.pressed_keys.remove(&keycode),
            };
            // Get the wayland object from the proxy
            let virtual_keyboard = ZwpVirtualKeyboardV1::from(self.virtual_keyboard.clone());
            // Send the request to the wayland server
            virtual_keyboard.key(self.get_time(), keycode, keymotion as u32);
            Ok(())
        } else {
            error!("Virtual_keyboard proxy was no longer alive");
            Err(SubmitError::NotAlive)
        }
    }

    /// Release all modifiers
    pub fn release_all_modifiers(&mut self) -> Result<(), SubmitError> {
        let new_modifier_state = ModifiersBitflag::NO_MODIFIERS;
        self.send_modifiers_bitflag(new_modifier_state)
    }

    /// Toggle the specified modifier
    /// If it was pressed before, release it
    /// If it was released before, press it
    pub fn toggle_modifier(&mut self, modifier: keyboard::Modifier) -> Result<(), SubmitError> {
        let mut new_modifier_state = self.pressed_modifiers;
        new_modifier_state.toggle(ModifiersBitflag::from(modifier));
        self.send_modifiers_bitflag(new_modifier_state)
    }

    // Tries to send the bitflag of the pressed modifiers via the virtual_keyboard protocol
    fn send_modifiers_bitflag(&mut self, modifiers: ModifiersBitflag) -> Result<(), SubmitError> {
        if self.virtual_keyboard.is_alive() {
            // Get the wayland object from the proxy
            let virtual_keyboard = ZwpVirtualKeyboardV1::from(self.virtual_keyboard.clone());
            // Send the request to the wayland server
            virtual_keyboard.modifiers(
                modifiers.bits, //mods_depressed,
                0,              //mods_latched
                0,              //mods_locked
                0,              //group
            );
            self.pressed_modifiers = modifiers;
            Ok(())
        } else {
            error!("Virtual_keyboard proxy was no longer alive");
            Err(SubmitError::NotAlive)
        }
    }

    /// This method tries to submit a unicode string by entering each of its character individually with a combination of keypresses.
    /// There are multiple keypresses needed for each character and some applications do not support this!
    /// At least under GNOME this should work but it is very clumsy and should only be used as a last resort.
    pub fn send_unicode_str(&mut self, text: &str) -> Result<(), SubmitError> {
        warn!(
            "Trying to submit unicode string '{}' with virtual_keyboard protocol. Some applications do not support it. This is clumsy and should be avoided",
            text
        );

        // Save state of the keys and modifiers
        let previously_pressed_keys = self.pressed_keys.clone();
        let previously_pressed_modifiers = self.pressed_modifiers;

        // Release everything to start in a clean state
        unwrap_or_return!(self.release_all_keys_and_modifiers());

        // Submit each unicode character individually
        let mut result = Ok(());
        for unicode_char in text.chars() {
            match self.send_unicode_char(unicode_char) {
                Ok(()) => {}
                Err(err) => {
                    result = Err(err);
                    error!("Failed to submit the char '{}'", unicode_char);
                    break;
                }
            }
        }

        // Restore previous state of the keys and modifiers
        for keycode in previously_pressed_keys {
            unwrap_or_return!(self.send_keycode(keycode, KeyMotion::Press));
        }
        unwrap_or_return!(self.send_modifiers_bitflag(previously_pressed_modifiers));
        result
    }

    /// This method tries to submit a unicode char by looking up its hex value and then entering CTRL + SHIFT + u, the keycodes for the hex values and then 'SPACE'
    /// At least under GNOME this should be converted to the corresponding unicode character. This is very clumsy and should only be used as a last resort.
    fn send_unicode_char(&mut self, unicode_char: char) -> Result<(), SubmitError> {
        // Press CTRL
        unwrap_or_return!(self.send_modifiers_bitflag(ModifiersBitflag::CONTROL));

        // Press CTRL + SHIFT
        let ctrl_and_shift = ModifiersBitflag::CONTROL | ModifiersBitflag::SHIFT;
        unwrap_or_return!(self.send_modifiers_bitflag(ctrl_and_shift));

        // Press and release 'U'
        unwrap_or_return!(self.press_release_key(22)); // 22 is the keycode for 'U'

        // Get which codes to enter for the unicode char and enter each of the codes
        // escape_unicode() returns \u{XXXX} but only the XXXX (hex code) are of interest so the rest is skipped. The number of X depends on the unicode character
        for hexadecimal_unicode_escape in unicode_char
            .escape_unicode()
            .skip(3)
            .take_while(char::is_ascii_alphanumeric)
        {
            let keycode = String::from(hexadecimal_unicode_escape.to_ascii_uppercase()); // Necessary because all keys in the HashMap are uppercase
                                                                                         // Get the keycode of the unicode escape
            let keycode = if let Some(keycode) = input_event_codes_hashmap::KEY.get::<str>(&keycode)
            {
                keycode
            } else {
                error!("Keycode for '{}' was not found", hexadecimal_unicode_escape);
                return Err(SubmitError::InvalidKeycode);
            };
            unwrap_or_return!(self.press_release_key(*keycode));
        }

        // Press and release 'SPACE'
        // The keycode for 'SPACE' is 57
        unwrap_or_return!(self.press_release_key(57));
        // Release CTRL + SHIFT
        unwrap_or_return!(self.send_modifiers_bitflag(ModifiersBitflag::NO_MODIFIERS));
        Ok(())
    }

    /// Overwrites the handle of the CTRL+C signal so that all keys and modifiers are released before the application is ended
    fn release_keys_on_ctrl_c(vk_service: Arc<Mutex<VKService>>) {
        ctrlc::set_handler(move || {
        warn!("Received CTRL+C signal. Aborting program!");
        if vk_service
            .lock()
            .unwrap()
            .release_all_keys_and_modifiers()
            .is_err()
        {
            error!("Some keys or modifiers could not be released and are still registered as pressed!");
        }
        std::process::exit(130);
    })
    .expect("Error setting Ctrl-C handler");
    }
}
