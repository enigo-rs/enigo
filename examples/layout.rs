use enigo::{Direction::Click, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::try_init().ok();
    thread::sleep(Duration::from_secs(4));
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    enigo.key(Key::PageDown, Click).unwrap();
    enigo.key(enigo::Key::UpArrow, Click).unwrap();
    enigo.key(enigo::Key::UpArrow, Click).unwrap();
    enigo.key(enigo::Key::DownArrow, Click).unwrap();
    enigo.key(enigo::Key::LeftArrow, Click).unwrap();
    enigo.key(enigo::Key::LeftArrow, Click).unwrap();
    enigo.key(enigo::Key::RightArrow, Click).unwrap();
    enigo.text("𝕊").unwrap(); // Special char which needs two u16s to be encoded
}
