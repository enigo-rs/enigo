extern crate enigo;
use enigo::{Enigo, KeyboardControllable, Key};
use std::{thread, time};

fn main() {
    let wait_time = time::Duration::from_millis(2000);
    thread::sleep(wait_time);
    let mut enigo = Enigo::new();

    //write text
    enigo.key_sequence("Hello World! ❤️");
    
    //select all
    enigo.key_down(Key::Control);
    enigo.key_click(Key::Layout("a".into()));
    enigo.key_up(Key::Control);
}
