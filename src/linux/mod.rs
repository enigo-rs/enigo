use xkbcommon::xkb::Keysym;
/// The "empty" keyboard symbol.
// TODO: Replace it with the NO_SYMBOL from xkbcommon, once it is available
// there
pub const NO_SYMBOL: Keysym = Keysym::new(0);

use crate::{
    Axis, Coordinate, Direction, Key, KeyboardControllableNext, MouseButton, MouseControllableNext,
};

#[cfg_attr(feature = "x11rb", path = "x11rb.rs")]
#[cfg_attr(not(feature = "x11rb"), path = "xdo.rs")]
mod x11;

#[cfg(feature = "wayland")]
pub mod wayland;

#[cfg(feature = "wayland")]
pub mod constants;
#[cfg(feature = "wayland")]
use constants::{KEYMAP_BEGINNING, KEYMAP_END};

mod keymap;

pub type ModifierBitflag = u32; // TODO: Maybe create a proper type for this

#[derive(Debug)]
pub enum ConnectionError {
    MappingFailed(Keysym),
    Connection(String),
    Format(std::io::Error),
    General(String),
    LostConnection,
    NoKeycode,
    SetLayoutFailed(String),
    Unimplemented,
    Utf(std::string::FromUtf8Error),
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionError::MappingFailed(e) => write!(f, "Allocation failed: {e:?}"),
            ConnectionError::Connection(e) => write!(f, "Connection: {e}"),
            ConnectionError::Format(e) => write!(f, "Format: {e}"),
            ConnectionError::General(e) => write!(f, "General: {e}"),
            ConnectionError::LostConnection => write!(f, "Lost connection"),
            ConnectionError::NoKeycode => write!(f, "No keycode mapped"),
            ConnectionError::SetLayoutFailed(e) => write!(f, "set_layout() failed: {e}"),
            ConnectionError::Unimplemented => write!(f, "Unimplemented"),
            ConnectionError::Utf(e) => write!(f, "UTF: {e}"),
        }
    }
}

impl From<std::io::Error> for ConnectionError {
    fn from(e: std::io::Error) -> Self {
        ConnectionError::Format(e)
    }
}

pub struct Enigo {
    held: Vec<Key>, // Currently held keys
    #[cfg(feature = "wayland")]
    wayland: Option<wayland::Con>,
    x11: Option<x11::Con>,
}

impl Enigo {
    /// Get the delay per keypress.
    /// Default value is 12.
    /// This is Linux-specific.
    #[must_use]
    pub fn delay(&self) -> u32 {
        self.x11.as_ref().unwrap().delay()
    }
    /// Set the delay per keypress.
    /// This is Linux-specific.
    pub fn set_delay(&mut self, delay: u32) {
        self.x11.as_mut().unwrap().set_delay(delay);
    }
    /// Returns a list of all currently pressed keys
    pub fn held(&mut self) -> Vec<Key> {
        self.held.clone()
    }
}

impl Default for Enigo {
    /// Create a new `Enigo` instance
    fn default() -> Self {
        let held = Vec::new();
        #[cfg(feature = "wayland")]
        let wayland = wayland::Con::new().ok();
        let x11 = Some(x11::Con::default());
        Self {
            held,
            #[cfg(feature = "wayland")]
            wayland,
            x11,
        }
    }
}

impl MouseControllableNext for Enigo {
    fn send_mouse_button_event(&mut self, button: MouseButton, direction: Direction, delay: u32) {
        #[cfg(feature = "wayland")]
        if let Some(wayland) = self.wayland.as_mut() {
            wayland.send_mouse_button_event(button, direction, delay);
        }
        self.x11
            .as_mut()
            .unwrap()
            .send_mouse_button_event(button, direction, delay);
    }

    // Sends a motion notify event to the X11 server via `XTest` extension
    // TODO: Check if using x11rb::protocol::xproto::warp_pointer would be better
    fn send_motion_notify_event(&mut self, x: i32, y: i32, coordinate: Coordinate) {
        #[cfg(feature = "wayland")]
        if let Some(wayland) = self.wayland.as_mut() {
            wayland.send_motion_notify_event(x, y, coordinate);
        }
        self.x11
            .as_mut()
            .unwrap()
            .send_motion_notify_event(x, y, coordinate);
    }

    // Sends a scroll event to the X11 server via `XTest` extension
    fn mouse_scroll_event(&mut self, length: i32, axis: Axis) {
        #[cfg(feature = "wayland")]
        if let Some(wayland) = self.wayland.as_mut() {
            wayland.mouse_scroll_event(length, axis);
        }
        self.x11.as_mut().unwrap().mouse_scroll_event(length, axis);
    }

    fn main_display(&self) -> (i32, i32) {
        #[cfg(feature = "wayland")]
        if let Some(wayland) = self.wayland.as_ref() {
            return wayland.main_display();
        }
        self.x11.as_ref().unwrap().main_display()
    }

    fn mouse_loc(&self) -> (i32, i32) {
        #[cfg(feature = "wayland")]
        if let Some(wayland) = self.wayland.as_ref() {
            return wayland.mouse_loc();
        }
        self.x11.as_ref().unwrap().mouse_loc()
    }
}

impl KeyboardControllableNext for Enigo {
    fn fast_text_entry(&mut self, text: &str) -> Option<()> {
        #[cfg(feature = "wayland")]
        if let Some(wayland) = self.wayland.as_mut() {
            wayland.enter_text(text);
        }
        self.x11.as_mut().unwrap().enter_text(text);
        Some(())
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

        #[cfg(feature = "wayland")]
        if let Some(wayland) = self.wayland.as_mut() {
            wayland.enter_key(key, direction);
        }
        self.x11.as_mut().unwrap().enter_key(key, direction);
    }
}

// TODO: Keep track of the held keys on Windows and Mac too and release them
// when Enigo is dropped
impl Drop for Enigo {
    // Release the held keys before the connection is dropped
    fn drop(&mut self) {
        for &k in &self.held() {
            self.enter_key(k, Direction::Release);
        }
    }
}
