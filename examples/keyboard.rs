extern crate enigo;
use enigo::{Enigo, KeyboardControllable, Key};
use std::thread;
use std::time::Duration;

fn main() {
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new();

    // write text
    enigo.key_sequence("Hello World! ❤️");

    // select all
    enigo.key_down(Key::Control);
    enigo.key_click(Key::Layout('a'));
    enigo.key_up(Key::Control);
}
