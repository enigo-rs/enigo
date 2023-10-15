use crate::{
    Axis, Coordinate, Direction, InputError, InputResult, Key, KeyboardControllableNext,
    MouseButton, MouseControllableNext,
};

// If none of these features is enabled, there is no way to simulate input
#[cfg(not(any(feature = "wayland", feature = "x11rb", feature = "xdo")))]
compile_error!(
    "either feature `wayland`, `x11rb` or `xdo` must be enabled for this crate when using linux"
);

#[cfg(feature = "wayland")]
mod wayland;
#[cfg(any(feature = "x11rb", feature = "xdo"))]
#[cfg_attr(feature = "x11rb", path = "x11rb.rs")]
#[cfg_attr(not(feature = "x11rb"), path = "xdo.rs")]
mod x11;

#[cfg(feature = "wayland")]
mod constants;
#[cfg(feature = "wayland")]
use constants::{KEYMAP_BEGINNING, KEYMAP_END};

#[cfg(any(feature = "wayland", feature = "x11rb"))]
mod keymap;

pub struct Enigo {
    held: Vec<Key>, // Currently held keys
    #[cfg(feature = "wayland")]
    wayland: Option<wayland::Con>,
    #[cfg(any(feature = "x11rb", feature = "xdo"))]
    x11: Option<x11::Con>,
}

impl Enigo {
    /// Get the delay per keypress
    #[must_use]
    pub fn delay(&self) -> u32 {
        // On Wayland there is no delay

        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            return con.delay();
        }
        0 // TODO: Make this an Option
    }

    /// Set the delay per keypress
    #[allow(unused_variables)]
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
    fn send_mouse_button_event(
        &mut self,
        button: MouseButton,
        direction: Direction,
        delay: u32,
    ) -> InputResult<()> {
        let mut success = false;
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            con.send_mouse_button_event(button, direction, delay)?;
            success = true;
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.send_mouse_button_event(button, direction, delay)?;
            success = true;
        }
        if success {
            Ok(())
        } else {
            Err(InputError::Simulate("No protocol to enter the result"))
        }
    }

    fn send_motion_notify_event(
        &mut self,
        x: i32,
        y: i32,
        coordinate: Coordinate,
    ) -> InputResult<()> {
        let mut success = false;
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            con.send_motion_notify_event(x, y, coordinate)?;
            success = true;
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.send_motion_notify_event(x, y, coordinate)?;
            success = true;
        }
        if success {
            Ok(())
        } else {
            Err(InputError::Simulate("No protocol to enter the result"))
        }
    }

    fn mouse_scroll_event(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        let mut success = false;
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            con.mouse_scroll_event(length, axis)?;
            success = true;
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.mouse_scroll_event(length, axis)?;
            success = true;
        }
        if success {
            Ok(())
        } else {
            Err(InputError::Simulate("No protocol to enter the result"))
        }
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            return con.main_display();
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            return con.main_display();
        }
        Err(InputError::Simulate("No protocol to enter the result"))
    }

    fn mouse_loc(&self) -> InputResult<(i32, i32)> {
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            return con.mouse_loc();
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            return con.mouse_loc();
        }
        Err(InputError::Simulate("No protocol to enter the result"))
    }
}

impl KeyboardControllableNext for Enigo {
    fn fast_text_entry(&mut self, text: &str) -> InputResult<Option<()>> {
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            con.enter_text(text)?;
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            con.enter_text(text)?;
        }
        Ok(Some(()))
    }

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
            if self.enter_key(k, Direction::Release).is_err() {
                println!("unable to release {k:?}");
            };
        }
    }
}
