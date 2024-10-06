use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::init();
    thread::sleep(Duration::from_secs(1));
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    // write text
    enigo
        // .text("Test with lots of newlines")
        .text("Test\nwith \nlots \nof \nnewlines🔥")
        .unwrap();

    enigo.key(Key::Unicode('a'), Click).unwrap();
    enigo.key(Key::Return, Click).unwrap();
    // enigo.key(Key::Unicode('🔥'), Click).unwrap();
}
