use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_ulong, c_ushort, c_void};
use std::{
    thread,
    time::{Duration, Instant},
};

use core_graphics::{
    display::{CFIndex, CGDisplay, CGPoint},
    event::{
        CGEvent, CGEventRef, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton, EventField,
        KeyCode, ScrollEventUnit,
    },
    event_source::{CGEventSource, CGEventSourceStateID},
};
use foreign_types_shared::ForeignTypeRef as _;
use icrate::{AppKit, AppKit::NSEvent, Foundation::NSPoint};
use log::{debug, error, info};
use objc2::msg_send;

use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse,
    NewConError, Settings,
};

type CFDataRef = *const c_void;

#[repr(C)]
struct __TISInputSource;
type TISInputSourceRef = *const __TISInputSource;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct __CFString([u8; 0]);
type CFStringRef = *const __CFString;
type Boolean = c_uchar;
type UInt8 = c_uchar;
type SInt32 = c_int;
type UInt16 = c_ushort;
type UInt32 = c_uint;
type UniChar = UInt16;
type UniCharCount = c_ulong;

type OptionBits = UInt32;
type OSStatus = SInt32;

type CFStringEncoding = UInt32;

const TRUE: c_uint = 1;

#[allow(non_snake_case)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct UCKeyboardTypeHeader {
    keyboardTypeFirst: UInt32,
    keyboardTypeLast: UInt32,
    keyModifiersToTableNumOffset: UInt32,
    keyToCharTableIndexOffset: UInt32,
    keyStateRecordsIndexOffset: UInt32,
    keyStateTerminatorsOffset: UInt32,
    keySequenceDataIndexOffset: UInt32,
}

#[allow(non_snake_case)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct UCKeyboardLayout {
    keyLayoutHeaderFormat: UInt16,
    keyLayoutDataVersion: UInt16,
    keyLayoutFeatureInfoOffset: UInt32,
    keyboardTypeCount: UInt32,
    keyboardTypeList: [UCKeyboardTypeHeader; 1usize],
}

#[allow(non_upper_case_globals)]
const kUCKeyTranslateNoDeadKeysBit: u32 = 0;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct __CFAllocator([u8; 0]);
type CFAllocatorRef = *const __CFAllocator;

#[allow(non_upper_case_globals)]
const kCFStringEncodingUTF8: u32 = 0x0800_0100;

#[allow(improper_ctypes)]
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    fn TISCopyCurrentKeyboardLayoutInputSource() -> TISInputSourceRef;

    #[allow(non_upper_case_globals)]
    static kTISPropertyUnicodeKeyLayoutData: CFStringRef;

    #[allow(non_snake_case)]
    fn TISGetInputSourceProperty(
        inputSource: TISInputSourceRef,
        propertyKey: CFStringRef,
    ) -> *mut c_void;

    #[allow(non_snake_case)]
    fn CFDataGetBytePtr(theData: CFDataRef) -> *const UInt8;

    #[allow(non_snake_case)]
    fn UCKeyTranslate(
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

    fn LMGetKbdType() -> UInt8;

    #[allow(non_snake_case)]
    fn CFStringCreateWithCharacters(
        alloc: CFAllocatorRef,
        chars: *const UniChar,
        numChars: CFIndex,
    ) -> CFStringRef;

    #[allow(non_upper_case_globals)]
    static kCFAllocatorDefault: CFAllocatorRef;

    #[allow(non_snake_case)]
    fn CFStringGetLength(theString: CFStringRef) -> CFIndex;

    #[allow(non_snake_case)]
    fn CFStringGetCString(
        theString: CFStringRef,
        buffer: *mut c_char,
        bufferSize: CFIndex,
        encoding: CFStringEncoding,
    ) -> Boolean;
}

/// The main struct for handling the event emitting
pub struct Enigo {
    delay: u64,
    event_source: CGEventSource,
    display: CGDisplay,
    held: (Vec<Key>, Vec<CGKeyCode>), // Currently held keys
    event_source_user_data: i64,
    release_keys_when_dropped: bool,
    double_click_delay: Duration,
    // TODO: Use mem::variant_count::<Button>() here instead of 7 once it is stabilized
    last_mouse_click: [(i64, Instant); 7], /* For each of the seven Button variants, we
                                            * store the last time the button was clicked and
                                            * the nth click that was
                                            * This information is needed to
                                            * determine double clicks and handle cases where
                                            * another button is clicked while the other one has
                                            * not yet been released */
}

