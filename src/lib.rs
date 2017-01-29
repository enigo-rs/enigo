pub trait MouseControllable {
    fn new() -> Self;
    fn mouse_move_to(&mut self, x: i32, y: i32);
    fn mouse_move_relative(&mut self, x: i32, y: i32);
    fn mouse_down(&mut self, button: u32);
    fn mouse_up(&mut self, button: u32);
    fn mouse_click(&mut self, button: u32);
    fn mouse_scroll_x(&mut self, length: i32);
    fn mouse_scroll_y(&mut self, length: i32);
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Enigo;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
