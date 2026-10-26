use enigo::{
    Coordinate::{Abs, Rel},
    Direction::{Click, Press, Release},
    Key, Keyboard, Mouse as _,
};

use serial_test::serial;

mod common;
use common::enigo_test::EnigoTest as Enigo;

#[test]
#[serial(input)]
fn integration_browser_events() {
    // `EnigoTest` launches Firefox on the same protocol as the compiled
    // backend. In a Wayland session `DISPLAY` is the compositor's Xwayland, so
    // the x11rb run talks to the same X server as Firefox.
    let mut enigo = Enigo::new(&common::settings());

    enigo.key(Key::UpArrow, Click).unwrap();
    enigo.key(Key::DownArrow, Click).unwrap();

    // Skip on macOS because Firefox emits another "pressed" event when CapsLock
    // is released
    #[cfg(not(target_os = "macos"))]
    {
        enigo.key(Key::CapsLock, Click).unwrap();
        enigo.key(Key::CapsLock, Click).unwrap();
    }
    // Skip on macOS because Firefox does not emit an event for them or there is
    // a bug
    #[cfg(not(target_os = "macos"))]
    {
        enigo.key(Key::Numlock, Click).unwrap();
        enigo.key(Key::Numlock, Click).unwrap();
        enigo.key(Key::Help, Click).unwrap();
    }
    enigo.text("TestText❤️").unwrap();
    #[cfg(not(any(
        target_os = "macos",
        all(
            target_os = "linux",
            any(
                feature = "xdo",
                feature = "wayland",
                feature = "libei",
                feature = "xdg_desktop"
            )
        )
    )))]
    // The simulation fails for these variants. The native backends can only
    // enter characters that the compositor keymap can map, and xdo cannot
    // enter an arbitrary Unicode key.
    enigo.key(Key::Unicode('❤'), Click).unwrap();
    enigo.key(Key::F1, Click).unwrap();
    enigo.key(Key::Control, Click).unwrap();
    enigo.key(Key::Backspace, Click).unwrap();
    enigo.key(Key::PageUp, Click).unwrap();

    enigo.text("❤️TestText❤️").unwrap();
    enigo.key(Key::Backspace, Press).unwrap();
    enigo.key(Key::Backspace, Release).unwrap();

    // Skip when using xdo feature because xdotools doesn't properly simulate
    // right modifiers
    // https://github.com/jordansissel/xdotool/issues/487
    #[cfg(not(feature = "xdo"))]
    {
        log::debug!("Test if the left and right versions of keys can get differentiated");
        enigo.key(Key::Control, Press).unwrap();
        enigo.key(Key::Control, Release).unwrap();
        enigo.key(Key::LControl, Press).unwrap();
        enigo.key(Key::LControl, Release).unwrap();
        enigo.key(Key::RControl, Press).unwrap();
        enigo.key(Key::RControl, Release).unwrap();
        enigo.key(Key::Shift, Click).unwrap();
        enigo.key(Key::LShift, Click).unwrap();
        enigo.key(Key::RShift, Click).unwrap();
    }

    log::debug!("Test mouse");
    // enigo.button(Button::Left, Click).unwrap();
    //
    // The xdg_desktop backend can only move the pointer relatively: absolute
    // motion needs a ScreenCast stream, which it does not start. See
    // KNOWN_ISSUES.md.
    #[cfg(not(feature = "xdg_desktop"))]
    {
        enigo.move_mouse(100, 100, Abs).unwrap();
        enigo.move_mouse(200, 200, Abs).unwrap();
        // location() is only implemented by the X11 backend.
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        {
            let (x, y) = enigo.location().unwrap();
            assert_eq!((200, 200), (x, y));
        }
    }
    enigo.move_mouse(20, 20, Rel).unwrap();
    enigo.move_mouse(-20, 20, Rel).unwrap();
    enigo.move_mouse(20, -20, Rel).unwrap();
    enigo.move_mouse(-20, -20, Rel).unwrap();

    // Ensure mouse events don't need a sleep afterwards to work
    #[cfg(not(feature = "xdg_desktop"))]
    {
        enigo.move_mouse(137, 59, Abs).unwrap();
        enigo.key(Key::UpArrow, Click).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        {
            let (x, y) = enigo.location().unwrap();
            assert_eq!((137, 59), (x, y));
        }
    }

    // Stalls on Windows, macOS and Linux with x11rb
    // enigo.scroll(1, Vertical).unwrap();
    // enigo.scroll(1, Horizontal).unwrap();

    // main_display() is not implemented by libei and returns a screenshot URI
    // that the xdg_desktop backend passes to image::open as a path.
    #[cfg(any(feature = "wayland", feature = "x11rb", feature = "xdo"))]
    enigo.main_display().unwrap();

    // location() is only implemented by the X11 backend.
    #[cfg(any(feature = "x11rb", feature = "xdo"))]
    enigo.location().unwrap();
}
