extern crate libc;

use {KeyboardControllable, Key, MouseControllable, MouseButton};
use linux::keysyms::*;

use self::libc::{c_ulong, c_uint, c_int, c_char, c_void};
use std::ffi::CString;
use std::ops::Deref;
use std::ptr;

pub type Display = *const c_void;
pub type Window = c_int;

pub type KeySym = *const c_void;
pub type KeyCode = c_uint;

type Bool = c_int;

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(string: *const c_char) -> Display;
    fn XRootWindow(display: Display, index: c_int) -> Window;
    fn XFree(data: *const c_void) -> c_int;
    fn XFlush(display: Display) -> c_int;

    fn XStringToKeysym(string: *const c_char) -> KeySym;
    fn XKeysymToKeycode(display: Display, keysym: KeySym, index: c_int) -> KeyCode;
    fn XChangeKeyboardMapping(
        display: Display,
        first_keycode: c_int,
        keycode_count: c_int,
        keysyms: *const KeySym,
        keysyms_per_keycode_return: c_int,
    ) -> KeySym;
    fn XGetKeyboardMapping(
        display: Display,
        first_keycode: KeyCode,
        keycode_count: c_int,
        keysyms_per_keycode_return: *mut c_int,
    ) -> *mut KeySym;
    fn XDisplayKeycodes(
        display: Display,
        min_keycodes_return: *mut c_int,
        max_keycodes_return: *mut c_int,
    ) -> c_int;

    fn XWarpPointer(
        display: Display,
        src_w: Window,
        dest_w: Window,
        src_x: c_int,
        src_y: c_int,
        src_width: c_int,
        src_height: c_int,
        dest_x: c_int,
        dest_y: c_int,
    );
}

#[link(name = "Xtst")]
extern "C" {
    fn XTestFakeKeyEvent(display: Display, keycode: KeyCode, state: Bool, delay: c_ulong);
    fn XTestFakeButtonEvent(display: Display, keycode: KeyCode, state: Bool, delay: c_ulong);
}

/// The main struct for handling the event emitting
pub struct Enigo {
    display: Display,
    window: Window,
}

impl Enigo {
    /// Constructs a new `Enigo` instance.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// ```
    pub fn new() -> Self {
        let display;
        unsafe { display = XOpenDisplay(ptr::null()) };
        if display.is_null() {
            panic!("can't open display");
        }

        let window = unsafe { XRootWindow(display, 0) };

        Enigo {
            display: display,
            window: window,
        }
    }

    // TODO(dustin): implement drop
}

impl Default for Enigo {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        if self.display.is_null() {
            panic!("display is not available")
        }

        unsafe {
            XWarpPointer(self.display, 0, self.window, 0, 0, 0, 0, x, y);
            XFlush(self.display);
        }
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {

        if self.display.is_null() {
            panic!("display is not available")
        }

        unsafe {
            XWarpPointer(self.display, 0, 0, 0, 0, 0, 0, x, y);
            XFlush(self.display);
        }
    }

    fn mouse_down(&mut self, button: MouseButton) {
        if self.display.is_null() {
            panic!("display is not available")
        }

        unsafe {
            XTestFakeButtonEvent(
                self.display,
                match button {
                    MouseButton::Left => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Right => 3,
                    MouseButton::ScrollUp => 4,
                    MouseButton::ScrollDown => 5,
                    MouseButton::ScrollLeft => 6,
                    MouseButton::ScrollRight => 7,
                },
                1,
                0,
            );
            XFlush(self.display);
        }
    }

    fn mouse_up(&mut self, button: MouseButton) {
        if self.display.is_null() {
            panic!("display is not available")
        }

        unsafe {
            XTestFakeButtonEvent(
                self.display,
                match button {
                    MouseButton::Left => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Right => 3,
                    MouseButton::ScrollUp => 4,
                    MouseButton::ScrollDown => 5,
                    MouseButton::ScrollLeft => 6,
                    MouseButton::ScrollRight => 7,
                },
                0,
                0,
            );
            XFlush(self.display);
        }
    }

    fn mouse_click(&mut self, button: MouseButton) {
        use std::{thread, time};

        self.mouse_down(button);
        thread::sleep(time::Duration::from_millis(100));
        self.mouse_up(button);
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = MouseButton::ScrollLeft;
        } else {
            button = MouseButton::ScrollRight;
        }

