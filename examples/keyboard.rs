use enigo::{Enigo, EnigoSettings, Key, KeyboardControllable};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::init();
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();

    // write text
    enigo.key_sequence("Hello World! here is a lot of text  ❤️");

    // select all
    enigo.key_down(Key::Control);
    enigo.key_click(Key::Layout('a'));
    enigo.key_up(Key::Control);
}
