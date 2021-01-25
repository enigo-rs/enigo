use core_graphics;

// TODO(dustin): use only the things i need

use core_graphics::display::*;
use core_graphics::event::*;
use core_graphics::event_source::*;

use crate::macos::keycodes::*;
use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};
use objc::runtime::Class;
use std::os::raw::*;

// required for pressedMouseButtons on NSEvent
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

struct MyCGEvent;

#[allow(improper_ctypes)]
#[allow(non_snake_case)]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventPost(tapLocation: CGEventTapLocation, event: *mut MyCGEvent);
    // not present in servo/core-graphics
    fn CGEventCreateScrollWheelEvent(
        source: &CGEventSourceRef,
        units: ScrollUnit,
        wheelCount: u32,
        wheel1: i32,
        ...
    ) -> *mut MyCGEvent;
}

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

#[allow(non_upper_case_globals)]
pub const kUCKeyActionDisplay: _bindgen_ty_702 = _bindgen_ty_702::kUCKeyActionDisplay;

#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum _bindgen_ty_702 {
    // kUCKeyActionDown = 0,
    // kUCKeyActionUp = 1,
    // kUCKeyActionAutoKey = 2,
    kUCKeyActionDisplay = 3,
}

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

// not present in servo/core-graphics
#[allow(dead_code)]
#[derive(Debug)]
enum ScrollUnit {
    Pixel = 0,
    Line = 1,
}
// hack

/// The main struct for handling the event emitting
pub struct Enigo {
    event_source: CGEventSource,
}

