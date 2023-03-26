use std::os::raw::{c_uint, c_void};
use std::{
    thread,
    time::{Duration, Instant},
};

use objc::runtime::Class;

use core_foundation::base::{CFRelease, OSStatus};
use core_foundation::string::{CFStringRef, UniChar};
use core_foundation_sys::data::{CFDataGetBytePtr, CFDataRef};
use core_graphics::display::{CGDisplay, CGPoint};
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton, EventField, KeyCode,
    ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};

#[allow(non_upper_case_globals)]
static kUCKeyTranslateDeadKeysBit: c_uint = 1 << 31;
const BUF_LEN: usize = 0;

// required for NSEvent
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[repr(C)]
#[derive(Clone, Copy)]
struct NSPoint {
    x: f64,
    y: f64,
}

pub type TISInputSourceRef = *mut c_void;
pub type UniCharCount = usize;

#[link(name = "Cocoa", kind = "framework")]
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    fn TISCopyCurrentKeyboardLayoutInputSource() -> TISInputSourceRef;

    #[allow(non_snake_case)]
    fn UCKeyTranslate(
        layout: *const u8,
        code: u16,
        key_action: u16,
        modifier_state: u32,
        keyboard_type: u32,
        key_translate_options: c_uint,
        dead_key_state: *mut u32,
        max_length: UniCharCount,
        actual_length: *mut UniCharCount,
        unicode_string: *mut [UniChar; BUF_LEN],
    ) -> OSStatus;

    #[allow(non_upper_case_globals)]
    pub static kTISPropertyUnicodeKeyLayoutData: CFStringRef;

    #[allow(non_snake_case)]
    pub fn TISGetInputSourceProperty(
        inputSource: TISInputSourceRef,
        propertyKey: CFStringRef,
    ) -> CFDataRef;

    pub fn LMGetKbdType() -> u32;
}

/// The main struct for handling the event emitting
pub struct Enigo {
    event_source: CGEventSource,
    display: CGDisplay,
    double_click_delay: Duration,
    // TODO: Use mem::variant_count::<MouseButton>() here instead of 7 once it is stabalized
    last_mouse_click: [(i64, Instant); 7], /* For each of the seven MouseButton variants, we
                                            * store the last time the button was clicked and
                                            * the nth click that was
                                            * This information is needed to
                                            * determine double clicks and handle cases where
                                            * another button is clicked while the other one has
                                            * not yet been released */
}

impl Default for Enigo {
    fn default() -> Self {
        let double_click_delay = Duration::from_secs(1);
        let double_click_delay_setting: f64 =
            unsafe { msg_send![class!(NSEvent), doubleClickInterval] }; // Returns the double click interval (https://developer.apple.com/documentation/appkit/nsevent/1528384-doubleclickinterval). This is a TimeInterval which is a f64 of the number of seconds
        let double_click_delay = double_click_delay.mul_f64(double_click_delay_setting);

        Enigo {
            // TODO(dustin): return error rather than panic here
            event_source: CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
                .expect("Failed creating event source"),
            display: CGDisplay::main(),
            double_click_delay,
            last_mouse_click: [(0, Instant::now()); 7],
        }
    }
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        let pressed = Self::pressed_buttons();

        let event_type = if pressed & 1 > 0 {
            CGEventType::LeftMouseDragged
        } else if pressed & 2 > 0 {
            CGEventType::RightMouseDragged
        } else {
            CGEventType::MouseMoved
        };

        let dest = CGPoint::new(x as f64, y as f64);
        let event = CGEvent::new_mouse_event(
            self.event_source.clone(),
            event_type,
            dest,
            CGMouseButton::Left,
        )
        .unwrap();
        event.post(CGEventTapLocation::HID);
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        let (current_x, current_y) = self.mouse_location();
        let new_x = current_x + x;
        let new_y = current_y + y;

