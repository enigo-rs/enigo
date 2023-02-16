use enigo::{Enigo, Key, KeyboardControllable, MouseControllable};
use std::thread;
use std::time::Duration;

fn main() {
    thread::sleep(Duration::from_secs(4));
    let mut enigo = Enigo::new();

    println!("Pressing pagedown");
    enigo.key_click(Key::PageDown);

    enigo.key_click(enigo::Key::UpArrow);
    enigo.key_click(enigo::Key::DownArrow);
    enigo.key_click(enigo::Key::LeftArrow);
    enigo.key_click(enigo::Key::RightArrow);
}
