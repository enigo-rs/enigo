//! Enigo lets you simulate mouse and keyboard input-events as if they were
//! made by the actual hardware. It is available on Linux (X11), macOS and
//! Windows.
//!
//! It can be used for testing user interfaces on different platforms, building
//! remote control applications or just automating tasks for user interfaces
//! unaccessible by a public API or scripting language.
//!
//! This library is in an early alpha status, the API will change in
//! in the future.
//!
//! In order to use the library, you only have to know about three
//! things:
//! - [`KeyboardControllable`] (trait): used to simulate a key click, enter text
//!   or something similar
//! - [`MouseControllable`] (trait): do something with the mouse or you find out
//!   the display
//! size
//! - [`Enigo`] (struct): implements the two traits [`KeyboardControllable`] and
//!   [`MouseControllable`]
//!
//! A simple [DSL](https://en.wikipedia.org/wiki/Domain-specific_language)
//! is available. It is documented in the [`dsl`] module.

//! # Examples
//! ```no_run
//! use enigo::*;
//! let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
//! //paste
//! enigo.key_down(Key::Control);
//! enigo.key_click(Key::Unicode('v'));
//! enigo.key_up(Key::Control);
//! ```
//!
//! ```no_run
//! use enigo::*;
//! let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
//! enigo.mouse_move_to(500, 200);
//! enigo.mouse_down(MouseButton::Left);
//! enigo.mouse_move_relative(100, 100);
//! enigo.mouse_up(MouseButton::Left);
//! enigo.key_sequence("hello world");
//! ```

#![deny(clippy::pedantic)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(deprecated)]

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

use log::{debug, error};

/// DSL parser module
///
/// The current status is that you can just print [unicode](http://unicode.org/) characters like [emoji](http://getemoji.com/) without the `{+SHIFT}`
/// [DSL](https://en.wikipedia.org/wiki/Domain-specific_language) or any other "special" key on the Linux, macOS and Windows operating system.
pub mod dsl;

#[cfg_attr(target_os = "linux", path = "linux/mod.rs")]
#[cfg_attr(target_os = "macos", path = "macos/mod.rs")]
#[cfg_attr(target_os = "windows", path = "win/mod.rs")]
mod platform;
pub use platform::Enigo;

#[cfg(target_os = "windows")]
pub use platform::EXT;

/// Contains the available keycodes
pub mod keycodes;
pub use keycodes::Key;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// [`MouseButton`] represents a mouse button and is used in e.g
/// [`MouseControllable::mouse_click`].

// Warning! If there are ANY CHANGES to this enum, we
// need to change the size of the array in the macOS implementation of the Enigo
// struct that stores the nth click for each MouseButton
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Middle mouse button
    Middle,
    /// Right mouse button
    Right,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    /// 4th mouse button. Typically performs the same function as Browser_Back
    Back,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    /// 5th mouse button. Typically performs the same function as
    /// Browser_Forward
    Forward,

    /// Scroll up button. It is better to use the
    /// [MouseControllable::mouse_scroll_y] method to scroll.
    ScrollUp,
    /// Scroll down button. It is better to use the
    /// [MouseControllable::mouse_scroll_y] method to scroll.
    ScrollDown,
    /// Scroll left button. It is better to use the
    /// [MouseControllable::mouse_scroll_x] method to scroll.
    ScrollLeft,
    /// Scroll right button. It is better to use the
    /// [MouseControllable::mouse_scroll_x] method to scroll.
    ScrollRight,
}

