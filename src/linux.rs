use libc;

use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};

use libc::{c_char, c_int, c_void, useconds_t};
use std::{borrow::Cow, ffi::CString, ptr};

const CURRENT_WINDOW: c_int = 0;
const DEFAULT_DELAY: u64 = 12000;
type Window = c_int;
type Xdo = *const c_void;

#[link(name = "xdo")]
extern "C" {
    fn xdo_free(xdo: Xdo);
    fn xdo_new(display: *const c_char) -> Xdo;

    fn xdo_click_window(xdo: Xdo, window: Window, button: c_int) -> c_int;
    fn xdo_mouse_down(xdo: Xdo, window: Window, button: c_int) -> c_int;
    fn xdo_mouse_up(xdo: Xdo, window: Window, button: c_int) -> c_int;
    fn xdo_move_mouse(xdo: Xdo, x: c_int, y: c_int, screen: c_int) -> c_int;
    fn xdo_move_mouse_relative(xdo: Xdo, x: c_int, y: c_int) -> c_int;

    fn xdo_enter_text_window(
        xdo: Xdo,
        window: Window,
        string: *const c_char,
        delay: useconds_t,
    ) -> c_int;
    fn xdo_send_keysequence_window(
        xdo: Xdo,
        window: Window,
        string: *const c_char,
        delay: useconds_t,
    ) -> c_int;
    fn xdo_send_keysequence_window_down(
        xdo: Xdo,
        window: Window,
        string: *const c_char,
        delay: useconds_t,
    ) -> c_int;
    fn xdo_send_keysequence_window_up(
        xdo: Xdo,
        window: Window,
        string: *const c_char,
        delay: useconds_t,
    ) -> c_int;

    fn xdo_get_viewport_dimensions(
        xdo: Xdo,
        width: *mut c_int,
        height: *mut c_int,
        screen: c_int,
    ) -> c_int;

    fn xdo_get_mouse_location2(
        xdo: Xdo,
        x: *mut c_int,
        y: *mut c_int,
        screen: *mut c_int,
        window: *mut Window,
    ) -> c_int;
}

fn mousebutton(button: MouseButton) -> c_int {
    match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::ScrollUp => 4,
        MouseButton::ScrollDown => 5,
        MouseButton::ScrollLeft => 6,
        MouseButton::ScrollRight => 7,
        MouseButton::Back => 8,
        MouseButton::Forward => 9,
    }
}

/// The main struct for handling the event emitting
pub struct Enigo {
    xdo: Xdo,
    delay: u64,
}
// This is safe, we have a unique pointer.
// TODO: use Unique<c_char> once stable.
unsafe impl Send for Enigo {}

impl Default for Enigo {
    /// Create a new Enigo instance
    fn default() -> Self {
        Self {
            xdo: unsafe { xdo_new(ptr::null()) },
            delay: DEFAULT_DELAY,
        }
    }
}
impl Enigo {
    /// Get the delay per keypress.
    /// Default value is 12000.
    /// This is Linux-specific.
    #[must_use]
    pub fn delay(&self) -> u64 {
        self.delay
    }
    /// Set the delay per keypress.
    /// This is Linux-specific.
    pub fn set_delay(&mut self, delay: u64) {
        self.delay = delay;
    }
}
impl Drop for Enigo {
    fn drop(&mut self) {
        unsafe {
            xdo_free(self.xdo);
        }
    }
}
impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        unsafe {
            xdo_move_mouse(self.xdo, x as c_int, y as c_int, 0);
        }
    }
    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        unsafe {
            xdo_move_mouse_relative(self.xdo, x as c_int, y as c_int);
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
        let mut length = length;
        let button = if length < 0 {
            MouseButton::ScrollLeft
        } else {
            MouseButton::ScrollRight
        };

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }
    fn mouse_scroll_y(&mut self, length: i32) {
        let mut length = length;
        let button = if length < 0 {
            MouseButton::ScrollUp
        } else {
            MouseButton::ScrollDown
        };

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }
    fn main_display_size(&self) -> (i32, i32) {
        const MAIN_SCREEN: i32 = 0;
        let mut width = 0;
        let mut height = 0;
        unsafe { xdo_get_viewport_dimensions(self.xdo, &mut width, &mut height, MAIN_SCREEN) };
        (width, height)
    }
    fn mouse_location(&self) -> (i32, i32) {
        let mut x = 0;
        let mut y = 0;
        let mut unused_screen_index = 0;
        let mut unused_window_index = CURRENT_WINDOW;
        unsafe {
            xdo_get_mouse_location2(
                self.xdo,
                &mut x,
                &mut y,
                &mut unused_screen_index,
                &mut unused_window_index,
            )
        };
        (x, y)
    }
}

