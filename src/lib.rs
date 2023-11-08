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
//! - [`Keyboard`] (trait): used to simulate a key click, enter text or
//!   something similar
//! - [`Mouse`] (trait): do something with the mouse or you find out the display
//! size
//! - [`Enigo`] (struct): implements the two traits [`Keyboard`] and [`Mouse`]
//!
//! A simple [DSL](https://en.wikipedia.org/wiki/Domain-specific_language)
//! is available. It is documented in the [`dsl`] module.

//! # Examples
//! ```no_run
//! use enigo::{
//!     Enigo, Key, Keyboard, Settings,
//!     {Direction::Click, Direction::Press, Direction::Release},
//! };
//! let mut enigo = Enigo::new(&Settings::default()).unwrap();
//! //paste
//! enigo.key(Key::Control, Press);
//! enigo.key(Key::Unicode('v'), Click);
//! enigo.key(Key::Control, Release);
//! ```
//!
//! ```no_run
//! use enigo::*;
//! let mut enigo = Enigo::new(&Settings::default()).unwrap();
//! enigo.move_mouse(500, 200, Coordinate::Abs);
//! enigo.button(Button::Left, Direction::Press);
//! enigo.move_mouse(100, 100, Coordinate::Rel);
//! enigo.button(Button::Left, Direction::Release);
//! enigo.text("hello world");
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

mod keycodes;
/// Contains the available keycodes
pub use keycodes::Key;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Represents a mouse button and is used in e.g
/// [`Mouse::button`].

// Warning! If there are ANY CHANGES to this enum, we
// need to change the size of the array in the macOS implementation of the Enigo
// struct that stores the nth click for each Button
#[doc(alias = "MouseButton")]
pub enum Button {
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
    /// [`Mouse::scroll`] method to scroll.
    ScrollUp,
    /// Scroll down button. It is better to use the
    /// [`Mouse::scroll`] method to scroll.
    ScrollDown,
    /// Scroll left button. It is better to use the
    /// [`Mouse::scroll`] method to scroll.
    ScrollLeft,
    /// Scroll right button. It is better to use the
    /// [`Mouse::scroll`] method to scroll.
    ScrollRight,
}

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
/// Specifies the axis for scrolling
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Specifies if a coordinate is relative or absolute
pub enum Coordinate {
    #[doc(alias = "Relative")]
    Rel,
    #[doc(alias = "Absolute")]
    Abs,
}

// TODO: Remove this trait (better move it to dsl)
pub trait DSL
where
    Self: Keyboard,
{
    // TODO: Remove this from the trait (better move it to dsl)
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

    // TODO: Remove this from the trait (better move it to dsl)
    /// Same as [`DSL::key_sequence_parse`] except returns any
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
}

impl DSL for Enigo {}

/// Contains functions to simulate key presses/releases and to input text.
///
/// For entering text, the [`Keyboard::text`] function is best.
/// If you want to enter a key without having to worry about the layout or the
/// keymap, use the [`Keyboard::key`] function. If you want a
/// specific (physical) key to be pressed (e.g WASD for games), use the
/// [`Keyboard::raw`] function. The resulting keysym will depend
/// on the layout/keymap.
#[doc(alias = "KeyboardControllable")]
pub trait Keyboard {
    // TODO: Remove this from the trait (should not be public)
    /// Enter the whole text string instead of entering individual keys
    /// This is much faster if you type longer text at the cost of keyboard
    /// shortcuts not getting recognized.
    ///
    /// # Errors
    /// Have a look at the documentation of [`InputError`] to see under which
    /// conditions an error will be returned.
    fn fast_text_entry(&mut self, text: &str) -> InputResult<Option<()>>;

