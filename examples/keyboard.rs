use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard,
};
use std::thread;
use std::time::Duration;
fn main() {
    env_logger::try_init().ok();

    let token_path = "/tmp/restore_token.txt";

    // Load saved token to only see the permissions dialog once
    let saved_token = if cfg!(all(
        feature = "platform_specific",
        any(feature = "libei", feature = "xdg_desktop")
    )) {
        std::fs::read_to_string(token_path).ok()
    } else {
        None
    };

    let settings = enigo::Settings {
        restore_token: saved_token,
        ..Default::default()
    };

    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&settings).unwrap();

    // write text
    enigo
        .text("Hello World! here is a lot of text  ❤️")
        .unwrap();

    // select all
    enigo.key(Key::Control, Press).unwrap();
    enigo.key(Key::Unicode('a'), Click).unwrap();
    enigo.key(Key::Control, Release).unwrap();

    // Save the new token (tokens rotate on each session)
    #[cfg(all(
        feature = "platform_specific",
        any(feature = "libei", feature = "xdg_desktop")
    ))]
    if let Some(token) = enigo.restore_token() {
        std::fs::write(token_path, token).unwrap();
    }
}
