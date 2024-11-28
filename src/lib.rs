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
//!   size
//! - [`Enigo`] (struct): implements the two traits [`Keyboard`] and [`Mouse`]
//!
//! This crate previously included a simple DSL. This is no longer the case. In order to simplify the codebase and also allow serializing objects, you can now serialize and deserialize most enums and structs of this crate. You can use this instead of the DSL. This feature is hidden behind the `serde` feature. Have a look at the `serde` example to see how to use it to serialize Tokens in the [RON](https://crates.io/crates/ron) format.

//! # Examples
//! ```no_run
//! use enigo::{
//!     Button, Coordinate,
//!     Direction::{Click, Press, Release},
//!     Enigo, Key, Keyboard, Mouse, Settings,
//! };
//! let mut enigo = Enigo::new(&Settings::default()).unwrap();
//! // Paste
//! enigo.key(Key::Control, Press);
//! enigo.key(Key::Unicode('v'), Click);
//! enigo.key(Key::Control, Release);
//! // Do things with the mouse
//! enigo.move_mouse(500, 200, Coordinate::Abs);
//! enigo.button(Button::Left, Press);
//! enigo.move_mouse(100, 100, Coordinate::Rel);
//! enigo.button(Button::Left, Release);
//! // Enter text
//! enigo.text("hello world");
//! ```

#![deny(clippy::pedantic)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(deprecated)]

const DEFAULT_BUS_UPDATE_RATE: i32 = 125; // in HZ
const DEFAULT_POINTER_RESOLUTION: i32 = 400; // in mickey/inch
const DEFAULT_SCREEN_UPDATE_RATE: i32 = 75; // in HZ
const DEFAULT_SCREEN_RESOLUTION: i32 = 96; // in DPI

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

#[cfg(target_os = "windows")]
use fixed::{types::extra::U16, FixedI32};

use log::{debug, error};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(test)]
use strum_macros::EnumIter;

/// This crate contains the [`crate::agent::Token`] struct and the
/// [`crate::agent::Agent`] trait. A token is an instruction for the [`Enigo`]
/// struct to do something. If you want Enigo to simulate input, you then have
/// to tell the enigo struct to [`crate::agent::Agent::execute`] the token. Have
/// a look at the `serde` example if you'd like to read some code to see how it
/// works.
pub mod agent;

#[cfg_attr(all(unix, not(target_os = "macos")), path = "linux/mod.rs")]
#[cfg_attr(target_os = "macos", path = "macos/mod.rs")]
#[cfg_attr(target_os = "windows", path = "win/mod.rs")]
mod platform;
pub use platform::Enigo;

#[cfg(target_os = "windows")]
pub use platform::{
    mouse_curve, mouse_speed, mouse_thresholds_and_acceleration, set_mouse_curve, set_mouse_speed,
    set_mouse_thresholds_and_acceleration, system_dpi, EXT,
};

mod keycodes;
/// Contains the available keycodes
pub use keycodes::Key;

/// Arbitrary value to be able to distinguish events created by enigo
pub const EVENT_MARKER: u32 = 100;

/// Represents a mouse button and is used in e.g
/// [`Mouse::button`].

// Warning! If there are ANY CHANGES to this enum, we
// need to change the size of the array in the macOS implementation of the Enigo
// struct that stores the nth click for each Button
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(test, derive(EnumIter))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[doc(alias = "MouseButton")]
pub enum Button {
    /// Left mouse button
    #[cfg_attr(feature = "serde", serde(alias = "L"))]
    #[cfg_attr(feature = "serde", serde(alias = "l"))]
    #[default]
    Left,
    /// Middle mouse button
    #[cfg_attr(feature = "serde", serde(alias = "M"))]
    #[cfg_attr(feature = "serde", serde(alias = "m"))]
    Middle,
    /// Right mouse button
    #[cfg_attr(feature = "serde", serde(alias = "R"))]
    #[cfg_attr(feature = "serde", serde(alias = "r"))]
    Right,
    /// 4th mouse button. Typically performs the same function as `Browser_Back`
    #[cfg_attr(feature = "serde", serde(alias = "B"))]
    #[cfg_attr(feature = "serde", serde(alias = "b"))]
    Back,
    /// 5th mouse button. Typically performs the same function as
    /// `Browser_Forward`
    #[cfg_attr(feature = "serde", serde(alias = "F"))]
    #[cfg_attr(feature = "serde", serde(alias = "f"))]
    Forward,

