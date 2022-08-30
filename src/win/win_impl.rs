use winapi;

use self::winapi::shared::windef::POINT;
use self::winapi::ctypes::c_int;
use self::winapi::um::winuser::*;

use crate::win::keycodes::*;
use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};
use std::mem::*;

/// The main struct for handling the event emitting
#[derive(Default)]
pub struct Enigo;

fn mouse_event(flags: u32, data: u32, dx: i32, dy: i32) {
    let mut input = INPUT {
        type_: INPUT_MOUSE,
        u: unsafe {
            transmute(MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            })
        },
    };
    unsafe { SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int) };
}

fn keybd_event(flags: u32, vk: u16, scan: u16) {
    let mut input = INPUT {
        type_: INPUT_KEYBOARD,
        u: unsafe {
            let mut input_u: INPUT_u = std::mem::zeroed();
            *input_u.ki_mut() = KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            };
            input_u
        },
    };
    unsafe { SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int) };
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        mouse_event(
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            0,
            (x - unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) }) * 65535 / unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
            (y - unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) }) * 65535 / unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
        );
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        mouse_event(MOUSEEVENTF_MOVE, 0, x, y);
    }

    fn mouse_down(&mut self, button: MouseButton) {
        mouse_event(
            match button {
                MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
                MouseButton::Middle => MOUSEEVENTF_MIDDLEDOWN,
                MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,
                _ => unimplemented!(),
            },
            0,
            0,
            0,
        );
    }

    fn mouse_up(&mut self, button: MouseButton) {
        mouse_event(
            match button {
                MouseButton::Left => MOUSEEVENTF_LEFTUP,
                MouseButton::Middle => MOUSEEVENTF_MIDDLEUP,
                MouseButton::Right => MOUSEEVENTF_RIGHTUP,
                _ => unimplemented!(),
            },
            0,
            0,
            0,
        );
    }

    fn mouse_click(&mut self, button: MouseButton) {
        self.mouse_down(button);
        self.mouse_up(button);
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        mouse_event(MOUSEEVENTF_HWHEEL, unsafe { transmute(length * 120) }, 0, 0);
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        mouse_event(MOUSEEVENTF_WHEEL, unsafe { transmute(length * 120) }, 0, 0);
    }
}

impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        let mut buffer = [0; 2];

        for c in sequence.chars() {
            // Windows uses uft-16 encoding. We need to check
            // for variable length characters. As such some
            // characters can be 32 bit long and those are
            // encoded in such called hight and low surrogates
            // each 16 bit wide that needs to be send after
            // another to the SendInput function without
            // being interrupted by "keyup"
            let result = c.encode_utf16(&mut buffer);
            if result.len() == 1 {
                self.unicode_key_click(result[0]);
            } else {
                for utf16_surrogate in result {
                    self.unicode_key_down(utf16_surrogate.clone());
                }
                // do i need to produce a keyup?
                // self.unicode_key_up(0);
            }
        }
    }

    fn key_click(&mut self, key: Key) {
        let scancode = self.key_to_scancode(key);
        use std::{thread, time};
        keybd_event(KEYEVENTF_SCANCODE, 0, scancode);
        thread::sleep(time::Duration::from_millis(20));
        keybd_event(KEYEVENTF_KEYUP | KEYEVENTF_SCANCODE, 0, scancode);
    }

    fn key_down(&mut self, key: Key) {
        keybd_event(KEYEVENTF_SCANCODE, 0, self.key_to_scancode(key));
    }

    fn key_up(&mut self, key: Key) {
        keybd_event(
            KEYEVENTF_KEYUP | KEYEVENTF_SCANCODE,
            0,
            self.key_to_scancode(key),
        );
    }
}

impl Enigo {
    /// Gets the (width, height) of the main display in screen coordinates (pixels).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut size = Enigo::main_display_size();
    /// ```
    pub fn main_display_size() -> (usize, usize) {
      let w = unsafe { GetSystemMetrics(SM_CXSCREEN) as usize };
      let h = unsafe { GetSystemMetrics(SM_CYSCREEN) as usize };
      (w, h)
    }

