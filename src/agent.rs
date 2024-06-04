use crate::{Axis, Button, Coordinate, Direction, Enigo, InputResult, Key, Keyboard, Mouse};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    /// Call the [`Keyboard::text`] fn with the string as text
    #[cfg_attr(feature = "serde", serde(alias = "T"))]
    #[cfg_attr(feature = "serde", serde(alias = "t"))]
    Text(String),
    /// Call the [`Keyboard::key`] fn with the given key and direction
    #[cfg_attr(feature = "serde", serde(alias = "K"))]
    #[cfg_attr(feature = "serde", serde(alias = "k"))]
    Key(
        Key,
        #[cfg_attr(feature = "serde", serde(default))] Direction,
    ),
    /// Call the [`Keyboard::raw`] fn with the given keycode and direction
    #[cfg_attr(feature = "serde", serde(alias = "R"))]
    #[cfg_attr(feature = "serde", serde(alias = "r"))]
    Raw(
        u16,
        #[cfg_attr(feature = "serde", serde(default))] Direction,
    ),
    /// Call the [`Mouse::button`] fn with the given mouse button and direction
    #[cfg_attr(feature = "serde", serde(alias = "B"))]
    #[cfg_attr(feature = "serde", serde(alias = "b"))]
    Button(
        Button,
        #[cfg_attr(feature = "serde", serde(default))] Direction,
    ),
    /// Call the [`Mouse::move_mouse`] fn. The first i32 is the value to move on
    /// the x-axis and the second i32 is the value to move on the y-axis. The
    /// coordinate defines if the given coordinates are absolute of relative to
    /// the current position of the mouse.
    #[cfg_attr(feature = "serde", serde(alias = "M"))]
    #[cfg_attr(feature = "serde", serde(alias = "m"))]
    MoveMouse(
        i32,
        i32,
        #[cfg_attr(feature = "serde", serde(default))] Coordinate,
    ),
    /// Call the [`Mouse::scroll`] fn.
    #[cfg_attr(feature = "serde", serde(alias = "S"))]
    #[cfg_attr(feature = "serde", serde(alias = "s"))]
    Scroll(i32, #[cfg_attr(feature = "serde", serde(default))] Axis),
}

pub trait Agent
where
    Self: Keyboard,
    Self: Mouse,
{
    /// Execute the action associated with the token. A [`Token::Text`] will
    /// enter text, a [`Token::Scroll`] will scroll and so forth. Have a look at
    /// the documentation of the [`Token`] enum for more information.
    ///
    /// # Errors
    ///
    /// Same as the individual functions. Have a look at [`InputResult`] for a
    /// list of possible errors
    fn execute(&mut self, token: &Token) -> InputResult<()> {
        match token {
            Token::Text(text) => self.text(text),
            Token::Key(key, direction) => self.key(*key, *direction),
            Token::Raw(keycode, direction) => self.raw(*keycode, *direction),
            Token::Button(button, direction) => self.button(*button, *direction),
            Token::MoveMouse(x, y, coordinate) => self.move_mouse(*x, *y, *coordinate),
            Token::Scroll(length, axis) => self.scroll(*length, *axis),
        }
    }
}

impl Agent for Enigo {}
