use enigo::{Enigo, Key, KeyboardControllable, Settings};
use std::thread;
use std::time::Duration;

// This example will do different things depending on the platform
fn main() {
    env_logger::init();
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    #[cfg(target_os = "macos")]
    enigo.key_click(Key::Launchpad); // macOS: Open launchpad

    #[cfg(target_os = "linux")]
    enigo.key_click(Key::Meta); // linux: Open launcher

    #[cfg(target_os = "windows")]
    {
        use enigo::KeyboardControllableNext;

        // windows: Enter divide symbol (slash)
        enigo.key_click(Key::Divide);

        // windows: Press and release the NumLock key. Without the EXT bit set, it would
        // enter the Pause key
        enigo.raw(45 | enigo::EXT, enigo::Direction::Click).unwrap();
    }
}