    /// Enter the text
    /// Use a fast method to enter the text, if it is available. You can use
    /// unicode here like: ❤️. This works regardless of the current keyboard
    /// layout. You cannot use this function for entering shortcuts or
    /// something similar. For shortcuts, use the
    /// [`Keyboard::key`] method instead.
    ///
    /// # Errors
    /// The text should not contain any NULL bytes (`\0`). Have a look at the
    /// documentation of [`InputError`] to see under which other conditions an
    /// error will be returned.
    #[doc(alias = "key_sequence")]
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

    /// Sends an individual key event. It will enter the keysym (virtual key).
    /// Have a look at the [`Keyboard::raw`] function, if you
    /// want to enter a keycode.
    ///
    /// Some of the keys are specific to a platform.
    ///
    /// # Errors
    /// Have a look at the documentation of [`InputError`] to see under which
    /// conditions an error will be returned.
    #[doc(alias = "key_down", alias = "key_up", alias = "key_click")]
    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()>;

    /// Sends a raw keycode. The keycode may or may not be mapped on the current
    /// layout. You have to make sure of that yourself. This can be usefull if
    /// you want to simulate a press regardless of the layout (WASD on video
    /// games). Have a look at the [`Keyboard::key`] function,
    /// if you just want to enter a specific key and don't want to worry about
    /// the layout/keymap. Windows only: If you want to enter the keycode
    /// (scancode) of an extended key, you need to set extra bits. You can
    /// for example do: `enigo.raw(45 | EXT, Direction::Click)`
    ///
    /// # Errors
    /// Have a look at the documentation of [`InputError`] to see under which
    /// conditions an error will be returned.
    #[doc(alias = "Key::Raw")]
    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()>;
}

/// Contains functions to control the mouse and to get the size of the display.
/// Enigo uses a cartesian coordinate system for specifying coordinates. The
/// origin in this system is located in the top-left corner of the current
/// screen, with positive values extending along the axes down and to the
/// right of the origin point and it is measured in pixels. The same coordinate
/// system is used on all operating systems.
#[doc(alias = "MouseControllable")]
pub trait Mouse {
    /// Sends an individual mouse button event. You can use this for example to
    /// simulate a click of the left mouse key. Some of the buttons are specific
    /// to a platform.
    ///
    /// # Errors
    /// Have a look at the documentation of [`InputError`] to see under which
    /// conditions an error will be returned.
    #[doc(alias = "mouse_down", alias = "mouse_up", alias = "mouse_click")]
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()>;

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
    /// Have a look at the documentation of [`InputError`] to see under which
    /// conditions an error will be returned.
    #[doc(alias = "mouse_move_to", alias = "mouse_move_relative")]
    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()>;

    /// Send a mouse scroll event
    ///
    /// # Arguments
    /// * `axis` - The axis to scroll on
    /// * `length` - Number of 15° (click) rotations of the mouse wheel to
    ///   scroll. How many lines will be scrolled depends on the current setting
    ///   of the operating system.
    ///
    /// With [`Axis::Vertical`], a positive length will result in scrolling down
    /// and negative ones up. With [`Axis::Horizontal`], a positive length
    /// will result in scrolling to the right and negative ones to the left
    ///
    /// # Errors
    /// Have a look at the documentation of [`InputError`] to see under which
    /// conditions an error will be returned.
    #[doc(alias = "mouse_scroll_x", alias = "mouse_scroll_y")]
    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()>;

    /// Get the (width, height) of the main display in pixels. This currently
    /// only works on the main display
    ///
    /// # Errors
    /// Have a look at the documentation of [`InputError`] to see under which
    /// conditions an error will be returned.
    #[doc(alias = "main_display_size")]
    fn main_display(&self) -> InputResult<(i32, i32)>;

    /// Get the location of the mouse in pixels
    ///
    /// # Errors
    /// Have a look at the documentation of [`InputError`] to see under which
    /// conditions an error will be returned.
    #[doc(alias = "mouse_location")]
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
    /// bytes (`\0`)
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
pub struct Settings {
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

impl Default for Settings {
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
