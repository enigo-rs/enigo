//! Restore-token helpers shared by the unit tests, the integration test and the
//! examples.
//!
//! The same file is used by all three so the lookup order and the token path
//! stay identical. The parent module imports `Settings` and `Enigo`; `super::`
//! resolves to those imports in every context.

use std::path::PathBuf;

#[must_use]
pub(crate) fn token_path() -> PathBuf {
    std::env::var_os("ENIGO_RESTORE_TOKEN_FILE")
        .map_or_else(|| PathBuf::from("/tmp/enigo_restore_token"), PathBuf::from)
}

/// Read a restore token from `--restore-token`, `ENIGO_RESTORE_TOKEN` or the
/// token file, in that order.
#[must_use]
pub(crate) fn restore_token() -> Option<String> {
    let mut token = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--restore-token" {
            token = args.next();
        } else if let Some(value) = arg.strip_prefix("--restore-token=") {
            token = Some(value.to_owned());
        }
    }

    if token.is_none() {
        if let Ok(value) = std::env::var("ENIGO_RESTORE_TOKEN") {
            token = Some(value);
        } else if let Ok(value) = std::fs::read_to_string(token_path()) {
            token = Some(value.trim().to_owned());
        }
    }

    token
}

/// Settings with the configured restore token.
#[must_use]
pub fn settings() -> super::Settings {
    super::Settings {
        restore_token: restore_token(),
        ..super::Settings::default()
    }
}

/// Create an `Enigo` and persist the rotated portal token.
///
/// # Panics
/// Panics if the connection cannot be established.
#[must_use]
pub fn new_enigo(settings: &super::Settings) -> super::Enigo {
    let enigo = super::Enigo::new(settings).expect("failed to create Enigo");

    #[cfg(all(
        feature = "platform_specific",
        any(feature = "libei", feature = "xdg_desktop")
    ))]
    if let Some(token) = enigo.restore_token() {
        save_restore_token(&token);
    }

    enigo
}

/// Save a rotated restore token for the next run.
///
/// # Panics
/// Panics if the token file cannot be written.
#[allow(dead_code)] // Only used when a portal backend is compiled in.
pub(crate) fn save_restore_token(token: &str) {
    let path = token_path();
    if let Err(err) = std::fs::write(&path, token) {
        panic!(
            "failed to save the restore token to {}: {err}",
            path.display()
        );
    }
}