        if length < 0 {
            length *= -1;
        }

        for _ in 0..length {
            self.mouse_down(button);
            self.mouse_up(button);
        }
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = MouseButton::ScrollUp;
        } else {
            button = MouseButton::ScrollDown;
        }

        if length < 0 {
            length *= -1;
        }

        for _ in 0..length {
            self.mouse_down(button);
            self.mouse_up(button);
        }
    }
}

impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        for c in sequence.chars() {
            let rust_unicode: String = format!("U{:x}", c as u32);
            let keycode = self.unicode_string_to_keycode(&rust_unicode);

            self.keycode_click(*keycode);
        }
    }

    fn key_click(&mut self, key: Key) {
        self.keycode_click(self.key_to_keycode(key));
    }

    fn key_down(&mut self, key: Key) {
        self.keycode_down(self.key_to_keycode(key));
    }

    fn key_up(&mut self, key: Key) {
        self.keycode_up(self.key_to_keycode(key));
    }
}

struct KeyGuard<'a>(&'a Enigo, u32);

impl<'a> Deref for KeyGuard<'a> {
	type Target = u32;

	fn deref(&self) -> &Self::Target {
		return &self.1;
	}
}
impl<'a> Drop for KeyGuard<'a> {
	fn drop(&mut self) {
		self.0.reset_keycode(self.1);
	}
}

impl Enigo {
    fn reset_keycode(&self, keycode: u32) {
        unsafe {
            let keysym_list = [0 as KeySym, 0 as KeySym];
            let keysym_list = keysym_list.as_ptr();
            XChangeKeyboardMapping(self.display, keycode as i32, 2, keysym_list, 1);
        }
    }

    fn unicode_string_to_keycode(&self, unicode_string: &str) -> KeyGuard {
        let mut keysyms_per_keycode = 0;
        // scratch space for temporary keycode bindings
        let mut scratch_keycode = 0;
        let mut keycode_low = 0;
        let mut keycode_high = 0;

        let keysyms = unsafe {
            // get the range of keycodes usually from 8 - 255
            XDisplayKeycodes(self.display, &mut keycode_low, &mut keycode_high);
            // get all the mapped keysysms available
            let keycode_count = keycode_high - keycode_low;
            XGetKeyboardMapping(
                self.display,
                keycode_low as u32,
                keycode_count,
                &mut keysyms_per_keycode,
            )
        };

        // find unused keycode for unmapped keysyms so we can
        // hook up our own keycode and map every keysym on it
        // so we just need to 'click' our once unmapped keycode
        for cidx in keycode_low..keycode_high + 1 {
            let mut key_is_empty = true;
            for sidx in 0..keysyms_per_keycode {
                let map_idx = (cidx - keycode_low) * keysyms_per_keycode + sidx;
                let sym_at_idx = unsafe { keysyms.offset(map_idx as isize) };
                if unsafe { *sym_at_idx } != 0 as *const c_void {
                    key_is_empty = false;
                } else {
                    break;
                }
            }
            if key_is_empty {
                scratch_keycode = cidx;
                break;
            }
        }

        unsafe {
            XFree(keysyms as *mut c_void);
            XFlush(self.display);
        }

        // TODO(dustin) make this an error!
        if scratch_keycode == 0 {
            panic!("cannot find free keycode");
        }

        // find the keysym for the given unicode char
        // map that keysym to our previous unmapped keycode
        // click that keycode/'button' with our keysym on it
        let unicode_as_c_string = CString::new(unicode_string).unwrap();
        let keysym = unsafe { XStringToKeysym(unicode_as_c_string.as_ptr() as *mut c_char) };
        let keysym_list = [keysym, keysym];
        let keysym_list = keysym_list.as_ptr();
        unsafe {
            XChangeKeyboardMapping(self.display, scratch_keycode, 2, keysym_list, 1);
            XFlush(self.display);
        }

        KeyGuard(self, scratch_keycode as u32)
    }

