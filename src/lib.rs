#[macro_use]
extern crate lazy_static;

pub trait MouseControllable {
    fn mouse_move_to(&self, x: i32, y: i32);
    fn mouse_move_relative(&self, x: i32, y: i32);
    fn mouse_down(&self, button: u32);
    fn mouse_up(&self, button: u32);
    fn mouse_click(&self, button: u32);
    fn mouse_scroll_x(&self, length: i32);
    fn mouse_scroll_y(&self, length: i32);
}

pub trait KeyboardControllable {
    fn key_sequence(&self, sequence: &str);
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::Enigo;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Enigo;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
