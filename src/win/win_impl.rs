use std::{mem::size_of, thread, time};

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, VkKeyScanW, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE,
    MAP_VIRTUAL_KEY_TYPE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL,
    MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

use crate::win::keycodes::{
    EVK_BACK, EVK_CAPITAL, EVK_DELETE, EVK_DOWN, EVK_END, EVK_ESCAPE, EVK_F1, EVK_F10, EVK_F11,
    EVK_F12, EVK_F13, EVK_F14, EVK_F15, EVK_F16, EVK_F17, EVK_F18, EVK_F19, EVK_F2, EVK_F20,
    EVK_F3, EVK_F4, EVK_F5, EVK_F6, EVK_F7, EVK_F8, EVK_F9, EVK_HOME, EVK_LCONTROL, EVK_LEFT,
    EVK_LWIN, EVK_MENU, EVK_NEXT, EVK_PRIOR, EVK_RETURN, EVK_RIGHT, EVK_SHIFT, EVK_SPACE, EVK_TAB,
    EVK_UP,
};
use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};

type KeyCode = u16;
type ScanCode = u16;

/// The main struct for handling the event emitting
#[derive(Default)]
pub struct Enigo;

fn mouse_event(flags: MOUSE_EVENT_FLAGS, data: i32, dx: i32, dy: i32) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(
            &[input as INPUT],
            size_of::<INPUT>()
                .try_into()
                .expect("Could not convert the size of INPUT to i32"),
        )
    };
}

fn keybd_event(flags: KEYBD_EVENT_FLAGS, vk: VIRTUAL_KEY, scan: ScanCode) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(
            &[input as INPUT],
            size_of::<INPUT>()
                .try_into()
                .expect("Could not convert the size of INPUT to i32"),
        )
    };
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        mouse_event(
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            0,
            (x - unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) }) * 65535
                / unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
            (y - unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) }) * 65535
                / unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
        );
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        let (current_x, current_y) = self.mouse_location();
        self.mouse_move_to(current_x + x, current_y + y);
    }

    fn mouse_down(&mut self, button: MouseButton) {
        mouse_event(
            match button {
                MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
                MouseButton::Middle => MOUSEEVENTF_MIDDLEDOWN,
                MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,
                MouseButton::ScrollUp => return self.mouse_scroll_x(-1),
                MouseButton::ScrollDown => return self.mouse_scroll_x(1),
                MouseButton::ScrollLeft => return self.mouse_scroll_y(-1),
                MouseButton::ScrollRight => return self.mouse_scroll_y(1),
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
                MouseButton::ScrollUp
                | MouseButton::ScrollDown
                | MouseButton::ScrollLeft
                | MouseButton::ScrollRight => {
                    println!("On Windows the mouse_up function has no effect when called with one of the Scroll buttons");
                    return;
                }
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
        mouse_event(MOUSEEVENTF_HWHEEL, length * 120, 0, 0);
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        mouse_event(MOUSEEVENTF_WHEEL, length * -120, 0, 0);
    }

    fn main_display_size(&self) -> (i32, i32) {
        let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        (w, h)
    }

    fn mouse_location(&self) -> (i32, i32) {
        let mut point = POINT { x: 0, y: 0 };
        let result = unsafe { GetCursorPos(&mut point) };
        if result.as_bool() {
            (point.x, point.y)
        } else {
            (0, 0)
        }
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
                    self.unicode_key_down(*utf16_surrogate);
                }
                // do i need to produce a keyup?
                // self.unicode_key_up(0);
            }
        }
    }

    fn key_click(&mut self, key: Key) {
        if let Key::Layout(c) = key {
            let scancodes = self.get_scancode(c);
            for scan in &scancodes {
                keybd_event(KEYEVENTF_SCANCODE, VIRTUAL_KEY(0), *scan);
            }
            thread::sleep(time::Duration::from_millis(20));
            for scan in &scancodes {
                keybd_event(KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP, VIRTUAL_KEY(0), *scan);
            }
        } else {
            keybd_event(KEYBD_EVENT_FLAGS::default(), key_to_keycode(key), 0u16);
            thread::sleep(time::Duration::from_millis(20));
            keybd_event(
                KEYBD_EVENT_FLAGS::default() | KEYEVENTF_KEYUP,
                key_to_keycode(key),
                0u16,
            );
        };
    }

    fn key_down(&mut self, key: Key) {
        if let Key::Layout(c) = key {
            let scancodes = self.get_scancode(c);
            for scan in &scancodes {
                keybd_event(KEYEVENTF_SCANCODE, VIRTUAL_KEY(0), *scan);
            }
        } else {
            keybd_event(KEYBD_EVENT_FLAGS::default(), key_to_keycode(key), 0u16);
        };
    }

    fn key_up(&mut self, key: Key) {
        if let Key::Layout(c) = key {
            let scancodes = self.get_scancode(c);

            for scan in &scancodes {
                keybd_event(KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP, VIRTUAL_KEY(0), *scan);
            }
        } else {
            keybd_event(
                KEYBD_EVENT_FLAGS::default() | KEYEVENTF_KEYUP,
                key_to_keycode(key),
                0u16,
            );
        };
    }
}

