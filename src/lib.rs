//! Enigo lets you simulate mouse and keyboard input-events as if they were
//! made by the actual hardware. The goal is to make it available on different
//! operating systems like Linux, macOS and Windows – possibly many more but
//! [Redox](https://redox-os.org/) and *BSD are planned. Please see the
//! [Repo](https://github.com/pythoneer/enigo) for the current status.
//!
//! I consider this library in an early alpha status, the API will change in
//! in the future. The keyboard handling is far from being very usable. I plan
//! to build a simple
//! [DSL](https://en.wikipedia.org/wiki/Domain-specific_language)
//! that will resemble something like:
//!
//! `"hello {+SHIFT}world{-SHIFT} and break line{ENTER}"`
//!
//! The current status is that you can just print plain
//! [ASCII](https://en.wikipedia.org/wiki/ASCII)
//! characters without the `{+SHIFT}`
//! [DSL](https://en.wikipedia.org/wiki/Domain-specific_language)
//! or any other "special" key on the linux operating system.
//!
//! Possible use cases could be for testing user interfaces on different
//! plattforms,
//! building remote control applications or just automating tasks for user
//! interfaces unaccessible by a public API or scripting laguage.
//!
//! # Examples
//! ```no_run
//! use enigo::*;
//! let mut enigo = Enigo::new();
//! enigo.mouse_move_to(500, 200);
//! enigo.mouse_down(1);
//! enigo.mouse_move_relative(100, 100);
//! enigo.mouse_up(1);
//! enigo.key_sequence("hello world");
//! ```

#![deny(missing_docs)]

#[macro_use]
extern crate lazy_static;

#[cfg(target_os = "macos")]
extern crate libc;

// TODO(dustin) use interior mutability not &mut self

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
    /// value for `x` lets the mouse cursor go to the right. A positive value
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
    /// Push down the mouse button specified by the parameter `button`
    /// and holds it until it is released by [mouse_up]
    /// (trait.MouseControllable.html#tymethod.mouse_up).
    /// Calls to [mouse_move_to](trait.MouseControllable.html#tymethod.
    /// mouse_move_to) or
    /// [mouse_move_relative](trait.MouseControllable.html#tymethod.
    /// mouse_move_relative)
    /// will work like expected and will e.g. drag widgets or highlight text.
    ///
    /// buttons are currently mapped like 1=left, 2=right, 3=middle
    /// in the linux implementation. On macOS and Windows only leftclicks are
    /// generated regardless of the parameter button – this will change
    /// in future version of course.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_down(1);
    /// ```
    fn mouse_down(&mut self, button: u32);

    /// Lift up a pushed down mouse button
    ///
    /// Lift up a previously pushed down button (by invoking
    /// [mouse_down](trait.MouseControllable.html#tymethod.mouse_down)).
    /// If the button was not pushed down or consecutive calls without
    /// invoking [mouse_down](trait.MouseControllable.html#tymethod.mouse_down)
    /// will emit lift up events. It depends on the
    /// operating system whats actually happening – my guess is it will just
    /// get ignored.
    ///
    /// buttons are currently mapped like 1=left, 2=right, 3=middle
    /// in the linux implementation. On macOS and Windows only leftclicks are
    /// generated regardless of the parameter button – this will change
    /// in future version of course.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_up(1);
    /// ```
    fn mouse_up(&mut self, button: u32);

    /// Click a mouse button
    ///
    /// it's esentially just a consecutive invokation of
    /// [mouse_down](trait.MouseControllable.html#tymethod.mouse_down) followed
    /// by a [mouse_up](trait.MouseControllable.html#tymethod.mouse_up). Just
    /// for
    /// convenience.
    ///
    /// buttons are currently mapped like 1=left, 2=right, 3=middle
    /// in the linux implementation. On macOS and Windows only leftclicks are
    /// generated regardless of the parameter button – this will change
    /// in future version of course.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.mouse_click(1);
    /// ```
    fn mouse_click(&mut self, button: u32);

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

/// Keys to be used TODO(dustin): make a real documentation
#[derive(Debug)]
pub enum Key {
    ///shift key 
    SHIFT,
    ///tab key 
    TAB,
    ///return key 
    RETURN,
    ///unicode key
    UNICODE(String),
}

/// Representing an interface and a set of keyboard functions every
/// operating system implementation _should_ implement.
pub trait KeyboardControllable {
    /// Types the string
    ///
    /// Emits keystrokes such that the given string is inputted.
    ///
    /// This is currently only implemented on Linux and Windows (macOS waiting for core-graphics crate update).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// enigo.key_sequence("hello world");
    /// ```
    fn key_sequence(&mut self, sequence: &str);

    ///key_down
    fn key_down(&mut self, key: Key);

    ///key_down
    fn key_up(&mut self, key: Key);

    ///key_down
    fn key_click(&mut self, key: Key);
}

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub use win::Enigo;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::Enigo;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Enigo;

mod parser;


#[cfg(test)]
mod tests {}
