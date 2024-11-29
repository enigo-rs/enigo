use std::mem::size_of;

use log::{debug, error, info, warn};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{
        GetKeyboardLayout, MapVirtualKeyExW, SendInput, HKL, INPUT, INPUT_0, INPUT_KEYBOARD,
        INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
        KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC_EX, MAPVK_VSC_TO_VK_EX,
        MAP_VIRTUAL_KEY_TYPE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
        MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
        MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN,
        MOUSEEVENTF_XUP, MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY,
    },
    WindowsAndMessaging::{
        GetCursorPos, GetForegroundWindow, GetSystemMetrics, GetWindowThreadProcessId, SM_CXSCREEN,
        SM_CYSCREEN, WHEEL_DELTA,
    },
};

#[cfg(feature = "test_mouse")]
use fixed::{types::extra::U16, FixedI32};

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
    dw_extra_info: usize,
    windows_subject_to_mouse_speed_and_acceleration_level: bool,
}

fn send_input(input: &[INPUT]) -> InputResult<()> {
    if input.is_empty() {
        return Ok(());
    }
    let Ok(input_size): Result<i32, _> = size_of::<INPUT>().try_into() else {
        return Err(InputError::InvalidInput(
            "the size of the INPUT was so large, the size exceeded i32::MAX",
        ));
    };
    let Ok(input_len): Result<u32, _> = input.len().try_into() else {
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

fn mouse_event(
    flags: MOUSE_EVENT_FLAGS,
    data: i32,
    dx: i32,
    dy: i32,
    dw_extra_info: usize,
) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data as u32, /* mouseData unfortunately is defined as unsigned even
                                         * though we need negative values as well */
                dwFlags: flags,
                time: 0, /* Always set it to 0 (see https://web.archive.org/web/20231004113147/https://devblogs.microsoft.com/oldnewthing/20121101-00/?p=6193) */
                dwExtraInfo: dw_extra_info,
            },
        },
    }
}

