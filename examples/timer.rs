use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use std::{
    thread,
    time::{Duration, Instant},
};

fn main() {
    env_logger::try_init().ok();
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    let now = Instant::now();

    // write text
    enigo.text("Hello World! ❤️").unwrap();

    let time = now.elapsed();
    println!("{time:?}");

    // select all
    let control_or_command = if cfg!(target_os = "macos") {
        Key::Meta
    } else {
        Key::Control
    };
    enigo.key(control_or_command, Press).unwrap();
    enigo.key(Key::Unicode('a'), Click).unwrap();
    enigo.key(control_or_command, Release).unwrap();
}
