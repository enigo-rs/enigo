extern crate core_graphics;
extern crate libc;

// TODO(dustin): use only the things i need

use self::core_graphics::display::*;
use self::core_graphics::event::*;
use self::core_graphics::event_source::*;
use self::core_graphics::geometry::*;
use self::libc::*;

use ::{KeyboardControllable, Key, MouseControllable, MouseButton};
use macos::keycodes::*;
use std::mem;
use self::libc::{c_void};

use std::ptr;

// little hack until servo fixed a bug in core_graphics
// https://github.com/servo/core-graphics-rs/issues/70
// https://github.com/servo/core-graphics-rs/pull/71
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventCreateMouseEvent(source: CGEventSourceRef,
                               mouseType: FIXMEEventType,
                               mouseCursorPosition: CGPoint,
                               mouseButton: CGMouseButton)
                               -> CGEventRef;

    fn CGEventPost(tapLocation: CGEventTapLocation, event: CGEventRef);

    fn CGEventCreateKeyboardEvent(source: CGEventSourceRef, 
                                  keycode: CGKeyCode, 
                                  keydown: bool) -> CGEventRef;

    // not present in servo/core-graphics
    fn CGEventCreateScrollWheelEvent(source: CGEventSourceRef,
                                     units: ScrollUnit,
                                     wheelCount: uint32_t,
                                     wheel1: int32_t,
                                     ...)
                                     -> CGEventRef;
}

pub type CFDataRef = *const ::std::os::raw::c_void;//c_void;

#[repr(C)]
pub struct __TISInputSource;
pub type TISInputSourceRef = *const __TISInputSource;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __CFString([u8; 0]);
pub type CFStringRef = *const __CFString;
pub type Boolean = ::std::os::raw::c_uchar;
pub type UInt8 = ::std::os::raw::c_uchar;
pub type SInt32 = ::std::os::raw::c_int;
pub type UInt16 = ::std::os::raw::c_ushort;
pub type UInt32 = ::std::os::raw::c_uint;
pub type UniChar = UInt16;
pub type UniCharCount = ::std::os::raw::c_ulong;

pub type OptionBits = UInt32;
pub type OSStatus = SInt32;


pub type CFStringEncoding = UInt32;

pub const TRUE: ::std::os::raw::c_uint = 1;

pub const kUCKeyActionDisplay: _bindgen_ty_702 =
    _bindgen_ty_702::kUCKeyActionDisplay;
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum _bindgen_ty_702 {
    kUCKeyActionDown = 0,
    kUCKeyActionUp = 1,
    kUCKeyActionAutoKey = 2,
    kUCKeyActionDisplay = 3,
}

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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UCKeyboardLayout {
    pub keyLayoutHeaderFormat: UInt16,
    pub keyLayoutDataVersion: UInt16,
    pub keyLayoutFeatureInfoOffset: UInt32,
    pub keyboardTypeCount: UInt32,
    pub keyboardTypeList: [UCKeyboardTypeHeader; 1usize],
}

pub const kUCKeyTranslateNoDeadKeysBit: _bindgen_ty_703 =
    _bindgen_ty_703::kUCKeyTranslateNoDeadKeysBit;
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum _bindgen_ty_703 { kUCKeyTranslateNoDeadKeysBit = 0, }

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

pub const kCFStringEncodingUTF8: u32 = 134217984;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;

//     extern void * 
// TISGetInputSourceProperty(
//   TISInputSourceRef   inputSource,
//   CFStringRef         propertyKey)

    #[link_name = "kTISPropertyUnicodeKeyLayoutData"]
    pub static kTISPropertyUnicodeKeyLayoutData: CFStringRef;

    pub fn TISGetInputSourceProperty(inputSource: TISInputSourceRef,
                                     propertyKey: CFStringRef)
     -> *mut ::std::os::raw::c_void;


    pub fn CFDataGetBytePtr(theData: CFDataRef) -> *const UInt8;

    pub fn UCKeyTranslate(keyLayoutPtr: *const UInt8,//*const UCKeyboardLayout,
                          virtualKeyCode: UInt16, keyAction: UInt16,
                          modifierKeyState: UInt32, keyboardType: UInt32,
                          keyTranslateOptions: OptionBits,
                          deadKeyState: *mut UInt32,
                          maxStringLength: UniCharCount,
                          actualStringLength: *mut UniCharCount,
                          unicodeString: *mut UniChar) -> OSStatus;

    pub fn LMGetKbdType() -> UInt8;

    pub fn CFStringCreateWithCharacters(alloc: CFAllocatorRef,
                                    chars: *const UniChar,
                                    numChars: CFIndex) -> CFStringRef;

    #[link_name = "kCFAllocatorDefault"]
    pub static kCFAllocatorDefault: CFAllocatorRef;   

    pub fn CFStringGetLength(theString: CFStringRef) -> CFIndex;   

    pub fn CFStringGetCString(theString: CFStringRef,
                              buffer: *mut ::std::os::raw::c_char,
                              bufferSize: CFIndex, encoding: CFStringEncoding)
     -> Boolean;                              
}


