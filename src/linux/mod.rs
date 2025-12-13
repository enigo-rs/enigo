use log::{debug, error, trace, warn};

use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse,
    NewConError, Settings,
};

// If none of these features is enabled, there is no way to simulate input
#[cfg(not(any(
    feature = "wayland",
    feature = "x11rb",
    feature = "xdo",
    feature = "libei",
    feature = "xdg_desktop"
)))]
compile_error!(
    "either feature `wayland`, `x11rb`, `xdo` or `libei` must be enabled for this crate when using linux"
);

#[cfg(all(
    any(feature = "tokio", feature = "smol"),
    not(any(feature = "libei", feature = "xdg_desktop"))
))]
compile_error!(
    "You activated a feature (`tokio` or `smol`) to provide an async runtime but did not activate a protocol that needs it"
);

#[cfg(all(feature = "tokio", feature = "smol"))]
compile_error!(
    "the features `tokio` and `smol` are mutually exclusive! You have to chose which async runtime you want to use"
);

#[cfg(all(feature = "xdg_desktop", not(any(feature = "tokio", feature = "smol"))))]
compile_error!(
    "the xdg_desktop feature can only be used if either feature `tokio` or `smol` is enabled to provide an async runtime"
);

#[cfg(all(feature = "libei", not(any(feature = "tokio", feature = "smol"))))]
compile_error!(
    "the libei feature can only be used if either feature `tokio` or `smol` is enabled to provide an async runtime"
);

#[cfg(any(
    all(feature = "libei", feature = "tokio"),
    all(feature = "libei", feature = "smol")
))]
mod libei;

#[cfg(feature = "wayland")]
mod wayland;
#[cfg(any(feature = "x11rb", feature = "xdo"))]
#[cfg_attr(feature = "x11rb", path = "x11rb.rs")]
#[cfg_attr(not(feature = "x11rb"), path = "xdo.rs")]
mod x11;

#[cfg(feature = "xdg_desktop")]
mod xdg_desktop;

#[cfg(any(feature = "wayland", feature = "x11rb"))]
mod keymap;

#[cfg(feature = "wayland")]
pub mod keymap2;

pub struct Enigo<'a> {
    held: (Vec<Key>, Vec<u16>), // Currently held keys and held keycodes
    release_keys_when_dropped: bool,
    #[cfg(feature = "wayland")]
    wayland: Option<wayland::Con>,
    #[cfg(any(feature = "x11rb", feature = "xdo"))]
    x11: Option<x11::Con>,
    #[cfg(any(
        all(feature = "libei", feature = "tokio"),
        all(feature = "libei", feature = "smol")
    ))]
    libei: Option<libei::Con>,
    #[cfg(any(
        all(feature = "xdg_desktop", feature = "tokio"),
        all(feature = "xdg_desktop", feature = "smol")
    ))]
    xdg_desktop: Option<xdg_desktop::Con<'a>>,
    #[cfg(not(any(
        all(feature = "xdg_desktop", feature = "tokio"),
        all(feature = "xdg_desktop", feature = "smol")
    )))]
    _phantom: std::marker::PhantomData<&'a ()>, /* Needed to fix compiler complaining about
                                                 * unused lifetime
                                                 * parameter */
}

