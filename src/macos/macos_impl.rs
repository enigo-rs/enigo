use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_ulong, c_ushort, c_void};
use std::{
    thread,
    time::{Duration, Instant},
};

use core_graphics::display::{CFIndex, CGDisplay, CGPoint};
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton, EventField, KeyCode,
    ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use objc::runtime::Class;

use crate::{
    Axis, Coordinate, Direction, Key, KeyboardControllableNext, MouseButton, MouseControllableNext,
};

// required for NSEvent
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

pub type CFDataRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
pub struct __TISInputSource;
pub type TISInputSourceRef = *const __TISInputSource;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __CFString([u8; 0]);
pub type CFStringRef = *const __CFString;
pub type Boolean = c_uchar;
pub type UInt8 = c_uchar;
pub type SInt32 = c_int;
pub type UInt16 = c_ushort;
pub type UInt32 = c_uint;
pub type UniChar = UInt16;
pub type UniCharCount = c_ulong;

pub type OptionBits = UInt32;
pub type OSStatus = SInt32;

pub type CFStringEncoding = UInt32;

pub const TRUE: c_uint = 1;

#[allow(non_snake_case)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UCKeyboardTypeHeader {
    pub keyboardTypeFirst: UInt32,
    pub keyboardTypeLast: UInt32,
    pub keyModifiersToTableNumOffset: UInt32,
    pub keyToCharTableIndexOffset: UInt32,
    pub keyStateRecordsIndexOffset: UInt32,
    pub keyStateTerminatorsOffset: UInt32,
    pub keySequenceDataIndexOffset: UInt32,
}

#[allow(non_snake_case)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UCKeyboardLayout {
    pub keyLayoutHeaderFormat: UInt16,
    pub keyLayoutDataVersion: UInt16,
    pub keyLayoutFeatureInfoOffset: UInt32,
    pub keyboardTypeCount: UInt32,
    pub keyboardTypeList: [UCKeyboardTypeHeader; 1usize],
}

#[allow(non_upper_case_globals)]
pub const kUCKeyTranslateNoDeadKeysBit: _bindgen_ty_703 =
    _bindgen_ty_703::kUCKeyTranslateNoDeadKeysBit;

#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum _bindgen_ty_703 {
    kUCKeyTranslateNoDeadKeysBit = 0,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __CFAllocator([u8; 0]);
pub type CFAllocatorRef = *const __CFAllocator;

// #[repr(u32)]
// #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
// pub enum _bindgen_ty_15 {
//     kCFStringEncodingMacRoman = 0,
//     kCFStringEncodingWindowsLatin1 = 1280,
//     kCFStringEncodingISOLatin1 = 513,
//     kCFStringEncodingNextStepLatin = 2817,
//     kCFStringEncodingASCII = 1536,
//     kCFStringEncodingUnicode = 256,
//     kCFStringEncodingUTF8 = 134217984,
//     kCFStringEncodingNonLossyASCII = 3071,
//     kCFStringEncodingUTF16BE = 268435712,
//     kCFStringEncodingUTF16LE = 335544576,
//     kCFStringEncodingUTF32 = 201326848,
//     kCFStringEncodingUTF32BE = 402653440,
//     kCFStringEncodingUTF32LE = 469762304,
// }

#[allow(non_upper_case_globals)]
pub const kCFStringEncodingUTF8: u32 = 134_217_984;

#[allow(improper_ctypes)]
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    fn TISCopyCurrentKeyboardLayoutInputSource() -> TISInputSourceRef;

    //     extern void *
    // TISGetInputSourceProperty(
    //   TISInputSourceRef   inputSource,
    //   CFStringRef         propertyKey)

    #[allow(non_upper_case_globals)]
    #[link_name = "kTISPropertyUnicodeKeyLayoutData"]
    pub static kTISPropertyUnicodeKeyLayoutData: CFStringRef;

    #[allow(non_snake_case)]
    pub fn TISGetInputSourceProperty(
        inputSource: TISInputSourceRef,
        propertyKey: CFStringRef,
    ) -> *mut c_void;

    #[allow(non_snake_case)]
    pub fn CFDataGetBytePtr(theData: CFDataRef) -> *const UInt8;

    #[allow(non_snake_case)]
    pub fn UCKeyTranslate(
        keyLayoutPtr: *const UInt8, //*const UCKeyboardLayout,
        virtualKeyCode: UInt16,
        keyAction: UInt16,
        modifierKeyState: UInt32,
        keyboardType: UInt32,
        keyTranslateOptions: OptionBits,
        deadKeyState: *mut UInt32,
        maxStringLength: UniCharCount,
        actualStringLength: *mut UniCharCount,
        unicodeString: *mut UniChar,
    ) -> OSStatus;

    pub fn LMGetKbdType() -> UInt8;

    #[allow(non_snake_case)]
    pub fn CFStringCreateWithCharacters(
        alloc: CFAllocatorRef,
        chars: *const UniChar,
        numChars: CFIndex,
    ) -> CFStringRef;

    #[allow(non_upper_case_globals)]
    #[link_name = "kCFAllocatorDefault"]
    pub static kCFAllocatorDefault: CFAllocatorRef;

    #[allow(non_snake_case)]
    pub fn CFStringGetLength(theString: CFStringRef) -> CFIndex;

    #[allow(non_snake_case)]
    pub fn CFStringGetCString(
        theString: CFStringRef,
        buffer: *mut c_char,
        bufferSize: CFIndex,
        encoding: CFStringEncoding,
    ) -> Boolean;
}