impl Enigo {
    fn unicode_key_click(&self, unicode_char: u16) {
        self.unicode_key_down(unicode_char);
        thread::sleep(time::Duration::from_millis(20));
        self.unicode_key_up(unicode_char);
    }

    #[allow(clippy::unused_self)]
    fn unicode_key_down(&self, unicode_char: u16) {
        keybd_event(KEYEVENTF_UNICODE, VIRTUAL_KEY(0), unicode_char);
    }

    #[allow(clippy::unused_self)]
    fn unicode_key_up(&self, unicode_char: u16) {
        keybd_event(
            KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
            VIRTUAL_KEY(0),
            unicode_char,
        );
    }

    #[allow(clippy::unused_self)]
    fn get_scancode(&self, c: char) -> Vec<ScanCode> {
        let mut buffer = [0; 2]; // A buffer of length 2 is large enough to encode any char
        let utf16: Vec<u16> = c.encode_utf16(&mut buffer).into();
        let keycode_and_shiftstate: Vec<ScanCode> = utf16
            .iter()
            .map(|&x| unsafe { VkKeyScanW(x) as KeyCode })
            .map(|x| unsafe { MapVirtualKeyW(x as u32, MAP_VIRTUAL_KEY_TYPE(0)) as ScanCode })
            .collect();
        //assert!(utf16.len() == 2, "This char is not allowed");
        // TODO: Allow
        // entering utf16 chars that have a length of two (such as \U0001d54a)
        // NOTE VkKeyScanW uses the current keyboard layout
        // to specify a layout use VkKeyScanExW and GetKeyboardLayout
        // or load one with LoadKeyboardLayoutW
        keycode_and_shiftstate
    }
}

fn key_to_keycode(key: Key) -> VIRTUAL_KEY {
    // do not use the codes from crate winapi they're
    // wrongly typed with i32 instead of i16 use the
    // ones provided by win/keycodes.rs that are prefixed
    // with an 'E' infront of the original name

    // I mean duh, we still need to support deprecated keys until they're removed
    match key {
        Key::Alt | Key::Option => VIRTUAL_KEY(EVK_MENU),
        Key::Backspace => VIRTUAL_KEY(EVK_BACK),
        Key::CapsLock => VIRTUAL_KEY(EVK_CAPITAL),
        Key::Control => VIRTUAL_KEY(EVK_LCONTROL),
        Key::Delete => VIRTUAL_KEY(EVK_DELETE),
        Key::DownArrow => VIRTUAL_KEY(EVK_DOWN),
        Key::End => VIRTUAL_KEY(EVK_END),
        Key::Escape => VIRTUAL_KEY(EVK_ESCAPE),
        Key::F1 => VIRTUAL_KEY(EVK_F1),
        Key::F2 => VIRTUAL_KEY(EVK_F2),
        Key::F3 => VIRTUAL_KEY(EVK_F3),
        Key::F4 => VIRTUAL_KEY(EVK_F4),
        Key::F5 => VIRTUAL_KEY(EVK_F5),
        Key::F6 => VIRTUAL_KEY(EVK_F6),
        Key::F7 => VIRTUAL_KEY(EVK_F7),
        Key::F8 => VIRTUAL_KEY(EVK_F8),
        Key::F9 => VIRTUAL_KEY(EVK_F9),
        Key::F10 => VIRTUAL_KEY(EVK_F10),
        Key::F11 => VIRTUAL_KEY(EVK_F11),
        Key::F12 => VIRTUAL_KEY(EVK_F12),
        Key::F13 => VIRTUAL_KEY(EVK_F13),
        Key::F14 => VIRTUAL_KEY(EVK_F14),
        Key::F15 => VIRTUAL_KEY(EVK_F15),
        Key::F16 => VIRTUAL_KEY(EVK_F16),
        Key::F17 => VIRTUAL_KEY(EVK_F17),
        Key::F18 => VIRTUAL_KEY(EVK_F18),
        Key::F19 => VIRTUAL_KEY(EVK_F19),
        Key::F20 => VIRTUAL_KEY(EVK_F20),
        Key::Home => VIRTUAL_KEY(EVK_HOME),
        Key::LeftArrow => VIRTUAL_KEY(EVK_LEFT),
        Key::PageDown => VIRTUAL_KEY(EVK_NEXT),
        Key::PageUp => VIRTUAL_KEY(EVK_PRIOR),
        Key::Return => VIRTUAL_KEY(EVK_RETURN),
        Key::RightArrow => VIRTUAL_KEY(EVK_RIGHT),
        Key::Shift => VIRTUAL_KEY(EVK_SHIFT),
        Key::Space => VIRTUAL_KEY(EVK_SPACE),
        Key::Tab => VIRTUAL_KEY(EVK_TAB),
        Key::UpArrow => VIRTUAL_KEY(EVK_UP),
        Key::Raw(raw_keycode) => VIRTUAL_KEY(raw_keycode),
        Key::Layout(_) => panic!(), // TODO: Don't panic here
        Key::Super | Key::Command | Key::Windows | Key::Meta => VIRTUAL_KEY(EVK_LWIN),
    }
}
