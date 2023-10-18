use std::mem::size_of;

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, VkKeyScanW, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    KEYEVENTF_UNICODE, MAP_VIRTUAL_KEY_TYPE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY,
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
    GetCursorPos, GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN, WHEEL_DELTA,
};

use crate::{
    Axis, Coordinate, Direction, EnigoSettings, InputError, InputResult, Key,
    KeyboardControllableNext, MouseButton, MouseControllableNext, NewConError,
};

type ScanCode = u16;

/// The main struct for handling the event emitting
pub struct Enigo {
    held: Vec<Key>, // Currently held keys
    release_keys_when_dropped: bool,
}

fn send_input(input: &[INPUT]) -> InputResult<()> {
    let Ok(input_size): Result<i32, _> = size_of::<INPUT>().try_into() else {
        return Err(InputError::InvalidInput(
            "the size of the INPUT was so large, the size exceeded i32::MAX",
        ));
    };
    if unsafe { SendInput(input, input_size) } == input.len().try_into().unwrap() {
        Ok(())
    } else {
        let last_err = std::io::Error::last_os_error();
        println!("{last_err}");
        Err(InputError::Simulate(
            "not all input events were sent. they may have been blocked by UIPI",
        ))
    }
}

fn mouse_event(flags: MOUSE_EVENT_FLAGS, data: i32, dx: i32, dy: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0, /* Always set it to 0 (see https://web.archive.org/web/20231004113147/https://devblogs.microsoft.com/oldnewthing/20121101-00/?p=6193) */
                dwExtraInfo: 0,
            },
        },
    }
}

fn keybd_event(flags: KEYBD_EVENT_FLAGS, vk: VIRTUAL_KEY, scan: ScanCode) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0, /* Always set it to 0 (see https://web.archive.org/web/20231004113147/https://devblogs.microsoft.com/oldnewthing/20121101-00/?p=6193) */
                dwExtraInfo: 0,
            },
        },
    }
}

impl MouseControllableNext for Enigo {
    // Sends a button event to the X11 server via `XTest` extension
    fn send_mouse_button_event(
        &mut self,
        button: MouseButton,
        direction: Direction,
    ) -> InputResult<()> {
        let mut input = vec![];
        let button_no = match button {
            MouseButton::Back => 1,
            MouseButton::Forward => 2,
            _ => 0,
        };
        if direction == Direction::Click || direction == Direction::Press {
            let mouse_event_flag = match button {
                MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
                MouseButton::Middle => MOUSEEVENTF_MIDDLEDOWN,
                MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,
                MouseButton::Back | MouseButton::Forward => MOUSEEVENTF_XDOWN,
                MouseButton::ScrollUp => return self.mouse_scroll_event(-1, Axis::Vertical),
                MouseButton::ScrollDown => return self.mouse_scroll_event(1, Axis::Vertical),
                MouseButton::ScrollLeft => return self.mouse_scroll_event(-1, Axis::Horizontal),
                MouseButton::ScrollRight => return self.mouse_scroll_event(1, Axis::Horizontal),
            };
            input.push(mouse_event(mouse_event_flag, button_no, 0, 0));
        }
        if direction == Direction::Click || direction == Direction::Release {
            let mouse_event_flag = match button {
                MouseButton::Left => MOUSEEVENTF_LEFTUP,
                MouseButton::Middle => MOUSEEVENTF_MIDDLEUP,
                MouseButton::Right => MOUSEEVENTF_RIGHTUP,
                MouseButton::Back | MouseButton::Forward => MOUSEEVENTF_XUP,
                MouseButton::ScrollUp
                | MouseButton::ScrollDown
                | MouseButton::ScrollLeft
                | MouseButton::ScrollRight => {
                    println!("On Windows the mouse_up function has no effect when called with one of the Scroll buttons");
                    return Ok(());
                }
            };
            input.push(mouse_event(mouse_event_flag, button_no, 0, 0));
        }
        send_input(&input)
    }

    // Sends a motion notify event to the X11 server via `XTest` extension
    // TODO: Check if using x11rb::protocol::xproto::warp_pointer would be better
    fn send_motion_notify_event(
        &mut self,
        x: i32,
        y: i32,
        coordinate: Coordinate,
    ) -> InputResult<()> {
        let (x_absolute, y_absolute) = if coordinate == Coordinate::Relative {
            let (x_absolute, y_absolute) = self.mouse_loc()?;
            (x_absolute + x, y_absolute + y)
        } else {
            (x, y)
        };

        let input = mouse_event(
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
            0,
            x_absolute,
            y_absolute,
        );
        send_input(&vec![input])?;

        // This also moves the mouse but is not subject to mouse accelleration
        // Sometimes the send_input is not enough
        if unsafe { SetCursorPos(x_absolute, y_absolute) }.is_ok() {
            Ok(())
        } else {
            Err(InputError::Simulate(
                "could not set a new position of the mouse pointer",
            ))
        }
    }