/// The main struct for handling the event emitting
pub struct Enigo {
    event_source: CGEventSource,
    display: CGDisplay,
    held: Vec<Key>, // Currently held keys
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
        let held = Vec::new();

        let double_click_delay = Duration::from_secs(1);
        let double_click_delay_setting: f64 =
            unsafe { msg_send![class!(NSEvent), doubleClickInterval] }; // Returns the double click interval (https://developer.apple.com/documentation/appkit/nsevent/1528384-doubleclickinterval). This is a TimeInterval which is a f64 of the number of seconds
        let double_click_delay = double_click_delay.mul_f64(double_click_delay_setting);

        Enigo {
            // TODO(dustin): return error rather than panic here
            event_source: CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
                .expect("Failed creating event source"),
            display: CGDisplay::main(),
            held,
            double_click_delay,
            last_mouse_click: [(0, Instant::now()); 7],
        }
    }
}

impl MouseControllableNext for Enigo {
    // Sends a button event to the X11 server via `XTest` extension
    fn send_mouse_button_event(&mut self, button: MouseButton, direction: Direction, delay: u32) {
        let (current_x, current_y) = self.mouse_loc();

        if direction == Direction::Click || direction == Direction::Press {
            let click_count = self.nth_button_press(button, Direction::Press);
            let (button, event_type) = match button {
                MouseButton::Left => (CGMouseButton::Left, CGEventType::LeftMouseDown),
                MouseButton::Middle => (CGMouseButton::Center, CGEventType::OtherMouseDown),
                MouseButton::Right => (CGMouseButton::Right, CGEventType::RightMouseDown),
                MouseButton::ScrollUp => return self.mouse_scroll_event(-1, Axis::Vertical),
                MouseButton::ScrollDown => return self.mouse_scroll_event(1, Axis::Vertical),
                MouseButton::ScrollLeft => return self.mouse_scroll_event(-1, Axis::Horizontal),
                MouseButton::ScrollRight => return self.mouse_scroll_event(1, Axis::Horizontal),
            };
            let dest = CGPoint::new(current_x as f64, current_y as f64);
            let event =
                CGEvent::new_mouse_event(self.event_source.clone(), event_type, dest, button)
                    .unwrap();

            event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
            event.post(CGEventTapLocation::HID);
        }
        if direction == Direction::Click || direction == Direction::Release {
            let click_count = self.nth_button_press(button, Direction::Press);
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
                CGEvent::new_mouse_event(self.event_source.clone(), event_type, dest, button)
                    .unwrap();

            event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
            event.post(CGEventTapLocation::HID);
        }
    }

    // Sends a motion notify event to the X11 server via `XTest` extension
    // TODO: Check if using x11rb::protocol::xproto::warp_pointer would be better
    fn send_motion_notify_event(&mut self, x: i32, y: i32, coordinate: Coordinate) {
        let (x_absolute, y_absolute) = if coordinate == Coordinate::Relative {
            let (current_x, current_y) = self.mouse_loc();
            (current_x + x, current_y + y)
        } else {
            (x, y)
        };

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

    // Sends a scroll event to the X11 server via `XTest` extension
    fn mouse_scroll_event(&mut self, length: i32, axis: Axis) {
        let (ax, len_x, len_y) = match axis {
            Axis::Horizontal => (2, 0, -length),
            Axis::Vertical => (1, -length, 0),
        };

        let event = CGEvent::new_scroll_event(
            self.event_source.clone(),
            ScrollEventUnit::LINE,
            ax,
            len_x,
            len_y,
            0,
        )
        .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);
    }

    fn main_display(&self) -> (i32, i32) {
        (
            self.display.pixels_wide() as i32,
            self.display.pixels_high() as i32,
        )
    }

    fn mouse_loc(&self) -> (i32, i32) {
        let ns_event = Class::get("NSEvent").unwrap();
        let pt: NSPoint = unsafe { msg_send![ns_event, mouseLocation] };
        let (x, y_inv) = (pt.x as i32, pt.y as i32);
        (x, self.display.pixels_high() as i32 - y_inv)
    }
}

