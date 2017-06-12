extern crate enigo;
use enigo::{Enigo, KeyboardControllable, Key};
use std::{thread, time};

fn main() {
    let wait_time = time::Duration::from_millis(2000);
    thread::sleep(wait_time);
    let mut enigo = Enigo::new();

    //write text and select all
    enigo.key_sequence_parse("{+SHIFT}hello{-SHIFT}{+CTRL}a{-CTRL}");
}