impl Default for Enigo {
    fn default() -> Self {
        Enigo {
            // TODO(dustin): return error rather than panic here
            event_source: CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
                .expect("Failed creating event source"),
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
        let (display_width, display_height) = Self::main_display_size();
        let (current_x, y_inv) = Self::mouse_location_raw_coords();
        let current_y = (display_height as i32) - y_inv;
        let new_x = current_x + x;
        let new_y = current_y + y;

        if new_x < 0
            || new_x as usize > display_width
            || new_y < 0
            || new_y as usize > display_height
        {
            return;
        }

        self.mouse_move_to(new_x, new_y);
    }

    fn mouse_down(&mut self, button: MouseButton) {
        self.mouse_nth_down(button, 1);
    }

    fn mouse_up(&mut self, button: MouseButton) {
        self.mouse_nth_up(button, 1);
    }

    fn mouse_click(&mut self, button: MouseButton) {
        self.mouse_nth_click(button, 1);
    }

    fn mouse_nth_click(&mut self, button: MouseButton, click_count: u32) {
        self.mouse_nth_down(button, click_count);
        self.mouse_nth_up(button, click_count);
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        let mut scroll_direction = -1; // 1 left -1 right;
        let mut length = length;

        if length < 0 {
            length *= -1;
            scroll_direction *= -1;
        }

        for _ in 0..length {
            unsafe {
                let mouse_ev = CGEventCreateScrollWheelEvent(
                    &self.event_source,
                    ScrollUnit::Line,
                    2, // CGWheelCount 1 = y 2 = xy 3 = xyz
                    0,
                    scroll_direction,
                );

                CGEventPost(CGEventTapLocation::HID, mouse_ev);
                CFRelease(mouse_ev as *const std::ffi::c_void);
            }
        }
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        let mut scroll_direction = -1; // 1 left -1 right;
        let mut length = length;

        if length < 0 {
            length *= -1;
            scroll_direction *= -1;
        }

        for _ in 0..length {
            unsafe {
                let mouse_ev = CGEventCreateScrollWheelEvent(
                    &self.event_source,
                    ScrollUnit::Line,
                    1, // CGWheelCount 1 = y 2 = xy 3 = xyz
                    scroll_direction,
                );

                CGEventPost(CGEventTapLocation::HID, mouse_ev);
                CFRelease(mouse_ev as *const std::ffi::c_void);
            }
        }
    }
}

// https://stackoverflow.
// com/questions/1918841/how-to-convert-ascii-character-to-cgkeycode

impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        // NOTE(dustin): This is a fix for issue https://github.com/enigo-rs/enigo/issues/68
        // TODO(dustin): This could be improved by aggregating 20 bytes worth of graphemes at a time
        // but i am unsure what would happen for grapheme clusters greater than 20 bytes ...
        use unicode_segmentation::UnicodeSegmentation;
        let clusters = UnicodeSegmentation::graphemes(sequence, true).collect::<Vec<&str>>();
        for cluster in clusters {
            let event = CGEvent::new_keyboard_event(self.event_source.clone(), 0, true)
                .expect("Failed creating event");
            event.set_string(cluster);
            event.post(CGEventTapLocation::HID);
        }
    }

    fn key_click(&mut self, key: Key) {
        let keycode = self.key_to_keycode(key);

        use std::{thread, time};
        thread::sleep(time::Duration::from_millis(20));
        let event = CGEvent::new_keyboard_event(self.event_source.clone(), keycode, true)
            .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);

        thread::sleep(time::Duration::from_millis(20));
        let event = CGEvent::new_keyboard_event(self.event_source.clone(), keycode, false)
            .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);
    }

    fn key_down(&mut self, key: Key) {
        use std::{thread, time};
        thread::sleep(time::Duration::from_millis(20));
        let event =
            CGEvent::new_keyboard_event(self.event_source.clone(), self.key_to_keycode(key), true)
                .expect("Failed creating event");
        event.post(CGEventTapLocation::HID);
    }

    fn key_up(&mut self, key: Key) {
        use std::{thread, time};
        thread::sleep(time::Duration::from_millis(20));
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

    /// Fetches the `(width, height)` in pixels of the main display
    pub fn main_display_size() -> (usize, usize) {
        let display_id = unsafe { CGMainDisplayID() };
        let width = unsafe { CGDisplayPixelsWide(display_id) };
        let height = unsafe { CGDisplayPixelsHigh(display_id) };
        (width, height)
    }

    /// Returns the current mouse location in Cocoa coordinates which have Y
    /// inverted from the Carbon coordinates used in the rest of the API.
    /// This function exists so that mouse_move_relative only has to fetch
    /// the screen size once.
    fn mouse_location_raw_coords() -> (i32, i32) {
        let ns_event = Class::get("NSEvent").unwrap();
        let pt: NSPoint = unsafe { msg_send![ns_event, mouseLocation] };
        (pt.x as i32, pt.y as i32)
    }

    /// The mouse coordinates in points, only works on the main display
    pub fn mouse_location() -> (i32, i32) {
        let (x, y_inv) = Self::mouse_location_raw_coords();
        let (_, display_height) = Self::main_display_size();
        (x, (display_height as i32) - y_inv)
    }

    fn key_to_keycode(&self, key: Key) -> CGKeyCode {
        #[allow(deprecated)]
        // I mean duh, we still need to support deprecated keys until they're removed
        match key {
            Key::Alt => kVK_Option,
            Key::Backspace => kVK_Delete,
            Key::CapsLock => kVK_CapsLock,
            Key::Control => kVK_Control,
            Key::Delete => kVK_ForwardDelete,
            Key::DownArrow => kVK_DownArrow,
            Key::End => kVK_End,
            Key::Escape => kVK_Escape,
            Key::F1 => kVK_F1,
            Key::F10 => kVK_F10,
            Key::F11 => kVK_F11,
            Key::F12 => kVK_F12,
            Key::F2 => kVK_F2,
            Key::F3 => kVK_F3,
            Key::F4 => kVK_F4,
            Key::F5 => kVK_F5,
            Key::F6 => kVK_F6,
            Key::F7 => kVK_F7,
            Key::F8 => kVK_F8,
            Key::F9 => kVK_F9,
            Key::Home => kVK_Home,
            Key::LeftArrow => kVK_LeftArrow,
            Key::Option => kVK_Option,
            Key::PageDown => kVK_PageDown,
            Key::PageUp => kVK_PageUp,
            Key::Return => kVK_Return,
            Key::RightArrow => kVK_RightArrow,
            Key::Shift => kVK_Shift,
            Key::Space => kVK_Space,
            Key::Tab => kVK_Tab,
            Key::UpArrow => kVK_UpArrow,
            Key::Raw(raw_keycode) => raw_keycode,
            Key::Layout(c) => self.get_layoutdependent_keycode(c.to_string()),

            Key::Super | Key::Command | Key::Windows | Key::Meta => kVK_Command,
        }
    }

    fn get_layoutdependent_keycode(&self, string: String) -> CGKeyCode {
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

    fn create_string_for_key(&self, keycode: u16, modifier: u32) -> CFStringRef {
        let current_keyboard = unsafe { TISCopyCurrentKeyboardInputSource() };
        let layout_data = unsafe {
            TISGetInputSourceProperty(current_keyboard, kTISPropertyUnicodeKeyLayoutData)
        };
        let keyboard_layout = unsafe { CFDataGetBytePtr(layout_data) };

        let mut keys_down: UInt32 = 0;
        // let mut chars: *mut c_void;//[UniChar; 4];
        let mut chars: u16 = 0;
        let mut real_length: UniCharCount = 0;
        unsafe {
            UCKeyTranslate(
                keyboard_layout,
                keycode,
                kUCKeyActionDisplay as u16,
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

    fn mouse_nth_down(&mut self, button: MouseButton, click_count: u32) {
        let (current_x, current_y) = Self::mouse_location();
        let (button, event_type) = match button {
            MouseButton::Left => (CGMouseButton::Left, CGEventType::LeftMouseDown),
            MouseButton::Middle => (CGMouseButton::Center, CGEventType::OtherMouseDown),
            MouseButton::Right => (CGMouseButton::Right, CGEventType::RightMouseDown),
            _ => unimplemented!(),
        };
        let dest = CGPoint::new(current_x as f64, current_y as f64);
        let event =
            CGEvent::new_mouse_event(self.event_source.clone(), event_type, dest, button).unwrap();
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count as i64);
        event.post(CGEventTapLocation::HID);
    }

    fn mouse_nth_up(&mut self, button: MouseButton, click_count: u32) {
        let (current_x, current_y) = Self::mouse_location();
        let (button, event_type) = match button {
            MouseButton::Left => (CGMouseButton::Left, CGEventType::LeftMouseUp),
            MouseButton::Middle => (CGMouseButton::Center, CGEventType::OtherMouseUp),
            MouseButton::Right => (CGMouseButton::Right, CGEventType::RightMouseUp),
            _ => unimplemented!(),
        };
        let dest = CGPoint::new(current_x as f64, current_y as f64);
        let event =
            CGEvent::new_mouse_event(self.event_source.clone(), event_type, dest, button).unwrap();
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count as i64);
        event.post(CGEventTapLocation::HID);
    }
}
