//! Password generator command (CRYPTO_SPEC §5). Stateless, no vault access.

use crate::error::AppResult;
use crate::services::generator::{generate, GenOptions};

#[tauri::command]
pub fn generate_password(
    length: usize,
    lowercase: bool,
    uppercase: bool,
    digits: bool,
    symbols: bool,
) -> AppResult<String> {
    let opts = GenOptions { length, lowercase, uppercase, digits, symbols };
    // The zeroizing buffer is cloned out to cross the IPC boundary, then wiped.
    Ok(generate(&opts)?.to_string())
}
