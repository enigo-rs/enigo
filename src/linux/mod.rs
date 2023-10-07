use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

use xkbcommon::xkb::Keysym;
/// The "empty" keyboard symbol.
// TODO: Replace it with the NO_SYMBOL from xkbcommon, once it is available
// there
pub const NO_SYMBOL: Keysym = Keysym::new(0);

use crate::{
    Axis, Coordinate, Direction, InputResult, Key, KeyboardControllableNext, MouseButton,
    MouseControllableNext,
};

// If none of these features is enabled, there is no way to simulate input
#[cfg(not(any(feature = "wayland", feature = "x11rb", feature = "xdo")))]
compile_error!(
    "either feature `wayland`, `x11rb` or `xdo` must be enabled for this crate when using linux"
);

#[cfg(feature = "wayland")]
pub mod wayland;
#[cfg(any(feature = "x11rb", feature = "xdo"))]
#[cfg_attr(feature = "x11rb", path = "x11rb.rs")]
#[cfg_attr(not(feature = "x11rb"), path = "xdo.rs")]
mod x11;

#[cfg(feature = "wayland")]
pub mod constants;
#[cfg(feature = "wayland")]
use constants::{KEYMAP_BEGINNING, KEYMAP_END};

mod keymap;

pub type ModifierBitflag = u32; // TODO: Maybe create a proper type for this

#[derive(Debug)]
pub enum NewConError {
    EstablishCon,
    Reply,
    NoEmptyKeycodes, // "There was no space to map any keycodes"
}

impl Display for NewConError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "error establishing X11 connection with x11rb")
    }
}

impl Error for NewConError {}

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
    #[cfg(any(feature = "x11rb", feature = "xdo"))]
    x11: Option<x11::Con>,
}

impl Enigo {
    /// Get the delay per keypress.
    /// Default value is 12.
    /// This is Linux-specific.
    #[must_use]
    pub fn delay(&self) -> u32 {
        // On Wayland there is no delay

        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            return con.delay();
        }
        0 // TODO: Make this an Option
    }
    /// Set the delay per keypress.
    /// This is Linux-specific.
    pub fn set_delay(&mut self, delay: u32) {
        // On Wayland there is no delay

        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.set_delay(delay);
        }
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
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        let x11 = Some(x11::Con::try_default().unwrap());
        Self {
            held,
            #[cfg(feature = "wayland")]
            wayland,
            #[cfg(any(feature = "x11rb", feature = "xdo"))]
            x11,
        }
    }
}

impl MouseControllableNext for Enigo {
    fn send_mouse_button_event(&mut self, button: MouseButton, direction: Direction, delay: u32) {
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            con.send_mouse_button_event(button, direction, delay);
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.send_mouse_button_event(button, direction, delay);
        }
    }

    // Sends a motion notify event to the X11 server via `XTest` extension
    // TODO: Check if using x11rb::protocol::xproto::warp_pointer would be better
    fn send_motion_notify_event(&mut self, x: i32, y: i32, coordinate: Coordinate) {
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            con.send_motion_notify_event(x, y, coordinate);
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.send_motion_notify_event(x, y, coordinate);
        }
    }

    // Sends a scroll event to the X11 server via `XTest` extension
    fn mouse_scroll_event(&mut self, length: i32, axis: Axis) {
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            con.mouse_scroll_event(length, axis);
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.mouse_scroll_event(length, axis);
        }
    }

    fn main_display(&self) -> (i32, i32) {
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            return con.main_display();
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            return con.main_display();
        }
        (0, 0) // TODO: Make this an err
    }

    fn mouse_loc(&self) -> (i32, i32) {
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            return con.mouse_loc();
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            return con.mouse_loc();
        }
        (0, 0) // TODO: Make this an err
    }
}

impl KeyboardControllableNext for Enigo {
    fn fast_text_entry(&mut self, text: &str) -> InputResult<Option<()>> {
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            con.enter_text(text);
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.enter_text(text);
        }
        Ok(Some(()))
    }

    /// Sends a key event to the X11 server via `XTest` extension
    fn enter_key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        // Nothing to do
        if key == Key::Layout('\0') {
            return Ok(());
        }
        match direction {
            Direction::Press => self.held.push(key),
            Direction::Release => self.held.retain(|&k| k != key),
            Direction::Click => (),
        }

        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            con.enter_key(key, direction)?;
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.enter_key(key, direction)?;
        }
        Ok(())
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