fn keybd_event(
    flags: KEYBD_EVENT_FLAGS,
    vk: VIRTUAL_KEY,
    scan: ScanCode,
    dw_extra_info: usize,
) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0, /* Always set it to 0 (see https://web.archive.org/web/20231004113147/https://devblogs.microsoft.com/oldnewthing/20121101-00/?p=6193) */
                dwExtraInfo: dw_extra_info,
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
            input.push(mouse_event(
                mouse_event_flag,
                button_no,
                0,
                0,
                self.dw_extra_info,
            ));
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
            input.push(mouse_event(
                mouse_event_flag,
                button_no,
                0,
                0,
                self.dw_extra_info,
            ));
        }
        send_input(&input)
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        debug!("\x1b[93mmove_mouse(x: {x:?}, y: {y:?}, coordinate:{coordinate:?})\x1b[0m");
        let (flags, x, y) = if coordinate == Coordinate::Abs {
            // 0-screen width/height - 1 map to 0-65535
            // Add w/2 or h/2 to round off
            // See https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mouse_event#remarks
            let (w, h) = self.main_display()?;
            let w = w as i64 - 1;
            let h = h as i64 - 1;
            let x = x as i64;
            let y = y as i64;
            let x = (x * 65535 + w / 2 * x.signum()) / w;
            let y = (y * 65535 + h / 2 * y.signum()) / h;
            // TODO: Check if we should use MOUSEEVENTF_VIRTUALDESK too
            (MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE, x as i32, y as i32)
        } else if self.windows_subject_to_mouse_speed_and_acceleration_level {
            // Quote from documentation (http://web.archive.org/web/20241118235853/https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mouse_event):
            // Relative mouse motion is subject to the settings for mouse speed and
            // acceleration level. An end user sets these values using the Mouse application
            // in Control Panel. An application obtains and sets these values with the
            // SystemParametersInfo function.
            //
            // The system applies two tests to the specified relative mouse motion when
            // applying acceleration. If the specified distance along either the x or y axis
            // is greater than the first mouse threshold value, and the mouse acceleration
            // level is not zero, the operating system doubles the distance. If the
            // specified distance along either the x- or y-axis is greater than the second
            // mouse threshold value, and the mouse acceleration level is equal to two, the
            // operating system doubles the distance that resulted from applying the first
            // threshold test. It is thus possible for the operating system to multiply
            // relatively-specified mouse motion along the x- or y-axis by up to four times.
            //
            // Once acceleration has been applied, the system scales the resultant value by
            // the desired mouse speed. Mouse speed can range from 1 (slowest) to 20
            // (fastest) and represents how much the pointer moves based on the distance the
            // mouse moves. The default value is 10, which results in no additional
            // modification to the mouse motion.
            debug!("\x1b[93mRelative mouse move is subject to mouse speed and acceleration level\x1b[0m");
            (MOUSEEVENTF_MOVE, x, y)
        } else {
            // Instead of moving the mouse by a relative amount, we calculate the resulting
            // location and move it to the absolute location so it is not subject to mouse
            // speed and acceleration levels
            debug!("\x1b[93mRelative mouse move is NOT subject to mouse speed and acceleration level\x1b[0m");
            let (current_x, current_y) = self.location()?;
            return self.move_mouse(current_x + x, current_y + y, Coordinate::Abs);
        };
        let input = mouse_event(flags, 0, x, y, self.dw_extra_info);
        send_input(&[input])
    }

    // Sends a scroll event to the X11 server via `XTest` extension
    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        debug!("\x1b[93mscroll(length: {length:?}, axis: {axis:?})\x1b[0m");
        let input = match axis {
            Axis::Horizontal => mouse_event(
                MOUSEEVENTF_HWHEEL,
                length * (WHEEL_DELTA as i32),
                0,
                0,
                self.dw_extra_info,
            ),
            Axis::Vertical => mouse_event(
                MOUSEEVENTF_WHEEL,
                -length * (WHEEL_DELTA as i32),
                0,
                0,
                self.dw_extra_info,
            ),
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
    fn fast_text(&mut self, _text: &str) -> InputResult<Option<()>> {
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
        let mut buffer = [0; 2]; // A buffer of length 2 is large enough to encode any char in utf16

        let mut input = Vec::with_capacity(2 * text.len()); // Each char needs at least one event to press and one to release it
        for c in text.chars() {
            // Enter special characters as keys
            match c {
                '\n' => self.queue_key(&mut input, Key::Return, Direction::Click)?,
                '\r' => { // TODO: What is the correct key to type here?
                }
                '\t' => self.queue_key(&mut input, Key::Tab, Direction::Click)?,
                '\0' => Err(InputError::InvalidInput("the text contained a null byte"))?,
                _ => (),
            }

            self.queue_char(&mut input, c, &mut buffer);
        }
        send_input(&input)
    }

    /// Sends a key event to the X11 server via `XTest` extension
    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mkey(key: {key:?}, direction: {direction:?})\x1b[0m");
        let mut input = Vec::with_capacity(2);

        self.queue_key(&mut input, key, direction)?;
        send_input(&input)?;

        match direction {
            Direction::Press => {
                debug!("added the key {key:?} to the held keys");
                self.held.0.push(key);
                // TODO: Make it work that they can get released with the raw
                // function as well
            }
            Direction::Release => {
                debug!("removed the key {key:?} from the held keys");
                self.held.0.retain(|&k| k != key);
                // TODO: Make it work that they can get released with the raw
                // function as well
            }
            Direction::Click => (),
        }

        Ok(())
    }

    fn raw(&mut self, scan: u16, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mraw(scan: {scan:?}, direction: {direction:?})\x1b[0m");
        let mut input = vec![];

        let vk = VIRTUAL_KEY(Enigo::translate_key(scan, MAPVK_VSC_TO_VK_EX)?); // translate scan code to virtual key

        let mut keyflags = KEYEVENTF_SCANCODE;
        // TODO: Check if the first bytes need to be truncated if it is an extended key
        if Enigo::is_extended_key(vk) {
            keyflags |= KEYEVENTF_EXTENDEDKEY;
        }

        if direction == Direction::Click || direction == Direction::Press {
            input.push(keybd_event(keyflags, vk, scan, self.dw_extra_info));
        }
        if direction == Direction::Click || direction == Direction::Release {
            input.push(keybd_event(
                keyflags | KEYEVENTF_KEYUP,
                vk,
                scan,
                self.dw_extra_info,
            ));
        }

        send_input(&input)?;

        match direction {
            Direction::Press => {
                debug!("added the key {scan:?} to the held keys");
                self.held.1.push(scan);
                // TODO: Make it work that they can get released with the key
                // function as well
            }
            Direction::Release => {
                debug!("removed the key {scan:?} from the held keys");
                self.held.1.retain(|&k| k != scan);
                // TODO: Make it work that they can get released with the key
                // function as well
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
            windows_dw_extra_info: dw_extra_info,
            release_keys_when_dropped,
            windows_subject_to_mouse_speed_and_acceleration_level,
            ..
        } = settings;

        let held = (vec![], vec![]);

        debug!("\x1b[93mconnection established on windows\x1b[0m");

        Ok(Self {
            held,
            release_keys_when_dropped: *release_keys_when_dropped,
            dw_extra_info: dw_extra_info.unwrap_or(crate::EVENT_MARKER as usize),
            windows_subject_to_mouse_speed_and_acceleration_level:
                *windows_subject_to_mouse_speed_and_acceleration_level,
        })
    }

    pub(crate) fn keyboard_layout() -> HKL {
        let current_window_thread_id =
            unsafe { GetWindowThreadProcessId(GetForegroundWindow(), None) };
        unsafe { GetKeyboardLayout(current_window_thread_id) }
    }

    /// Generic function to translate between virtual keys and scan codes
    fn translate_key(input: u16, map_type: MAP_VIRTUAL_KEY_TYPE) -> InputResult<u16> {
        let layout = Enigo::keyboard_layout();

        // Call MapVirtualKeyExW using the provided map_type and input
        match unsafe { MapVirtualKeyExW(input.into(), map_type, layout) }.try_into() {
            Ok(output) => {
                if output == 0 {
                    warn!("The result for the input {:?} is zero. This usually means there was no mapping", input);
                }
                Ok(output)
            }
            Err(e) => {
                error!("{e:?}");
                Err(InputError::InvalidInput("result did not fit into u16"))
            }
        }
    }

    fn queue_key(
        &mut self,
        input_queue: &mut Vec<INPUT>,
        key: Key,
        direction: Direction,
    ) -> InputResult<()> {
        let Ok(vk) = VIRTUAL_KEY::try_from(key) else {
            if let Key::Unicode(c) = key {
                warn!("Unable to enter the key as a virtual key.");
                warn!("Falling back to entering it as text.");
                let mut buffer = [0; 2]; // A buffer of length 2 is large enough to encode any char in utf16
                self.queue_char(input_queue, c, &mut buffer);
                return Ok(());
            }
            return Err(InputError::Mapping(
                "This should never happen. There is a bug in the implementation".to_string(),
            ));
        };
        let scan = Enigo::translate_key(vk.0, MAPVK_VK_TO_VSC_EX)?; // Translate virtual key to scan code

        let mut keyflags = KEYBD_EVENT_FLAGS::default();

        // TODO: Check if this is needed
        //       We have a virtual key and a scan code at the end anyways
        if let Key::Unicode(_) = key {
            keyflags |= KEYEVENTF_SCANCODE;
        };

        if Enigo::is_extended_key(vk) {
            keyflags |= KEYEVENTF_EXTENDEDKEY;
        }

        if direction == Direction::Click || direction == Direction::Press {
            input_queue.push(keybd_event(keyflags, vk, scan, self.dw_extra_info));
        }
        if direction == Direction::Click || direction == Direction::Release {
            input_queue.push(keybd_event(
                keyflags | KEYEVENTF_KEYUP,
                vk,
                scan,
                self.dw_extra_info,
            ));
        }

        Ok(())
    }

    fn queue_char(&mut self, input_queue: &mut Vec<INPUT>, character: char, buffer: &mut [u16; 2]) {
        // Windows uses uft-16 encoding. We need to check
        // for variable length characters. As such some
        // characters can be 32 bit long and those are
        // encoded in what is called high and low surrogates.
        // Each are 16 bit wide and need to be sent after
        // another to the SendInput function
        let result = character.encode_utf16(buffer);
        for &utf16_surrogate in &*result {
            input_queue.push(keybd_event(
                // No need to check if it is an extended key because we only enter unicode
                // chars here
                KEYEVENTF_UNICODE,
                // Must be zero
                VIRTUAL_KEY(0),
                utf16_surrogate,
                self.dw_extra_info,
            ));
            input_queue.push(keybd_event(
                // No need to check if it is an extended key because we only enter unicode
                // chars here
                KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                // Must be zero
                VIRTUAL_KEY(0),
                // TODO: Double check if this could also be utf16_surrogate (I think it doesn't
                // make a difference)
                result[0],
                self.dw_extra_info,
            ));
        }
    }

    /// Returns a list of all currently pressed keys
    pub fn held(&mut self) -> (Vec<Key>, Vec<ScanCode>) {
        self.held.clone()
    }

    /// Returns the value that enigo's events are marked with
    #[must_use]
    pub fn get_marker_value(&self) -> usize {
        self.dw_extra_info
    }

    /// Test if the virtual key is one of the keys that need the
    /// `KEYEVENTF_EXTENDEDKEY` flag to be set
    fn is_extended_key(vk: VIRTUAL_KEY) -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_NEXT,
            VK_NUMLOCK, VK_PRIOR, VK_RCONTROL, VK_RIGHT, VK_RMENU, VK_SNAPSHOT, VK_UP,
        };

        match vk {
            // Navigation keys should be injected with the extended flag to distinguish
            // them from the Numpad navigation keys. Otherwise, input Shift+<Navigation key>
            // may not have the expected result and depends on whether NUMLOCK is enabled/disabled.
            // A list of the extended keys can be found here:
            // https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input#extended-key-flag
            // TODO: The keys "BREAK (CTRL+PAUSE) key" and "ENTER key in the numeric keypad" are
            // missing
            VK_RMENU | VK_RCONTROL | VK_UP | VK_DOWN | VK_LEFT | VK_RIGHT | VK_INSERT
            | VK_DELETE | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_NUMLOCK | VK_SNAPSHOT
            | VK_DIVIDE => true,
            _ => false,
        }
    }
}