    /// Scroll up button. It is better to use the
    /// [`Mouse::scroll`] method to scroll.
    #[cfg_attr(feature = "serde", serde(alias = "SU"))]
    #[cfg_attr(feature = "serde", serde(alias = "su"))]
    ScrollUp,
    /// Scroll down button. It is better to use the
    /// [`Mouse::scroll`] method to scroll.
    #[cfg_attr(feature = "serde", serde(alias = "SD"))]
    #[cfg_attr(feature = "serde", serde(alias = "sd"))]
    ScrollDown,
    /// Scroll left button. It is better to use the
    /// [`Mouse::scroll`] method to scroll.
    #[cfg_attr(feature = "serde", serde(alias = "SL"))]
    #[cfg_attr(feature = "serde", serde(alias = "sl"))]
    ScrollLeft,
    /// Scroll right button. It is better to use the
    /// [`Mouse::scroll`] method to scroll.
    #[cfg_attr(feature = "serde", serde(alias = "SR"))]
    #[cfg_attr(feature = "serde", serde(alias = "sr"))]
    ScrollRight,
}

impl fmt::Debug for Enigo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Enigo")
    }
}

/// The direction of a key or button
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    #[cfg_attr(feature = "serde", serde(alias = "P"))]
    #[cfg_attr(feature = "serde", serde(alias = "p"))]
    #[cfg_attr(feature = "serde", serde(alias = "Pressed"))]
    #[cfg_attr(feature = "serde", serde(alias = "pressed"))]
    Press,
    #[cfg_attr(feature = "serde", serde(alias = "R"))]
    #[cfg_attr(feature = "serde", serde(alias = "r"))]
    #[cfg_attr(feature = "serde", serde(alias = "Released"))]
    #[cfg_attr(feature = "serde", serde(alias = "released"))]
    Release,
    /// Equivalent to a press followed by a release
    #[cfg_attr(feature = "serde", serde(alias = "C"))]
    #[cfg_attr(feature = "serde", serde(alias = "c"))]
    #[cfg_attr(feature = "serde", serde(alias = "Clicked"))]
    #[cfg_attr(feature = "serde", serde(alias = "clicked"))]
    #[default]
    Click,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// Specifies the axis for scrolling
pub enum Axis {
    #[cfg_attr(feature = "serde", serde(alias = "H"))]
    #[cfg_attr(feature = "serde", serde(alias = "h"))]
    Horizontal,
    #[cfg_attr(feature = "serde", serde(alias = "V"))]
    #[cfg_attr(feature = "serde", serde(alias = "v"))]
    #[default]
    Vertical,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// Specifies if a coordinate is relative or absolute
pub enum Coordinate {
    #[doc(alias = "Absolute")]
    #[cfg_attr(feature = "serde", serde(alias = "A"))]
    #[cfg_attr(feature = "serde", serde(alias = "a"))]
    #[default]
    Abs,
    #[doc(alias = "Relative")]
    #[cfg_attr(feature = "serde", serde(alias = "R"))]
    #[cfg_attr(feature = "serde", serde(alias = "r"))]
    Rel,
}

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
    /// Do not use this directly. Use the [`Keyboard::text`] function.
    ///
    /// Enter the whole text string instead of entering individual keys
    /// This is much faster if you type longer text at the cost of keyboard
    /// shortcuts not getting recognized.
    ///
    /// # Errors
    /// Have a look at the documentation of [`InputError`] to see under which
    /// conditions an error will be returned.
    #[doc(hidden)]
    fn fast_text(&mut self, text: &str) -> InputResult<Option<()>>;

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
        let fast_text_res = self.fast_text(text);
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
    /// layout. You have to make sure of that yourself. This can be useful if
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
    /// If you use absolute coordinates, the top left corner of your monitor
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
    /// The application does not have the permission to simulate input
    NoPermission,
    /// Error when receiving a reply
    Reply,
    /// The keymap is full, so there was no space to map any keycodes to keysyms
    NoEmptyKeycodes,
}