/// Contains functions to control the mouse and to get the size of the display.
/// Enigo uses a Cartesian coordinate system for specifying coordinates. The
/// origin in this system is located in the top-left corner of the current
/// screen, with positive values extending along the axes down and to the
/// right of the origin point and it is measured in pixels. The same coordinate
/// system is used on all operating systems.
pub trait MouseControllable
where
    Self: MouseControllableNext,
{
    /// Move the mouse cursor to the specified x and y coordinates.
    ///
    /// The topleft corner of your monitor screen is x=0 y=0. Move
    /// the cursor down the screen by increasing the y and to the right
    /// by increasing x coordinate.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// enigo.mouse_move_to(500, 200);
    /// ```
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        match self.move_mouse(x, y, Coordinate::Absolute) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Move the mouse cursor the specified amount in the x and y
    /// direction. A positive x value moves the mouse cursor `x` pixels to the
    /// right. A negative value for `x` moves the mouse cursor to the left.
    /// A positive value of y moves the mouse cursor down, a negative one
    /// moves the mouse cursor up.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// enigo.mouse_move_relative(100, 100);
    /// ```
    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        match self.move_mouse(x, y, Coordinate::Relative) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Push down the mouse button specified by the parameter
    /// `button` of type [`MouseButton`] and hold it until it is released by
    /// [`MouseControllable::mouse_up`].
    /// Calls to [`MouseControllable::mouse_move_to`] or
    /// [`MouseControllable::mouse_move_relative`] will
    /// work like expected and will e.g. drag widgets or highlight text.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// enigo.mouse_down(MouseButton::Left);
    /// ```
    fn mouse_down(&mut self, button: MouseButton) {
        match self.mouse_button(button, Direction::Press) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Release a pushed down mouse button
    ///
    /// Lift up a previously pushed down button (by invoking
    /// [`MouseControllable::mouse_down`]).
    /// If the button was not pushed down or consecutive calls without
    /// invoking [`MouseControllable::mouse_down`] will emit lift up
    /// events. It depends on the operating system whats actually happening
    /// – my guess is it will just get ignored.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// enigo.mouse_down(MouseButton::Right);
    /// enigo.mouse_up(MouseButton::Right);
    /// ```
    fn mouse_up(&mut self, button: MouseButton) {
        match self.mouse_button(button, Direction::Release) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Click a mouse button
    ///
    /// It is essentially just a consecutive invocation of
    /// [`MouseControllable::mouse_down`]
    /// followed by a [`MouseControllable::mouse_up`]. Just for
    /// convenience.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// enigo.mouse_click(MouseButton::Right);
    /// ```
    fn mouse_click(&mut self, button: MouseButton) {
        match self.mouse_button(button, Direction::Click) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Scroll the mouse (wheel) left or right
    ///
    /// Positive numbers for `length` scroll to the right and negative ones to
    /// the left. The value that is specified translates to `lines` defined
    /// by the operating system and is essentially one 15° (click) rotation
    /// on the mouse wheel. How many lines it moves depends on the current
    /// setting in the operating system.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// enigo.mouse_scroll_x(2);
    /// ```
    fn mouse_scroll_x(&mut self, length: i32) {
        match self.scroll(length, Axis::Horizontal) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Scroll the mouse (wheel) up or down
    ///
    /// Positive numbers for `length` scroll down and negative ones up. The
    /// value that is specified translates to `lines` defined by the
    /// operating system and is essentially one 15° (click) rotation on the
    /// mouse wheel. How many lines it moves depends on the current setting
    /// in the operating system.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// enigo.mouse_scroll_y(2);
    /// ```
    fn mouse_scroll_y(&mut self, length: i32) {
        match self.scroll(length, Axis::Vertical) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Get the (width, height) of the main display in screen coordinates
    /// (pixels). This currently only works on the main display
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// let (width, height) = enigo.main_display_size();
    /// ```
    #[must_use]
    fn main_display_size(&self) -> (i32, i32) {
        match self.main_display() {
            Ok(dpy) => dpy,
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Get the location of the mouse in screen coordinates (pixels).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// let (x, y) = enigo.mouse_location();
    /// ```
    #[must_use]
    fn mouse_location(&self) -> (i32, i32) {
        match self.location() {
            Ok(loc) => loc,
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }
}

/// Contains functions to simulate key presses and to input text.
///
/// For the keyboard there are currently two modes you can use. The first mode
/// is represented by the [`key_sequence`](KeyboardControllable::key_sequence)
/// function. It's purpose is to simply write unicode characters.
/// Please note that you're not able to use modifier keys like Control
/// to influence the outcome. If you want to use shortcuts to e.g.
/// copy/paste, enter the modifier and enter the appropriate `Key::Unicode`
/// variant.
pub trait KeyboardControllable
where
    Self: KeyboardControllableNext,
{
    /// Type the string parsed with DSL.
    ///
    /// Typing {+SHIFT}hello{-SHIFT} becomes HELLO.
    /// Please have a look at the [dsl] module for more information.
    fn key_sequence_parse(&mut self, sequence: &str)
    where
        Self: Sized,
    {
        self.key_sequence_parse_try(sequence)
            .expect("Could not parse sequence");
    }

    /// Same as [`KeyboardControllable::key_sequence_parse`] except returns any
    /// errors
    ///  # Errors
    ///
    /// Returns a [`dsl::ParseError`] if the sequence cannot be parsed
    fn key_sequence_parse_try(&mut self, sequence: &str) -> Result<(), dsl::ParseError>
    where
        Self: Sized,
    {
        dsl::eval(self, sequence)
    }

    /// Enter the text. You can use unicode here like: ❤️. This works
    /// regardless of the current keyboardlayout. You cannot use this function
    /// for entering shortcuts or something similar. For shortcuts, use the
    /// [`KeyboardControllable::key_click`] method instead.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();
    /// enigo.key_sequence("hello world ❤️");
    /// ```
    fn key_sequence(&mut self, sequence: &str) {
        match self.text(sequence) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Press down the given key
    fn key_down(&mut self, key: Key) {
        match self.key(key, Direction::Press) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Release a pressed down key
    fn key_up(&mut self, key: Key) {
        match self.key(key, Direction::Release) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }

    /// Press and release the key. It is the same as calling the
    /// [`KeyboardControllable::key_down`] and
    /// [`KeyboardControllable::key_up`] functions consecutively
    fn key_click(&mut self, key: Key) {
        match self.key(key, Direction::Click) {
            Ok(()) => {}
            Err(e) => {
                error!("{e}");
                panic!()
            }
        }
    }
}

impl KeyboardControllable for Enigo {}
impl MouseControllable for Enigo {}

impl fmt::Debug for Enigo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Enigo")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// The direction of a key or button
pub enum Direction {
    Press,
    Release,
    /// Equivalent to a press followed by a release
    Click,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Specifies the axis to scroll on
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Specifies if a coordinate is relative or absolute
pub enum Coordinate {
    Relative,
    Absolute,
}

pub trait KeyboardControllableNext {
    /// Enter the whole text string instead of entering individual keys
    /// This is much faster if you type longer text at the cost of keyboard
    /// shortcuts not getting recognized.
    ///
    /// # Errors
    /// Have a look at the documentation of `InputError` to see under which
    /// conditions an error will be returned.
    fn fast_text_entry(&mut self, text: &str) -> InputResult<Option<()>>;

    /// Enter the text
    /// Use a fast method to enter the text, if it is available. The text should
    /// not contain any NULL bytes ('\0'). You can use unicode here like: ❤️.
    /// This works regardless of the current keyboard layout. You cannot use
    /// this function for entering shortcuts or something similar. For
    /// shortcuts, use the [`KeyboardControllableNext::key`] method
    /// instead.
    ///
    /// # Errors
    /// Have a look at the documentation of `InputError` to see under which
    /// conditions an error will be returned.
    fn text(&mut self, text: &str) -> InputResult<()> {
        if text.is_empty() {
            debug!("The text to enter was empty");
            return Ok(()); // Nothing to simulate.
        }

        // Fall back to entering single keys if no fast text entry is available
        let fast_text_res = self.fast_text_entry(text);
        match fast_text_res {
            Ok(Some(())) => {
                debug!("fast text entry was successful");
                Ok(())
            }
            Ok(None) => {
                debug!("fast text entry not available. Trying to enter individual letters now");
                for c in text.chars() {
                    self.key(Key::Unicode(c), Direction::Click)?;
                }
                Ok(())
            }
            Err(e) => {
                error!("{e}");
                Err(e)
            }
        }
    }

    /// Sends an individual key event. Some of the keys are specific to a
    /// platform.
    ///
    /// # Errors
    /// Have a look at the documentation of `InputError` to see under which
    /// conditions an error will be returned.
    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()>;

    /// Sends a raw keycode. The keycode may or may not be mapped on the current
    /// layout. You have to make sure of that yourself. This can be usefull if
    /// you want to simulate a press regardless of the layout (WASD on video
    /// games)
    /// Windows only: If you want to enter the keycode (scancode) of an extended
    /// key, you need to set extra bits. You can for example do: `enigo.raw(45 |
    /// EXT, Direction::Click)`
    ///
    /// # Errors
    /// Have a look at the documentation of `InputError` to see under which
    /// conditions an error will be returned.
    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()>;
}

/// Contains functions to control the mouse and to get the size of the display.
/// Enigo uses a cartesian coordinate system for specifying coordinates. The
/// origin in this system is located in the top-left corner of the current
/// screen, with positive values extending along the axes down and to the
/// right of the origin point and it is measured in pixels. The same coordinate
/// system is used on all operating systems.
pub trait MouseControllableNext {
    /// Sends an individual mouse button event. You can use this for example to
    /// simulate a click of the left mouse key. Some of the buttons are specific
    /// to a platform.
    ///
    /// # Errors
    /// Have a look at the documentation of `InputError` to see under which
    /// conditions an error will be returned.
    fn mouse_button(&mut self, button: MouseButton, direction: Direction) -> InputResult<()>;

    /// Move the mouse cursor to the specified x and y coordinates.
    ///
    /// You can specify absolute coordinates or relative from the current
    /// position.
    ///
    /// If you use absolute coordinates, the topleft corner of your monitor
    /// screen is x=0 y=0. Move the cursor down the screen by increasing the y
    /// and to the right by increasing x coordinate.
    ///
    /// If you use relative coordinates, a positive x value moves the mouse
    /// cursor `x` pixels to the right. A negative value for `x` moves the mouse
    /// cursor to the left. A positive value of y moves the mouse cursor down, a
    /// negative one moves the mouse cursor up.
    ///
    /// # Errors
    /// Have a look at the documentation of `InputError` to see under which
    /// conditions an error will be returned.
    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()>;

    /// Send a mouse scroll event
    ///
    /// # Arguments
    /// * `axis` - The axis to scroll on
    /// * `length` - Number of 15° (click) rotations of the mouse wheel to
    ///   scroll. How many lines will be scrolled depends on the current setting
    ///   of the operating system.
    ///
    /// With `Axis::Vertical`, a positive length will result in scrolling down
    /// and negative ones up. With `Axis::Horizontal`, a positive length
    /// will result in scrolling to the right and negative ones to the left
    ///
    /// # Errors
    /// Have a look at the documentation of `InputError` to see under which
    /// conditions an error will be returned.
    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()>;

    /// Get the (width, height) of the main display in pixels. This currently
    /// only works on the main display
    ///
    /// # Errors
    /// Have a look at the documentation of `InputError` to see under which
    /// conditions an error will be returned.
    fn main_display(&self) -> InputResult<(i32, i32)>;

    /// Get the location of the mouse in pixels
    ///
    /// # Errors
    /// Have a look at the documentation of `InputError` to see under which
    /// conditions an error will be returned.
    fn location(&self) -> InputResult<(i32, i32)>;
}

pub type InputResult<T> = Result<T, InputError>;

/// Error when simulating input
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InputError {
    /// Mapping a keycode to a keysym failed
    Mapping(String),
    /// Unmapping a keycode failed
    Unmapping(String),
    /// There was no space to map any keycodes
    NoEmptyKeycodes,
    /// There was an error with the protocol
    Simulate(&'static str),
    /// The input you want to simulate is invalid
    /// This happens for example if you want to enter text that contains NULL
    /// bytes ('/0')
    InvalidInput(&'static str),
}

impl Display for InputError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let string = match self {
            InputError::Mapping(e) => format!("error when mapping keycode to keysym: ({e})"),
            InputError::Unmapping(e) => format!("error when unmapping keysym: ({e})"),
            InputError::NoEmptyKeycodes => {
                "there were no empty keycodes that could be used".to_string()
            }
            InputError::Simulate(e) => format!("simulating input failed: ({e})"),
            InputError::InvalidInput(e) => format!("you tried to simulate invalid input: ({e})"),
        };
        write!(f, "{string}")
    }
}

impl Error for InputError {}

/// Error when establishing a new connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NewConError {
    /// Error while creating the connection
    EstablishCon(&'static str),
    /// Error when receiving a reply
    Reply,
    /// The keymap is full, so there was no space to map any keycodes to keysyms
    NoEmptyKeycodes,
}

impl Display for NewConError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let string = match self {
            NewConError::EstablishCon(e) => format!("no connection could be established: ({e})"),
            NewConError::Reply => {
                "there was an error with the reply from the display server. this should not happen"
                    .to_string()
            }
            NewConError::NoEmptyKeycodes => {
                "there were no empty keycodes that could be used".to_string()
            }
        };
        write!(f, "{string}")
    }
}

impl Error for NewConError {}

/// Settings for creating the Enigo stuct and it's behaviour
#[allow(dead_code)] // It is not dead code on other platforms
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnigoSettings {
    /// Sleep delay on macOS
    pub mac_delay: u32,
    /// Sleep delay on Linux X11
    pub linux_delay: u32,
    /// Display name to connect to when using Linux X11
    pub x11_display: Option<String>,
    /// Display name to connect to when using Linux Wayland
    pub wayland_display: Option<String>,
    /// Set this to true if you want all held keys to get released when Enigo
    /// gets dropped
    pub release_keys_when_dropped: bool,
}

impl Default for EnigoSettings {
    fn default() -> Self {
        debug!("using default settings");
        Self {
            mac_delay: 20,
            linux_delay: 12,
            x11_display: None,
            wayland_display: None,
            release_keys_when_dropped: true,
        }
    }
}
