use enigo::{Enigo, Extension};
use std::thread;
use std::time::Duration;

fn main() {
    let enigo = Enigo::new();

    thread::sleep(Duration::from_secs(4));
    println!("screen dimensions: {:?}", enigo.main_display_size());
    println!("mouse location: {:?}", enigo.mouse_location());
}
