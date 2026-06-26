//! Vault lifecycle commands — the IPC surface (DESIGN §5). Thin wrappers over
//! `services::vault` that also drive the in-memory session in `AppState`.
//!
//! Passwords arrive as `String` across the IPC boundary; we wipe the Rust copy
//! after use (best-effort, the JS copy is outside our control — §6 CRYPTO_SPEC).

use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::services::vault;
use crate::state::AppState;

/// What the UI needs to pick a screen: create vault / unlock / show vault.
#[derive(Serialize)]
pub struct VaultStatus {
    pub initialized: bool,
    pub unlocked: bool,
}

fn session_unavailable() -> AppError {
    AppError::Other("session de coffre indisponible".into())
}

#[tauri::command]
pub async fn vault_status(state: State<'_, AppState>) -> AppResult<VaultStatus> {
    let initialized = vault::is_initialized(&state.pool).await?;
    let unlocked = state
        .session
        .lock()
        .map_err(|_| session_unavailable())?
        .is_unlocked();
    Ok(VaultStatus { initialized, unlocked })
}

#[tauri::command]
pub async fn create_vault(
    state: State<'_, AppState>,
    master_password: String,
) -> AppResult<()> {
    let vault_key = vault::create_vault(&state.pool, master_password.as_bytes()).await?;
    vault::zeroize_password(master_password);
    state
        .session
        .lock()
        .map_err(|_| session_unavailable())?
        .unlock(vault_key);
    Ok(())
}

#[tauri::command]
pub async fn unlock(state: State<'_, AppState>, master_password: String) -> AppResult<()> {
    // Derive + unwrap first; only open the session on success. A wrong password
    // surfaces as the generic "opération de coffre invalide" (THREAT F5).
    let vault_key = vault::unlock(&state.pool, master_password.as_bytes()).await?;
    vault::zeroize_password(master_password);
    state
        .session
        .lock()
        .map_err(|_| session_unavailable())?
        .unlock(vault_key);
    Ok(())
}

#[tauri::command]
pub async fn lock(state: State<'_, AppState>) -> AppResult<()> {
    state
        .session
        .lock()
        .map_err(|_| session_unavailable())?
        .lock();
    Ok(())
}

#[tauri::command]
pub async fn change_master_password(
    state: State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> AppResult<()> {
    vault::change_master_password(
        &state.pool,
        current_password.as_bytes(),
        new_password.as_bytes(),
    )
    .await?;
    vault::zeroize_password(current_password);
    vault::zeroize_password(new_password);
    Ok(())
}
