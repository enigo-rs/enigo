use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};

#[cfg_attr(feature = "x11rb", path = "x11rb.rs")]
#[cfg_attr(not(feature = "x11rb"), path = "xdo.rs")]
mod x11_impl;
use self::x11_impl::EnigoX11;

pub struct Enigo {
    x11: Option<EnigoX11>,
}

impl Enigo {
    /// Get the delay per keypress.
    /// Default value is 12.
    /// This is Linux-specific.
    #[must_use]
    pub fn delay(&self) -> u32 {
        self.x11.as_ref().unwrap().delay()
    }
    /// Set the delay per keypress.
    /// This is Linux-specific.
    pub fn set_delay(&mut self, delay: u32) {
        self.x11.as_mut().unwrap().set_delay(delay);
    }
}

impl Default for Enigo {
    /// Create a new `EnigoX11` instance
    fn default() -> Self {
        let x11 = Some(EnigoX11::default());
        Self { x11 }
    }
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        self.x11.as_mut().unwrap().mouse_move_to(x, y);
    }
    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        self.x11.as_mut().unwrap().mouse_move_relative(x, y);
    }
    fn mouse_down(&mut self, button: MouseButton) {
        self.x11.as_mut().unwrap().mouse_down(button);
    }
    fn mouse_up(&mut self, button: MouseButton) {
        self.x11.as_mut().unwrap().mouse_up(button);
    }
    fn mouse_click(&mut self, button: MouseButton) {
        self.x11.as_mut().unwrap().mouse_click(button);
    }
    fn mouse_scroll_x(&mut self, length: i32) {
        self.x11.as_mut().unwrap().mouse_scroll_x(length);
    }
    fn mouse_scroll_y(&mut self, length: i32) {
        self.x11.as_mut().unwrap().mouse_scroll_y(length);
    }
    fn main_display_size(&self) -> (i32, i32) {
        self.x11.as_ref().unwrap().main_display_size()
    }
    fn mouse_location(&self) -> (i32, i32) {
        self.x11.as_ref().unwrap().mouse_location()
    }
}

impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        self.x11.as_mut().unwrap().key_sequence(sequence);
    }
    fn key_down(&mut self, key: Key) {
        self.x11.as_mut().unwrap().key_down(key);
    }
    fn key_up(&mut self, key: Key) {
        self.x11.as_mut().unwrap().key_up(key);
    }
    fn key_click(&mut self, key: Key) {
        self.x11.as_mut().unwrap().key_click(key);
    }
}
