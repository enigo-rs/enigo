//! Enigo lets you simulate mouse and keyboard input-events as if they were
//! made by the actual hardware. The goal is to make it available on different
//! operating systems like Linux, macOS and Windows – possibly many more but
//! [Redox](https://redox-os.org/) and *BSD are planned. Please see the
//! [Repo](https://github.com/enigo-rs/enigo) for the current status.
//!
//! I consider this library in an early alpha status, the API will change in
//! in the future. The keyboard handling is far from being very usable. I plan
//! to build a simple
//! [DSL](https://en.wikipedia.org/wiki/Domain-specific_language)
//! that will resemble something like:
//!
//! `"hello {+SHIFT}world{-SHIFT} and break line{ENTER}"`
//!
//! The current status is that you can just print
//! [unicode](http://unicode.org/)
//! characters like [emoji](http://getemoji.com/) without the `{+SHIFT}`
//! [DSL](https://en.wikipedia.org/wiki/Domain-specific_language)
//! or any other "special" key on the Linux, macOS and Windows operating system.
//!
//! Possible use cases could be for testing user interfaces on different
//! platforms,
//! building remote control applications or just automating tasks for user
//! interfaces unaccessible by a public API or scripting language.
//!
//! For the keyboard there are currently two modes you can use. The first mode
//! is represented by the [`key_sequence`](KeyboardControllable::key_sequence)
//! function its purpose is to simply write unicode characters. This is
//! independent of the keyboard layout. Please note that
//! you're not be able to use modifier keys like Control
//! to influence the outcome. If you want to use modifier keys to e.g.
//! copy/paste, use the Layout variant. Please note that this is indeed layout
//! dependent.

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
pub mod dsl;

#[cfg(feature = "with_serde")]
#[macro_use]
extern crate serde_derive;

#[cfg(feature = "with_serde")]
extern crate serde;

#[cfg_attr(feature = "with_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// [`MouseButton`] represents a mouse button,
/// and is used in for example
/// [`MouseControllable::mouse_click`].
/// WARNING: Types with the prefix Scroll
/// IS NOT intended to be used, and may not work on
/// all operating systems.
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Middle mouse button
    Middle,
    /// Right mouse button
    Right,

    /// Scroll up button
    ScrollUp,
    /// Left right button
    ScrollDown,
    /// Left right button
    ScrollLeft,
    /// Left right button
    ScrollRight,
}

/// Representing an interface and a set of mouse functions every
/// operating system implementation _should_ implement.
pub trait MouseControllable {
    /// Lets the mouse cursor move to the specified x and y coordinates.
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

    /// Lets the mouse cursor move the specified amount in the x and y
    /// direction.
    ///
    /// The amount specified in the x and y parameters are added to the
    /// current location of the mouse cursor. A positive x values lets
    /// the mouse cursor move an amount of `x` pixels to the right. A negative
    /// value for `x` lets the mouse cursor go to the left. A positive value
    /// of y
    /// lets the mouse cursor go down, a negative one lets the mouse cursor go
    /// up.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_move_relative(100, 100);
    /// ```
    fn mouse_move_relative(&mut self, x: i32, y: i32);

    /// Push down one of the mouse buttons
    ///
    /// Push down the mouse button specified by the parameter `button` of
    /// type [`MouseButton`]
    /// and holds it until it is released by
    /// [`MouseControllable::mouse_up`](.
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

    /// Lift up a pushed down mouse button
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
    /// enigo.mouse_up(MouseButton::Right);
    /// ```
    fn mouse_up(&mut self, button: MouseButton);

    /// Click a mouse button
    ///
    /// it's essentially just a consecutive invocation of
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
    /// Positive numbers for length lets the mouse wheel scroll to the right
    /// and negative ones to the left. The value that is specified translates
    /// to `lines` defined by the operating system and is essentially one 15°
    /// (click)rotation on the mouse wheel. How many lines it moves depends
    /// on the current setting in the operating system.
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
    /// Positive numbers for length lets the mouse wheel scroll down
    /// and negative ones up. The value that is specified translates
    /// to `lines` defined by the operating system and is essentially one 15°
    /// (click)rotation on the mouse wheel. How many lines it moves depends
    /// on the current setting in the operating system.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_scroll_y(2);
    /// ```
    fn mouse_scroll_y(&mut self, length: i32);
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

/// Representing an interface and a set of keyboard functions every
/// operating system implementation _should_ implement.
pub trait KeyboardControllable {
    /// Types the string parsed with DSL.
    ///
    /// Typing {+SHIFT}hello{-SHIFT} becomes HELLO.
    /// TODO: Full documentation
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

    /// Types the string
    ///
    /// Emits keystrokes such that the given string is inputted.
    ///
    /// You can use many unicode here like: ❤️. This works
    /// regardless of the current keyboardlayout.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.key_sequence("hello world ❤️");
    /// ```
    fn key_sequence(&mut self, sequence: &str);

    /// presses a given key down
    fn key_down(&mut self, key: Key);

    /// release a given key formally pressed down by
    /// [`KeyboardControllable::key_down`]
    fn key_up(&mut self, key: Key);

    /// Much like the [`KeyboardControllable::key_down`] and
    /// [`KeyboardControllable::key_up`] function they're just invoked
    /// consecutively
    fn key_click(&mut self, key: Key);
}

/// Lets you do operations that are not directly specific to controlling the
/// mouse or keyboard but are convenient  while using Enigo
pub trait Extension {
    /// Gets the (width, height) of the main display in screen coordinates
    /// (pixels).
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

    /// Gets the location of mouse in screen coordinates (pixels).
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
