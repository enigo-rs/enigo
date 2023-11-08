use enigo::{
    Enigo, Key, Keyboard, Settings,
    {Direction::Click, Direction::Press, Direction::Release},
};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::init();
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    // write text
    enigo.text("Hello World! here is a lot of text  ❤️").unwrap();

    // select all
    enigo.key(Key::Control, Press).unwrap();
    enigo.key(Key::Unicode('a'), Click).unwrap();
    enigo.key(Key::Control, Release).unwrap();
}