        /*
                if new_x < 0
                    || new_x as usize > display_width
                    || new_y < 0
                    || new_y as usize > display_height
                {
                    return;
                }
        */
        self.mouse_move_to(new_x, new_y);
    }

    fn mouse_down(&mut self, button: MouseButton) {
        let (current_x, current_y) = self.mouse_location();
        let click_count = self.nth_button_press(button, true);
        let (button, event_type) = match button {
            MouseButton::Left => (CGMouseButton::Left, CGEventType::LeftMouseDown),
            MouseButton::Middle => (CGMouseButton::Center, CGEventType::OtherMouseDown),
            MouseButton::Right => (CGMouseButton::Right, CGEventType::RightMouseDown),
            MouseButton::ScrollUp => return self.mouse_scroll_x(-1),
            MouseButton::ScrollDown => return self.mouse_scroll_x(1),
            MouseButton::ScrollLeft => return self.mouse_scroll_y(-1),
            MouseButton::ScrollRight => return self.mouse_scroll_y(1),
        };
        let dest = CGPoint::new(current_x as f64, current_y as f64);
        let event =
            CGEvent::new_mouse_event(self.event_source.clone(), event_type, dest, button).unwrap();

        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
        event.post(CGEventTapLocation::HID);
    }

    fn mouse_up(&mut self, button: MouseButton) {
        let (current_x, current_y) = self.mouse_location();
        let click_count = self.nth_button_press(button, false);
        let (button, event_type) = match button {
            MouseButton::Left => (CGMouseButton::Left, CGEventType::LeftMouseUp),
            MouseButton::Middle => (CGMouseButton::Center, CGEventType::OtherMouseUp),
            MouseButton::Right => (CGMouseButton::Right, CGEventType::RightMouseUp),
            MouseButton::ScrollUp
            | MouseButton::ScrollDown
            | MouseButton::ScrollLeft
            | MouseButton::ScrollRight => {
                println!("On macOS the mouse_up function has no effect when called with one of the Scroll buttons");
                return;
            }
        };
        let dest = CGPoint::new(current_x as f64, current_y as f64);
        let event =
            CGEvent::new_mouse_event(self.event_source.clone(), event_type, dest, button).unwrap();

        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
        event.post(CGEventTapLocation::HID);
    }

    fn mouse_click(&mut self, button: MouseButton) {
        self.mouse_down(button);
        self.mouse_up(button);
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        let event = CGEvent::new_scroll_event(
            self.event_source.clone(),
            ScrollEventUnit::LINE,
            2,
            0,
            -length,
            0,
        )
        .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        let event = CGEvent::new_scroll_event(
            self.event_source.clone(),
            ScrollEventUnit::LINE,
            1,
            -length,
            0,
            0,
        )
        .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);
    }

    fn main_display_size(&self) -> (i32, i32) {
        (
            self.display.pixels_wide() as i32,
            self.display.pixels_high() as i32,
        )
    }

    fn mouse_location(&self) -> (i32, i32) {
        let ns_event = Class::get("NSEvent").unwrap();
        let pt: NSPoint = unsafe { msg_send![ns_event, mouseLocation] };
        let (x, y_inv) = (pt.x as i32, pt.y as i32);
        (x, self.display.pixels_high() as i32 - y_inv)
    }
}

// https://stackoverflow.
// com/questions/1918841/how-to-convert-ascii-character-to-cgkeycode

impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        // NOTE(dustin): This is a fix for issue https://github.com/enigo-rs/enigo/issues/68
        // The CGEventKeyboardSetUnicodeString function (used inside of
        // event.set_string(cluster)) truncates strings down to 20 characters
        let chars: Vec<char> = sequence.chars().collect();
        let mut string: String;
        for chunk in chars.chunks(20) {
            let event = CGEvent::new_keyboard_event(self.event_source.clone(), 0, true)
                .expect("Failed creating event");
            string = chunk.iter().collect();
            event.set_string(&string);
            event.post(CGEventTapLocation::HID);
        }
        thread::sleep(Duration::from_millis(2));
    }

    fn key_click(&mut self, key: Key) {
        let keycode = self.key_to_keycode(key);
        thread::sleep(Duration::from_millis(20));
        let event = CGEvent::new_keyboard_event(self.event_source.clone(), keycode, true)
            .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);

        thread::sleep(Duration::from_millis(20));
        let event = CGEvent::new_keyboard_event(self.event_source.clone(), keycode, false)
            .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);
    }

    fn key_down(&mut self, key: Key) {
        thread::sleep(Duration::from_millis(20));
        let event =
            CGEvent::new_keyboard_event(self.event_source.clone(), self.key_to_keycode(key), true)
                .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);
    }

    fn key_up(&mut self, key: Key) {
        thread::sleep(Duration::from_millis(20));
        let event =
            CGEvent::new_keyboard_event(self.event_source.clone(), self.key_to_keycode(key), false)
                .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);
    }
}

impl Enigo {
    fn pressed_buttons() -> usize {
        let ns_event = Class::get("NSEvent").unwrap();
        unsafe { msg_send![ns_event, pressedMouseButtons] }
    }

    // On macOS, we have to determine ourselves if it was a double click of a mouse
    // button. The Enigo struct stores the information needed to do so. This
    // function checks if the button was pressed down again fast enough to issue a
    // double (or nth) click and returns the nth click it was. It also takes care of
    // updating the information the Enigo struct stores.
    fn nth_button_press(&mut self, button: MouseButton, press: bool) -> i64 {
        if press {
            let last_time = self.last_mouse_click[button as usize].1;
            self.last_mouse_click[button as usize].1 = Instant::now();

            if last_time.elapsed() < self.double_click_delay {
                self.last_mouse_click[button as usize].0 += 1;
            } else {
                self.last_mouse_click[button as usize].0 = 1;
            }
        }
        self.last_mouse_click[button as usize].0
    }
    fn key_to_keycode(&self, key: Key) -> CGKeyCode {
        // I mean duh, we still need to support deprecated keys until they're removed
        match key {
            Key::Alt | Key::Option => KeyCode::OPTION,
            Key::Backspace => KeyCode::DELETE,
            Key::CapsLock => KeyCode::CAPS_LOCK,
            Key::Control | Key::LControl => KeyCode::CONTROL,
            Key::Delete => KeyCode::FORWARD_DELETE,
            Key::DownArrow => KeyCode::DOWN_ARROW,
            Key::End => KeyCode::END,
            Key::Escape => KeyCode::ESCAPE,
            Key::F1 => KeyCode::F1,
            Key::F2 => KeyCode::F2,
            Key::F3 => KeyCode::F3,
            Key::F4 => KeyCode::F4,
            Key::F5 => KeyCode::F5,
            Key::F6 => KeyCode::F6,
            Key::F7 => KeyCode::F7,
            Key::F8 => KeyCode::F8,
            Key::F9 => KeyCode::F9,
            Key::F10 => KeyCode::F10,
            Key::F11 => KeyCode::F11,
            Key::F12 => KeyCode::F12,
            Key::F13 => KeyCode::F13,
            Key::F14 => KeyCode::F14,
            Key::F15 => KeyCode::F15,
            Key::F16 => KeyCode::F16,
            Key::F17 => KeyCode::F17,
            Key::F18 => KeyCode::F18,
            Key::F19 => KeyCode::F19,
            Key::F20 => KeyCode::F20,
            Key::Function => KeyCode::FUNCTION,
            Key::Help => KeyCode::HELP,
            Key::Home => KeyCode::HOME,
            Key::Launchpad => 160,
            Key::LeftArrow => KeyCode::LEFT_ARROW,
            Key::MissionControl => 131,
            Key::PageDown => KeyCode::PAGE_DOWN,
            Key::PageUp => KeyCode::PAGE_UP,
            Key::RCommand => KeyCode::RIGHT_COMMAND,
            Key::RControl => KeyCode::RIGHT_CONTROL,
            Key::Return => KeyCode::RETURN,
            Key::RightArrow => KeyCode::RIGHT_ARROW,
            Key::RShift => KeyCode::RIGHT_SHIFT,
            Key::ROption => KeyCode::RIGHT_OPTION,
            Key::Shift | Key::LShift => KeyCode::SHIFT,
            Key::Space => KeyCode::SPACE,
            Key::Tab => KeyCode::TAB,
            Key::UpArrow => KeyCode::UP_ARROW,
            Key::VolumeDown => KeyCode::VOLUME_DOWN,
            Key::VolumeUp => KeyCode::VOLUME_UP,
            Key::VolumeMute => KeyCode::MUTE,
            Key::Raw(raw_keycode) => raw_keycode,
            Key::Layout(c) => self.get_layoutdependent_keycode(&c.to_string()),
            Key::Super | Key::Command | Key::Windows | Key::Meta => KeyCode::COMMAND,
        }
    }

