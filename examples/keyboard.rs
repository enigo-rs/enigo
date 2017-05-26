extern crate enigo;
use enigo::{Enigo, KeyboardControllable};
use std::{thread, time};

fn main() {
    let wait_time = time::Duration::from_millis(2000);
    let mut enigo = Enigo::new();

    thread::sleep(wait_time);
    enigo.key_sequence("hello world");s

