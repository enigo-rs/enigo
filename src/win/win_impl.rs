extern crate winapi;
extern crate user32;


use self::user32::*;
use self::winapi::*;

use ::{KeyboardControllable, Key, MouseControllable, MouseButton};
use win::keycodes::*;
use std::mem::*;

/// The main struct for handling the event emitting
pub struct Enigo {
    current_x: i32,
    current_y: i32,
}

impl Enigo {
    // TODO(dustin): do the right initialisation

    /// Constructs a new `Enigo` instance.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// ```
    pub fn new() -> Self {
        Enigo {
            current_x: 0,
            current_y: 0,
        }
    }
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        // TODO(dustin): use interior mutability
        self.current_x = x;
        self.current_y = y;
        unsafe { SetCursorPos(self.current_x, self.current_y) };
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        // TODO(dustin): use interior mutability
        self.current_x += x;
        self.current_y += y;
        unsafe { SetCursorPos(self.current_x, self.current_y) };
    }

    fn mouse_down(&mut self, button: MouseButton) {
        unsafe {
            let mut input = INPUT {
                type_: INPUT_MOUSE,
                u: transmute_copy(&MOUSEINPUT {
                                      dx: 0,
                                      dy: 0,
                                      mouseData: 0,
                                      dwFlags: match button {
                                          MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
                                          MouseButton::Middle => MOUSEEVENTF_MIDDLEDOWN,
                                          MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,

                                          _ => unimplemented!(),
                                      },
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }
    }

    fn mouse_up(&mut self, button: MouseButton) {
        unsafe {
            let mut input = INPUT {
                type_: INPUT_MOUSE,
                u: transmute_copy(&MOUSEINPUT {
                                      dx: 0,
                                      dy: 0,
                                      mouseData: 0,
                                      dwFlags: match button {
                                          MouseButton::Left => MOUSEEVENTF_LEFTUP,
                                          MouseButton::Middle => MOUSEEVENTF_MIDDLEUP,
                                          MouseButton::Right => MOUSEEVENTF_RIGHTUP,

                                          _ => unimplemented!(),
                                      },
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }
    }

    fn mouse_click(&mut self, button: MouseButton) {
        self.mouse_down(button);
        self.mouse_up(button);
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        let mut scroll_direction = 1 * 50; // 1 left -1 right
        let mut length = length;

        if length < 0 {
            length *= -1;
            scroll_direction *= -1;
        }

        for _ in 0..length {
            unsafe {
                let mut input = INPUT {
                    type_: INPUT_MOUSE,
                    u: transmute_copy(&MOUSEINPUT {
                                          dx: 0,
                                          dy: 0,
                                          mouseData: transmute_copy(&scroll_direction),
                                          dwFlags: MOUSEEVENTF_HWHEEL,
                                          time: 0,
                                          dwExtraInfo: 0,
                                      }),
                };

                SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
            }
        }
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        let mut scroll_direction = -1 * 50; // 1 left -1 right
        let mut length = length;

        if length < 0 {
            length *= -1;
            scroll_direction *= -1;
        }

        for _ in 0..length {
            unsafe {
                let mut input = INPUT {
                    type_: INPUT_MOUSE,
                    u: transmute_copy(&MOUSEINPUT {
                                          dx: 0,
                                          dy: 0,
                                          mouseData: transmute_copy(&scroll_direction),
                                          dwFlags: MOUSEEVENTF_WHEEL,
                                          time: 0,
                                          dwExtraInfo: 0,
                                      }),
                };

                SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
            }
        }
    }
}

impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        let mut buffer = [0; 2];

        for c in sequence.chars() {
            //Windows uses uft-16 encoding. We need to check
            //for variable length characters. As such some
            //characters can be 32 bit long and those are
            //encoded in such called hight and low surrogates
            //each 16 bit wide that needs to be send after
            //another to the SendInput function without
            //being interrupted by "keyup"
            let result = c.encode_utf16(&mut buffer);
            if result.len() == 1 {
                self.unicode_key_click(result[0]);
            } else {
                for utf16_surrogate in result {
                    self.unicode_key_down(utf16_surrogate.clone());
                }
                //do i need to produce a keyup?
                //self.unicode_key_up(0);
            }
        }
    }

    fn key_click(&mut self, key: Key) {
        let keycode = self.key_to_keycode(key);

        use std::{thread, time};
        thread::sleep(time::Duration::from_millis(20));

        unsafe {
            let mut input = INPUT {
                type_: INPUT_KEYBOARD,
                u: transmute_copy(&KEYBDINPUT {
                                      wVk: keycode,
                                      wScan: 0,
                                      dwFlags: 0,
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }

        thread::sleep(time::Duration::from_millis(20));

        unsafe {
            let mut input = INPUT {
                type_: INPUT_KEYBOARD,
                u: transmute_copy(&KEYBDINPUT {
                                      wVk: keycode,
                                      wScan: 0,
                                      dwFlags: KEYEVENTF_KEYUP,
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }

        thread::sleep(time::Duration::from_millis(20));
    }

    fn key_down(&mut self, key: Key) {
        unsafe {
            let mut input = INPUT {
                type_: INPUT_KEYBOARD,
                u: transmute_copy(&KEYBDINPUT {
                                      wVk: self.key_to_keycode(key),
                                      wScan: 0,
                                      dwFlags: 0,
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }
    }

    fn key_up(&mut self, key: Key) {
        unsafe {
            let mut input = INPUT {
                type_: INPUT_KEYBOARD,
                u: transmute_copy(&KEYBDINPUT {
                                      wVk: self.key_to_keycode(key),
                                      wScan: 0,
                                      dwFlags: KEYEVENTF_KEYUP,
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }
    }
}

impl Enigo {
    fn unicode_key_click(&self, unicode_char: u16) {
        use std::{thread, time};
        thread::sleep(time::Duration::from_millis(20));
        self.unicode_key_down(unicode_char);
        self.unicode_key_up(unicode_char);
        thread::sleep(time::Duration::from_millis(20));
    }

    fn unicode_key_down(&self, unicode_char: u16) {
        unsafe {
            let mut input = INPUT {
                type_: INPUT_KEYBOARD,
                u: transmute_copy(&KEYBDINPUT {
                                      wVk: 0,
                                      wScan: unicode_char,
                                      dwFlags: KEYEVENTF_UNICODE,
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }
    }

    fn unicode_key_up(&self, unicode_char: u16) {
        unsafe {
            let mut input = INPUT {
                type_: INPUT_KEYBOARD,
                u: transmute_copy(&KEYBDINPUT {
                                      wVk: 0,
                                      wScan: unicode_char,
                                      dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }
    }

    fn key_to_keycode(&self, key: Key) -> u16 {
        //do not use the codes from crate winapi they're 
        //wrongly typed with i32 instead of i16 use the
        //ones provided by win/keycodes.rs that are prefixed
        //with an 'E' infront of the original name
        match key {
            Key::Tab => EVK_TAB,
            Key::Return => EVK_RETURN,
            Key::Shift => EVK_SHIFT,
            Key::Control => EVK_CONTROL,
            //Key::A => EVK_A,
            Key::Raw(raw_keycode) => raw_keycode,
            key::Layout(string) => self.get_layoutdependent_keycode(string),
            _ => 0,
        }
    }

    fn get_layoutdependent_keycode(&self, string: String) -> u16 {
        //TODO(dustin): implement this method
        0x41 as u16 //key that has the letter 'a' on it on english like keylayout
    }
}