/// Returns the currently set threshold1, threshold2 and acceleration level of
/// the mouse
///
/// The default values on my system were (6, 10, 1)
// This is needed to calculate the location after a relative mouse move
#[must_use]
pub fn mouse_thresholds_and_acceleration() -> Option<(i32, i32, i32)> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETMOUSE, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    // Retrieve mouse acceleration thresholds and level
    let mut mouse_params = [0i32; 3];
    unsafe {
        if SystemParametersInfoW(
            SPI_GETMOUSE,
            0, // Not used
            Some(mouse_params.as_mut_ptr().cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0), /* We are not setting a parameter so it can
                                                     * be zero */
        )
        .is_err()
        {
            error!("unable to get the mouse params");
            let last_err = std::io::Error::last_os_error();
            error!("{last_err}");
            return None;
        }
    }
    debug!("mouse_params: {mouse_params:?}");
    let [threshold1, threshold2, acceleration_level] = mouse_params;
    Some((threshold1, threshold2, acceleration_level))
}

/// Set the threshold1, threshold2 and acceleration level of
/// the mouse
/// The documentation says that the acceleration level can be 0, 1 or 2
/// However nowadays only 0 and 1 seem to be allowd as on my system I am unable
/// to set it to 2
///
/// The default values on my system were (6, 10, 1)
///
/// # Errors
/// Returns an error if the OS was unable to set the value or if the parameters
/// were invalid
pub fn set_mouse_thresholds_and_acceleration(
    threshold1: i32,
    threshold2: i32,
    acceleration_level: i32,
) -> Result<(), std::io::Error> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPI_SETMOUSE,
    };

    if acceleration_level != 0 && acceleration_level != 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid acceleration level",
        ));
    }

    let mut mouse_params = [threshold1, threshold2, acceleration_level];
    unsafe {
        if SystemParametersInfoW(
            SPI_SETMOUSE,
            0, // Not used
            Some(mouse_params.as_mut_ptr().cast()),
            SPIF_SENDCHANGE, /* Broadcasts the WM_SETTINGCHANGE message after updating the user
                              * profile, update Win.ini */
        )
        .is_err()
        {
            error!("unable to set the mouse params");
            let last_err = std::io::Error::last_os_error();
            error!("{last_err}");
            return Err(last_err);
        }
    }
    Ok(())
}

/// Returns the currently set scaling factor "`mouse_speed`". This is not the
/// actual speed of the mouse but a setting.
///
/// Default value of the mouse speed is 10
/// The returned value ranges between 1 (slowest) and 20 (fastest)
/// (Source: <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfoa>)
// This is needed to calculate the location after a relative mouse move
#[must_use]
pub fn mouse_speed() -> Option<i32> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETMOUSESPEED, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    // Retrieve mouse speed
    let mut mouse_speed = 0i32;

    unsafe {
        if SystemParametersInfoW(
            SPI_GETMOUSESPEED,
            0, // Not used
            Some((&raw mut mouse_speed).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0), /* We are not setting a parameter so it can
                                                     * be zero */
        )
        .is_err()
        {
            error!("unable to get the mouse params");
            let last_err = std::io::Error::last_os_error();
            error!("{last_err}");
            return None;
        }
    }
    Some(mouse_speed)
}

/// Sets the scaling factor "`mouse_speed`". This is not the
/// actual speed of the mouse but a setting.
/// Must be between 1 (slowest) and 20 (fastest) (1 <= `mouse_speed` <= 20)
///
/// Default value of the mouse speed is 10
/// (Source: <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfoa>)
///
/// # Errors
/// Returns an error if the OS was unable to set the value or if the parameters
/// were invalid
pub fn set_mouse_speed(mouse_speed: i32) -> Result<(), std::io::Error> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETMOUSESPEED,
    };

    if !(1..=20).contains(&mouse_speed) {
        error!("Not a valid mouse speed");
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid mouse speed",
        ));
    }

    unsafe {
        if SystemParametersInfoW(
            SPI_SETMOUSESPEED,
            0, // Not used
            Some((mouse_speed as *mut usize).cast()),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE, /* Broadcasts the WM_SETTINGCHANGE message
                                                   * after updating the user
                                                   * profile, update Win.ini */
        )
        .is_err()
        {
            error!("unable to set the mouse params");
            let last_err = std::io::Error::last_os_error();
            error!("{last_err}");
            // If the output was "The operation completed successfully. (os error 0)", then
            // the problem might be that the value to set the system parameter to was not
            // valid
            return Err(last_err);
        }
    }
    Ok(())
}

