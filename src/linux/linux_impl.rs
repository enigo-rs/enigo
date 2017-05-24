extern crate libc;
extern crate regex;

use self::regex::Regex;

use ::{KeyboardControllable, MouseControllable, Key};
use linux::keysyms::*;
use std::ffi::CString;
use std::ptr;
use self::libc::{c_ulong, c_uint, c_int, c_char, c_void};

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
    fn XChangeKeyboardMapping(display: Display,
                              first_keycode: c_int,
                              keycode_count: c_int,
                              keysyms: *const KeySym,
                              keysyms_per_keycode_return: c_int)
                              -> KeySym;
    fn XGetKeyboardMapping(display: Display,
                           first_keycode: KeyCode,
                           keycode_count: c_int,
                           keysyms_per_keycode_return: *mut c_int)
                           -> *mut KeySym;
    fn XDisplayKeycodes(display: Display,
                        min_keycodes_return: *mut c_int,
                        max_keycodes_return: *mut c_int)
                        -> c_int;

    fn XWarpPointer(display: Display,
                    src_w: Window,
                    dest_w: Window,
                    src_x: c_int,
                    src_y: c_int,
                    src_width: c_int,
                    src_height: c_int,
                    dest_x: c_int,
                    dest_y: c_int);
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

    // TODO(dustin): make button a new type
    fn mouse_down(&mut self, button: u32) {
        if self.display.is_null() {
            panic!("display is not available")
        }

        unsafe {
            // TODO(dustin): make 1, 0 / true false a new type
            XTestFakeButtonEvent(self.display, button, 1, 0);
            XFlush(self.display);
        }
    }

    fn mouse_up(&mut self, button: u32) {
        if self.display.is_null() {
            panic!("display is not available")
        }

        unsafe {
            // TODO(dustin): make 1, 0 / true false a new type
            XTestFakeButtonEvent(self.display, button, 0, 0);
            XFlush(self.display);
        }
    }

    fn mouse_click(&mut self, button: u32) {
        use std::{thread, time};

        self.mouse_down(button);
        thread::sleep(time::Duration::from_millis(100));
        self.mouse_up(button);
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = 6; // scroll left button
        } else {
            button = 7; // scroll right button
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
            button = 4; // scroll up button
        } else {
            button = 5; // scroll down button
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
        lazy_static! {
			//NOTE(dustin):   no error handling nessesary, this is a bug
			static ref RE: Regex = Regex::new(r"\\u\{(.*)\}").unwrap();
		}

        for c in sequence.chars() {
            let rust_unicode: String = c.escape_unicode().collect();
            // TODO(dustin): handle this error
            let unicode_string =
                format!("U{}",
                        RE.captures(&rust_unicode).unwrap().get(1).unwrap().as_str());
            let keycode = self.unicode_string_to_keycode(&unicode_string);
            self.keycode_click(keycode)
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

impl Enigo {
    fn unicode_string_to_keycode(&self, unicode_string: &str) -> u32 {
        let unicode_as_c_string = CString::new(unicode_string).unwrap();
        let key_sym = unsafe { XStringToKeysym(unicode_as_c_string.as_ptr() as *mut c_char) };

        let mut min = 0;
        let mut max = 0;
        let mut numcodes = 0;

        unsafe { XDisplayKeycodes(self.display, &mut min, &mut max) };

        let upper = max as i32 - min as i32 + 1;
        let key_sym_mapped =
            unsafe { XGetKeyboardMapping(self.display, min as u32, upper, &mut numcodes) };
        let idx = ((max as i32 - min as i32 - 1) * numcodes as i32) as isize;

        unsafe {
            let map = key_sym_mapped.offset(idx);
            *map = key_sym;
        }

        unsafe {
            XChangeKeyboardMapping(self.display,
                                   min as i32,
                                   numcodes as i32,
                                   key_sym_mapped,
                                   (max as i32 - min as i32));
            XFree(key_sym_mapped as *mut c_void);
            XFlush(self.display);
            let keycode = XKeysymToKeycode(self.display, key_sym, 0);

            keycode
        }

    }

    fn key_to_keycode(&self, key: Key) -> u32 {
        unsafe {
            match key {
                Key::RETURN => XKeysymToKeycode(self.display, XK_Return as *const c_void, 0),
                Key::TAB => XKeysymToKeycode(self.display, XK_Tab as *const c_void, 0),
                Key::SHIFT => XKeysymToKeycode(self.display, XK_Shift_L as *const c_void, 0),
                _ => 0,
            }
        }
    }

    fn keycode_click(&self, keycode: u32) {
        use std::{thread, time};
        thread::sleep(time::Duration::from_millis(20));
        self.keycode_down(keycode);
        self.keycode_up(keycode);
        thread::sleep(time::Duration::from_millis(20));
    }

    fn keycode_down(&self, keycode: u32) {
        unsafe {
            XTestFakeKeyEvent(self.display, keycode as u32, 1, 1);
            XFlush(self.display);
        }
    }

    fn keycode_up(&self, keycode: u32) {
        unsafe {
            XTestFakeKeyEvent(self.display, keycode as u32, 0, 1);
            XFlush(self.display);
        }
    }
}
