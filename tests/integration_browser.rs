use enigo::{
    Direction::{Click, Press, Release},
    Key, Keyboard, Settings,
};

mod common;
use common::enigo_test::EnigoTest as Enigo;

#[test]
fn integration_browser_events() {
    let mut enigo = Enigo::new(&Settings::default());

    enigo.maximize_browser();

    enigo.text("TestText❤️").unwrap(); // Fails on Windows (Message is empty???)
    enigo.key(Key::F1, Click).unwrap();
    enigo.key(Key::Control, Click).unwrap();
    enigo.key(Key::Backspace, Click).unwrap();
    enigo.key(Key::PageUp, Click).unwrap(); // Failing on Windows

    enigo.key(Key::Backspace, Press).unwrap();
    enigo.key(Key::Backspace, Release).unwrap();

    println!("Test mouse"); /*
                            enigo.button(Button::Left, Click).unwrap();
                            enigo.move_mouse(100, 100, Abs).unwrap();
                            enigo.move_mouse(200, 200, Abs).unwrap();
                            // let (x, y) = enigo.location().unwrap();
                            // assert_eq!((200, 200), (x, y));
                            // Relative moves fail on Windows
                            // For some reason the values are wrong
                            // enigo.move_mouse(20, 20, Rel).unwrap();
                            // enigo.move_mouse(-20, 20, Rel).unwrap();
                            // enigo.move_mouse(20, -20, Rel).unwrap();
                            // enigo.move_mouse(-20, -20, Rel).unwrap();
                            // enigo.scroll(1, Vertical).unwrap();
                            // enigo.scroll(1, Horizontal).unwrap(); Fails on Windows
                            enigo.main_display().unwrap();
                            enigo.location().unwrap(); */
}