impl Enigo<'_> {
    /// Create a new Enigo struct to establish the connection to simulate input
    /// with the specified settings
    ///
    /// # Errors
    /// Have a look at the documentation of `NewConError` to see under which
    /// conditions an error will be returned.
    pub fn new(settings: &Settings) -> Result<Self, NewConError> {
        let mut connection_established = false;
        #[allow(unused_variables)]
        let Settings {
            x11_display,
            wayland_display,
            release_keys_when_dropped,
            ..
        } = settings;

        let held = (Vec::new(), Vec::new());

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        let xdg_desktop = match xdg_desktop::Con::new() {
            Ok(con) => {
                connection_established = true;
                debug!("xdg_desktop connection established");
                Some(con)
            }
            Err(e) => {
                warn!("{e}");
                None
            }
        };
        #[cfg(feature = "wayland")]
        let wayland = match wayland::Con::new(wayland_display.as_deref()) {
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
                debug!("\x1b[93mtrying to establish a x11 connection to: {name}\x1b[0m");
            }
            None => {
                debug!("\x1b[93mtrying to establish a x11 connection to $DISPLAY\x1b[0m");
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        let x11 = match x11::Con::new(x11_display.as_deref()) {
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
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        let libei = match libei::Con::new() {
            Ok(con) => {
                connection_established = true;
                debug!("libei connection established");
                Some(con)
            }
            Err(e) => {
                warn!("failed to establish libei connection: {e}");
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
            #[cfg(any(
                all(feature = "libei", feature = "tokio"),
                all(feature = "libei", feature = "smol")
            ))]
            libei,
            #[cfg(any(
                all(feature = "xdg_desktop", feature = "tokio"),
                all(feature = "xdg_desktop", feature = "smol")
            ))]
            xdg_desktop,
            #[cfg(not(any(
                all(feature = "xdg_desktop", feature = "tokio"),
                all(feature = "xdg_desktop", feature = "smol")
            )))]
            _phantom: std::marker::PhantomData,
        })
    }

    /// Returns a list of all currently pressed keys
    pub fn held(&mut self) -> (Vec<Key>, Vec<u16>) {
        self.held.clone()
    }
}

