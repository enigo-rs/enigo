use enigo::{Direction::Click, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

// This example will do different things depending on the platform
fn main() {
    env_logger::try_init().ok();
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    #[cfg(target_os = "macos")]
    enigo.key(Key::Launchpad, Click).unwrap(); // macOS: Open launchpad

    #[cfg(all(unix, not(target_os = "macos")))]
    enigo.key(Key::Meta, Click).unwrap(); // linux: Open launcher

    #[cfg(target_os = "windows")]
    {
        // windows: Enter divide symbol (slash)
        enigo.key(Key::Divide, Click).unwrap();

        // Windows: Simulate pressing and releasing the Control key.
        // 0x1D       = Left Control (normal scancode)
        // 0xE01D     = Right Control (extended scancode; 0xE0 prefix in the high byte)
        enigo.raw(0x1D, enigo::Direction::Click).unwrap(); // LControl
        enigo.raw(0x1D | 0xE000, enigo::Direction::Click).unwrap(); // RControl
    }
}
