#[macro_use]
extern crate lazy_static;

//TODO(dustin) use interior mutability not &mut self
pub trait MouseControllable {
    fn mouse_move_to(&mut self, x: i32, y: i32);
    fn mouse_move_relative(&mut self, x: i32, y: i32);
    fn mouse_down(&mut self, button: u32);
    fn mouse_up(&mut self, button: u32);
    fn mouse_click(&mut self, button: u32);
    fn mouse_scroll_x(&mut self, length: i32);
    fn mouse_scroll_y(&mut self, length: i32);
}

pub trait KeyboardControllable {
    fn key_sequence(&self, sequence: &str);
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