/// Get the values of the `SmoothMouseXCurve` and the `SmoothMouseYCurve` values
/// from the registry If the first parameter is true, `SmoothMouseXCurve` will
/// be fetched. If the second parameter is true, `SmoothMouseYCurve` will be
/// fetched. They each hold five f16.16 values in binary form (little endian)
///
/// # Errors
/// An error will be thrown if the application does not have the permissions to
/// read the values or the values do not contain exactly the expected amount of
/// data. . For
/// some reason there is also the same amount of empty data after each value.
/// That means in total 2 * 4 * 5 = 40 bytes are expected
/// There might be more cases when an error is thrown. Check the Microsoft
/// Windows documentation if you need to know more
#[allow(clippy::similar_names)] // smooth_mouse_curve_x_key and smooth_mouse_curve_y_key are too similar
#[cfg(feature = "test_mouse")]
pub fn mouse_curve(
    get_x: bool,
    get_y: bool,
) -> Result<[Option<[FixedI32<U16>; 5]>; 2], windows::core::Error> {
    use windows::{
        core::PCWSTR,
        Win32::System::Registry::HKEY,
        Win32::System::Registry::{RegGetValueW, RegOpenCurrentUser, KEY_READ, RRF_RT_REG_BINARY},
    };

    let smooth_mouse_curve_x_key: Vec<u16> = "SmoothMouseXCurve\0".encode_utf16().collect();
    let smooth_mouse_curve_y_key: Vec<u16> = "SmoothMouseYCurve\0".encode_utf16().collect();

    let tasks = [
        (get_x, smooth_mouse_curve_x_key),
        (get_y, smooth_mouse_curve_y_key),
    ];

    let mut curve = [None, None];

    // Define the registry key and value names
    let path: Vec<u16> = "Control Panel\\Mouse\0".encode_utf16().collect();

    // Open the current user registry key
    let mut hkey_current_user: HKEY = HKEY::default();
    let res = unsafe { RegOpenCurrentUser(KEY_READ.0, &mut hkey_current_user) };
    if res.is_err() {
        println!("Failed to open current user registry key. Error: {res:?}");
        return Err(res.into());
    }

    for (idx, (get_it, key)) in tasks.iter().enumerate() {
        if *get_it {
            let mut return_data = [0u8; 2 * 5 * 4];
            let mut return_data_len = return_data.len() as u32;
            let result = unsafe {
                RegGetValueW(
                    hkey_current_user,
                    PCWSTR(path.as_ptr()),
                    PCWSTR(key.as_ptr()),
                    RRF_RT_REG_BINARY,
                    None,
                    Some(return_data.as_mut_ptr().cast()),
                    Some(&mut return_data_len),
                )
            };

            if result.is_err() {
                println!("Error getting the mouse curve {key:?}: {result:?}");
                return Err(result.into());
            }

            println!("mouse curve raw: {return_data:?}");

            // Fixed Point Math and Number Bounds
            //
            // The ballistic Windows XP pointer algorithm resides between ring0 and ring3.
            // Therefore, floating point math is not readily available, and because the
            // Windows XP ballistics required the use of division with a remainder,
            // fixed-point (16.16) integer math was used. This is important for the
            // subpixilation and the increased smoothness of the pointer movement.
            // Therefore, the maximum resultant number from two products is 2^16 (65536).
            // While an overflow is possible, it is very unlikely. If an overflow ever
            // becomes a problem in the future, the fixed point constants in the ballistics
            // code are easily changed to support a 20.12 fixed-point format.
            // source https://web.archive.org/web/20100315061825/http://www.microsoft.com/whdc/archive/pointer-bal.mspx
            let return_data: Vec<FixedI32<U16>> = return_data
                .chunks_exact(4)
                .step_by(2)
                // We use chunks_exact, so all chunks have a length of 4. Hence it is impossible for
                // try_into to fail
                .map(|chunk| chunk.try_into().unwrap_or([0, 0, 0, 0]))
                .map(FixedI32::from_le_bytes)
                .collect();
            let return_data: [FixedI32<U16>; 5] = return_data
                .try_into()
                .map_err(|_| windows::core::Error::empty())?;
            curve[idx] = Some(return_data);
        }
    }

    Ok(curve)
}