impl Display for NewConError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let string = match self {
            NewConError::EstablishCon(e) => format!("no connection could be established: ({e})"),
            NewConError::NoPermission => {
                "the application does not have the permission to simulate input".to_string()
            }
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

/// Settings for creating the Enigo struct and it's behavior
#[allow(dead_code)] // It is not dead code on other platforms
#[allow(clippy::struct_excessive_bools)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settings {
    /// Sleep delay on Linux X11
    pub linux_delay: u32,
    /// Display name to connect to when using Linux X11
    pub x11_display: Option<String>,
    /// Display name to connect to when using Linux Wayland
    pub wayland_display: Option<String>,
    /// Arbitrary value to be able to distinguish events created by enigo
    /// All events will be marked with this value in the dwExtraInfo field
    pub windows_dw_extra_info: Option<usize>,
    /// Arbitrary value to be able to distinguish events created by enigo
    /// All events will be marked with this value in the
    /// `EVENT_SOURCE_USER_DATA` field
    pub event_source_user_data: Option<i64>,
    /// Set this to true if you want all held keys to get released when Enigo
    /// gets dropped. The default is true.
    pub release_keys_when_dropped: bool,
    /// Open a prompt to ask the user for the permission to simulate input if
    /// they are missing. This only works on macOS. The default is true.
    pub open_prompt_to_get_permissions: bool,
    /// The simulated input is independent from the pressed keys on the
    /// physical keyboard. This only works on macOS.
    /// The default is true. If the Shift key for example is pressed,
    /// following simulated input will not be capitalized.
    pub independent_of_keyboard_state: bool,
    /// If this is set to true, the relative mouse motion will be subject to the
    /// settings for mouse speed and acceleration level. An end user sets
    /// these values using the Mouse application in Control Panel. An
    /// application obtains and sets these values with the
    /// `windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW`
    /// function. The default value is false.
    pub windows_subject_to_mouse_speed_and_acceleration_level: bool,
}

impl Default for Settings {
    fn default() -> Self {
        debug!("using default settings");
        Self {
            linux_delay: 12,
            x11_display: None,
            wayland_display: None,
            windows_dw_extra_info: None,
            event_source_user_data: None,
            release_keys_when_dropped: true,
            open_prompt_to_get_permissions: true,
            independent_of_keyboard_state: true,
            windows_subject_to_mouse_speed_and_acceleration_level: false,
        }
    }
}

