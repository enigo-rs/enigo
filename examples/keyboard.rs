use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::try_init().ok();
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    // write text
    enigo
        .text("Hello World! here is a lot of text  ❤️")
        .unwrap();

    // select all
    enigo.key(Key::Unicode('p'), Click).unwrap();
    enigo.key(Key::Unicode('o'), Click).unwrap();
    enigo.key(Key::Unicode('p'), Click).unwrap();
}