fn keysequence<'a>(key: Key) -> Cow<'a, str> {
    if let Key::Layout(c) = key {
        return Cow::Owned(format!("U{:X}", c as u32));
    }
    if let Key::Raw(k) = key {
        return Cow::Owned(format!("{k}"));
    }
    // The full list of names is available at https://cgit.freedesktop.org/xorg/proto/x11proto/plain/keysymdef.h
    Cow::Borrowed(match key {
        Key::Alt => "Alt",
        Key::Backspace => "BackSpace",
        Key::Begin => "Begin",
        Key::Break => "Break",
        Key::Cancel => "Cancel",
        Key::CapsLock => "Caps_Lock",
        Key::Clear => "Clear",
        Key::Control => "Control",
        Key::Delete => "Delete",
        Key::DownArrow => "Down",
        Key::End => "End",
        Key::Escape => "Escape",
        Key::Execute => "Execute",
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
        Key::F13 => "F13",
        Key::F14 => "F14",
        Key::F15 => "F15",
        Key::F16 => "F16",
        Key::F17 => "F17",
        Key::F18 => "F18",
        Key::F19 => "F19",
        Key::F20 => "F20",
        Key::F21 => "F21",
        Key::F22 => "F22",
        Key::F23 => "F23",
        Key::F24 => "F24",
        Key::F25 => "F25",
        Key::F26 => "F26",
        Key::F27 => "F27",
        Key::F28 => "F28",
        Key::F29 => "F29",
        Key::F30 => "F30",
        Key::F31 => "F31",
        Key::F32 => "F32",
        Key::F33 => "F33",
        Key::F34 => "F34",
        Key::F35 => "F35",
        Key::Find => "Find",
        Key::Hangul => "Hangul",
        Key::Hanja => "Hangul_Hanja",
        Key::Help => "Help",
        Key::Home => "Home",
        Key::Insert => "Insert",
        Key::Kanji => "Kanji",
        Key::LControl => "Control_L",
        Key::LeftArrow => "Left",
        Key::Linefeed => "Linefeed",
        Key::LMenu => "Menu",
        Key::LShift => "Shift_L",
        Key::ModeChange => "Mode_switch",
        Key::Numlock => "Num_Lock",
        Key::Option => "Option",
        Key::PageDown => "Page_Down",
        Key::PageUp => "Page_Up",
        Key::Pause => "Pause",
        Key::Print => "Print",
        Key::RControl => "Control_R",
        Key::Redo => "Redo",
        Key::Return => "Return",
        Key::RightArrow => "Right",
        Key::RShift => "Shift_R",
        Key::ScrollLock => "Scroll_Lock ",
        Key::Select => "Select",
        Key::ScriptSwitch => "script_switch",
        Key::Shift => "Shift",
        Key::ShiftLock => "Shift_Lock",
        Key::Space => "space",
        Key::SysReq => "Sys_Req",
        Key::Tab => "Tab",
        Key::Undo => "Undo",
        Key::UpArrow => "Up",
        Key::Layout(_) | Key::Raw(_) => unreachable!(),
        Key::Command | Key::Super | Key::Windows | Key::Meta => "Super",
    })
}
impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        let string = CString::new(sequence).unwrap();
        unsafe {
            xdo_enter_text_window(
                self.xdo,
                CURRENT_WINDOW,
                string.as_ptr(),
                self.delay as useconds_t,
            );
        }
    }
    fn key_down(&mut self, key: Key) {
        let string = CString::new(&*keysequence(key)).unwrap();
        unsafe {
            xdo_send_keysequence_window_down(
                self.xdo,
                CURRENT_WINDOW,
                string.as_ptr(),
                self.delay as useconds_t,
            );
        }
    }
    fn key_up(&mut self, key: Key) {
        let string = CString::new(&*keysequence(key)).unwrap();
        unsafe {
            xdo_send_keysequence_window_up(
                self.xdo,
                CURRENT_WINDOW,
                string.as_ptr(),
                self.delay as useconds_t,
            );
        }
    }
    fn key_click(&mut self, key: Key) {
        let string = CString::new(&*keysequence(key)).unwrap();
        unsafe {
            xdo_send_keysequence_window(
                self.xdo,
                CURRENT_WINDOW,
                string.as_ptr(),
                self.delay as useconds_t,
            );
        }
    }
}