/// IMPORTANT: This function does NOT simulate a relative mouse movement.
///
/// Windows: If `windows_subject_to_mouse_speed_and_acceleration_level` is set
/// to `false`, relative mouse movement is influenced by the system's mouse
/// speed and acceleration settings. This function calculates the new location
/// based on the relative movement but does not guarantee the exact future
/// location. It is intended to estimate the expected location and is useful for
/// testing relative mouse movement.
//
// Quote from documentation (http://web.archive.org/web/20241118235853/https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mouse_event):
// Relative mouse motion is subject to the settings for mouse speed and
// acceleration level. An end user sets these values using the Mouse application
// in Control Panel. An application obtains and sets these values with the
// SystemParametersInfo function.
//
// The system applies two tests to the specified relative mouse motion when
// applying acceleration. If the specified distance along either the x or y axis
// is greater than the first mouse threshold value, and the mouse acceleration
// level is not zero, the operating system doubles the distance. If the
// specified distance along either the x- or y-axis is greater than the second
// mouse threshold value, and the mouse acceleration level is equal to two, the
// operating system doubles the distance that resulted from applying the first
// threshold test. It is thus possible for the operating system to multiply
// relatively-specified mouse motion along the x- or y-axis by up to four times.
//
// Once acceleration has been applied, the system scales the resultant value by
// the desired mouse speed. Mouse speed can range from 1 (slowest) to 20
// (fastest) and represents how much the pointer moves based on the distance the
// mouse moves. The default value is 10, which results in no additional
// modification to the mouse motion.
//
// TODO: Improve the calculation of the new mouse location so that we can
// predict it exeactly. Right now there seem to be rounding errors and the
// location sometimes is off by 1
#[must_use]
pub fn win_future_rel_mouse_location(
    x: i32,
    y: i32,
    threshold1: i32,
    threshold2: i32,
    acceleration_level: i32,
    mouse_speed: i32,
) -> (i32, i32) {
    let mouse_speed = mouse_speed as f64;

    let mut multiplier = 1;
    if acceleration_level != 0 && (x.abs() > threshold1 || y.abs() > threshold1) {
        multiplier = 2;
    }
    if acceleration_level == 2 && (x.abs() > threshold2 || y.abs() > threshold2) {
        multiplier *= 2;
    }
    debug!("multiplier: {multiplier}");

    let accelerated_x = (multiplier * x) as f64;
    let accelerated_y = (multiplier * y) as f64;
    debug!("accelerated_x: {accelerated_x}, accelerated_y: {accelerated_y}");

    let scaled_x = (accelerated_x * (mouse_speed / 10.0)).round() as i32;
    let scaled_y = (accelerated_y * (mouse_speed / 10.0)).round() as i32;

    (scaled_x, scaled_y)
}

/// Get the scaling multipliers associated with the pointer speed slider
/// (sensitivity)
// Source https://web.archive.org/web/20241123143225/https://www.esreality.com/index.php?a=post&id=1945096
pub fn update_mouse_speed(
    mouse_sensitivity: i32,
    // enhanced_pointer_precision: i32,
) -> Result<f32, InputError> {
    let speed = match mouse_sensitivity {
        i32::MIN..1 | 21..=i32::MAX => {
            return Err(InputError::InvalidInput(
                "Mouse sensitivity must be between 1 and 20.",
            ));
        }
        1 => (0.03125, 0.1),
        2 => (0.0625, 0.2),
        3 => (0.125, 0.3), // Guessed value
        4 => (0.25, 0.4),
        5 => (0.375, 0.5), // Guessed value
        6 => (0.5, 0.6),
        7 => (0.625, 0.7), // Guessed value
        8 => (0.75, 0.8),
        9 => (0.875, 0.9), // Guessed value
        10 => (1.0, 1.0),
        11 => (1.25, 1.1), // Guessed value
        12 => (1.5, 1.2),
        13 => (1.75, 1.3), // Guessed value
        14 => (2.0, 1.4),
        15 => (2.25, 1.5), // Guessed value
        16 => (2.5, 1.6),
        17 => (2.75, 1.7), // Guessed value
        18 => (3.0, 1.8),
        19 => (3.25, 1.9), // Guessed value
        20 => (3.5, 2.0),
    };
    if true {
        Ok(speed.1)
    } else {
        Ok(speed.0)
    }
}

