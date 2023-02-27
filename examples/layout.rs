use enigo::{Enigo, Key, KeyboardControllable};
use std::thread;
use std::time::Duration;

fn main() {
    thread::sleep(Duration::from_secs(4));
    let mut enigo = Enigo::new();

    enigo.key_click(Key::PageDown);
    enigo.key_click(enigo::Key::UpArrow);
    enigo.key_click(enigo::Key::UpArrow);
    enigo.key_click(enigo::Key::DownArrow);
    enigo.key_click(enigo::Key::LeftArrow);
    enigo.key_click(enigo::Key::LeftArrow);
    enigo.key_click(enigo::Key::RightArrow);
    enigo.key_sequence("𝕊"); // Special char which needs two u16s to be encoded
}