impl Mouse for Enigo<'_> {
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mbutton(button: {button:?}, direction: {direction:?})\x1b[0m");
        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try sending button event via xdg_desktop");
            res = con.button(button, direction);
            if res.is_ok() {
                debug!("successfully sent button event via xdg_desktop");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try sending button event via wayland");
            res = con.button(button, direction);
            if res.is_ok() {
                debug!("successfully sent button event via wayland");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            trace!("try sending button event via x11");
            res = con.button(button, direction);
            if res.is_ok() {
                debug!("successfully sent button event via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try sending button event via libei");
            res = con.button(button, direction);
            if res.is_ok() {
                debug!("successfully sent button event via libei");
                return res;
            }
        }
        res
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        debug!("\x1b[93mmove_mouse(x: {x:?}, y: {y:?}, coordinate:{coordinate:?})\x1b[0m");
        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));
        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try moving the mouse via xdg_desktop");
            res = con.move_mouse(x, y, coordinate);
            if res.is_ok() {
                debug!("successfully moved the mouse via xdg_desktop");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try moving the mouse via wayland");
            res = con.move_mouse(x, y, coordinate);
            if res.is_ok() {
                debug!("successfully moved the mouse via wayland");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            trace!("try moving the mouse via x11");
            res = con.move_mouse(x, y, coordinate);
            if res.is_ok() {
                debug!("successfully moved the mouse via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try moving the mouse via libei");
            res = con.move_mouse(x, y, coordinate);
            if res.is_ok() {
                debug!("successfully moved the mouse via libei");
                return res;
            }
        }
        res
    }

    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        debug!("\x1b[93mscroll(length: {length:?}, axis: {axis:?})\x1b[0m");
        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try scrolling via xdg_desktop");
            res = con.scroll(length, axis);
            if res.is_ok() {
                debug!("successfully scrolled via xdg_desktop");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try scrolling via wayland");
            res = con.scroll(length, axis);
            if res.is_ok() {
                debug!("successfully scrolled via wayland");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            trace!("try scrolling via x11");
            res = con.scroll(length, axis);
            if res.is_ok() {
                debug!("successfully scrolled via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try scrolling via libei");
            res = con.scroll(length, axis);
            if res.is_ok() {
                debug!("successfully scrolled via libei");
                return res;
            }
        }
        res
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mmain_display()\x1b[0m");
        let mut res = Err(InputError::Simulate(
            "No protocol to get the main display dimensions",
        ));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_ref() {
            trace!("try getting the dimensions of the display via xdg_desktop");
            res = con.main_display();
            if res.is_ok() {
                debug!("successfully got the dimensions");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            trace!("try getting the dimensions of the display via wayland");
            res = con.main_display();
            if res.is_ok() {
                debug!("successfully got the dimensions");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            trace!("try getting the dimensions of the display via x11");
            res = con.main_display();
            if res.is_ok() {
                debug!("successfully got the dimensions");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_ref() {
            trace!("try getting the dimensions of the display via libei");
            res = con.main_display();
            if res.is_ok() {
                debug!("successfully got the dimensions");
                return res;
            }
        }
        res
    }

    fn location(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mlocation()\x1b[0m");
        let mut res = Err(InputError::Simulate(
            "No protocol to get the mouse location",
        ));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_ref() {
            trace!("try getting the mouse location via xdg_desktop");
            res = con.location();
            if res.is_ok() {
                debug!("successfully got the mouse location");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            trace!("try getting the mouse location via wayland");
            res = con.location();
            if res.is_ok() {
                debug!("successfully got the mouse location");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            trace!("try getting the mouse location via x11");
            res = con.location();
            if res.is_ok() {
                debug!("successfully got the mouse location");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_ref() {
            trace!("try getting the mouse location via libei");
            res = con.location();
            if res.is_ok() {
                debug!("successfully got the mouse location");
                return res;
            }
        }
        res
    }
}

impl Keyboard for Enigo<'_> {
    fn fast_text(&mut self, text: &str) -> InputResult<Option<()>> {
        debug!("\x1b[93mfast_text(text: {text})\x1b[0m");
        #[allow(unused_mut)]
        let mut res = Ok(None); // Don't return an error here so it can be retried entering individual letters

        /*
        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try entering text fast via xdg_desktop");
            res = con.fast_text(text);
            if res.is_ok() {
                debug!("successfully entered text fast via xdg_desktop");
                return res;
            }
        }*/
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try entering text fast via wayland");
            res = con.fast_text(text);
            if res.is_ok() {
                debug!("successfully entered text fast via wayland");
                return res;
            }
        }
        //#[cfg(any(feature = "x11rb", feature = "xdo"))] // Not possible on x11rb
        #[cfg(feature = "xdo")]
        if let Some(con) = self.x11.as_mut() {
            trace!("try entering text fast via x11");
            res = con.fast_text(text);
            if res.is_ok() {
                debug!("successfully entered text fast via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try entering text fast via libei");
            res = con.fast_text(text);
            if res.is_ok() {
                debug!("successfully entered text fast via libei");
                return res;
            }
        }
        res
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mkey(key: {key:?}, direction: {direction:?})\x1b[0m");
        // Nothing to do
        if key == Key::Unicode('\0') {
            debug!("entering the null byte is a noop");
            return Ok(());
        }

        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try entering the key via xdg_desktop");
            res = con.key(key, direction);
            if res.is_ok() {
                debug!("successfully entered the key via xdg_desktop");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try entering the key via wayland");
            res = con.key(key, direction);
            if res.is_ok() {
                debug!("successfully entered the key via wayland");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            trace!("try entering the key via x11");
            res = con.key(key, direction);
            if res.is_ok() {
                debug!("successfully entered the key via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try entering the key via libei");
            res = con.key(key, direction);
            if res.is_ok() {
                debug!("successfully entered the key via libei");
                return res;
            }
        }

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

        res
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mraw(keycode: {keycode:?}, direction: {direction:?})\x1b[0m");

        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try entering the keycode via xdg_desktop");
            res = con.raw(keycode, direction);
            if res.is_ok() {
                debug!("successfully entered the raw key via xdg_desktop");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try entering the keycode via wayland");
            res = con.raw(keycode, direction);
            if res.is_ok() {
                debug!("successfully entered the raw key via wayland");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            trace!("try entering the keycode via x11");
            res = con.raw(keycode, direction);
            if res.is_ok() {
                debug!("successfully entered the raw key via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try entering the keycode via libei");
            res = con.raw(keycode, direction);
            if res.is_ok() {
                debug!("successfully entered the raw key via libei");
                return res;
            }
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

        res
    }
}

impl Drop for Enigo<'_> {
    // Release the held keys before the connection is dropped
    fn drop(&mut self) {
        if !self.release_keys_when_dropped {
            return;
        }
        let (held_keys, held_keycodes) = self.held();
        for &key in &held_keys {
            if self.key(key, Direction::Release).is_err() {
                error!("unable to release {key:?}");
            }
        }
        for &keycode in &held_keycodes {
            if self.raw(keycode, Direction::Release).is_err() {
                error!("unable to release {keycode:?}");
            }
        }
        debug!("released all held keys and held keycodes");
    }
}
