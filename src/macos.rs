

use super::MouseControllable;

pub struct Enigo {
}

impl Enigo {
    fn new() -> Self {
        Enigo{}
    }
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&self, x: i32, y: i32) {
        unimplemented!()
    }
    fn mouse_move_relative(&self, x: i32, y: i32) {
        unimplemented!()
    }
    fn mouse_down(&self, button: u32) {
        unimplemented!()
    }
    fn mouse_up(&self, button: u32) {
        unimplemented!()
    }
    fn mouse_click(&self, button: u32) {
        unimplemented!()
    }
    fn mouse_scroll_x(&self, length: i32) {
        unimplemented!()
    }
    fn mouse_scroll_y(&self, length: i32) {
        unimplemented!()
    }
}