extern crate x11_dl;
extern crate regex;

use self::regex::Regex;
use self::x11_dl::{xlib, xtest};

use super::MouseControllable;
use std::ffi::CString;
use std::{io, ptr};
use std::os::raw::{c_char, c_void};

/// The main struct for handling the event emitting
pub struct Enigo {
    display: *mut xlib::Display,
    window: xlib::Window,
    xlib: xlib::Xlib,
    xtest: xtest::Xf86vmode,
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
        unsafe {
            let xlib = xlib::Xlib::open().unwrap();
            let xtest = xtest::Xf86vmode::open().unwrap();

            let display = (xlib.XOpenDisplay)(ptr::null());
            if display.is_null() {
                panic!("can't open display");
            }

            let window = (xlib.XDefaultRootWindow)(display);

            Enigo {
                display: display,
                window: window,
                xlib: xlib,
                xtest: xtest,
            }
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
            (self.xlib.XWarpPointer)(self.display, 0, self.window, 0, 0, 0, 0, x, y);
            (self.xlib.XFlush)(self.display);
        }
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        if self.display.is_null() {
            panic!("display is not available")
        }

        unsafe {
            (self.xlib.XWarpPointer)(self.display, 0, 0, 0, 0, 0, 0, x, y);
            (self.xlib.XFlush)(self.display);
        }
    }

    // TODO(dustin): make button a new type
    fn mouse_down(&mut self, button: u32) {
        if self.display.is_null() {
            panic!("display is not available")
        }

        unsafe {
            // TODO(dustin): make 1, 0 / true false a new type
            (self.xtest.XTestFakeButtonEvent)(self.display, button, 1, 0);
            (self.xlib.XFlush)(self.display);
        }
    }

    fn mouse_up(&mut self, button: u32) {
        if self.display.is_null() {
            panic!("display is not available")
        }

        unsafe {
            // TODO(dustin): make 1, 0 / true false a new type
            (self.xtest.XTestFakeButtonEvent)(self.display, button, 0, 0);
            (self.xlib.XFlush)(self.display);
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

impl io::Write for Enigo {
    /*
	fn write_fmt(&mut self, fmt: fmt::Arguments) -> io::Result<()> {
		// TODO
	}
	*/
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let sequence = ::std::str::from_utf8(buf).unwrap();

        lazy_static! {
			//NOTE(dustin):   no error handling nessesary, this is a bug
			static ref RE: Regex = Regex::new(r"\\u\{(.*)\}").unwrap();
		}

        for c in sequence.chars() {
            let rust_unicode: String = c.escape_unicode().collect();
            // TODO(dustin): handle this error
            let unicode_string = format!("U{}",
                                         RE.captures(&rust_unicode)
                                             .unwrap()
                                             .get(1)
                                             .unwrap()
                                             .as_str());
            let keycode = self.unicode_string_to_keycode(&unicode_string);
            self.keycode_click(keycode)
        }
        Ok(())
    }
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Enigo {
    fn unicode_string_to_keycode(&self, unicode_string: &str) -> i32 {
        let unicode_as_c_string = CString::new(unicode_string).unwrap();
        let key_sym =
            unsafe { (self.xlib.XStringToKeysym)(unicode_as_c_string.as_ptr() as *mut c_char) };

        let mut min = 0;
        let mut max = 0;
        let mut numcodes = 0;

        unsafe { (self.xlib.XDisplayKeycodes)(self.display, &mut min, &mut max) };

        let upper = max as i32 - min as i32 + 1;
        let key_sym_mapped = unsafe {
            (self.xlib.XGetKeyboardMapping)(self.display, min as u8, upper, &mut numcodes)
        };
        let idx = ((max as i32 - min as i32 - 1) * numcodes as i32) as isize;

        unsafe {
            let map = key_sym_mapped.offset(idx);
            *map = key_sym;
        }

        unsafe {
            (self.xlib.XChangeKeyboardMapping)(self.display,
                                               min as i32,
                                               numcodes as i32,
                                               key_sym_mapped,
                                               (max as i32 - min as i32));
            (self.xlib.XFree)(key_sym_mapped as *mut c_void);
            (self.xlib.XFlush)(self.display);
            let keycode = (self.xlib.XKeysymToKeycode)(self.display, key_sym);

            keycode as i32
        }

    }

    fn keycode_click(&self, keycode: i32) {
        use std::{thread, time};
        thread::sleep(time::Duration::from_millis(20));
        self.keycode_down(keycode);
        self.keycode_up(keycode);
        thread::sleep(time::Duration::from_millis(20));
    }

    fn keycode_down(&self, keycode: i32) {
        unsafe {
            (self.xtest.XTestFakeKeyEvent)(self.display, keycode as u32, 1, 1);
            (self.xlib.XFlush)(self.display);
        }
    }

    fn keycode_up(&self, keycode: i32) {
        unsafe {
            (self.xtest.XTestFakeKeyEvent)(self.display, keycode as u32, 0, 1);
            (self.xlib.XFlush)(self.display);
        }
    }
}