    // Sends a scroll event to the X11 server via `XTest` extension
    fn mouse_scroll_event(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        let input = match axis {
            Axis::Horizontal => {
                mouse_event(MOUSEEVENTF_HWHEEL, length * (WHEEL_DELTA as i32), 0, 0)
            }
            Axis::Vertical => mouse_event(MOUSEEVENTF_WHEEL, -length * (WHEEL_DELTA as i32), 0, 0),
        };
        send_input(&vec![input])?;
        Ok(())
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if w == 0 || h == 0 {
            // Last error does not contain information about why there was an issue so it is
            // not used here
            Err(InputError::Simulate(
                "could not get the dimensions of the screen",
            ))
        } else {
            Ok((w, h))
        }
    }

    fn mouse_loc(&self) -> InputResult<(i32, i32)> {
        let mut point = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut point) }.is_ok() {
            Ok((point.x, point.y))
        } else {
            Err(InputError::Simulate(
                "could not get the current mouse location",
            ))
        }
    }
}

impl KeyboardControllableNext for Enigo {
    fn fast_text_entry(&mut self, _text: &str) -> InputResult<Option<()>> {
        Ok(None)
    }

    /// Enter the whole text string instead of entering individual keys
    /// This is much faster if you type longer text at the cost of keyboard
    /// shortcuts not getting recognized
    fn enter_text(&mut self, text: &str) -> InputResult<()> {
        if text.is_empty() {
            return Ok(()); // Nothing to simulate.
        }
        let mut buffer = [0; 2];

        let mut input = vec![];
        for c in text.chars() {
            // Handle special characters seperately
            match c {
                '\n' => return self.enter_key(Key::Return, Direction::Click),
                '\r' => { // TODO: What is the correct key to type here?
                }
                '\t' => return self.enter_key(Key::Tab, Direction::Click),
                '\0' => return Err(InputError::InvalidInput("the text contained a null byte")),
                _ => (),
            }
            // Windows uses uft-16 encoding. We need to check
            // for variable length characters. As such some
            // characters can be 32 bit long and those are
            // encoded in what is called high and low surrogates.
            // Each are 16 bit wide and need to be sent after
            // another to the SendInput function without
            // being interrupted by "keyup"
            let result = c.encode_utf16(&mut buffer);
            for &utf16_surrogate in &*result {
                input.push(keybd_event(
                    KEYEVENTF_UNICODE,
                    VIRTUAL_KEY(0),
                    utf16_surrogate,
                ));
            }
            // Only if the length was 1 do we have to send a "keyup"
            if result.len() == 1 {
                input.push(keybd_event(
                    KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    VIRTUAL_KEY(0),
                    result[0],
                ));
            }
        }
        send_input(&input)
    }

    /// Sends a key event to the X11 server via `XTest` extension
    fn enter_key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        let mut input = vec![];
        match direction {
            Direction::Press => self.held.push(key),
            Direction::Release => self.held.retain(|&k| k != key),
            Direction::Click => (),
        }

        if let Key::Layout(c) = key {
            // Handle special characters seperately
            match c {
                '\n' => return self.enter_key(Key::Return, direction),
                '\r' => { // TODO: What is the correct key to type here?
                }
                '\t' => return self.enter_key(Key::Tab, direction),
                '\0' => return Ok(()),
                _ => (),
            }
            let scancodes = self.get_scancode(c)?;
            if direction == Direction::Click || direction == Direction::Press {
                for scan in &scancodes {
                    input.push(keybd_event(KEYEVENTF_SCANCODE, VIRTUAL_KEY(0), *scan));
                }
            }
            if direction == Direction::Click || direction == Direction::Release {
                for scan in &scancodes {
                    input.push(keybd_event(
                        KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP,
                        VIRTUAL_KEY(0),
                        *scan,
                    ));
                }
            }
        } else {
            // It is okay to unwrap here because key_to_keycode only returns a None for
            // Key::Layout and we already ensured that is not the case
            let keycode = key_to_keycode(key).unwrap();
            let keyflags = get_key_flags(keycode);
            if direction == Direction::Click || direction == Direction::Press {
                input.push(keybd_event(keyflags, keycode, 0u16));
            }
            if direction == Direction::Click || direction == Direction::Release {
                input.push(keybd_event(keyflags | KEYEVENTF_KEYUP, keycode, 0u16));
            }
        };
        send_input(&input)
    }
}