    /// Gets the location of mouse in screen coordinates (pixels).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut location = Enigo::mouse_location();
    /// ```
    pub fn mouse_location() -> (i32, i32) {
        let mut point = POINT { x: 0, y: 0 };
        let result = unsafe { GetCursorPos(&mut point) };
        if result != 0 {
          (point.x, point.y)
        } else {
          (0, 0)
        }
    }

    fn unicode_key_click(&self, unicode_char: u16) {
        use std::{thread, time};
        self.unicode_key_down(unicode_char);
        thread::sleep(time::Duration::from_millis(20));
        self.unicode_key_up(unicode_char);
    }

    fn unicode_key_down(&self, unicode_char: u16) {
        keybd_event(KEYEVENTF_UNICODE, 0, unicode_char);
    }

    fn unicode_key_up(&self, unicode_char: u16) {
        keybd_event(KEYEVENTF_UNICODE | KEYEVENTF_KEYUP, 0, unicode_char);
    }

    fn key_to_keycode(&self, key: Key) -> u16 {
        // do not use the codes from crate winapi they're
        // wrongly typed with i32 instead of i16 use the
        // ones provided by win/keycodes.rs that are prefixed
        // with an 'E' infront of the original name
        #[allow(deprecated)]
        // I mean duh, we still need to support deprecated keys until they're removed
        match key {
            Key::Alt => EVK_MENU,
            Key::Backspace => EVK_BACK,
            Key::CapsLock => EVK_CAPITAL,
            Key::Control => EVK_LCONTROL,
            Key::Delete => EVK_DELETE,
            Key::DownArrow => EVK_DOWN,
            Key::End => EVK_END,
            Key::Escape => EVK_ESCAPE,
            Key::F1 => EVK_F1,
            Key::F10 => EVK_F10,
            Key::F11 => EVK_F11,
            Key::F12 => EVK_F12,
            Key::F2 => EVK_F2,
            Key::F3 => EVK_F3,
            Key::F4 => EVK_F4,
            Key::F5 => EVK_F5,
            Key::F6 => EVK_F6,
            Key::F7 => EVK_F7,
            Key::F8 => EVK_F8,
            Key::F9 => EVK_F9,
            Key::Home => EVK_HOME,
            Key::LeftArrow => EVK_LEFT,
            Key::Option => EVK_MENU,
            Key::PageDown => EVK_NEXT,
            Key::PageUp => EVK_PRIOR,
            Key::Return => EVK_RETURN,
            Key::RightArrow => EVK_RIGHT,
            Key::Shift => EVK_SHIFT,
            Key::Space => EVK_SPACE,
            Key::Tab => EVK_TAB,
            Key::UpArrow => EVK_UP,

            Key::Raw(raw_keycode) => raw_keycode,
            Key::Layout(c) => self.get_layoutdependent_keycode(c.to_string()),
            //_ => 0,
            Key::Super | Key::Command | Key::Windows | Key::Meta => EVK_LWIN,
        }
    }

    fn key_to_scancode(&self, key: Key) -> u16 {
        let keycode = self.key_to_keycode(key);
        unsafe { MapVirtualKeyW(keycode as u32, 0) as u16 }
    }

    fn get_layoutdependent_keycode(&self, string: String) -> u16 {
        let mut buffer = [0; 2];
        // get the first char from the string ignore the rest
        // ensure its not a multybyte char
        let utf16 = string
            .chars()
            .nth(0)
            .expect("no valid input") //TODO(dustin): no panic here make an error
            .encode_utf16(&mut buffer);
        if utf16.len() != 1 {
            // TODO(dustin) don't panic here use an apropriate errors
            panic!("this char is not allowd");
        }
        // NOTE VkKeyScanW uses the current keyboard layout
        // to specify a layout use VkKeyScanExW and GetKeyboardLayout
        // or load one with LoadKeyboardLayoutW
        let keycode_and_shiftstate = unsafe { VkKeyScanW(utf16[0]) };
        // 0x41 as u16 //key that has the letter 'a' on it on english like keylayout
        keycode_and_shiftstate as u16
    }
}
