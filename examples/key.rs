use enigo::{Enigo, EnigoSettings, Key, KeyboardControllable};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::init();
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();

    enigo.key_down(Key::Unicode('a'));
    thread::sleep(Duration::from_secs(1));
    enigo.key_up(Key::Unicode('a'));
}