/// Set the values of the `SmoothMouseXCurve` and the `SmoothMouseYCurve` values
/// from the registry. Theymust each hold five f16.16 values in binary form
/// (little endian)
///
/// # Errors
/// An error will be thrown if the application does not have the permissions to
/// write the values.
/// There might be more cases when an error is thrown. Check the Microsoft
/// Windows documentation if you need to know more
#[cfg(feature = "test_mouse")]
pub fn set_mouse_curve(
    mouse_curve_x: Option<[FixedI32<U16>; 5]>,
    mouse_curve_y: Option<[FixedI32<U16>; 5]>,
) -> Result<(), windows::core::Error> {
    use windows::{
        core::PCWSTR,
        Win32::System::Registry::HKEY,
        Win32::System::Registry::{
            RegCloseKey, RegOpenCurrentUser, RegOpenKeyExW, RegSetValueExW, KEY_READ, KEY_WRITE,
            REG_BINARY,
        },
    };
    // Check if there is anything to do
    if mouse_curve_x.is_none() && mouse_curve_y.is_none() {
        return Ok(());
    }

    let key_x: Vec<_> = "SmoothMouseXCurve\0".encode_utf16().collect();
    let key_y = "SmoothMouseYCurve\0".encode_utf16().collect();

    // Define the registry key and value names
    let path: Vec<u16> = "Control Panel\\Mouse\0".encode_utf16().collect();

    // Open the current user registry key
    let mut hkey_current_user: HKEY = HKEY::default();
    let res = unsafe { RegOpenCurrentUser(KEY_READ.0, &mut hkey_current_user) };
    if res.is_err() {
        error!("Failed to open current user registry key. Error: {:?}", res);
        return Err(res.into());
    }

    // Open the registry key at the given path
    let mut hkey: HKEY = HKEY::default();
    let res = unsafe {
        RegOpenKeyExW(
            hkey_current_user,
            PCWSTR(path.as_ptr()),
            0,                    // Reserved, must be zero
            KEY_READ | KEY_WRITE, // Open with read and write permissions
            &mut hkey,
        )
    };

    if res.is_err() {
        error!("Failed to open registry key at path. Error: {:?}", res);
        return Err(res.into());
    }

    let mut tasks = Vec::with_capacity(2);
    if let Some(mouse_curve) = mouse_curve_x {
        tasks.push((mouse_curve, key_x));
    }
    if let Some(mouse_curve) = mouse_curve_y {
        tasks.push((mouse_curve, key_y));
    }

    for (mouse_curve, key) in tasks {
        let mut interspersed_curve = [0u8; 8 * 5];
        for (i, &value) in mouse_curve.iter().enumerate() {
            let start = i * 8; // Each block is 8 bytes: 4 for value, 4 for zero
            interspersed_curve[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }

        let result = unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(key.as_ptr()),
                0, // Reserved, must be zero
                REG_BINARY,
                Some(&interspersed_curve),
            )
        };

        if result.is_err() {
            error!("Error getting the mouse curve x: {:?}", result);
            return Err(result.into());
        }
    }

    // Close the opened registry key
    let res = unsafe { RegCloseKey(hkey) };
    if res.is_err() {
        error!("Failed to close registry key. Error: {:?}", res);
        return Err(res.into());
    }

    Ok(())
}

