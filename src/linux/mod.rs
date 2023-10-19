use log::{debug, error, warn};

use crate::{
    Axis, Coordinate, Direction, EnigoSettings, InputError, InputResult, Key,
    KeyboardControllableNext, MouseButton, MouseControllableNext, NewConError,
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
    release_keys_when_dropped: bool,
    #[cfg(feature = "wayland")]
    wayland: Option<wayland::Con>,
    #[cfg(any(feature = "x11rb", feature = "xdo"))]
    x11: Option<x11::Con>,
}

impl Enigo {
    /// Create a new Enigo struct to establish the connection to simulate input
    /// with the specified settings
    ///
    /// # Errors
    /// Have a look at the documentation of `NewConError` to see under which
    /// conditions an error will be returned.
    pub fn new(settings: &EnigoSettings) -> Result<Self, NewConError> {
        let mut connection_established = false;
        #[allow(unused_variables)]
        let EnigoSettings {
            linux_delay,
            x11_display,
            wayland_display,
            release_keys_when_dropped,
            ..
        } = settings;

        let held = Vec::new();
        #[cfg(feature = "wayland")]
        let wayland = match wayland::Con::new(wayland_display) {
            Ok(con) => {
                connection_established = true;
                debug!("wayland connection established");
                Some(con)
            }
            Err(e) => {
                warn!("{e}");
                None
            }
        };
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        match x11_display {
            Some(name) => {
                debug!(
                    "\x1b[93mtrying to establish a x11 connection to: {}\x1b[0m",
                    name
                );
            }
            None => {
                debug!("\x1b[93mtrying to establish a x11 connection to $DISPLAY\x1b[0m");
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        let x11 = match x11::Con::new(x11_display, *linux_delay) {
            Ok(con) => {
                connection_established = true;
                debug!("x11 connection established");
                Some(con)
            }
            Err(e) => {
                warn!("failed to establish x11 connection: {e}");
                None
            }
        };
        if !connection_established {
            error!("no successful connection");
            return Err(NewConError::EstablishCon("no successful connection"));
        }

        Ok(Self {
            held,
            release_keys_when_dropped: *release_keys_when_dropped,
            #[cfg(feature = "wayland")]
            wayland,
            #[cfg(any(feature = "x11rb", feature = "xdo"))]
            x11,
        })
    }

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

impl MouseControllableNext for Enigo {
    fn send_mouse_button_event(
        &mut self,
        button: MouseButton,
        direction: Direction,
    ) -> InputResult<()> {
        debug!(
            "\x1b[93msend_mouse_button_event(button: {button:?}, direction: {direction:?})\x1b[0m"
        );
        let mut success = false;
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            debug!("try sending button event via wayland");
            con.send_mouse_button_event(button, direction)?;
            debug!("successfully sent button event via wayland");
            success = true;
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            debug!("try sending button event via x11");
            con.send_mouse_button_event(button, direction)?;
            debug!("successfully sent button event via x11");
            success = true;
        }
        if success {
            debug!("successfully sent button event");
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
        debug!("\x1b[93msend_motion_notify_event(x: {x:?}, y: {y:?}, coordinate:{coordinate:?})\x1b[0m");
        let mut success = false;
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            debug!("try moving the mouse via wayland");
            con.send_motion_notify_event(x, y, coordinate)?;
            debug!("successfully moved the mouse via wayland");
            success = true;
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            debug!("try moving the mouse via x11");
            con.send_motion_notify_event(x, y, coordinate)?;
            debug!("successfully moved the mouse via x11");
            success = true;
        }
        if success {
            debug!("successfully moved the mouse");
            Ok(())
        } else {
            Err(InputError::Simulate("No protocol to enter the result"))
        }
    }

    fn mouse_scroll_event(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        debug!("\x1b[93mmouse_scroll_event(length: {length:?}, axis: {axis:?})\x1b[0m");
        let mut success = false;
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            debug!("try scrolling via wayland");
            con.mouse_scroll_event(length, axis)?;
            debug!("successfully scrolled via wayland");
            success = true;
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            debug!("try scrolling via x11");
            con.mouse_scroll_event(length, axis)?;
            debug!("successfully scrolled via x11");
            success = true;
        }
        if success {
            debug!("successfully scrolled");
            Ok(())
        } else {
            Err(InputError::Simulate("No protocol to enter the result"))
        }
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mmain_display()\x1b[0m");
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            debug!("try getting the dimensions of the display via wayland");
            return con.main_display();
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            debug!("try getting the dimensions of the display via x11");
            return con.main_display();
        }
        Err(InputError::Simulate("No protocol to enter the result"))
    }

    fn mouse_loc(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mmouse_loc()\x1b[0m");
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            debug!("try getting the mouse location via wayland");
            return con.mouse_loc();
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            debug!("try getting the mouse location via x11");
            return con.mouse_loc();
        }
        Err(InputError::Simulate("No protocol to enter the result"))
    }
}

impl KeyboardControllableNext for Enigo {
    fn fast_text_entry(&mut self, text: &str) -> InputResult<Option<()>> {
        debug!("\x1b[93mfast_text_entry(text: {text})\x1b[0m");
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            debug!("try entering text fast via wayland");
            con.enter_text(text)?;
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            debug!("try entering text fast via x11");
            con.enter_text(text)?;
        }
        debug!("successfully entered text fast");
        Ok(Some(()))
    }

    fn enter_key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93menter_key(key: {key:?}, direction: {direction:?})\x1b[0m");
        // Nothing to do
        if key == Key::Layout('\0') {
            debug!("entering the null byte is a noop");
            return Ok(());
        }
        match direction {
            Direction::Press => {
                debug!("added the key {key:?} to the held keys");
                self.held.push(key);
            }
            Direction::Release => {
                debug!("removed the key {key:?} from the held keys");
                self.held.retain(|&k| k != key);
            }
            Direction::Click => (),
        }

        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            debug!("try entering the key via wayland");
            con.enter_key(key, direction)?;
            debug!("successfully entered the key via wayland");
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            debug!("try entering the key via x11");
            con.enter_key(key, direction)?;
            debug!("successfully entered the key via x11");
        }
        debug!("successfully entered the key");
        Ok(())
    }
}

impl Drop for Enigo {
    // Release the held keys before the connection is dropped
    fn drop(&mut self) {
        if !self.release_keys_when_dropped {
            return;
        }
        for &k in &self.held() {
            if self.enter_key(k, Direction::Release).is_err() {
                error!("unable to release {:?}", k);
            };
        }
        debug!("released all held keys");
    }
}
