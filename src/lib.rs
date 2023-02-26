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
//! let mut enigo = Enigo::new();
//! //paste
//! enigo.key_down(Key::Control);
//! enigo.key_click(Key::Layout('v'));
//! enigo.key_up(Key::Control);
//! ```
//!
//! ```no_run
//! use enigo::*;
//! let mut enigo = Enigo::new();
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

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

// TODO(dustin) use interior mutability not &mut self

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub use crate::win::Enigo;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use crate::macos::Enigo;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use crate::linux::Enigo;

/// DSL parser module
///
/// The current status is that you can just print [unicode](http://unicode.org/) characters like [emoji](http://getemoji.com/) without the `{+SHIFT}`
/// [DSL](https://en.wikipedia.org/wiki/Domain-specific_language) or any other "special" key on the Linux, macOS and Windows operating system.
pub mod dsl;

#[cfg(feature = "with_serde")]
#[macro_use]
extern crate serde_derive;

#[cfg(feature = "with_serde")]
extern crate serde;

#[cfg_attr(feature = "with_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// [`MouseButton`] represents a mouse button and is used in e.g
/// [`MouseControllable::mouse_click`].
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Middle mouse button
    Middle,
    /// Right mouse button
    Right,

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
pub trait MouseControllable {
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
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_move_to(500, 200);
    /// ```
    fn mouse_move_to(&mut self, x: i32, y: i32);

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
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_move_relative(100, 100);
    /// ```
    fn mouse_move_relative(&mut self, x: i32, y: i32);

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
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_down(MouseButton::Left);
    /// ```
    fn mouse_down(&mut self, button: MouseButton);

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
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_down(MouseButton::Right);
    /// enigo.mouse_up(MouseButton::Right);
    /// ```
    fn mouse_up(&mut self, button: MouseButton);

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
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_click(MouseButton::Right);
    /// ```
    fn mouse_click(&mut self, button: MouseButton);

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
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_scroll_x(2);
    /// ```
    fn mouse_scroll_x(&mut self, length: i32);

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
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_scroll_y(2);
    /// ```
    fn mouse_scroll_y(&mut self, length: i32);

    /// Get the (width, height) of the main display in screen coordinates
    /// (pixels). This currently only works on the main display
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// let (width, height) = enigo.main_display_size();
    /// ```
    #[must_use]
    fn main_display_size(&self) -> (i32, i32);

    /// Get the location of the mouse in screen coordinates (pixels).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// let (x, y) = enigo.mouse_location();
    /// ```
    #[must_use]
    fn mouse_location(&self) -> (i32, i32);
}

/// A key on the keyboard.
/// For alphabetical keys, use [`Key::Layout`] for a system independent key.
/// If a key is missing, you can use the raw keycode with [`Key::Raw`].
#[cfg_attr(feature = "with_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// alt key on Linux and Windows (option key on macOS)
    Alt,
    /// backspace key
    Backspace,
    /// caps lock key
    CapsLock,
    #[deprecated(since = "0.0.12", note = "now renamed to Meta")]
    /// command key on macOS (super key on Linux, windows key on Windows)
    Command,
    /// control key
    Control,
    /// delete key
    Delete,
    /// down arrow key
    DownArrow,
    /// end key
    End,
    /// escape key (esc)
    Escape,
    /// F1 key
    F1,
    /// F2 key
    F2,
    /// F3 key
    F3,
    /// F4 key
    F4,
    /// F5 key
    F5,
    /// F6 key
    F6,
    /// F7 key
    F7,
    /// F8 key
    F8,
    /// F9 key
    F9,
    /// F10 key
    F10,
    /// F11 key
    F11,
    /// F12 key
    F12,
    /// F13 key
    F13,
    /// F14 key
    F14,
    /// F15 key
    F15,
    /// F16 key
    F16,
    /// F17 key
    F17,
    /// F18 key
    F18,
    /// F19 key
    F19,
    /// F20 key
    F20,
    /// home key
    Home,
    /// left arrow key
    LeftArrow,
    /// meta key (also known as "windows", "super", and "command")
    Meta,
    /// option key on macOS (alt key on Linux and Windows)
    Option,
    /// page down key
    PageDown,
    /// page up key
    PageUp,
    /// return key
    Return,
    /// right arrow key
    RightArrow,
    /// shift key
    Shift,
    /// space key
    Space,
    #[deprecated(since = "0.0.12", note = "now renamed to Meta")]
    /// super key on linux (command key on macOS, windows key on Windows)
    Super,
    /// tab key (tabulator)
    Tab,
    /// up arrow key
    UpArrow,
    #[deprecated(since = "0.0.12", note = "now renamed to Meta")]
    /// windows key on Windows (super key on Linux, command key on macOS)
    Windows,
    /// keyboard layout dependent key
    Layout(char),
    /// raw keycode eg 0x38
    Raw(u16),
}

/// Contains functions to simulate key presses and to input text.
///
/// For the keyboard there are currently two modes you can use. The first mode
/// is represented by the [`key_sequence`](KeyboardControllable::key_sequence)
/// function. It's purpose is to simply write unicode characters. This is
/// independent of the keyboard layout. Please note that
/// you're not be able to use modifier keys like Control
/// to influence the outcome. If you want to use modifier keys to e.g.
/// copy/paste, use the Layout variant. Please note that this is indeed layout
/// dependent.
pub trait KeyboardControllable {
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
    /// let mut enigo = Enigo::new();
    /// enigo.key_sequence("hello world ❤️");
    /// ```
    fn key_sequence(&mut self, sequence: &str);

    /// Press down the given key
    fn key_down(&mut self, key: Key);

    /// Release a pressed down key
    fn key_up(&mut self, key: Key);

    /// Press and release the key. It is the same as calling the
    /// [`KeyboardControllable::key_down`] and
    /// [`KeyboardControllable::key_up`] functions consecutively
    fn key_click(&mut self, key: Key);
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

use std::fmt;

impl fmt::Debug for Enigo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Enigo")
    }
}
