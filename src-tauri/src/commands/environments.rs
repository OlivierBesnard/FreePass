//! Environment CRUD commands — the IPC surface (PLAN Phase 10 contract). All
//! fail closed with `VaultLocked` when locked (criterion #8). `create_environment`
//! needs the in-memory vaultKey to seal the fresh envKey it mints.

use tauri::State;

use crate::crypto::SecretKey;
use crate::error::{AppError, AppResult};
use crate::models::entry::EnvironmentInfo;
use crate::services::environments;
use crate::state::AppState;

fn session_unavailable() -> AppError {
    AppError::Other("session de coffre indisponible".into())
}

/// Fail closed unless the vault is unlocked.
fn require_unlocked(state: &AppState) -> AppResult<()> {
    if !state.session.lock().map_err(|_| session_unavailable())?.is_unlocked() {
        return Err(AppError::VaultLocked);
    }
    Ok(())
}

/// Clone the in-memory vault key (fails closed if locked). The Mutex is never
/// held across an `.await`; the returned `SecretKey` zeroizes on drop.
fn vault_key(state: &AppState) -> AppResult<SecretKey> {
    let session = state.session.lock().map_err(|_| session_unavailable())?;
    session.vault_key().ok_or(AppError::VaultLocked).cloned()
}

#[tauri::command]
pub async fn create_environment(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
) -> AppResult<EnvironmentInfo> {
    let vk = vault_key(&state)?;
    environments::create_environment(&state.pool, &vk, &project_id, &name).await
}

#[tauri::command]
pub async fn list_environments(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<EnvironmentInfo>> {
    require_unlocked(&state)?;
    environments::list_environments(&state.pool, &project_id).await
}

#[tauri::command]
pub async fn rename_environment(
    state: State<'_, AppState>,
    env_id: String,
    name: String,
) -> AppResult<()> {
    require_unlocked(&state)?;
    environments::rename_environment(&state.pool, &env_id, &name).await
}

#[tauri::command]
pub async fn archive_environment(state: State<'_, AppState>, env_id: String) -> AppResult<()> {
    require_unlocked(&state)?;
    environments::archive_environment(&state.pool, &env_id).await
}
