extern crate winapi;
extern crate user32;


use self::user32::*;
use self::winapi::*;

use ::{KeyboardControllable, MouseControllable, Key};
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

    // TODO(dustin): use button parameter, current implementation
    // is using the left mouse button every time
    fn mouse_down(&mut self, button: u32) {
        unsafe {
            let mut input = INPUT {
                type_: INPUT_MOUSE,
                u: transmute_copy(&MOUSEINPUT {
                                      dx: 0,
                                      dy: 0,
                                      mouseData: 0,
                                      dwFlags: MOUSEEVENTF_LEFTDOWN,
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }
    }

    // TODO(dustin): use button parameter, current implementation
    // is using the left mouse button every time
    fn mouse_up(&mut self, button: u32) {
        unsafe {
            let mut input = INPUT {
                type_: INPUT_MOUSE,
                u: transmute_copy(&MOUSEINPUT {
                                      dx: 0,
                                      dy: 0,
                                      mouseData: 0,
                                      dwFlags: MOUSEEVENTF_LEFTUP,
                                      time: 0,
                                      dwExtraInfo: 0,
                                  }),
            };

            SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int);
        }
    }

    fn mouse_click(&mut self, button: u32) {
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
        //unimplemented!();
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
        //unimplemented!();
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
        //ones provided by keycodes.re that are prefixed
        //with an 'E' infront of the original name
        match key {
            Key::TAB => EVK_TAB,
            Key::RETURN => EVK_RETURN,
            Key::SHIFT => EVK_SHIFT,
            Key::CONTROL => EVK_CONTROL,
            Key::A => EVK_A,
            _ => 0,
        }
    }
}
