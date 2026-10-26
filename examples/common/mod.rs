use std::path::PathBuf;

use enigo::Settings;

/// Path used to read the portal restore token between runs.
fn token_path() -> PathBuf {
    std::env::var_os("ENIGO_RESTORE_TOKEN_FILE")
        .map_or_else(|| PathBuf::from("/tmp/enigo_restore_token"), PathBuf::from)
}

/// Build settings for the examples.
///
/// A token can be supplied with `--restore-token <token>`,
/// `--restore-token=<token>`, `ENIGO_RESTORE_TOKEN`, or a token file
/// (`ENIGO_RESTORE_TOKEN_FILE`, default `/tmp/enigo_restore_token`).
pub fn settings() -> Settings {
    let mut settings = Settings::default();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--restore-token" {
            settings.restore_token = args.next();
        } else if let Some(token) = arg.strip_prefix("--restore-token=") {
            settings.restore_token = Some(token.to_owned());
        }
    }

    if settings.restore_token.is_none() {
        if let Ok(token) = std::env::var("ENIGO_RESTORE_TOKEN") {
            settings.restore_token = Some(token);
        } else if let Ok(token) = std::fs::read_to_string(token_path()) {
            settings.restore_token = Some(token.trim().to_owned());
        }
    }

    settings
}