/// Calculate the next location of the mouse using the smooth mouse curve and
/// the remaining subpixels
#[cfg(target_os = "windows")]
#[must_use]
pub fn calc_ballistic_location(
    x: i32,
    y: i32,
    remainder_x: FixedI32<U16>,
    remainder_y: FixedI32<U16>,
    mouse_speed: FixedI32<U16>,
    smooth_mouse_curve: [[FixedI32<U16>; 5]; 2],
) -> Option<(
    (FixedI32<U16>, FixedI32<U16>),
    (FixedI32<U16>, FixedI32<U16>),
)> {
    if x == 0 && y == 0 {
        return Some((
            (FixedI32::<U16>::from_num(0), FixedI32::<U16>::from_num(0)),
            (remainder_x, remainder_y),
        ));
    }

    // The following list summarizes the ballistic algorithm used in Windows XP, in
    // sequence and was taken unchanged from https://web.archive.org/web/20100315061825/http://www.microsoft.com/whdc/archive/pointer-bal.mspx

    // Summary of the Ballistic Algorithm for Windows XP
    //
    // 1. When the system is started or the mouse speed setting is changed, the
    //    translation table is recalculated and stored. The parent values are stored
    //    in the registry and in physical units that are now converted to virtual
    //    units by scaling them based on system parameters: screen refresh rate,
    //    screen resolution, default values of the mouse refresh rate (USB 125 Hz),
    //    and default mouse resolution (400 dpi). (This may change in the future to
    //    actually reflect the pointer parameters.) Then the curves are speed-scaled
    //    based on the pointer slider speed setting in the Mouse Properties dialog
    //    box (Pointer Options tab).
    let scaled_mouse_curve = scale_mouse_curve(smooth_mouse_curve, mouse_speed);

    // 2. Incoming mouse X and Y values are first converted to fixed-point 16.16
    //    format.
    let mut x_fix = FixedI32::<U16>::checked_from_num(x).unwrap();
    let mut y_fix = FixedI32::<U16>::checked_from_num(y).unwrap();

    // 3. The magnitude of the X and Y values is calculated and used to look up the
    //    acceleration value in the lookup table.
    let magnitude = i32::isqrt(x.checked_mul(x).unwrap() + y.checked_mul(y).unwrap());
    // println!(" magnitude: {:?}", magnitude);
    let magnitude = FixedI32::<U16>::checked_from_num(magnitude).unwrap();
    println!(" magnitude: {:?}", magnitude.to_num::<f64>());

    // 4. The lookup table consists of six points (the first is [0,0]). Each point
    //    represents an inflection point, and the lookup value typically resides
    //    between the inflection points, so the acceleration multiplier value is
    //    interpolated.
    let acceleration = get_acceleration(magnitude, scaled_mouse_curve).unwrap();
    println!(" acceleration: {:?}", acceleration.to_num::<f64>());

    if acceleration == 0 {
        return Some((
            (FixedI32::<U16>::from_num(0), FixedI32::<U16>::from_num(0)),
            (remainder_x, remainder_y),
        ));
    }

    // 5. The remainder from the previous calculation is added to both X and Y, and
    //    then the acceleration multiplier is applied to transform the values. The
    //    remainder is stored to be added to the next incoming values, which is how
    //    subpixilation is enabled.

    // TODO: I interpret the doc to say that the multiplication should be done AFTER
    // adding the remainder. Doesnt make sense to me. Double check this
    x_fix = x_fix.checked_mul(acceleration).unwrap();
    y_fix = y_fix.checked_mul(acceleration).unwrap();

    x_fix = x_fix.checked_add(remainder_x).unwrap();
    y_fix = y_fix.checked_add(remainder_y).unwrap();

    let remainder_x = x_fix.frac();
    let remainder_y = y_fix.frac();

    // 6. The values are sent on to move the pointer.
    Some(((x_fix, y_fix), (remainder_x, remainder_y)))

    // 7. If the feature is turned off (by clearing the Enhance pointer
    //    precision check box underneath the mouse speed slider in the Mouse
    //    Properties dialog box [Pointer Options tab]), the system works as it
    //    did before without acceleration. All these functions are bypassed, and
    //    the system takes the raw mouse values and multiplies them by a scalar
    //    set based on the speed slider setting.
}

