mod browser;
mod browser_events;
pub mod enigo_test;

use enigo::Settings;

/// Settings used by the browser integration test.
///
/// The libei and xdg_desktop backends can reuse a portal restore token so the
/// permission dialog is only shown once. The session scripts seed one and
/// point `ENIGO_RESTORE_TOKEN_FILE` at it. This mirrors the helper the unit
/// tests use in `src/tests/mod.rs`.
pub fn settings() -> Settings {
    #[allow(unused_mut)]
    let mut settings = Settings::default();
    #[cfg(any(feature = "libei", feature = "xdg_desktop"))]
    {
        if let Ok(token) = std::env::var("ENIGO_RESTORE_TOKEN") {
            settings.restore_token = Some(token);
        } else if let Ok(token) = std::fs::read_to_string(token_path()) {
            settings.restore_token = Some(token.trim().to_owned());
        }
    }
    settings
}

#[cfg(any(feature = "libei", feature = "xdg_desktop"))]
pub fn token_path() -> std::path::PathBuf {
    std::env::var_os("ENIGO_RESTORE_TOKEN_FILE").map_or_else(
        || std::path::PathBuf::from("/tmp/enigo_restore_token"),
        std::path::PathBuf::from,
    )
}
