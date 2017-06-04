extern crate enigo;
use enigo::{Enigo, KeyboardControllable, Key};
use std::{thread, time};

fn main() {
    let wait_time = time::Duration::from_millis(2000);
    let mut enigo = Enigo::new();

    //enigo.key_sequence("Hello World!");
    enigo.key_click(Key::LAYOUT("a".into()));
}