#[derive(Debug)]
enum FIXMEEventType {
    LeftMouseDown = 1,
    LeftMouseUp = 2,
    MouseMoved = 5,
}

// not present in servo/core-graphics
#[derive(Debug)]
enum ScrollUnit {
    Pixel = 0,
    Line = 1,
}
// hack

/// The main struct for handling the event emitting
pub struct Enigo {
    current_x: i32,
    current_y: i32,
    display_width: usize,
    display_height: usize,
}

impl Enigo {
    /// Constructs a new `Enigo` instance.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// ```
    pub fn new() -> Self {
        let displayID = unsafe { CGMainDisplayID() };
        let width = unsafe { CGDisplayPixelsWide(displayID) };
        let height = unsafe { CGDisplayPixelsHigh(displayID) };

        Enigo {
            current_x: 500,
            current_y: 500,
            display_width: width,
            display_height: height,
        }
    }
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        self.current_x = x;
        self.current_y = y;

        unsafe {
            let mouse_ev = CGEventCreateMouseEvent(ptr::null(),
                                                   FIXMEEventType::MouseMoved,
                                                   CGPoint::new(self.current_x as f64,
                                                                self.current_y as f64),
                                                   CGMouseButton::Left);

            CGEventPost(CGEventTapLocation::HID, mouse_ev);
            CFRelease(mem::transmute(mouse_ev));
        }
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        let new_x = self.current_x + x;
        let new_y = self.current_y + y;

        if new_x < 0 || new_x as usize > self.display_width || new_y < 0 ||
           new_y as usize > self.display_height {
            return;
        }

        unsafe {
            let mouse_ev = CGEventCreateMouseEvent(ptr::null(),
                                                   FIXMEEventType::MouseMoved,
                                                   CGPoint::new(new_x as f64, new_y as f64),
                                                   CGMouseButton::Left);

            CGEventPost(CGEventTapLocation::HID, mouse_ev);
            CFRelease(mem::transmute(mouse_ev));
        }

        // TODO(dustin): use interior mutability
        self.current_x = new_x;
        self.current_y = new_y;
    }

    // TODO(dustin): use button parameter, current implementation
    // is using the left mouse button every time
    fn mouse_down(&mut self, button: MouseButton) {
        unsafe {
            let mouse_ev = CGEventCreateMouseEvent(ptr::null(),
                                                   FIXMEEventType::LeftMouseDown,
                                                   CGPoint::new(self.current_x as f64,
                                                                self.current_y as f64),
                                                   match button {
                                                       MouseButton::Left => CGMouseButton::Left,
                                                       MouseButton::Middle => CGMouseButton::Center,
                                                       MouseButton::Right => CGMouseButton::Right,

                                                       _ => unimplemented!(),
                                                   });

            CGEventPost(CGEventTapLocation::HID, mouse_ev);
            CFRelease(mem::transmute(mouse_ev));
        }
    }

    // TODO(dustin): use button parameter, current implementation
    // is using the left mouse button every time
    fn mouse_up(&mut self, button: MouseButton) {
        unsafe {
            let mouse_ev = CGEventCreateMouseEvent(ptr::null(),
                                                   FIXMEEventType::LeftMouseUp,
                                                   CGPoint::new(self.current_x as f64,
                                                                self.current_y as f64),
                                                   match button {
                                                       MouseButton::Left => CGMouseButton::Left,
                                                       MouseButton::Middle => CGMouseButton::Center,
                                                       MouseButton::Right => CGMouseButton::Right,

                                                       _ => unimplemented!(),
                                                   });

            CGEventPost(CGEventTapLocation::HID, mouse_ev);
            CFRelease(mem::transmute(mouse_ev));
        }
    }

    fn mouse_click(&mut self, button: MouseButton) {
        self.mouse_down(button);
        self.mouse_up(button);
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
                let mouse_ev = CGEventCreateScrollWheelEvent(ptr::null(),
                                                             ScrollUnit::Line,
                                                             2, // CGWheelCount 1 = y 2 = xy 3 = xyz
                                                             0,
                                                             scroll_direction);

                CGEventPost(CGEventTapLocation::HID, mouse_ev);
                CFRelease(mem::transmute(mouse_ev));
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
                let mouse_ev = CGEventCreateScrollWheelEvent(ptr::null(),
                                                             ScrollUnit::Line,
                                                             1, // CGWheelCount 1 = y 2 = xy 3 = xyz
                                                             scroll_direction);

                CGEventPost(CGEventTapLocation::HID, mouse_ev);
                CFRelease(mem::transmute(mouse_ev));
            }
        }
    }
}

//https://stackoverflow.com/questions/1918841/how-to-convert-ascii-character-to-cgkeycode

impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        //TODO(dustin): return error rather than panic here
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).expect("Failed creating event source");
        let event = CGEvent::new_keyboard_event(source, 0, true).expect("Failed creating event");
        event.set_string(sequence);
        event.post(CGEventTapLocation::HID);
    }

    fn key_click(&mut self, key: Key) {
        unsafe {
            let keycode = self.key_to_keycode(key);

            use std::{thread, time};
            thread::sleep(time::Duration::from_millis(20));
            //TODO(dustin): return error rather than panic here
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).expect("Failed creating event source");
            let event = CGEvent::new_keyboard_event(source, keycode, true).expect("Failed creating event");
            event.post(CGEventTapLocation::HID);

            thread::sleep(time::Duration::from_millis(20));
            //TODO(dustin): return error rather than panic here
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).expect("Failed creating event source");
            let event = CGEvent::new_keyboard_event(source, keycode, false).expect("Failed creating event");
            event.post(CGEventTapLocation::HID);
        }
    }

    fn key_down(&mut self, key: Key) {
        unsafe {
            use std::{thread, time};
            thread::sleep(time::Duration::from_millis(20));
            //TODO(dustin): return error rather than panic here
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).expect("Failed creating event source");
            let event = CGEvent::new_keyboard_event(source, self.key_to_keycode(key), true).expect("Failed creating event");
            event.post(CGEventTapLocation::HID);
        }
    }

    fn key_up(&mut self, key: Key) {
        unsafe {
            use std::{thread, time};
            thread::sleep(time::Duration::from_millis(20));
            //TODO(dustin): return error rather than panic here
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).expect("Failed creating event source");
            let event = CGEvent::new_keyboard_event(source, self.key_to_keycode(key), false).expect("Failed creating event");
            event.post(CGEventTapLocation::HID);
        }
    }
}

impl Enigo {
    fn key_to_keycode(&self, key: Key) -> CGKeyCode {
        match key {
            Key::Return => kVK_Return,
            Key::Tab => kVK_Tab,
            Key::Space => kVK_Space,
            Key::Backspace => kVK_Delete,
            Key::Escape => kVK_Escape,
            Key::Super => kVK_Command,
            Key::Command => kVK_Command,
            Key::Windows => kVK_Command,
            Key::Shift => kVK_Shift,
            Key::CapsLock => kVK_CapsLock,
            Key::Alt => kVK_Option,
            Key::Option => kVK_Option,
            Key::Control => kVK_Control,
            Key::Home => kVK_Home,
            Key::PageUp => kVK_PageUp,
            Key::PageDown => kVK_PageDown,
            Key::LeftArrow => kVK_LeftArrow,
            Key::RightArrow => kVK_RightArrow,
            Key::DownArrow => kVK_DownArrow,
            Key::UpArrow => kVK_UpArrow,
            Key::F1 => kVK_F1,
            Key::F2 => kVK_F2,
            Key::F3 => kVK_F3,
            Key::F4 => kVK_F4,
            Key::F5 => kVK_F5,
            Key::F6 => kVK_F6,
            Key::F7 => kVK_F7,
            Key::F8 => kVK_F8,
            Key::F9 => kVK_F9,
            Key::F10 => kVK_F10,
            Key::F11 => kVK_F11,
            Key::F12 => kVK_F12,
            Key::Raw(raw_keycode) => raw_keycode,
            Key::Layout(string) => self.get_layoutdependent_keycode(string), 
            _ => 0,
        }
    }

    fn get_layoutdependent_keycode(&self, string: String) -> CGKeyCode {
        let mut pressed_keycode = 0;

        //loop through every keycode (0 - 127) 
        for keycode in 0..128 {

            // no modifier
            if let Some(key_string) = self.keycode_to_string(keycode, 0x100) {
                //println!("{:?}", string);
                if string == key_string {
                    pressed_keycode = keycode;
                }
            }

            //shift modifier
            if let Some(key_string) = self.keycode_to_string(keycode, 0x20102) {
                //println!("{:?}", string);
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
        let bufferSize = unsafe { CFStringGetLength(cf_string) + 1 };
            let mut buffer:i8 = 0xffff; 
            let success = unsafe { CFStringGetCString(  cf_string, 
                                                        &mut buffer, 
                                                        bufferSize, 
                                                        kCFStringEncodingUTF8) };
            if success == TRUE as u8 {
                let rust_string = String::from_utf8(vec![buffer as u8]).unwrap();
                return Some(rust_string);
            }

            None
    }

    fn create_string_for_key(&self, keycode: u16, modifier: u32) -> CFStringRef {

        let currentKeyboard = unsafe { TISCopyCurrentKeyboardInputSource() };
        let layoutData = unsafe { TISGetInputSourceProperty(currentKeyboard, kTISPropertyUnicodeKeyLayoutData) }; 
        let keyboardLayout = unsafe { CFDataGetBytePtr(layoutData) };

        let mut keysDown: UInt32 = 0;
        //let mut chars: *mut ::std::os::raw::c_void;//[UniChar; 4];
        let mut chars: u16 = 0;
        let mut realLength: UniCharCount = 0;
        unsafe {
            UCKeyTranslate(keyboardLayout,
                keycode,
                kUCKeyActionDisplay as u16,
                modifier,
                LMGetKbdType() as u32,
                kUCKeyTranslateNoDeadKeysBit as u32,
                &mut keysDown,
                8,//sizeof(chars) / sizeof(chars[0]),
                &mut realLength,
                &mut chars);
        }
        
        let stringRef = unsafe { CFStringCreateWithCharacters(kCFAllocatorDefault, &mut chars, 1) };

        stringRef
    }
}