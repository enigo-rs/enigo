use std::{mem::size_of, thread, time};

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, VkKeyScanW, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE,
    MAP_VIRTUAL_KEY_TYPE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    VIRTUAL_KEY,
};

use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK__none_, VK_0, VK_1, VK_2, VK_3, VK_4, VK_5, VK_6, VK_7, VK_8, VK_9, VK_A, VK_ABNT_C1,
    VK_ABNT_C2, VK_ACCEPT, VK_ADD, VK_APPS, VK_ATTN, VK_B, VK_BACK, VK_BROWSER_BACK,
    VK_BROWSER_FAVORITES, VK_BROWSER_FORWARD, VK_BROWSER_HOME, VK_BROWSER_REFRESH,
    VK_BROWSER_SEARCH, VK_BROWSER_STOP, VK_C, VK_CANCEL, VK_CAPITAL, VK_CLEAR, VK_CONTROL,
    VK_CONVERT, VK_CRSEL, VK_D, VK_DBE_ALPHANUMERIC, VK_DBE_CODEINPUT, VK_DBE_DBCSCHAR,
    VK_DBE_DETERMINESTRING, VK_DBE_ENTERDLGCONVERSIONMODE, VK_DBE_ENTERIMECONFIGMODE,
    VK_DBE_ENTERWORDREGISTERMODE, VK_DBE_FLUSHSTRING, VK_DBE_HIRAGANA, VK_DBE_KATAKANA,
    VK_DBE_NOCODEINPUT, VK_DBE_NOROMAN, VK_DBE_ROMAN, VK_DBE_SBCSCHAR, VK_DECIMAL, VK_DELETE,
    VK_DIVIDE, VK_DOWN, VK_E, VK_END, VK_EREOF, VK_ESCAPE, VK_EXECUTE, VK_EXSEL, VK_F, VK_F1,
    VK_F10, VK_F11, VK_F12, VK_F13, VK_F14, VK_F15, VK_F16, VK_F17, VK_F18, VK_F19, VK_F2, VK_F20,
    VK_F21, VK_F22, VK_F23, VK_F24, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8, VK_F9, VK_FINAL,
    VK_G, VK_GAMEPAD_A, VK_GAMEPAD_B, VK_GAMEPAD_DPAD_DOWN, VK_GAMEPAD_DPAD_LEFT,
    VK_GAMEPAD_DPAD_RIGHT, VK_GAMEPAD_DPAD_UP, VK_GAMEPAD_LEFT_SHOULDER,
    VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON, VK_GAMEPAD_LEFT_THUMBSTICK_DOWN,
    VK_GAMEPAD_LEFT_THUMBSTICK_LEFT, VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT,
    VK_GAMEPAD_LEFT_THUMBSTICK_UP, VK_GAMEPAD_LEFT_TRIGGER, VK_GAMEPAD_MENU,
    VK_GAMEPAD_RIGHT_SHOULDER, VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON,
    VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN, VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT,
    VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT, VK_GAMEPAD_RIGHT_THUMBSTICK_UP, VK_GAMEPAD_RIGHT_TRIGGER,
    VK_GAMEPAD_VIEW, VK_GAMEPAD_X, VK_GAMEPAD_Y, VK_H, VK_HANGEUL, VK_HANGUL, VK_HANJA, VK_HELP,
    VK_HOME, VK_I, VK_ICO_00, VK_ICO_CLEAR, VK_ICO_HELP, VK_IME_OFF, VK_IME_ON, VK_INSERT, VK_J,
    VK_JUNJA, VK_K, VK_KANA, VK_KANJI, VK_L, VK_LAUNCH_APP1, VK_LAUNCH_APP2, VK_LAUNCH_MAIL,
    VK_LAUNCH_MEDIA_SELECT, VK_LBUTTON, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_M,
    VK_MBUTTON, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK, VK_MEDIA_STOP,
    VK_MENU, VK_MODECHANGE, VK_MULTIPLY, VK_N, VK_NAVIGATION_ACCEPT, VK_NAVIGATION_CANCEL,
    VK_NAVIGATION_DOWN, VK_NAVIGATION_LEFT, VK_NAVIGATION_MENU, VK_NAVIGATION_RIGHT,
    VK_NAVIGATION_UP, VK_NAVIGATION_VIEW, VK_NEXT, VK_NONAME, VK_NONCONVERT, VK_NUMLOCK,
    VK_NUMPAD0, VK_NUMPAD1, VK_NUMPAD2, VK_NUMPAD3, VK_NUMPAD4, VK_NUMPAD5, VK_NUMPAD6, VK_NUMPAD7,
    VK_NUMPAD8, VK_NUMPAD9, VK_O, VK_OEM_1, VK_OEM_102, VK_OEM_2, VK_OEM_3, VK_OEM_4, VK_OEM_5,
    VK_OEM_6, VK_OEM_7, VK_OEM_8, VK_OEM_ATTN, VK_OEM_AUTO, VK_OEM_AX, VK_OEM_BACKTAB,
    VK_OEM_CLEAR, VK_OEM_COMMA, VK_OEM_COPY, VK_OEM_CUSEL, VK_OEM_ENLW, VK_OEM_FINISH,
    VK_OEM_FJ_JISHO, VK_OEM_FJ_LOYA, VK_OEM_FJ_MASSHOU, VK_OEM_FJ_ROYA, VK_OEM_FJ_TOUROKU,
    VK_OEM_JUMP, VK_OEM_MINUS, VK_OEM_NEC_EQUAL, VK_OEM_PA1, VK_OEM_PA2, VK_OEM_PA3, VK_OEM_PERIOD,
    VK_OEM_PLUS, VK_OEM_RESET, VK_OEM_WSCTRL, VK_P, VK_PA1, VK_PACKET, VK_PAUSE, VK_PLAY, VK_PRINT,
    VK_PRIOR, VK_PROCESSKEY, VK_Q, VK_R, VK_RBUTTON, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_S, VK_SCROLL, VK_SELECT, VK_SEPARATOR, VK_SHIFT, VK_SLEEP, VK_SNAPSHOT,
    VK_SPACE, VK_SUBTRACT, VK_T, VK_TAB, VK_U, VK_UP, VK_V, VK_VOLUME_DOWN, VK_VOLUME_MUTE,
    VK_VOLUME_UP, VK_W, VK_X, VK_XBUTTON1, VK_XBUTTON2, VK_Y, VK_Z, VK_ZOOM,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN,
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
        let result = unsafe { SetCursorPos(x, y) };
        assert!(result.as_bool(), "Unable to move mouse");
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
                MouseButton::Back | MouseButton::Forward => MOUSEEVENTF_XDOWN,
                MouseButton::ScrollUp => return self.mouse_scroll_x(-1),
                MouseButton::ScrollDown => return self.mouse_scroll_x(1),
                MouseButton::ScrollLeft => return self.mouse_scroll_y(-1),
                MouseButton::ScrollRight => return self.mouse_scroll_y(1),
            },
            match button {
                MouseButton::Back => 1,
                MouseButton::Forward => 2,
                _ => 0,
            },
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
                MouseButton::Back | MouseButton::Forward => MOUSEEVENTF_XUP,
                MouseButton::ScrollUp
                | MouseButton::ScrollDown
                | MouseButton::ScrollLeft
                | MouseButton::ScrollRight => {
                    println!("On Windows the mouse_up function has no effect when called with one of the Scroll buttons");
                    return;
                }
            },
            match button {
                MouseButton::Back => 1,
                MouseButton::Forward => 2,
                _ => 0,
            },
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