    fn get_layoutdependent_keycode(&self, string: &str) -> CGKeyCode {
        let mut pressed_keycode = 0;

        // loop through every keycode (0 - 127)
        for keycode in 0..128 {
            // no modifier
            if let Ok(key_string) = self.create_string_for_key(keycode, 0x100) {
                // println!("{:?}", string);
                if string == key_string {
                    pressed_keycode = keycode;
                }
            }

            // shift modifier
            if let Ok(key_string) = self.create_string_for_key(keycode, 0x20102) {
                // println!("{:?}", string);
                if string == key_string {
                    pressed_keycode = keycode;
                }
            }

            // alt modifier
            // if let Ok(string) = self.create_string_for_key(keycode,
            // 0x80120) {     println!("{:?}", string);
            // }
            // alt + shift modifier
            // if let Ok(string) = self.create_string_for_key(keycode,
            // 0xa0122) {     println!("{:?}", string);
            // }
        }

        pressed_keycode
    }

    #[allow(clippy::unused_self)]
    fn create_string_for_key(
        &self,
        keycode: u16,
        modifier: u32,
    ) -> Result<String, std::string::FromUtf16Error> {
        let mut current_keyboard = unsafe { TISCopyCurrentKeyboardInputSource() };
        let mut layout_data = unsafe {
            TISGetInputSourceProperty(current_keyboard, kTISPropertyUnicodeKeyLayoutData)
        };
        if layout_data.is_null() {
            // TISGetInputSourceProperty returns null with some keyboard layout.
            // Using TISCopyCurrentKeyboardLayoutInputSource to fix NULL return.
            // See also: https://github.com/microsoft/node-native-keymap/blob/089d802efd387df4dce1f0e31898c66e28b3f67f/src/keyboard_mac.mm#L90
            current_keyboard = unsafe { TISCopyCurrentKeyboardLayoutInputSource() };
            layout_data = unsafe {
                TISGetInputSourceProperty(current_keyboard, kTISPropertyUnicodeKeyLayoutData)
            };
            debug_assert!(!layout_data.is_null());
        }
        let keyboard_layout = unsafe { CFDataGetBytePtr(layout_data) };

        let mut keys_down: u32 = 0;
        // let mut chars: *mut c_void;//[UniChar; 4];
        let mut chars: [u16; 0] = [0; 0];
        let mut real_length: UniCharCount = 0;
        unsafe {
            UCKeyTranslate(
                keyboard_layout,
                keycode,
                3, // kUCKeyActionDisplay = 3
                modifier,
                LMGetKbdType(),
                kUCKeyTranslateDeadKeysBit,
                &mut keys_down,
                8, // sizeof(chars) / sizeof(chars[0]),
                &mut real_length,
                &mut chars,
            );
        }
        unsafe { CFRelease(current_keyboard) };

        String::from_utf16(&chars)
    }
}
