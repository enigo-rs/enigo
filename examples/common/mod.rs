use enigo::{Enigo, Settings};

#[path = "../../tests/common/token.rs"]
mod token;

/// Build `Enigo` for the examples, reusing a portal restore token.
///
/// A token can be supplied with `--restore-token <token>`,
/// `--restore-token=<token>`, `ENIGO_RESTORE_TOKEN`, or a token file
/// (`ENIGO_RESTORE_TOKEN_FILE`, default `/tmp/enigo_restore_token`).
pub fn new_enigo() -> Enigo {
    token::new_enigo(&token::settings())
}
