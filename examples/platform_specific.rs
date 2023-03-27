use enigo::{Enigo, Key, KeyboardControllable};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "Windows")]
use enigo::{MouseButton, MouseControllable};

fn main() {
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new();

    #[cfg(target_os = "macos")]
    enigo.key_click(Key::Launchpad); // Opens launchpad

    #[cfg(target_os = "linux")]
    enigo.key_click(Key::Meta); // Opens launcher

    #[cfg(target_os = "Windows")]
    enigo.mouse_click(MouseButton::XButton1); // Clicks the 4th mouse button
                                              // (usually Browser_Back)
}