#[cfg(target_os = "windows")]
fn get_acceleration(
    magnitude: FixedI32<U16>,
    smooth_mouse_curve: [[FixedI32<U16>; 5]; 2],
) -> Option<FixedI32<U16>> {
    if magnitude == FixedI32::<U16>::from_num(0) {
        return Some(FixedI32::<U16>::from_num(0));
    }

    let mut gain_factor = FixedI32::<U16>::from_num(0);

    let (mut x1, mut y1);
    let (mut x2, mut y2);

    // For each pair of points...
    for i in 0..5 {
        (x1, y1) = (smooth_mouse_curve[0][i], smooth_mouse_curve[1][i]);
        (x2, y2) = (smooth_mouse_curve[0][i + 1], smooth_mouse_curve[1][i + 1]);

        if x1 == x2 {
            continue;
        }

        let x = std::cmp::min(magnitude, x2);
        // Linear interpolation
        gain_factor += (x - x1) * ((y2 - y1) / (x2 - x1));

        // Check if x is within the range of the current segment
        if magnitude <= x2 {
            break;
        }
    }
    gain_factor /= magnitude;
    Some(gain_factor)
}

fn physical_mouse_speed(mickey: i32) -> Option<FixedI32<U16>> {
    let mickey = FixedI32::<U16>::from_num(mickey);
    let bus_update_rate = FixedI32::<U16>::from_num(DEFAULT_BUS_UPDATE_RATE);
    let pointer_resolution = FixedI32::<U16>::from_num(DEFAULT_POINTER_RESOLUTION);

    let factor = bus_update_rate.checked_div(pointer_resolution)?;
    let speed = mickey.checked_mul(factor)?;
    Some(speed)
}

fn virtual_pointer_speed(mickey: i32) -> Option<FixedI32<U16>> {
    let mickey = FixedI32::<U16>::from_num(mickey);
    let screen_update_rate = FixedI32::<U16>::from_num(DEFAULT_SCREEN_UPDATE_RATE);
    let screen_resolution = FixedI32::<U16>::from_num(DEFAULT_SCREEN_RESOLUTION);

    let factor = screen_update_rate.checked_div(screen_resolution)?;
    let speed = mickey.checked_mul(factor)?;
    Some(speed)
}

fn scale_mouse_curve(
    smooth_mouse_curve: [[FixedI32<U16>; 5]; 2],
    mouse_speed: FixedI32<U16>,
) -> [[FixedI32<U16>; 5]; 2] {
    // let bus_update_rate = FixedI32::<U16>::from_num(DEFAULT_BUS_UPDATE_RATE);
    // let pointer_resolution =
    // FixedI32::<U16>::from_num(DEFAULT_POINTER_RESOLUTION); let p_mouse_factor
    // = bus_update_rate.checked_div(pointer_resolution)?;
    let p_mouse_factor = FixedI32::<U16>::from_num(3.5);
    let screen_update_rate = FixedI32::<U16>::from_num(DEFAULT_SCREEN_UPDATE_RATE);
    //let screen_resolution = system_dpi();
    //println!("DPI: {screen_resolution}");
    // let screen_resolution = FixedI32::<U16>::from_num(screen_resolution);
    let screen_resolution = FixedI32::<U16>::from_num(DEFAULT_SCREEN_RESOLUTION);
    let v_pointer_factor = screen_update_rate.checked_div(screen_resolution).unwrap();
    // let v_pointer_factor = FixedI32::<U16>::from_num(150 as f32 / 96 as f32);

    let scaled_smooth_mouse_curve_x: Vec<_> = smooth_mouse_curve[0]
        .iter()
        .map(|&v| v.checked_mul(p_mouse_factor).unwrap())
        .collect();
    let scaled_smooth_mouse_curve_y: Vec<_> = smooth_mouse_curve[1]
        .iter()
        .map(|&v| {
            v.checked_mul(v_pointer_factor)
                .unwrap()
                .checked_mul(mouse_speed)
                .unwrap()
        })
        .collect();

    let smooth_mouse_curve = [
        scaled_smooth_mouse_curve_x.try_into().unwrap(),
        scaled_smooth_mouse_curve_y.try_into().unwrap(),
    ];

    println!("Scaled smooth mouse: {smooth_mouse_curve:?}");
    smooth_mouse_curve
}

#[cfg(test)]
/// Module containing all the platform independent tests for the traits
mod tests;
