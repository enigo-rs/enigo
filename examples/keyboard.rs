extern crate enigo;
use enigo::{Enigo, KeyboardControllable};
use std::{thread, time};

#[allow(unused_mut)] // for now.
fn main() {
    let wait_time = time::Duration::from_millis(200);
    let mut enigo = Enigo::new();

    thread::sleep(wait_time);
    enigo.key_sequence("hello world");
}