#[must_use]
pub fn system_dpi() -> u32 {
    unsafe { windows::Win32::UI::HiDpi::GetDpiForSystem() }
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

mod test {

    #[test]
    #[allow(clippy::too_many_lines)]
    fn unit_set_mouse_thresholds_and_acceleration() {
        use super::{mouse_thresholds_and_acceleration, set_mouse_thresholds_and_acceleration};

        // The acceleration level can only be 0 or 1. Previously 2 was allowed as well,
        // but apparently that was changed
        let valid_acceleration_levels = vec![0, 1];
        let valid_thresholds = vec![
            (6, 10),
            (-1, 0),
            (0, -1),
            (-1, -1),
            (-1000, -1000),
            (i32::MIN, 0),
            (0, i32::MIN),
            (i32::MIN, i32::MIN),
            (100, 1),
            (1, 100),
            (1, 100),
            (1, 10000),
            (1000000, 10000),
            (453, 5673),
            (-4532, 856),
            (436, -5783),
            (994974, 0),
        ];

        let invalid_test_cases = vec![
            (0, 0, i32::MIN), // Negative acceleration level
            (0, 0, -1000),    // Negative acceleration level
            (0, 0, -1),       // Negative acceleration level
            (0, 0, 2),        // acceleration level > 1
            (0, 0, 3),        // acceleration level > 1
            (0, 0, 4),        // acceleration level > 1
            (0, 0, 10),       // acceleration level > 1
            (0, 0, 5435674),  // acceleration level > 1
            (0, 0, i32::MAX), // acceleration level > 1
            (0, 0, i32::MIN), // Negative acceleration level
            (6, 10, -1000),   // Negative acceleration level
            (6, 10, -1),      // Negative acceleration level
            (6, 10, 2),       // acceleration level > 1
            (6, 10, 3),       // acceleration level > 1
            (6, 10, 4),       // acceleration level > 1
            (6, 10, 10),      // acceleration level > 1
            (6, 10, 5435674), // acceleration level > 1
        ];

        // Store current setting
        let (old_threshold1, old_threshold2, old_acceleration_level) =
            mouse_thresholds_and_acceleration().unwrap();
        println!("old_threshold1: {old_threshold1}");
        println!("old_threshold2: {old_threshold2}");
        println!("old_acceleration_level: {old_acceleration_level}");
        println!();
        println!();

        for valid_acceleration_level in valid_acceleration_levels {
            for (valid_threshold1, valid_threshold2) in &valid_thresholds {
                println!("old_threshold1: {valid_threshold1}");
                println!("old_threshold2: {valid_threshold2}");
                println!("old_acceleration_level: {valid_acceleration_level}");
                println!();
                set_mouse_thresholds_and_acceleration(
                    *valid_threshold1,
                    *valid_threshold2,
                    valid_acceleration_level,
                )
                .unwrap();
                let actual_params = mouse_thresholds_and_acceleration().unwrap();
                assert_eq!(
                    (
                        *valid_threshold1,
                        *valid_threshold2,
                        valid_acceleration_level
                    ),
                    actual_params
                );
            }
        }

        // Restore old setting
        set_mouse_thresholds_and_acceleration(
            old_threshold1,
            old_threshold2,
            old_acceleration_level,
        )
        .unwrap();

        for (invalid_threshold1, invalid_threshold2, invalid_acceleration_level) in
            invalid_test_cases
        {
            println!("old_threshold1: {invalid_threshold1}");
            println!("old_threshold2: {invalid_threshold2}");
            println!("old_acceleration_level: {invalid_acceleration_level}");
            println!();
            if set_mouse_thresholds_and_acceleration(
                invalid_threshold1,
                invalid_threshold2,
                invalid_acceleration_level,
            )
            .is_ok()
            {
                // Restore old setting ()
                set_mouse_thresholds_and_acceleration(
                    old_threshold1,
                    old_threshold2,
                    old_acceleration_level,
                )
                .unwrap();
                panic!("Successfully set an invalid mouse speed");
            };
            let actual_params = mouse_thresholds_and_acceleration().unwrap();
            assert_eq!(
                (old_threshold1, old_threshold2, old_acceleration_level),
                actual_params
            );
        }
    }

    #[test]
    fn unit_get_set_mouse_speed() {
        use super::{mouse_speed, set_mouse_speed};

        let valid_speeds = 1..=20;
        let invalid_speeds = vec![i32::MIN, -1000, -1, 0, 21, 22, 1000, i32::MAX];

        // Store current setting
        let old_mouse_speed = mouse_speed().unwrap();
        println!("old_mouse_speed: {old_mouse_speed}");

        for valid_mouse_speed in valid_speeds {
            println!("valid mouse speed: {valid_mouse_speed}");
            set_mouse_speed(valid_mouse_speed).unwrap();
            let actual_mouse_speed = mouse_speed().unwrap();
            assert_eq!(valid_mouse_speed, actual_mouse_speed);
        }
        // Restore old setting
        set_mouse_speed(old_mouse_speed).unwrap();

        for invalid_mouse_speed in invalid_speeds {
            println!("invalid mouse speed: {invalid_mouse_speed}");
            if set_mouse_speed(invalid_mouse_speed).is_ok() {
                // Restore old setting ()
                set_mouse_speed(old_mouse_speed).unwrap();
                panic!("Successfully set an invalid mouse speed");
            };
            let actual_mouse_speed = mouse_speed().unwrap();
            assert_eq!(old_mouse_speed, actual_mouse_speed);
        }
    }

    #[test]
    #[cfg(feature = "test_mouse")]
    fn unit_get_set_mouse_curve() {
        use fixed::FixedI32;
        let [old_mouse_curve_x, old_mouse_curve_y] = crate::mouse_curve(true, true).unwrap();

        let test_cases = vec![
            (
                [
                    FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                    FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                    FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                    FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                    FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                ],
                [0.0, 0.0, 0.0, 0.0, 0.0],
            ),
            (
                [
                    FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                    FixedI32::from_le_bytes([0x15, 0x6e, 0x00, 0x00]), // 0.43
                    FixedI32::from_le_bytes([0x00, 0x40, 0x01, 0x00]), // 1.25
                    FixedI32::from_le_bytes([0x29, 0xdc, 0x03, 0x00]), // 3.86
                    FixedI32::from_le_bytes([0x00, 0x00, 0x28, 0x00]), // 40.0
                ],
                [0.0, 0.43001, 1.25, 3.86001, 40.0],
            ),
            (
                [
                    FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                    FixedI32::from_le_bytes([0xb8, 0x5e, 0x01, 0x00]), // 1.37
                    FixedI32::from_le_bytes([0xcd, 0x4c, 0x05, 0x00]), // 5.3
                    FixedI32::from_le_bytes([0xcd, 0x4c, 0x18, 0x00]), // 24.3
                    FixedI32::from_le_bytes([0x00, 0x00, 0x38, 0x02]), // 568.0
                ],
                [0.0, 1.37, 5.30001, 24.30001, 568.0],
            ),
        ];
        for (mouse_curve, floats) in test_cases {
            mouse_curve
                .iter()
                .zip(floats.iter())
                .for_each(|(fixed, float)| {
                    println!("fixed: {fixed:?}, float: {float}");
                });

            crate::set_mouse_curve(Some(mouse_curve), None).unwrap();
            let mouse_curve_actual = crate::mouse_curve(true, false).unwrap();
            assert_eq!([Some(mouse_curve), None], mouse_curve_actual);
            crate::set_mouse_curve(None, Some(mouse_curve)).unwrap();
            let mouse_curve_actual = crate::mouse_curve(false, true).unwrap();
            assert_eq!([None, Some(mouse_curve)], mouse_curve_actual);
        }

        crate::set_mouse_curve(old_mouse_curve_x, old_mouse_curve_y).unwrap();
    }

    #[test]
    fn extended_key() {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_NEXT,
            VK_NUMLOCK, VK_PRIOR, VK_RCONTROL, VK_RIGHT, VK_RMENU, VK_SNAPSHOT, VK_UP,
        };

        let known_extended_keys = [
            VK_RMENU,    // 165
            VK_RCONTROL, // 163
            VK_UP,       // 38
            VK_DOWN,     // 40
            VK_LEFT,     // 37
            VK_RIGHT,    // 39
            VK_INSERT,   // 45
            VK_DELETE,   // 46
            VK_HOME,     // 36
            VK_END,      // 35
            VK_PRIOR,    // 33
            VK_NEXT,     // 34
            VK_NUMLOCK,  // 144
            VK_SNAPSHOT, // 44
            VK_DIVIDE,   // 111
        ];

        for key in known_extended_keys {
            assert!(super::Enigo::is_extended_key(key), "Failed for {key:#?}");
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn regular_key() {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            VK__none_, VK_0, VK_1, VK_2, VK_3, VK_4, VK_5, VK_6, VK_7, VK_8, VK_9, VK_A,
            VK_ABNT_C1, VK_ABNT_C2, VK_ACCEPT, VK_ADD, VK_APPS, VK_ATTN, VK_B, VK_BACK,
            VK_BROWSER_BACK, VK_BROWSER_FAVORITES, VK_BROWSER_FORWARD, VK_BROWSER_HOME,
            VK_BROWSER_REFRESH, VK_BROWSER_SEARCH, VK_BROWSER_STOP, VK_C, VK_CANCEL, VK_CAPITAL,
            VK_CLEAR, VK_CONTROL, VK_CONVERT, VK_CRSEL, VK_D, VK_DBE_ALPHANUMERIC,
            VK_DBE_CODEINPUT, VK_DBE_DBCSCHAR, VK_DBE_DETERMINESTRING,
            VK_DBE_ENTERDLGCONVERSIONMODE, VK_DBE_ENTERIMECONFIGMODE, VK_DBE_ENTERWORDREGISTERMODE,
            VK_DBE_FLUSHSTRING, VK_DBE_HIRAGANA, VK_DBE_KATAKANA, VK_DBE_NOCODEINPUT,
            VK_DBE_NOROMAN, VK_DBE_ROMAN, VK_DBE_SBCSCHAR, VK_DECIMAL, VK_E, VK_EREOF, VK_ESCAPE,
            VK_EXECUTE, VK_EXSEL, VK_F, VK_F1, VK_F10, VK_F11, VK_F12, VK_F13, VK_F14, VK_F15,
            VK_F16, VK_F17, VK_F18, VK_F19, VK_F2, VK_F20, VK_F21, VK_F22, VK_F23, VK_F24, VK_F3,
            VK_F4, VK_F5, VK_F6, VK_F7, VK_F8, VK_F9, VK_FINAL, VK_G, VK_GAMEPAD_A, VK_GAMEPAD_B,
            VK_GAMEPAD_DPAD_DOWN, VK_GAMEPAD_DPAD_LEFT, VK_GAMEPAD_DPAD_RIGHT, VK_GAMEPAD_DPAD_UP,
            VK_GAMEPAD_LEFT_SHOULDER, VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON,
            VK_GAMEPAD_LEFT_THUMBSTICK_DOWN, VK_GAMEPAD_LEFT_THUMBSTICK_LEFT,
            VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT, VK_GAMEPAD_LEFT_THUMBSTICK_UP,
            VK_GAMEPAD_LEFT_TRIGGER, VK_GAMEPAD_MENU, VK_GAMEPAD_RIGHT_SHOULDER,
            VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON, VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN,
            VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT, VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT,
            VK_GAMEPAD_RIGHT_THUMBSTICK_UP, VK_GAMEPAD_RIGHT_TRIGGER, VK_GAMEPAD_VIEW,
            VK_GAMEPAD_X, VK_GAMEPAD_Y, VK_H, VK_HANGEUL, VK_HANGUL, VK_HANJA, VK_HELP, VK_I,
            VK_ICO_00, VK_ICO_CLEAR, VK_ICO_HELP, VK_IME_OFF, VK_IME_ON, VK_J, VK_JUNJA, VK_K,
            VK_KANA, VK_KANJI, VK_L, VK_LAUNCH_APP1, VK_LAUNCH_APP2, VK_LAUNCH_MAIL,
            VK_LAUNCH_MEDIA_SELECT, VK_LBUTTON, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_M,
            VK_MBUTTON, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
            VK_MEDIA_STOP, VK_MENU, VK_MODECHANGE, VK_MULTIPLY, VK_N, VK_NAVIGATION_ACCEPT,
            VK_NAVIGATION_CANCEL, VK_NAVIGATION_DOWN, VK_NAVIGATION_LEFT, VK_NAVIGATION_MENU,
            VK_NAVIGATION_RIGHT, VK_NAVIGATION_UP, VK_NAVIGATION_VIEW, VK_NONAME, VK_NONCONVERT,
            VK_NUMPAD0, VK_NUMPAD1, VK_NUMPAD2, VK_NUMPAD3, VK_NUMPAD4, VK_NUMPAD5, VK_NUMPAD6,
            VK_NUMPAD7, VK_NUMPAD8, VK_NUMPAD9, VK_O, VK_OEM_1, VK_OEM_102, VK_OEM_2, VK_OEM_3,
            VK_OEM_4, VK_OEM_5, VK_OEM_6, VK_OEM_7, VK_OEM_8, VK_OEM_ATTN, VK_OEM_AUTO, VK_OEM_AX,
            VK_OEM_BACKTAB, VK_OEM_CLEAR, VK_OEM_COMMA, VK_OEM_COPY, VK_OEM_CUSEL, VK_OEM_ENLW,
            VK_OEM_FINISH, VK_OEM_FJ_JISHO, VK_OEM_FJ_LOYA, VK_OEM_FJ_MASSHOU, VK_OEM_FJ_ROYA,
            VK_OEM_FJ_TOUROKU, VK_OEM_JUMP, VK_OEM_MINUS, VK_OEM_NEC_EQUAL, VK_OEM_PA1, VK_OEM_PA2,
            VK_OEM_PA3, VK_OEM_PERIOD, VK_OEM_PLUS, VK_OEM_RESET, VK_OEM_WSCTRL, VK_P, VK_PA1,
            VK_PACKET, VK_PAUSE, VK_PLAY, VK_PRINT, VK_PROCESSKEY, VK_Q, VK_R, VK_RBUTTON,
            VK_RETURN, VK_RSHIFT, VK_RWIN, VK_S, VK_SCROLL, VK_SELECT, VK_SEPARATOR, VK_SHIFT,
            VK_SLEEP, VK_SPACE, VK_SUBTRACT, VK_T, VK_TAB, VK_U, VK_V, VK_VOLUME_DOWN,
            VK_VOLUME_MUTE, VK_VOLUME_UP, VK_W, VK_X, VK_XBUTTON1, VK_XBUTTON2, VK_Y, VK_Z,
            VK_ZOOM,
        };

        let known_ordinary_keys = [
            VK__none_,  // 255
            VK_0,       // 48
            VK_1,       // 49
            VK_2,       // 50
            VK_3,       // 51
            VK_4,       // 52
            VK_5,       // 53
            VK_6,       // 54
            VK_7,       // 55
            VK_8,       // 56
            VK_9,       // 57
            VK_A,       // 65
            VK_ABNT_C1, // 193
            VK_ABNT_C2, // 194
            VK_ACCEPT,  // 30
            VK_ADD,     // 107
            VK_APPS,    // 93
            VK_ATTN,
            VK_B,
            VK_BACK,
            VK_BROWSER_BACK,
            VK_BROWSER_FAVORITES,
            VK_BROWSER_FORWARD,
            VK_BROWSER_HOME,
            VK_BROWSER_REFRESH,
            VK_BROWSER_SEARCH,
            VK_BROWSER_STOP,
            VK_C,
            VK_CANCEL,
            VK_CAPITAL,
            VK_CLEAR,
            VK_CONTROL,
            VK_CONVERT,
            VK_CRSEL,
            VK_D,
            VK_DBE_ALPHANUMERIC,
            VK_DBE_CODEINPUT,
            VK_DBE_DBCSCHAR,
            VK_DBE_DETERMINESTRING,
            VK_DBE_ENTERDLGCONVERSIONMODE,
            VK_DBE_ENTERIMECONFIGMODE,
            VK_DBE_ENTERWORDREGISTERMODE,
            VK_DBE_FLUSHSTRING,
            VK_DBE_HIRAGANA,
            VK_DBE_KATAKANA,
            VK_DBE_NOCODEINPUT,
            VK_DBE_NOROMAN,
            VK_DBE_ROMAN,
            VK_DBE_SBCSCHAR,
            VK_DECIMAL,
            VK_E,
            VK_EREOF,
            VK_ESCAPE,
            VK_EXECUTE,
            VK_EXSEL,
            VK_F,
            VK_F1,
            VK_F10,
            VK_F11,
            VK_F12,
            VK_F13,
            VK_F14,
            VK_F15,
            VK_F16,
            VK_F17,
            VK_F18,
            VK_F19,
            VK_F2,
            VK_F20,
            VK_F21,
            VK_F22,
            VK_F23,
            VK_F24,
            VK_F3,
            VK_F4,
            VK_F5,
            VK_F6,
            VK_F7,
            VK_F8,
            VK_F9,
            VK_FINAL,
            VK_G,
            VK_GAMEPAD_A,
            VK_GAMEPAD_B,
            VK_GAMEPAD_DPAD_DOWN,
            VK_GAMEPAD_DPAD_LEFT,
            VK_GAMEPAD_DPAD_RIGHT,
            VK_GAMEPAD_DPAD_UP,
            VK_GAMEPAD_LEFT_SHOULDER,
            VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON,
            VK_GAMEPAD_LEFT_THUMBSTICK_DOWN,
            VK_GAMEPAD_LEFT_THUMBSTICK_LEFT,
            VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT,
            VK_GAMEPAD_LEFT_THUMBSTICK_UP,
            VK_GAMEPAD_LEFT_TRIGGER,
            VK_GAMEPAD_MENU,
            VK_GAMEPAD_RIGHT_SHOULDER,
            VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON,
            VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN,
            VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT,
            VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT,
            VK_GAMEPAD_RIGHT_THUMBSTICK_UP,
            VK_GAMEPAD_RIGHT_TRIGGER,
            VK_GAMEPAD_VIEW,
            VK_GAMEPAD_X,
            VK_GAMEPAD_Y,
            VK_H,
            VK_HANGEUL,
            VK_HANGUL,
            VK_HANJA,
            VK_HELP,
            VK_I,
            VK_ICO_00,
            VK_ICO_CLEAR,
            VK_ICO_HELP,
            VK_IME_OFF,
            VK_IME_ON,
            VK_J,
            VK_JUNJA,
            VK_K,
            VK_KANA,
            VK_KANJI,
            VK_L,
            VK_LAUNCH_APP1,
            VK_LAUNCH_APP2,
            VK_LAUNCH_MAIL,
            VK_LAUNCH_MEDIA_SELECT,
            VK_LBUTTON,
            VK_LCONTROL,
            VK_LMENU,
            VK_LSHIFT,
            VK_LWIN,
            VK_M,
            VK_MBUTTON,
            VK_MEDIA_NEXT_TRACK,
            VK_MEDIA_PLAY_PAUSE,
            VK_MEDIA_PREV_TRACK,
            VK_MEDIA_STOP,
            VK_MENU,
            VK_MODECHANGE,
            VK_MULTIPLY,
            VK_N,
            VK_NAVIGATION_ACCEPT,
            VK_NAVIGATION_CANCEL,
            VK_NAVIGATION_DOWN,
            VK_NAVIGATION_LEFT,
            VK_NAVIGATION_MENU,
            VK_NAVIGATION_RIGHT,
            VK_NAVIGATION_UP,
            VK_NAVIGATION_VIEW,
            VK_NONAME,
            VK_NONCONVERT,
            VK_NUMPAD0,
            VK_NUMPAD1,
            VK_NUMPAD2,
            VK_NUMPAD3,
            VK_NUMPAD4,
            VK_NUMPAD5,
            VK_NUMPAD6,
            VK_NUMPAD7,
            VK_NUMPAD8,
            VK_NUMPAD9,
            VK_O,
            VK_OEM_1,
            VK_OEM_102,
            VK_OEM_2,
            VK_OEM_3,
            VK_OEM_4,
            VK_OEM_5,
            VK_OEM_6,
            VK_OEM_7,
            VK_OEM_8,
            VK_OEM_ATTN,
            VK_OEM_AUTO,
            VK_OEM_AX,
            VK_OEM_BACKTAB,
            VK_OEM_CLEAR,
            VK_OEM_COMMA,
            VK_OEM_COPY,
            VK_OEM_CUSEL,
            VK_OEM_ENLW,
            VK_OEM_FINISH,
            VK_OEM_FJ_JISHO,
            VK_OEM_FJ_LOYA,
            VK_OEM_FJ_MASSHOU,
            VK_OEM_FJ_ROYA,
            VK_OEM_FJ_TOUROKU,
            VK_OEM_JUMP,
            VK_OEM_MINUS,
            VK_OEM_NEC_EQUAL,
            VK_OEM_PA1,
            VK_OEM_PA2,
            VK_OEM_PA3,
            VK_OEM_PERIOD,
            VK_OEM_PLUS,
            VK_OEM_RESET,
            VK_OEM_WSCTRL,
            VK_P,
            VK_PA1,
            VK_PACKET,
            VK_PAUSE,
            VK_PLAY,
            VK_PRINT,
            VK_PROCESSKEY,
            VK_Q,
            VK_R,
            VK_RBUTTON,
            VK_RETURN,
            VK_RSHIFT,
            VK_RWIN,
            VK_S,
            VK_SCROLL,
            VK_SELECT,
            VK_SEPARATOR,
            VK_SHIFT,
            VK_SLEEP,
            VK_SPACE,
            VK_SUBTRACT,
            VK_T,
            VK_TAB,
            VK_U,
            VK_V,
            VK_VOLUME_DOWN,
            VK_VOLUME_MUTE,
            VK_VOLUME_UP,
            VK_W,
            VK_X,
            VK_XBUTTON1,
            VK_XBUTTON2,
            VK_Y,
            VK_Z,
            VK_ZOOM,
        ];

        for key in known_ordinary_keys {
            assert!(!super::Enigo::is_extended_key(key), "Failed for {key:#?}");
        }
    }
}
