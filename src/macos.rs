extern crate core_graphics;
extern crate libc;

// TODO(dustin): use only the things i need

use self::core_graphics::display::*;
use self::core_graphics::event::*;
use self::core_graphics::event_source::*;
use self::core_graphics::geometry::*;
use self::libc::*;

use super::{KeyboardControllable, MouseControllable};
use std::mem;

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

    // not present in servo/core-graphics
    fn CGEventCreateScrollWheelEvent(source: CGEventSourceRef,
                                     units: ScrollUnit,
                                     wheelCount: uint32_t,
                                     wheel1: int32_t,
                                     ...)
                                     -> CGEventRef;
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
    fn mouse_down(&mut self, button: u32) {
        unsafe {
            let mouse_ev = CGEventCreateMouseEvent(ptr::null(),
                                                   FIXMEEventType::LeftMouseDown,
                                                   CGPoint::new(self.current_x as f64,
                                                                self.current_y as f64),
                                                   CGMouseButton::Left);

            CGEventPost(CGEventTapLocation::HID, mouse_ev);
            CFRelease(mem::transmute(mouse_ev));
        }
    }

    // TODO(dustin): use button parameter, current implementation
    // is using the left mouse button every time
    fn mouse_up(&mut self, button: u32) {
        unsafe {
            let mouse_ev = CGEventCreateMouseEvent(ptr::null(),
                                                   FIXMEEventType::LeftMouseUp,
                                                   CGPoint::new(self.current_x as f64,
                                                                self.current_y as f64),
                                                   CGMouseButton::Left);

            CGEventPost(CGEventTapLocation::HID, mouse_ev);
            CFRelease(mem::transmute(mouse_ev));
        }
    }

    fn mouse_click(&mut self, button: u32) {
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

impl KeyboardControllable for Enigo {
    fn key_sequence(&self, sequence: &str) {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
			.expect("Failed creating event source");
        let event = CGEvent::new_keyboard_event(source, 0, true).expect("Failed creating event");
        event.set_string(sequence);
        event.post(CGEventTapLocation::HID);
    }
}
