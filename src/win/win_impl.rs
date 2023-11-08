use std::mem::size_of;

use log::{debug, error, info};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, VkKeyScanW, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    KEYEVENTF_UNICODE, MAP_VIRTUAL_KEY_TYPE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY,
};

use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN, WHEEL_DELTA,
};

use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse,
    NewConError, Settings,
};

type ScanCode = u16;
pub const EXT: u16 = 0xFF00;

/// The main struct for handling the event emitting
pub struct Enigo {
    held: (Vec<Key>, Vec<ScanCode>), // Currently held keys
    release_keys_when_dropped: bool,
}

fn send_input(input: &[INPUT]) -> InputResult<()> {
    let Ok(input_size): Result<i32, _> = size_of::<INPUT>().try_into() else {
        return Err(InputError::InvalidInput(
            "the size of the INPUT was so large, the size exceeded i32::MAX",
        ));
    };
    let Ok(input_len) = input.len().try_into() else {
        return Err(InputError::InvalidInput(
            "the number of INPUT was so large, the length of the Vec exceeded i32::MAX",
        ));
    };
    if unsafe { SendInput(input, input_size) } == input_len {
        Ok(())
    } else {
        let last_err = std::io::Error::last_os_error();
        error!("{last_err}");
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

impl Mouse for Enigo {
    // Sends a button event to the X11 server via `XTest` extension
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mbutton(button: {button:?}, direction: {direction:?})\x1b[0m");
        let mut input = vec![];
        let button_no = match button {
            Button::Back => 1,
            Button::Forward => 2,
            _ => 0,
        };
        if direction == Direction::Click || direction == Direction::Press {
            let mouse_event_flag = match button {
                Button::Left => MOUSEEVENTF_LEFTDOWN,
                Button::Middle => MOUSEEVENTF_MIDDLEDOWN,
                Button::Right => MOUSEEVENTF_RIGHTDOWN,
                Button::Back | Button::Forward => MOUSEEVENTF_XDOWN,
                Button::ScrollUp => return self.scroll(-1, Axis::Vertical),
                Button::ScrollDown => return self.scroll(1, Axis::Vertical),
                Button::ScrollLeft => return self.scroll(-1, Axis::Horizontal),
                Button::ScrollRight => return self.scroll(1, Axis::Horizontal),
            };
            input.push(mouse_event(mouse_event_flag, button_no, 0, 0));
        }
        if direction == Direction::Click || direction == Direction::Release {
            let mouse_event_flag = match button {
                Button::Left => MOUSEEVENTF_LEFTUP,
                Button::Middle => MOUSEEVENTF_MIDDLEUP,
                Button::Right => MOUSEEVENTF_RIGHTUP,
                Button::Back | Button::Forward => MOUSEEVENTF_XUP,
                Button::ScrollUp
                | Button::ScrollDown
                | Button::ScrollLeft
                | Button::ScrollRight => {
                    info!("On Windows the mouse_up function has no effect when called with one of the Scroll buttons");
                    return Ok(());
                }
            };
            input.push(mouse_event(mouse_event_flag, button_no, 0, 0));
        }
        send_input(&input)
    }

    // Sends a motion notify event to the X11 server via `XTest` extension
    // TODO: Check if using x11rb::protocol::xproto::warp_pointer would be better
    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        debug!("\x1b[93mmove_mouse(x: {x:?}, y: {y:?}, coordinate:{coordinate:?})\x1b[0m");
        let (x_absolute, y_absolute) = if coordinate == Coordinate::Rel {
            let (x_absolute, y_absolute) = self.location()?;
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
        send_input(&[input])?;

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
    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        debug!("\x1b[93mscroll(length: {length:?}, axis: {axis:?})\x1b[0m");
        let input = match axis {
            Axis::Horizontal => {
                mouse_event(MOUSEEVENTF_HWHEEL, length * (WHEEL_DELTA as i32), 0, 0)
            }
            Axis::Vertical => mouse_event(MOUSEEVENTF_WHEEL, -length * (WHEEL_DELTA as i32), 0, 0),
        };
        send_input(&[input])?;
        Ok(())
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mmain_display()\x1b[0m");
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

    fn location(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mlocation()\x1b[0m");
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

impl Keyboard for Enigo {
    fn fast_text_entry(&mut self, _text: &str) -> InputResult<Option<()>> {
        Ok(None)
    }

    /// Enter the whole text string instead of entering individual keys
    /// This is much faster if you type longer text at the cost of keyboard
    /// shortcuts not getting recognized
    fn text(&mut self, text: &str) -> InputResult<()> {
        debug!("\x1b[93mtext(text: {text})\x1b[0m");
        if text.is_empty() {
            return Ok(()); // Nothing to simulate.
        }
        let mut buffer = [0; 2];

        let mut input = vec![];
        for c in text.chars() {
            // Handle special characters seperately
            match c {
                '\n' => return self.key(Key::Return, Direction::Click),
                '\r' => { // TODO: What is the correct key to type here?
                }
                '\t' => return self.key(Key::Tab, Direction::Click),
                '\0' => return Err(InputError::InvalidInput("the text contained a null byte")),
                _ => (),
            }
            // Windows uses uft-16 encoding. We need to check
            // for variable length characters. As such some
            // characters can be 32 bit long and those are
            // encoded in what is called high and low surrogates.
            // Each are 16 bit wide and need to be sent after
            // another to the SendInput function
            let result = c.encode_utf16(&mut buffer);
            for &utf16_surrogate in &*result {
                input.push(keybd_event(
                    KEYEVENTF_UNICODE,
                    VIRTUAL_KEY(0),
                    utf16_surrogate,
                ));
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
    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mkey(key: {key:?}, direction: {direction:?})\x1b[0m");
        let mut input = vec![];

        if let Key::Unicode(c) = key {
            // Handle special characters seperately
            match c {
                '\n' => return self.key(Key::Return, direction),
                '\r' => { // TODO: What is the correct key to type here?
                }
                '\t' => return self.key(Key::Tab, direction),
                '\0' => {
                    debug!("entering Key::Unicode('\\0') is a noop");
                    return Ok(());
                }
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
            // Key::Unicode and we already ensured that is not the case
            let keycode = VIRTUAL_KEY::try_from(key).unwrap();
            let keyflags = get_key_flags(keycode);
            if direction == Direction::Click || direction == Direction::Press {
                input.push(keybd_event(keyflags, keycode, 0u16));
            }
            if direction == Direction::Click || direction == Direction::Release {
                input.push(keybd_event(keyflags | KEYEVENTF_KEYUP, keycode, 0u16));
            }
        };
        send_input(&input)?;

        match direction {
            Direction::Press => {
                debug!("added the key {key:?} to the held keys");
                self.held.0.push(key);
            }
            Direction::Release => {
                debug!("removed the key {key:?} from the held keys");
                self.held.0.retain(|&k| k != key);
            }
            Direction::Click => (),
        }

        Ok(())
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mraw(keycode: {keycode:?}, direction: {direction:?})\x1b[0m");
        let mut input = vec![];

        // Some keycodes also need to have the KEYEVENTF_EXTENDEDKEY flag set because
        // the code is used for two different keys. The maximum for a scancode (raw
        // keycode) is 7F on Windows. Since we use an u16 here, we can use the remaining
        // bits to allow specifying the user of the library if the flag should get set
        // or not. It is assumed that all keycodes larger than 7F need the flag to be
        // set
        let (keycode, keyflags) = if keycode > 0x7F {
            (
                keycode & 0x7F, /* remove the bits used for signaling if the extendend flag
                                 * should get set */
                KEYEVENTF_SCANCODE | KEYEVENTF_EXTENDEDKEY,
            )
        } else {
            (keycode, KEYEVENTF_SCANCODE)
        };
        if direction == Direction::Click || direction == Direction::Press {
            input.push(keybd_event(keyflags, VIRTUAL_KEY(0), keycode));
        }
        if direction == Direction::Click || direction == Direction::Release {
            input.push(keybd_event(
                keyflags | KEYEVENTF_KEYUP,
                VIRTUAL_KEY(0),
                keycode,
            ));
        }

        send_input(&input)?;

        match direction {
            Direction::Press => {
                debug!("added the key {keycode:?} to the held keys");
                self.held.1.push(keycode);
            }
            Direction::Release => {
                debug!("removed the key {keycode:?} from the held keys");
                self.held.1.retain(|&k| k != keycode);
            }
            Direction::Click => (),
        }

        Ok(())
    }
}

impl Enigo {
    /// Create a new Enigo struct to establish the connection to simulate input
    /// with the specified settings
    ///
    /// # Errors
    /// Have a look at the documentation of `NewConError` to see under which
    /// conditions an error will be returned.
    pub fn new(settings: &Settings) -> Result<Self, NewConError> {
        let Settings {
            release_keys_when_dropped,
            ..
        } = settings;

        let held = (vec![], vec![]);

        debug!("\x1b[93mconnection established on windows\x1b[0m");

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
                    error!("{e:?}");
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
                    error!("{e:?}");
                    return Err(InputError::InvalidInput("scan code did not fit into u16"));
                }
            };
        }
        Ok(scancodes)
    }

    /// Returns a list of all currently pressed keys
    pub fn held(&mut self) -> (Vec<Key>, Vec<ScanCode>) {
        self.held.clone()
    }
}

fn get_key_flags(vk: VIRTUAL_KEY) -> KEYBD_EVENT_FLAGS {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_NEXT, VK_NUMLOCK,
        VK_PRIOR, VK_RCONTROL, VK_RIGHT, VK_RMENU, VK_SNAPSHOT, VK_UP,
    };

    match vk {
        // Navigation keys should be injected with the extended flag to distinguish
        // them from the Numpad navigation keys. Otherwise, input Shift+<Navigation key>
        // may not have the expected result and depends on whether NUMLOCK is enabled/disabled.
        // A list of the extended keys can be found here:
        // https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input#extended-key-flag
        // TODO: The keys "BREAK (CTRL+PAUSE) key" and "ENTER key in the numeric keypad" are missing
        VK_RMENU | VK_RCONTROL | VK_UP | VK_DOWN | VK_LEFT | VK_RIGHT | VK_INSERT | VK_DELETE
        | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_NUMLOCK | VK_SNAPSHOT | VK_DIVIDE => {
            debug!("extended key detected");
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
        let (held_keys, held_keycodes) = self.held();
        for key in held_keys {
            if self.key(key, Direction::Release).is_err() {
                error!("unable to release {key:?}");
            };
        }
        for keycode in held_keycodes {
            if self.raw(keycode, Direction::Release).is_err() {
                error!("unable to release {keycode:?}");
            };
        }
        debug!("released all held keys");
    }
}