#[allow(clippy::too_many_lines)]
fn key_to_keycode(key: Key) -> VIRTUAL_KEY {
    // I mean duh, we still need to support deprecated keys until they're removed
    match key {
        Key::Num0 => VK_0,
        Key::Num1 => VK_1,
        Key::Num2 => VK_2,
        Key::Num3 => VK_3,
        Key::Num4 => VK_4,
        Key::Num5 => VK_5,
        Key::Num6 => VK_6,
        Key::Num7 => VK_7,
        Key::Num8 => VK_8,
        Key::Num9 => VK_9,
        Key::A => VK_A,
        Key::B => VK_B,
        Key::C => VK_C,
        Key::D => VK_D,
        Key::E => VK_E,
        Key::F => VK_F,
        Key::G => VK_G,
        Key::H => VK_H,
        Key::I => VK_I,
        Key::J => VK_J,
        Key::K => VK_K,
        Key::L => VK_L,
        Key::M => VK_M,
        Key::N => VK_N,
        Key::O => VK_O,
        Key::P => VK_P,
        Key::Q => VK_Q,
        Key::R => VK_R,
        Key::S => VK_S,
        Key::T => VK_T,
        Key::U => VK_U,
        Key::V => VK_V,
        Key::W => VK_W,
        Key::X => VK_X,
        Key::Y => VK_Y,
        Key::Z => VK_Z,
        Key::AbntC1 => VK_ABNT_C1,
        Key::AbntC2 => VK_ABNT_C2,
        Key::Accept => VK_ACCEPT,
        Key::Add => VK_ADD,
        Key::Alt | Key::Option => VK_MENU,
        Key::Apps => VK_APPS,
        Key::Attn => VK_ATTN,
        Key::Backspace => VK_BACK,
        Key::BrowserBack => VK_BROWSER_BACK,
        Key::BrowserFavorites => VK_BROWSER_FAVORITES,
        Key::BrowserForward => VK_BROWSER_FORWARD,
        Key::BrowserHome => VK_BROWSER_HOME,
        Key::BrowserRefresh => VK_BROWSER_REFRESH,
        Key::BrowserSearch => VK_BROWSER_SEARCH,
        Key::BrowserStop => VK_BROWSER_STOP,
        Key::Cancel => VK_CANCEL,
        Key::CapsLock => VK_CAPITAL,
        Key::Clear => VK_CLEAR,
        Key::Control => VK_CONTROL,
        Key::Convert => VK_CONVERT,
        Key::Crsel => VK_CRSEL,
        Key::DBEAlphanumeric => VK_DBE_ALPHANUMERIC,
        Key::DBECodeinput => VK_DBE_CODEINPUT,
        Key::DBEDetermineString => VK_DBE_DETERMINESTRING,
        Key::DBEEnterDLGConversionMode => VK_DBE_ENTERDLGCONVERSIONMODE,
        Key::DBEEnterIMEConfigMode => VK_DBE_ENTERIMECONFIGMODE,
        Key::DBEEnterWordRegisterMode => VK_DBE_ENTERWORDREGISTERMODE,
        Key::DBEFlushString => VK_DBE_FLUSHSTRING,
        Key::DBEHiragana => VK_DBE_HIRAGANA,
        Key::DBEKatakana => VK_DBE_KATAKANA,
        Key::DBENoCodepoint => VK_DBE_NOCODEINPUT,
        Key::DBENoRoman => VK_DBE_NOROMAN,
        Key::DBERoman => VK_DBE_ROMAN,
        Key::DBESBCSChar => VK_DBE_SBCSCHAR,
        Key::DBESChar => VK_DBE_DBCSCHAR,
        Key::Decimal => VK_DECIMAL,
        Key::Delete => VK_DELETE,
        Key::Divide => VK_DIVIDE,
        Key::DownArrow => VK_DOWN,
        Key::End => VK_END,
        Key::Ereof => VK_EREOF,
        Key::Escape => VK_ESCAPE,
        Key::Execute => VK_EXECUTE,
        Key::Exsel => VK_EXSEL,
        Key::F1 => VK_F1,
        Key::F2 => VK_F2,
        Key::F3 => VK_F3,
        Key::F4 => VK_F4,
        Key::F5 => VK_F5,
        Key::F6 => VK_F6,
        Key::F7 => VK_F7,
        Key::F8 => VK_F8,
        Key::F9 => VK_F9,
        Key::F10 => VK_F10,
        Key::F11 => VK_F11,
        Key::F12 => VK_F12,
        Key::F13 => VK_F13,
        Key::F14 => VK_F14,
        Key::F15 => VK_F15,
        Key::F16 => VK_F16,
        Key::F17 => VK_F17,
        Key::F18 => VK_F18,
        Key::F19 => VK_F19,
        Key::F20 => VK_F20,
        Key::F21 => VK_F21,
        Key::F22 => VK_F22,
        Key::F23 => VK_F23,
        Key::F24 => VK_F24,
        Key::Final => VK_FINAL,
        Key::GamepadA => VK_GAMEPAD_A,
        Key::GamepadB => VK_GAMEPAD_B,
        Key::GamepadDPadDown => VK_GAMEPAD_DPAD_DOWN,
        Key::GamepadDPadLeft => VK_GAMEPAD_DPAD_LEFT,
        Key::GamepadDPadRight => VK_GAMEPAD_DPAD_RIGHT,
        Key::GamepadDPadUp => VK_GAMEPAD_DPAD_UP,
        Key::GamepadLeftShoulder => VK_GAMEPAD_LEFT_SHOULDER,
        Key::GamepadLeftThumbstickButton => VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON,
        Key::GamepadLeftThumbstickDown => VK_GAMEPAD_LEFT_THUMBSTICK_DOWN,
        Key::GamepadLeftThumbstickLeft => VK_GAMEPAD_LEFT_THUMBSTICK_LEFT,
        Key::GamepadLeftThumbstickRight => VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT,
        Key::GamepadLeftThumbstickUp => VK_GAMEPAD_LEFT_THUMBSTICK_UP,
        Key::GamepadLeftTrigger => VK_GAMEPAD_LEFT_TRIGGER,
        Key::GamepadMenu => VK_GAMEPAD_MENU,
        Key::GamepadRightShoulder => VK_GAMEPAD_RIGHT_SHOULDER,
        Key::GamepadRightThumbstickButton => VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON,
        Key::GamepadRightThumbstickDown => VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN,
        Key::GamepadRightThumbstickLeft => VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT,
        Key::GamepadRightThumbstickRight => VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT,
        Key::GamepadRightThumbstickUp => VK_GAMEPAD_RIGHT_THUMBSTICK_UP,
        Key::GamepadRightTrigger => VK_GAMEPAD_RIGHT_TRIGGER,
        Key::GamepadView => VK_GAMEPAD_VIEW,
        Key::GamepadX => VK_GAMEPAD_X,
        Key::GamepadY => VK_GAMEPAD_Y,
        Key::Hangeul => VK_HANGEUL,
        Key::Hangul => VK_HANGUL,
        Key::Hanja => VK_HANJA,
        Key::Help => VK_HELP,
        Key::Home => VK_HOME,
        Key::Ico00 => VK_ICO_00,
        Key::IcoClear => VK_ICO_CLEAR,
        Key::IcoHelp => VK_ICO_HELP,
        Key::IMEOff => VK_IME_OFF,
        Key::IMEOn => VK_IME_ON,
        Key::Insert => VK_INSERT,
        Key::Junja => VK_JUNJA,
        Key::Kana => VK_KANA,
        Key::Kanji => VK_KANJI,
        Key::LaunchApp1 => VK_LAUNCH_APP1,
        Key::LaunchApp2 => VK_LAUNCH_APP2,
        Key::LaunchMail => VK_LAUNCH_MAIL,
        Key::LaunchMediaSelect => VK_LAUNCH_MEDIA_SELECT,
        Key::LButton => VK_LBUTTON,
        Key::LControl => VK_LCONTROL,
        Key::LeftArrow => VK_LEFT,
        Key::LMenu => VK_LMENU,
        Key::LShift => VK_LSHIFT,
        Key::MButton => VK_MBUTTON,
        Key::MediaNextTrack => VK_MEDIA_NEXT_TRACK,
        Key::MediaPlayPause => VK_MEDIA_PLAY_PAUSE,
        Key::MediaPrevTrack => VK_MEDIA_PREV_TRACK,
        Key::MediaStop => VK_MEDIA_STOP,
        Key::ModeChange => VK_MODECHANGE,
        Key::Multiply => VK_MULTIPLY,
        Key::NavigationAccept => VK_NAVIGATION_ACCEPT,
        Key::NavigationCancel => VK_NAVIGATION_CANCEL,
        Key::NavigationDown => VK_NAVIGATION_DOWN,
        Key::NavigationLeft => VK_NAVIGATION_LEFT,
        Key::NavigationMenu => VK_NAVIGATION_MENU,
        Key::NavigationRight => VK_NAVIGATION_RIGHT,
        Key::NavigationUp => VK_NAVIGATION_UP,
        Key::NavigationView => VK_NAVIGATION_VIEW,
        Key::NoName => VK_NONAME,
        Key::NonConvert => VK_NONCONVERT,
        Key::None => VK__none_,
        Key::Numlock => VK_NUMLOCK,
        Key::Numpad0 => VK_NUMPAD0,
        Key::Numpad1 => VK_NUMPAD1,
        Key::Numpad2 => VK_NUMPAD2,
        Key::Numpad3 => VK_NUMPAD3,
        Key::Numpad4 => VK_NUMPAD4,
        Key::Numpad5 => VK_NUMPAD5,
        Key::Numpad6 => VK_NUMPAD6,
        Key::Numpad7 => VK_NUMPAD7,
        Key::Numpad8 => VK_NUMPAD8,
        Key::Numpad9 => VK_NUMPAD9,
        Key::OEM1 => VK_OEM_1,
        Key::OEM102 => VK_OEM_102,
        Key::OEM2 => VK_OEM_2,
        Key::OEM3 => VK_OEM_3,
        Key::OEM4 => VK_OEM_4,
        Key::OEM5 => VK_OEM_5,
        Key::OEM6 => VK_OEM_6,
        Key::OEM7 => VK_OEM_7,
        Key::OEM8 => VK_OEM_8,
        Key::OEMAttn => VK_OEM_ATTN,
        Key::OEMAuto => VK_OEM_AUTO,
        Key::OEMAx => VK_OEM_AX,
        Key::OEMBacktab => VK_OEM_BACKTAB,
        Key::OEMClear => VK_OEM_CLEAR,
        Key::OEMComma => VK_OEM_COMMA,
        Key::OEMCopy => VK_OEM_COPY,
        Key::OEMCusel => VK_OEM_CUSEL,
        Key::OEMEnlw => VK_OEM_ENLW,
        Key::OEMFinish => VK_OEM_FINISH,
        Key::OEMFJJisho => VK_OEM_FJ_JISHO,
        Key::OEMFJLoya => VK_OEM_FJ_LOYA,
        Key::OEMFJMasshou => VK_OEM_FJ_MASSHOU,
        Key::OEMFJRoya => VK_OEM_FJ_ROYA,
        Key::OEMFJTouroku => VK_OEM_FJ_TOUROKU,
        Key::OEMJump => VK_OEM_JUMP,
        Key::OEMMinus => VK_OEM_MINUS,
        Key::OEMNECEqual => VK_OEM_NEC_EQUAL,
        Key::OEMPA1 => VK_OEM_PA1,
        Key::OEMPA2 => VK_OEM_PA2,
        Key::OEMPA3 => VK_OEM_PA3,
        Key::OEMPeriod => VK_OEM_PERIOD,
        Key::OEMPlus => VK_OEM_PLUS,
        Key::OEMReset => VK_OEM_RESET,
        Key::OEMWsctrl => VK_OEM_WSCTRL,
        Key::PA1 => VK_PA1,
        Key::Packet => VK_PACKET,
        Key::PageDown => VK_NEXT,
        Key::PageUp => VK_PRIOR,
        Key::Pause => VK_PAUSE,
        Key::Play => VK_PLAY,
        Key::Print => VK_PRINT,
        Key::Processkey => VK_PROCESSKEY,
        Key::RButton => VK_RBUTTON,
        Key::RControl => VK_RCONTROL,
        Key::Return => VK_RETURN,
        Key::RightArrow => VK_RIGHT,
        Key::RMenu => VK_RMENU,
        Key::RShift => VK_RSHIFT,
        Key::RWin => VK_RWIN,
        Key::Scroll => VK_SCROLL,
        Key::Select => VK_SELECT,
        Key::Separator => VK_SEPARATOR,
        Key::Shift => VK_SHIFT,
        Key::Sleep => VK_SLEEP,
        Key::Snapshot => VK_SNAPSHOT,
        Key::Space => VK_SPACE,
        Key::Subtract => VK_SUBTRACT,
        Key::Tab => VK_TAB,
        Key::UpArrow => VK_UP,
        Key::VolumeDown => VK_VOLUME_DOWN,
        Key::VolumeMute => VK_VOLUME_MUTE,
        Key::VolumeUp => VK_VOLUME_UP,
        Key::XButton1 => VK_XBUTTON1,
        Key::XButton2 => VK_XBUTTON2,
        Key::Zoom => VK_ZOOM,
        Key::Raw(raw_keycode) => VIRTUAL_KEY(raw_keycode),
        Key::Layout(_) => panic!(), // TODO: Don't panic here
        Key::Super | Key::Command | Key::Windows | Key::Meta | Key::LWin => VK_LWIN,
    }
}
