extern crate libc;

use {KeyboardControllable, Key, MouseControllable, MouseButton};
use linux::keysyms::*;

use self::libc::{c_ulong, c_uint, c_int, c_char, c_void};
use std::borrow::Cow;
use std::ffi::CString;
use std::ops::Deref;
use std::ptr;

const CURRENT_WINDOW: c_int = 0;
type Window  = c_int;
type Xdo     = *const c_void;

#[link(name = "xdo")]
extern "C" {
    fn xdo_free(xdo: Xdo);
    fn xdo_new(display: *const c_char) -> Xdo;

    fn xdo_click_window(xdo: Xdo, window: Window, button: c_int) -> c_int;
    fn xdo_mouse_down(xdo: Xdo, window: Window, button: c_int) -> c_int;
    fn xdo_mouse_up(xdo: Xdo, window: Window, button: c_int) -> c_int;
    fn xdo_move_mouse(xdo: Xdo, x: c_int, y: c_int, screen: c_int) -> c_int;
    fn xdo_move_mouse_relative(xdo: Xdo, x: c_int, y: c_int) -> c_int;

    fn xdo_enter_text_window(xdo: Xdo, window: Window, string: *const c_char, delay: c_ulong);
    fn xdo_send_keysequence_window(xdo: Xdo, window: Window, string: *const c_char, delay: c_ulong);
    fn xdo_send_keysequence_window_down(xdo: Xdo, window: Window, string: *const c_char, delay: c_ulong);
    fn xdo_send_keysequence_window_up(xdo: Xdo, window: Window, string: *const c_char, delay: c_ulong);
}

fn mousebutton(button: MouseButton) -> i32 {
    match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::ScrollUp => 4,
        MouseButton::ScrollDown => 5,
        MouseButton::ScrollLeft => 6,
        MouseButton::ScrollRight => 7
    }
}

/// The main struct for handling the event emitting
pub struct Enigo {
    xdo: Xdo
}
impl Enigo {
    /// Create a new Enigo instance
    pub fn new() -> Enigo {
        Enigo {
            xdo: unsafe { xdo_new(ptr::null()) }
        }
    }
}
impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        unsafe {
            xdo_move_mouse(self.xdo, x, y, 0);
        }
    }
    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        unsafe {
            xdo_move_mouse_relative(self.xdo, x, y);
        }
    }
    fn mouse_down(&mut self, button: MouseButton) {
        unsafe {
            xdo_mouse_down(self.xdo, CURRENT_WINDOW, mousebutton(button));
        }
    }
    fn mouse_up(&mut self, button: MouseButton) {
        unsafe {
            xdo_mouse_up(self.xdo, CURRENT_WINDOW, mousebutton(button));
        }
    }
    fn mouse_click(&mut self, button: MouseButton) {
        unsafe {
            xdo_click_window(self.xdo, CURRENT_WINDOW, mousebutton(button));
        }
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
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
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
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }
}
fn keysequence<'a>(key: Key) -> Cow<'a, str> {
    if let Key::Layout(c) = key {
        return Cow::from(c.to_string());
    }
    Cow::from(match key {
        Key::Alt => "Alt",
        Key::Backspace => "Backspace",
        Key::CapsLock => "CapsLock",
        Key::Command => "Command",
        Key::Control => "Control",
        Key::DownArrow => "DownArrow",
        Key::Escape => "Escape",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::Home => "Home",
        Key::Layout(_) => unreachable!(),
        Key::LeftArrow => "Left",
        Key::Option => "Option",
        Key::PageDown => "PageDown",
        Key::PageUp => "PageUp",
        Key::Raw(_) => unimplemented!(),
        Key::Return => "Return",
        Key::RightArrow => "Right",
        Key::Shift => "Shift",
        Key::Space => "Space",
        Key::Super => "Super",
        Key::Tab => "Tab",
        Key::UpArrow => "Up",
        Key::Windows => "Windows"
    })
}
impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        let string = CString::new(sequence).unwrap();
        unsafe {
            xdo_enter_text_window(self.xdo, CURRENT_WINDOW, string.as_ptr(), 12000);
        }
    }
    fn key_down(&mut self, key: Key) {
        let string = CString::new(&*keysequence(key)).unwrap();
        unsafe {
            xdo_send_keysequence_window_down(self.xdo, CURRENT_WINDOW, string.as_ptr(), 12000);
        }
    }
    fn key_up(&mut self, key: Key) {
        let string = CString::new(&*keysequence(key)).unwrap();
        unsafe {
            xdo_send_keysequence_window_up(self.xdo, CURRENT_WINDOW, string.as_ptr(), 12000);
        }
    }
    fn key_click(&mut self, key: Key) {
        let string = CString::new(&*keysequence(key)).unwrap();
        unsafe {
            xdo_send_keysequence_window(self.xdo, CURRENT_WINDOW, string.as_ptr(), 12000);
        }
    }
}