impl Enigo {
    /// Create a new Enigo struct to establish the connection to simulate input
    /// with the specified settings
    ///
    /// # Errors
    /// Have a look at the documentation of `NewConError` to see under which
    /// conditions an error will be returned.
    pub fn new(settings: &EnigoSettings) -> Result<Self, NewConError> {
        let EnigoSettings {
            release_keys_when_dropped,
            ..
        } = settings;

        let held = vec![];
        Ok(Self {
            held,
            release_keys_when_dropped: *release_keys_when_dropped,
        })
    }

    #[allow(clippy::unused_self)]
    fn get_scancode(&self, c: char) -> InputResult<Vec<ScanCode>> {
        let mut buffer = [0; 2]; // A buffer of length 2 is large enough to encode any char
        let utf16_surrogates: Vec<u16> = c.encode_utf16(&mut buffer).into();
        let mut scancodes = vec![];
        for &utf16_surrogate in &utf16_surrogates {
            // Translate a character to the corresponding virtual-key code and shift state.
            // If the function succeeds, the low-order byte of the return value contains the
            // virtual-key code and the high-order byte contains the shift state, which can
            // be a combination of the following flag bits. If the function finds no key
            // that translates to the passed character code, both the low-order and
            // high-order bytes contain –1
            let keystate = match u32::try_from(unsafe { VkKeyScanW(utf16_surrogate) }) {
                // TODO: Double check if u32::MAX is correct here. If I am not
                // mistaken, -1 is stored as 11111111 meaning u32::MAX should be
                // correct here
                Ok(u32::MAX) => {
                    return Err(InputError::Mapping("Could not translate the character to the corresponding virtual-key code and shift state for the current keyboard".to_string()));
                }
                Ok(keystate) => keystate,
                Err(e) => {
                    println!("{e:?}");
                    return Err(InputError::InvalidInput(
                        "key state could not be converted to u32",
                    ));
                }
            };
            // Translate the virtual-key code to a scan code
            match unsafe { MapVirtualKeyW(keystate, MAP_VIRTUAL_KEY_TYPE(0)) }.try_into() {
                // If there is no translation, the return value is zero
                Ok(0) => {
                    return Err(InputError::Mapping("Could not translate the character to the corresponding virtual-key code and shift state for the current keyboard".to_string()));
                }
                Ok(scan_code) => {
                    scancodes.push(scan_code);
                }
                Err(e) => {
                    println!("{e:?}");
                    return Err(InputError::InvalidInput("scan code did not fit into u16"));
                }
            };
        }
        Ok(scancodes)
    }

    /// Returns a list of all currently pressed keys
    pub fn held(&mut self) -> Vec<Key> {
        self.held.clone()
    }
}

fn get_key_flags(vk: VIRTUAL_KEY) -> KEYBD_EVENT_FLAGS {
    match vk {
        // Navigation keys should be injected with the extended flag to distinguish
        // them from the Numpad navigation keys. Otherwise, input Shift+<Navigation key>
        // may not have the expected result and depends on whether NUMLOCK is enabled/disabled.
        // A list of the extended keys can be found here:
        // https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input#extended-key-flag
        // TODO: The keys "BREAK (CTRL+PAUSE) key" and "ENTER key in the numeric keypad" are missing
        VK_RMENU | VK_RCONTROL | VK_UP | VK_DOWN | VK_LEFT | VK_RIGHT | VK_INSERT | VK_DELETE
        | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_NUMLOCK | VK_SNAPSHOT | VK_DIVIDE => {
            KEYBD_EVENT_FLAGS::default() | KEYEVENTF_EXTENDEDKEY
        }
        _ => KEYBD_EVENT_FLAGS::default(),
    }
}

impl Drop for Enigo {
    // Release the held keys before the connection is dropped
    fn drop(&mut self) {
        if !self.release_keys_when_dropped {
            return;
        }
        for &k in &self.held() {
            if self.enter_key(k, Direction::Release).is_err() {
                println!("unable to release {k:?}");
            };
        }
    }
}

#[allow(clippy::too_many_lines)]
fn key_to_keycode(key: Key) -> Option<VIRTUAL_KEY> {
    let vk = match key {
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
        Key::Layout(_) => return None,
        Key::Super | Key::Command | Key::Windows | Key::Meta | Key::LWin => VK_LWIN,
    };
    Some(vk)
}