// https://stackoverflow.com/questions/1918841/how-to-convert-ascii-character-to-cgkeycode
impl KeyboardControllableNext for Enigo {
    fn fast_text_entry(&mut self, _text: &str) -> Option<()> {
        None
    }
    /// Enter the text
    /// Use a fast method to enter the text, if it is available
    fn enter_text(&mut self, text: &str) {
        // NOTE(dustin): This is a fix for issue https://github.com/enigo-rs/enigo/issues/68
        // The CGEventKeyboardSetUnicodeString function (used inside of
        // event.set_string(cluster)) truncates strings down to 20 characters
        let chars: Vec<char> = text.chars().collect();
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

    /// Sends a key event to the X11 server via `XTest` extension
    fn enter_key(&mut self, key: Key, direction: Direction) {
        // Nothing to do
        if key == Key::Layout('\0') {
            return;
        }
        match direction {
            Direction::Press => self.held.push(key),
            Direction::Release => self.held.retain(|&k| k != key),
            Direction::Click => (),
        }

        let keycode = self.key_to_keycode(key);

        if direction == Direction::Click || direction == Direction::Press {
            thread::sleep(Duration::from_millis(20));
            let event = CGEvent::new_keyboard_event(self.event_source.clone(), keycode, true)
                .expect("Failed creating event");
            event.post(CGEventTapLocation::HID);
        }

        if direction == Direction::Click || direction == Direction::Release {
            thread::sleep(Duration::from_millis(20));
            let event = CGEvent::new_keyboard_event(self.event_source.clone(), keycode, false)
                .expect("Failed creating event");
            event.post(CGEventTapLocation::HID);
        }
    }
}

impl Enigo {
    /// Returns a list of all currently pressed keys
    pub fn held(&mut self) -> Vec<Key> {
        self.held.clone()
    }

    fn pressed_buttons() -> usize {
        let ns_event = Class::get("NSEvent").unwrap();
        unsafe { msg_send![ns_event, pressedMouseButtons] }
    }

    // On macOS, we have to determine ourselves if it was a double click of a mouse
    // button. The Enigo struct stores the information needed to do so. This
    // function checks if the button was pressed down again fast enough to issue a
    // double (or nth) click and returns the nth click it was. It also takes care of
    // updating the information the Enigo struct stores.
    fn nth_button_press(&mut self, button: MouseButton, direction: Direction) -> i64 {
        if direction == Direction::Press {
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
        // A list of names is available at:
        // https://docs.rs/core-graphics/latest/core_graphics/event/struct.KeyCode.html
        // https://github.com/phracker/MacOSX-SDKs/blob/master/MacOSX10.13.sdk/System/Library/Frameworks/Carbon.framework/Versions/A/Frameworks/HIToolbox.framework/Versions/A/Headers/Events.h
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
            if let Some(key_string) = self.keycode_to_string(keycode, 0x100) {
                // println!("{:?}", string);
                if string == key_string {
                    pressed_keycode = keycode;
                }
            }

            // shift modifier
            if let Some(key_string) = self.keycode_to_string(keycode, 0x20102) {
                // println!("{:?}", string);
                if string == key_string {
                    pressed_keycode = keycode;
                }
            }

            // alt modifier
            // if let Some(string) = self.keycode_to_string(keycode, 0x80120) {
            //     println!("{:?}", string);
            // }
            // alt + shift modifier
            // if let Some(string) = self.keycode_to_string(keycode, 0xa0122) {
            //     println!("{:?}", string);
            // }
        }

        pressed_keycode
    }

    fn keycode_to_string(&self, keycode: u16, modifier: u32) -> Option<String> {
        let cf_string = self.create_string_for_key(keycode, modifier);
        let buffer_size = unsafe { CFStringGetLength(cf_string) + 1 };
        let mut buffer: i8 = std::i8::MAX;
        let success = unsafe {
            CFStringGetCString(cf_string, &mut buffer, buffer_size, kCFStringEncodingUTF8)
        };
        if success == TRUE as u8 {
            let rust_string = String::from_utf8(vec![buffer as u8]).unwrap();
            return Some(rust_string);
        }

        None
    }

    #[allow(clippy::unused_self)]
    fn create_string_for_key(&self, keycode: u16, modifier: u32) -> CFStringRef {
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

        let mut keys_down: UInt32 = 0;
        // let mut chars: *mut c_void;//[UniChar; 4];
        let mut chars: u16 = 0;
        let mut real_length: UniCharCount = 0;
        unsafe {
            UCKeyTranslate(
                keyboard_layout,
                keycode,
                3, // kUCKeyActionDisplay = 3
                modifier,
                LMGetKbdType() as u32,
                kUCKeyTranslateNoDeadKeysBit as u32,
                &mut keys_down,
                8, // sizeof(chars) / sizeof(chars[0]),
                &mut real_length,
                &mut chars,
            );
        }

        unsafe { CFStringCreateWithCharacters(kCFAllocatorDefault, &chars, 1) }
    }
}

impl Drop for Enigo {
    // Release the held keys before the connection is dropped
    fn drop(&mut self) {
        for &k in &self.held() {
            self.enter_key(k, Direction::Release);
        }
    }
}
