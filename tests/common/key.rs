use std::sync::mpsc::Receiver;

use enigo::{
    Enigo, Key, Keyboard, Settings,
    {Direction::Press, Direction::Release},
};

use super::BrowserEvent;

pub fn run(recv: &Receiver<BrowserEvent>) {
    press(recv, Key::F1);
    press(recv, Key::Control);
    press(recv, Key::Backspace);
    // press(recv, Key::PageUp); Failing on Windows
}

fn press(recv: &Receiver<BrowserEvent>, key: Key) {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    enigo.key(key, Press).unwrap();
    let ev = recv
        .recv_timeout(std::time::Duration::from_millis(5000))
        .unwrap();
    if let BrowserEvent::KeyDown(pressed) = ev {
        assert_eq!(format!("{key:?}").to_lowercase(), pressed.to_lowercase());
    } else {
        panic!("Event wasn't KeyDown after mouse::press. {ev:?}");
    }
    enigo.key(key, Release).unwrap();
    let ev = recv
        .recv_timeout(std::time::Duration::from_millis(5000))
        .unwrap();
    if let BrowserEvent::KeyUp(pressed) = ev {
        assert_eq!(format!("{key:?}").to_lowercase(), pressed.to_lowercase());
    } else {
        panic!("Event wasn't KeyUp after mouse::press. {ev:?}");
    }
}