    fn key_to_keycode(&self, key: Key) -> u32 {
        unsafe {
            match key {
                Key::Return => XKeysymToKeycode(self.display, XK_RETURN as *const c_void, 0),
                Key::Tab => XKeysymToKeycode(self.display, XK_TAB as *const c_void, 0),
                Key::Space => XKeysymToKeycode(self.display, XK_SPACE as *const c_void, 0),
                Key::Backspace => XKeysymToKeycode(self.display, XK_BACKSPACE as *const c_void, 0),
                Key::Escape => XKeysymToKeycode(self.display, XK_ESCAPE as *const c_void, 0),
                Key::Super => XKeysymToKeycode(self.display, XK_SUPER_L as *const c_void, 0),
                Key::Command => XKeysymToKeycode(self.display, XK_SUPER_L as *const c_void, 0),
                Key::Windows => XKeysymToKeycode(self.display, XK_SUPER_L as *const c_void, 0),
                Key::Shift => XKeysymToKeycode(self.display, XK_SHIFT_L as *const c_void, 0),
                Key::CapsLock => XKeysymToKeycode(self.display, XK_CAPS_LOCK as *const c_void, 0),
                Key::Alt => XKeysymToKeycode(self.display, XK_ALT_L as *const c_void, 0),
                Key::Option => XKeysymToKeycode(self.display, XK_ALT_L as *const c_void, 0),
                Key::Control => XKeysymToKeycode(self.display, XK_CONTROL_L as *const c_void, 0),
                Key::Home => XKeysymToKeycode(self.display, XK_HOME as *const c_void, 0),
                Key::PageUp => XKeysymToKeycode(self.display, XK_PAGE_UP as *const c_void, 0),
                Key::PageDown => XKeysymToKeycode(self.display, XK_PAGE_DOWN as *const c_void, 0),
                Key::LeftArrow => XKeysymToKeycode(self.display, XK_LEFT as *const c_void, 0),
                Key::RightArrow => XKeysymToKeycode(self.display, XK_RIGHT as *const c_void, 0),
                Key::DownArrow => XKeysymToKeycode(self.display, XK_DOWN as *const c_void, 0),
                Key::UpArrow => XKeysymToKeycode(self.display, XK_UP as *const c_void, 0),
                Key::F1 => XKeysymToKeycode(self.display, XK_F1 as *const c_void, 0),
                Key::F2 => XKeysymToKeycode(self.display, XK_F2 as *const c_void, 0),
                Key::F3 => XKeysymToKeycode(self.display, XK_F3 as *const c_void, 0),
                Key::F4 => XKeysymToKeycode(self.display, XK_F4 as *const c_void, 0),
                Key::F5 => XKeysymToKeycode(self.display, XK_F5 as *const c_void, 0),
                Key::F6 => XKeysymToKeycode(self.display, XK_F6 as *const c_void, 0),
                Key::F7 => XKeysymToKeycode(self.display, XK_F7 as *const c_void, 0),
                Key::F8 => XKeysymToKeycode(self.display, XK_F8 as *const c_void, 0),
                Key::F9 => XKeysymToKeycode(self.display, XK_F9 as *const c_void, 0),
                Key::F10 => XKeysymToKeycode(self.display, XK_F10 as *const c_void, 0),
                Key::F11 => XKeysymToKeycode(self.display, XK_F11 as *const c_void, 0),
                Key::F12 => XKeysymToKeycode(self.display, XK_F12 as *const c_void, 0),

                Key::Raw(raw_keycode) => raw_keycode as u32,
                Key::Layout(c) => self.get_layoutdependent_keycode(c.to_string()),
            }
        }
    }

    fn get_layoutdependent_keycode(&self, string: String) -> u32 {
        let c_string = CString::new(string).unwrap();
        let keysym = unsafe { XStringToKeysym(c_string.as_ptr() as *mut c_char) };

        unsafe { XKeysymToKeycode(self.display, keysym, 0) }
    }

    fn keycode_click(&self, keycode: u32) {
        self.keycode_down(keycode);
        self.keycode_up(keycode);
    }

    fn keycode_down(&self, keycode: u32) {
        use std::{thread, time};
        thread::sleep(time::Duration::from_millis(10));
        unsafe {
            XTestFakeKeyEvent(self.display, keycode, 1, 0);
            XFlush(self.display);
        }
        thread::sleep(time::Duration::from_millis(10));
    }

    fn keycode_up(&self, keycode: u32) {
        use std::{thread, time};
        thread::sleep(time::Duration::from_millis(10));
        unsafe {
            XTestFakeKeyEvent(self.display, keycode, 0, 0);
            XFlush(self.display);
        }
        thread::sleep(time::Duration::from_millis(10));
    }
}