impl Mouse for Enigo {
    // Sends a button event to the X11 server via `XTest` extension
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mbutton(button: {button:?}, direction: {direction:?})\x1b[0m");
        let (current_x, current_y) = self.location()?;

        if direction == Direction::Click || direction == Direction::Press {
            let click_count = self.nth_button_press(button, Direction::Press);
            let (button, event_type) = match button {
                Button::Left => (CGMouseButton::Left, CGEventType::LeftMouseDown),
                Button::Middle => (CGMouseButton::Center, CGEventType::OtherMouseDown),
                Button::Right => (CGMouseButton::Right, CGEventType::RightMouseDown),
                Button::ScrollUp => return self.scroll(-1, Axis::Vertical),
                Button::ScrollDown => return self.scroll(1, Axis::Vertical),
                Button::ScrollLeft => return self.scroll(-1, Axis::Horizontal),
                Button::ScrollRight => return self.scroll(1, Axis::Horizontal),
            };
            let dest = CGPoint::new(current_x as f64, current_y as f64);

            let Ok(event) =
                CGEvent::new_mouse_event(self.event_source.clone(), event_type, dest, button)
            else {
                return Err(InputError::Simulate(
                    "failed creating event to enter mouse button",
                ));
            };
            event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
            event.set_integer_value_field(
                EventField::EVENT_SOURCE_USER_DATA,
                self.event_source_user_data,
            );
            event.post(CGEventTapLocation::HID);
        }
        if direction == Direction::Click || direction == Direction::Release {
            let click_count = self.nth_button_press(button, Direction::Release);
            let (button, event_type) = match button {
                Button::Left => (CGMouseButton::Left, CGEventType::LeftMouseUp),
                Button::Middle => (CGMouseButton::Center, CGEventType::OtherMouseUp),
                Button::Right => (CGMouseButton::Right, CGEventType::RightMouseUp),
                Button::ScrollUp
                | Button::ScrollDown
                | Button::ScrollLeft
                | Button::ScrollRight => {
                    info!("On macOS the mouse_up function has no effect when called with one of the Scroll buttons");
                    return Ok(());
                }
            };
            let dest = CGPoint::new(current_x as f64, current_y as f64);
            let Ok(event) =
                CGEvent::new_mouse_event(self.event_source.clone(), event_type, dest, button)
            else {
                return Err(InputError::Simulate(
                    "failed creating event to enter mouse button",
                ));
            };

            event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
            event.set_integer_value_field(
                EventField::EVENT_SOURCE_USER_DATA,
                self.event_source_user_data,
            );
            event.post(CGEventTapLocation::HID);
        }
        Ok(())
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        debug!("\x1b[93mmove_mouse(x: {x:?}, y: {y:?}, coordinate:{coordinate:?})\x1b[0m");
        let pressed = unsafe { AppKit::NSEvent::pressedMouseButtons() };
        let (current_x, current_y) = self.location()?;

        let (absolute, relative) = match coordinate {
            // TODO: Check the bounds
            Coordinate::Abs => ((x, y), (current_x - x, current_y - y)),
            Coordinate::Rel => ((current_x + x, current_y + y), (x, y)),
        };

        let (event_type, button) = if pressed & 1 > 0 {
            (CGEventType::LeftMouseDragged, CGMouseButton::Left)
        } else if pressed & 2 > 0 {
            (CGEventType::RightMouseDragged, CGMouseButton::Right)
        } else {
            (CGEventType::MouseMoved, CGMouseButton::Left) // The mouse button
                                                           // here is ignored so
                                                           // it can be anything
        };

        let dest = CGPoint::new(absolute.0 as f64, absolute.1 as f64);
        let Ok(event) =
            CGEvent::new_mouse_event(self.event_source.clone(), event_type, dest, button)
        else {
            return Err(InputError::Simulate(
                "failed creating event to move the mouse",
            ));
        };

        // Add information by how much the mouse was moved
        event.set_integer_value_field(
            core_graphics::event::EventField::MOUSE_EVENT_DELTA_X,
            relative.0.into(),
        );
        event.set_integer_value_field(
            core_graphics::event::EventField::MOUSE_EVENT_DELTA_Y,
            relative.1.into(),
        );

        event.set_integer_value_field(
            EventField::EVENT_SOURCE_USER_DATA,
            self.event_source_user_data,
        );
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    // Sends a scroll event to the X11 server via `XTest` extension
    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        debug!("\x1b[93mscroll(length: {length:?}, axis: {axis:?})\x1b[0m");
        let (ax, len_x, len_y) = match axis {
            Axis::Horizontal => (2, 0, -length),
            Axis::Vertical => (1, -length, 0),
        };

        let Ok(event) = CGEvent::new_scroll_event(
            self.event_source.clone(),
            ScrollEventUnit::LINE,
            ax,
            len_x,
            len_y,
            0,
        ) else {
            return Err(InputError::Simulate("failed creating event to scroll"));
        };

        event.set_integer_value_field(
            EventField::EVENT_SOURCE_USER_DATA,
            self.event_source_user_data,
        );
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mmain_display()\x1b[0m");
        Ok((
            self.display.pixels_wide() as i32,
            self.display.pixels_high() as i32,
        ))
    }

    fn location(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mlocation()\x1b[0m");
        let pt = unsafe { AppKit::NSEvent::mouseLocation() };
        let (x, y_inv) = (pt.x as i32, pt.y as i32);
        Ok((x, self.display.pixels_high() as i32 - y_inv))
    }
}

// https://stackoverflow.com/questions/1918841/how-to-convert-ascii-character-to-cgkeycode
impl Keyboard for Enigo {
    fn fast_text(&mut self, text: &str) -> InputResult<Option<()>> {
        // Fn to create an iterator over sub slices of a str that have the specified
        // length
        fn chunks(s: &str, len: usize) -> impl Iterator<Item = &str> {
            assert!(len > 0);
            let mut indices = s.char_indices().map(|(idx, _)| idx).peekable();

            std::iter::from_fn(move || {
                let start_idx = indices.next()?;
                for _ in 0..len - 1 {
                    indices.next();
                }
                let end_idx = match indices.peek() {
                    Some(idx) => *idx,
                    None => s.bytes().len(),
                };
                Some(&s[start_idx..end_idx])
            })
        }

        debug!("\x1b[93mfast_text(text: {text})\x1b[0m");
        // WORKAROUND: This is a fix for issue https://github.com/enigo-rs/enigo/issues/68
        // The CGEventKeyboardSetUnicodeString function (used inside of
        // event.set_string(chunk)) truncates strings down to 20 characters
        for mut chunk in chunks(text, 20) {
            let Ok(event) = CGEvent::new_keyboard_event(self.event_source.clone(), 0, true) else {
                return Err(InputError::Simulate(
                    "failed creating event to enter the text",
                ));
            };
            // WORKAROUND: This is a fix for issue https://github.com/enigo-rs/enigo/issues/260
            // This is needed to get rid of all leading line feed, tab and carriage return
            // characters. event.set_string(chunk)) silently fails if the chunk
            // starts with a newline character
            loop {
                if chunk.starts_with('\t') {
                    self.key(Key::Tab, Direction::Click)?;
                    chunk = &chunk[1..];
                    continue;
                }
                if chunk.starts_with('\r') {
                    self.text("\u{200B}\r")?;
                    chunk = &chunk[1..];
                    continue;
                }
                if chunk.starts_with('\n') {
                    self.text("\u{200B}\n")?;
                    chunk = &chunk[1..];
                    continue;
                }
                break;
            }

            event.set_string(chunk);
            event.set_integer_value_field(
                EventField::EVENT_SOURCE_USER_DATA,
                self.event_source_user_data,
            );
            event.post(CGEventTapLocation::HID);
        }
        thread::sleep(Duration::from_millis(2));
        Ok(Some(()))
    }

    #[allow(clippy::too_many_lines)]
    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mkey(key: {key:?}, direction: {direction:?})\x1b[0m");
        // Nothing to do
        if key == Key::Unicode('\0') {
            return Ok(());
        }
        match key {
            Key::VolumeUp => {
                debug!("special case for handling the VolumeUp key");
                self.special_keys(0, direction)?;
            }
            Key::VolumeDown => {
                debug!("special case for handling the VolumeDown key");
                self.special_keys(1, direction)?;
            }
            Key::BrightnessUp => {
                debug!("special case for handling the BrightnessUp key");
                self.special_keys(2, direction)?;
            }
            Key::BrightnessDown => {
                debug!("special case for handling the BrightnessDown key");
                self.special_keys(3, direction)?;
            }
            Key::Power => {
                debug!("special case for handling the VolumeMute key");
                self.special_keys(6, direction)?;
            }
            Key::VolumeMute => {
                debug!("special case for handling the VolumeMute key");
                self.special_keys(7, direction)?;
            }

            Key::ContrastUp => {
                debug!("special case for handling the VolumeUp key");
                self.special_keys(11, direction)?;
            }
            Key::ContrastDown => {
                debug!("special case for handling the VolumeDown key");
                self.special_keys(12, direction)?;
            }
            Key::LaunchPanel => {
                debug!("special case for handling the MediaPlayPause key");
                self.special_keys(13, direction)?;
            }
            Key::Eject => {
                debug!("special case for handling the MediaNextTrack key");
                self.special_keys(14, direction)?;
            }
            Key::VidMirror => {
                debug!("special case for handling the MediaPrevTrack key");
                self.special_keys(15, direction)?;
            }
            Key::MediaPlayPause => {
                debug!("special case for handling the MediaPlayPause key");
                self.special_keys(16, direction)?;
            }
            Key::MediaNextTrack => {
                debug!("special case for handling the MediaNextTrack key");
                self.special_keys(17, direction)?;
            }
            Key::MediaPrevTrack => {
                debug!("special case for handling the MediaPrevTrack key");
                self.special_keys(18, direction)?;
            }
            Key::MediaFast => {
                debug!("special case for handling the MediaNextTrack key");
                self.special_keys(19, direction)?;
            }
            Key::MediaRewind => {
                debug!("special case for handling the MediaPrevTrack key");
                self.special_keys(20, direction)?;
            }
            Key::IlluminationUp => {
                debug!("special case for handling the MediaPrevTrack key");
                self.special_keys(21, direction)?;
            }
            Key::IlluminationDown => {
                debug!("special case for handling the MediaNextTrack key");
                self.special_keys(22, direction)?;
            }
            Key::IlluminationToggle => {
                debug!("special case for handling the MediaPrevTrack key");
                self.special_keys(23, direction)?;
            }
            _ => {
                let Ok(keycode) = CGKeyCode::try_from(key) else {
                    return Err(InputError::InvalidInput(
                        "virtual keycodes on macOS have to fit into u16",
                    ));
                };
                self.raw(keycode, direction)?;
            }
        }

        // TODO: The list of keys will contain the key and also the associated keycode.
        // They are a duplicate
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

        if direction == Direction::Click || direction == Direction::Press {
            thread::sleep(Duration::from_millis(self.delay));
            let Ok(event) = CGEvent::new_keyboard_event(self.event_source.clone(), keycode, true)
            else {
                return Err(InputError::Simulate(
                    "failed creating event to press the key",
                ));
            };

            event.set_integer_value_field(
                EventField::EVENT_SOURCE_USER_DATA,
                self.event_source_user_data,
            );
            event.post(CGEventTapLocation::HID);
        }

        if direction == Direction::Click || direction == Direction::Release {
            thread::sleep(Duration::from_millis(self.delay));
            let Ok(event) = CGEvent::new_keyboard_event(self.event_source.clone(), keycode, false)
            else {
                return Err(InputError::Simulate(
                    "failed creating event to release the key",
                ));
            };

            event.set_integer_value_field(
                EventField::EVENT_SOURCE_USER_DATA,
                self.event_source_user_data,
            );
            event.post(CGEventTapLocation::HID);
        }

        match direction {
            Direction::Press => {
                debug!("added the keycode {keycode:?} to the held keys");
                self.held.1.push(keycode);
            }
            Direction::Release => {
                debug!("removed the keycode {keycode:?} from the held keys");
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
            mac_delay: delay,
            release_keys_when_dropped,
            event_source_user_data,
            ..
        } = settings;

        let held = (Vec::new(), Vec::new());

        let double_click_delay = Duration::from_secs(1);
        let double_click_delay_setting = unsafe { AppKit::NSEvent::doubleClickInterval() };
        // Returns the double click interval (https://developer.apple.com/documentation/appkit/nsevent/1528384-doubleclickinterval). This is a TimeInterval which is a f64 of the number of seconds
        let double_click_delay = double_click_delay.mul_f64(double_click_delay_setting);

        let Ok(event_source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        else {
            return Err(NewConError::EstablishCon("failed creating event source"));
        };

        debug!("\x1b[93mconnection established on macOS\x1b[0m");

        Ok(Enigo {
            delay: (*delay).into(),
            event_source,
            display: CGDisplay::main(),
            held,
            release_keys_when_dropped: *release_keys_when_dropped,
            double_click_delay,
            last_mouse_click: [(0, Instant::now()); 7],
            event_source_user_data: event_source_user_data.unwrap_or(crate::EVENT_MARKER as i64),
        })
    }

    /// Get the delay per keypress in milliseconds
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // It never panics
    pub fn delay(&self) -> u32 {
        self.delay.try_into().unwrap_or(u32::MAX)
    }

    /// Set the delay per keypress in milliseconds
    pub fn set_delay(&mut self, delay: u32) {
        self.delay = delay.into();
    }

    /// Returns a list of all currently pressed keys
    pub fn held(&mut self) -> (Vec<Key>, Vec<CGKeyCode>) {
        self.held.clone()
    }

    /// Returns the value that enigo's events are marked with
    #[must_use]
    pub fn get_marker_value(&self) -> i64 {
        self.event_source_user_data
    }

    // On macOS, we have to determine ourselves if it was a double click of a mouse
    // button. The Enigo struct stores the information needed to do so. This
    // function checks if the button was pressed down again fast enough to issue a
    // double (or nth) click and returns the nth click it was. It also takes care of
    // updating the information the Enigo struct stores.
    fn nth_button_press(&mut self, button: Button, direction: Direction) -> i64 {
        if direction == Direction::Press {
            let last_time = self.last_mouse_click[button as usize].1;
            self.last_mouse_click[button as usize].1 = Instant::now();

            if last_time.elapsed() < self.double_click_delay {
                self.last_mouse_click[button as usize].0 += 1;
            } else {
                self.last_mouse_click[button as usize].0 = 1;
            }
        }
        let nth_button_press = self.last_mouse_click[button as usize].0;
        debug!("nth_button_press: {nth_button_press}");
        nth_button_press
    }

    fn special_keys(&self, code: isize, direction: Direction) -> InputResult<()> {
        if direction == Direction::Press || direction == Direction::Click {
            let event = unsafe {
                AppKit::NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                AppKit::NSEventTypeSystemDefined, // 14
                NSPoint::ZERO,
                0xa00, // NSEventModifierFlagCapsLock and NSEventModifierFlagOption
                0.0,
                0,
                None,
                8,
                (code << 16) | (0xa << 8),
                -1
            )
            };

            if let Some(event) = event {
                let cg_event = unsafe { Self::ns_event_cg_event(&event).to_owned() };
                cg_event.set_integer_value_field(
                    EventField::EVENT_SOURCE_USER_DATA,
                    self.event_source_user_data,
                );
                cg_event.post(CGEventTapLocation::HID);
            } else {
                return Err(InputError::Simulate(
                    "failed creating event to press special key",
                ));
            }
        }

        if direction == Direction::Release || direction == Direction::Click {
            let event = unsafe {
                AppKit::NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                AppKit::NSEventTypeSystemDefined, // 14
                NSPoint::ZERO,
                0xb00, // NSEventModifierFlagCapsLock, NSEventModifierFlagOptionNSEventModifier and FlagShift
                0.0,
                0,
                None,
                8,
                (code << 16) | (0xb << 8),
                -1
            )
            };

            if let Some(event) = event {
                let cg_event = unsafe { Self::ns_event_cg_event(&event).to_owned() };
                cg_event.set_integer_value_field(
                    EventField::EVENT_SOURCE_USER_DATA,
                    self.event_source_user_data,
                );
                cg_event.post(CGEventTapLocation::HID);
            } else {
                return Err(InputError::Simulate(
                    "failed creating event to release special key",
                ));
            }
        }

        Ok(())
    }

    unsafe fn ns_event_cg_event(event: &NSEvent) -> &CGEventRef {
        let ptr: *mut c_void = unsafe { msg_send![event, CGEvent] };
        unsafe { CGEventRef::from_ptr(ptr.cast()) }
    }
}

/// Converts a `Key` to a `CGKeyCode`
impl TryFrom<Key> for core_graphics::event::CGKeyCode {
    type Error = ();

    #[allow(clippy::too_many_lines)]
    fn try_from(key: Key) -> Result<Self, Self::Error> {
        // A list of names is available at:
        // https://docs.rs/core-graphics/latest/core_graphics/event/struct.KeyCode.html
        // https://github.com/phracker/MacOSX-SDKs/blob/master/MacOSX10.13.sdk/System/Library/Frameworks/Carbon.framework/Versions/A/Frameworks/HIToolbox.framework/Versions/A/Headers/Events.h
        let key = match key {
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
            Key::Unicode(c) => get_layoutdependent_keycode(&c.to_string()),
            Key::Other(v) => {
                let Ok(v) = u16::try_from(v) else {
                    return Err(());
                };
                v
            }
            Key::Super | Key::Command | Key::Windows | Key::Meta => KeyCode::COMMAND,
            Key::BrightnessDown
            | Key::BrightnessUp
            | Key::ContrastUp
            | Key::ContrastDown
            | Key::Eject
            | Key::IlluminationDown
            | Key::IlluminationUp
            | Key::IlluminationToggle
            | Key::LaunchPanel
            | Key::MediaFast
            | Key::MediaNextTrack
            | Key::MediaPlayPause
            | Key::MediaPrevTrack
            | Key::MediaRewind
            | Key::Power
            | Key::VidMirror => return Err(()),
        };
        Ok(key)
    }
}

fn get_layoutdependent_keycode(string: &str) -> CGKeyCode {
    let mut pressed_keycode = 0;

    // loop through every keycode (0 - 127)
    for keycode in 0..128 {
        // no modifier
        if let Some(key_string) = keycode_to_string(keycode, 0x100) {
            // debug!("{:?}", string);
            if string == key_string {
                pressed_keycode = keycode;
            }
        }

        // shift modifier
        if let Some(key_string) = keycode_to_string(keycode, 0x20102) {
            // debug!("{:?}", string);
            if string == key_string {
                pressed_keycode = keycode;
            }
        }

        // alt modifier
        // if let Some(string) = keycode_to_string(keycode, 0x80120) {
        //     debug!("{:?}", string);
        // }
        // alt + shift modifier
        // if let Some(string) = keycode_to_string(keycode, 0xa0122) {
        //     debug!("{:?}", string);
        // }
    }

    pressed_keycode
}

fn keycode_to_string(keycode: u16, modifier: u32) -> Option<String> {
    let cf_string = create_string_for_key(keycode, modifier);
    let buffer_size = unsafe { CFStringGetLength(cf_string) + 1 };
    let mut buffer: i8 = std::i8::MAX;
    let success =
        unsafe { CFStringGetCString(cf_string, &mut buffer, buffer_size, kCFStringEncodingUTF8) };
    if success == TRUE as u8 {
        let rust_string = String::from_utf8(vec![buffer as u8]).unwrap();
        Some(rust_string)
    } else {
        None
    }
}

#[allow(clippy::unused_self)]
fn create_string_for_key(keycode: u16, modifier: u32) -> CFStringRef {
    let mut current_keyboard = unsafe { TISCopyCurrentKeyboardInputSource() };
    let mut layout_data =
        unsafe { TISGetInputSourceProperty(current_keyboard, kTISPropertyUnicodeKeyLayoutData) };
    if layout_data.is_null() {
        debug!("TISGetInputSourceProperty(current_keyboard, kTISPropertyUnicodeKeyLayoutData) returned NULL");
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
            kUCKeyTranslateNoDeadKeysBit,
            &mut keys_down,
            8, // sizeof(chars) / sizeof(chars[0]),
            &mut real_length,
            &mut chars,
        );
    }

    unsafe { CFStringCreateWithCharacters(kCFAllocatorDefault, &chars, 1) }
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
