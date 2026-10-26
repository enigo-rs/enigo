use std::time::Duration;

/// Module containing all the tests related to the `Keyboard` trait
/// that are platform independent
mod keyboard;
/// Module containing all the tests related to the `Mouse` trait
/// that are platform independent
mod mouse;

/// Settings used by the input tests.
///
/// The libei and xdg_desktop backends can reuse a portal restore token so the
/// permission dialog is only shown once. Set `ENIGO_RESTORE_TOKEN` or
/// `ENIGO_RESTORE_TOKEN_FILE` when running those test suites.
pub(crate) fn settings() -> crate::Settings {
    #[allow(unused_mut)]
    let mut settings = crate::Settings::default();
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
fn token_path() -> std::path::PathBuf {
    std::env::var_os("ENIGO_RESTORE_TOKEN_FILE").map_or_else(
        || std::path::PathBuf::from("/tmp/enigo_restore_token"),
        std::path::PathBuf::from,
    )
}

/// Create an `Enigo` for a test and persist the rotated portal token.
pub(crate) fn new_enigo() -> crate::Enigo {
    let enigo = crate::Enigo::new(&settings()).expect("failed to create Enigo");

    #[cfg(all(
        feature = "platform_specific",
        any(feature = "libei", feature = "xdg_desktop")
    ))]
    if let Some(token) = enigo.restore_token() {
        let path = token_path();
        if let Err(err) = std::fs::write(&path, token) {
            panic!(
                "failed to save the restore token to {}: {err}",
                path.display()
            );
        }
    }

    enigo
}

// Check if the code is running in the CI
fn is_ci() -> bool {
    matches!(std::env::var("CI").as_deref(), Ok("true" | "1"))
}

// Add a longer delay if it is not ran in the CI so the user can observe the
// mouse moves but don't waste time in the CI
fn get_delay() -> Duration {
    if is_ci() {
        Duration::from_millis(20)
    } else {
        Duration::from_secs(2)
    }
}
