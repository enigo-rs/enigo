use enigo::{Enigo, EnigoSettings, Key, KeyboardControllable};
use std::thread;
use std::time::Duration;

// This example will do different things depending on the platform
fn main() {
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&EnigoSettings::default()).unwrap();

    #[cfg(target_os = "macos")]
    enigo.key_click(Key::Launchpad); // macOS: Open launchpad

    #[cfg(target_os = "linux")]
    enigo.key_click(Key::Meta); // linux: Open launcher

    #[cfg(target_os = "windows")]
    enigo.key_click(Key::Divide); // windows: Enter divide symbol (slash)
}
