mod common;
use enigo::{
    Direction::{Press, Release},
    Key, Keyboard,
};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::try_init().ok();
    thread::sleep(Duration::from_secs(2));
    let mut enigo = common::new_enigo();

    enigo.key(Key::Unicode('a'), Press).unwrap();
    thread::sleep(Duration::from_secs(1));
    enigo.key(Key::Unicode('a'), Release).unwrap();
}
