extern crate enigo;
use enigo::{Enigo, KeyboardControllable};
use std::{thread, time};

fn main() {
    let wait_time = time::Duration::from_millis(2000);
    thread::sleep(wait_time);
    let mut enigo = Enigo::new();

    // write text and select all
    enigo.key_sequence_parse("{+UNICODE}Hello World! ❤️{-UNICODE}{+CTRL}a{-CTRL}");
}
