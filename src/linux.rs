extern crate x11_dl;
use self::x11_dl::{xlib, xtest};
use std::ptr;

use super::MouseControllable;

pub struct Enigo {
    pub display: *mut xlib::Display,
    pub window: xlib::Window,
    pub xlib: xlib::Xlib,
    pub xtest: xtest::Xf86vmode,
}

impl Enigo {
    pub fn new() -> Self {
        unsafe {
            let xlib = xlib::Xlib::open().unwrap();
            let xtest = xtest::Xf86vmode::open().unwrap();

            let display = (xlib.XOpenDisplay)(ptr::null());
            if display == ptr::null_mut() {
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
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&self, x: i32, y: i32) {
        if self.display == ptr::null_mut() {
            panic!("display is not available")
        }

        unsafe {
            (self.xlib.XWarpPointer)(self.display, 0, self.window, 0, 0, 0, 0, x, y);
            (self.xlib.XFlush)(self.display);
        }
    }

    fn mouse_move_relative(&self, x: i32, y: i32) {
        if self.display == ptr::null_mut() {
            panic!("display is not available")
        }

        unsafe {
            (self.xlib.XWarpPointer)(self.display, 0, 0, 0, 0, 0, 0, x, y);
            (self.xlib.XFlush)(self.display);
        }
    }

    //TODO(dustin): make button a new type
    fn mouse_down(&self, button: u32) {
        if self.display == ptr::null_mut() {
            panic!("display is not available")
        }

        unsafe {
            //TODO(dustin): make 1, 0 / true false a new type
            (self.xtest.XTestFakeButtonEvent)(self.display, button, 1, 0);
            (self.xlib.XFlush)(self.display);
        }
    }

    fn mouse_up(&self, button: u32) {
        if self.display == ptr::null_mut() {
            panic!("display is not available")
        }

        unsafe {
            //TODO(dustin): make 1, 0 / true false a new type
            (self.xtest.XTestFakeButtonEvent)(self.display, button, 0, 0);
            (self.xlib.XFlush)(self.display);
        }
    }

    fn mouse_click(&self, button: u32) {
        use std::{thread, time};

        self.mouse_down(button);
        thread::sleep(time::Duration::from_millis(100));
        self.mouse_up(button);
    }

    fn mouse_scroll_x(&self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = 6; //scroll left button
        } else {
            button = 7; //scroll right button
        }

        if length < 0 {
            length *= -1;
        }

        for _ in 0..length {
            self.mouse_down(button);
            self.mouse_up(button);
        }
    }

    fn mouse_scroll_y(&self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = 4; //scroll up button
        } else {
            button = 5; //scroll down button
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
