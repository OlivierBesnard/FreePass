//! Entry CRUD commands — the IPC surface (DESIGN §4-5). Each resolves the
//! environment key from the unlocked session (never from disk in the clear) and
//! delegates the crypto to `services::entries`.

use std::collections::HashMap;

use sqlx::Row;
use tauri::State;

use crate::crypto::SecretKey;
use crate::error::{AppError, AppResult};
use crate::models::entry::{EntryDetail, EntryInput, EntrySummary, EnvironmentInfo};
use crate::services::{entries, favicon, vault};
use crate::state::AppState;

fn session_unavailable() -> AppError {
    AppError::Other("session de coffre indisponible".into())
}

/// Resolve the environment key from the in-memory session, loading + caching it
/// the first time. The Mutex is never held across an `.await` (we clone the
/// `SecretKey`, which zeroizes on drop). Fails closed if the vault is locked.
async fn resolve_env_key(state: &AppState, env_id: &str) -> AppResult<SecretKey> {
    {
        let session = state.session.lock().map_err(|_| session_unavailable())?;
        if !session.is_unlocked() {
            return Err(AppError::VaultLocked);
        }
        if let Some(key) = session.env_key(env_id) {
            return Ok(key.clone());
        }
    }

    let vault_key = {
        let session = state.session.lock().map_err(|_| session_unavailable())?;
        session.vault_key().ok_or(AppError::VaultLocked)?.clone()
    };
    let env_key = vault::load_env_key(&state.pool, &vault_key, env_id).await?;

    state
        .session
        .lock()
        .map_err(|_| session_unavailable())?
        .cache_env_key(env_id.to_string(), env_key.clone());
    Ok(env_key)
}

#[tauri::command]
pub async fn default_environment(state: State<'_, AppState>) -> AppResult<EnvironmentInfo> {
    let id = vault::default_environment_id(&state.pool).await?;
    let name: String = sqlx::query("SELECT name FROM environments WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?
        .get("name");
    Ok(EnvironmentInfo { id, name })
}

#[tauri::command]
pub async fn list_entries(
    state: State<'_, AppState>,
    env_id: String,
    search: Option<String>,
) -> AppResult<Vec<EntrySummary>> {
    // List reads clear metadata only — no env key needed, but require unlock.
    if !state.session.lock().map_err(|_| session_unavailable())?.is_unlocked() {
        return Err(AppError::VaultLocked);
    }
    entries::list_entries(&state.pool, &env_id, search.as_deref()).await
}

#[tauri::command]
pub async fn get_entry(
    state: State<'_, AppState>,
    env_id: String,
    entry_id: String,
) -> AppResult<EntryDetail> {
    let env_key = resolve_env_key(&state, &env_id).await?;
    entries::get_entry(&state.pool, &env_key, &env_id, &entry_id).await
}

/// Fetch the favicon for an entry directly from its site and store it encrypted,
/// returning the `data:` URL (or None if nothing usable was found). Best-effort:
/// called automatically by the UI after a save when a URL is present, and never
/// blocks the save itself. The fetch only contacts the entry's own site.
#[tauri::command]
pub async fn refresh_entry_icon(
    state: State<'_, AppState>,
    env_id: String,
    entry_id: String,
    url: String,
) -> AppResult<Option<String>> {
    let env_key = resolve_env_key(&state, &env_id).await?;
    let icon = favicon::fetch_favicon(&url).await?;
    entries::set_icon(&state.pool, &env_key, &env_id, &entry_id, icon.as_deref()).await?;
    Ok(icon)
}

/// All stored favicons for the environment, keyed by entry id — the UI overlays
/// these on the list without decrypting secret fields.
#[tauri::command]
pub async fn entry_icons(
    state: State<'_, AppState>,
    env_id: String,
) -> AppResult<HashMap<String, String>> {
    let env_key = resolve_env_key(&state, &env_id).await?;
    entries::load_icons(&state.pool, &env_key, &env_id).await
}

#[tauri::command]
pub async fn create_entry(
    state: State<'_, AppState>,
    env_id: String,
    input: EntryInput,
) -> AppResult<String> {
    let env_key = resolve_env_key(&state, &env_id).await?;
    entries::create_entry(&state.pool, &env_key, &env_id, &input).await
}

#[tauri::command]
pub async fn update_entry(
    state: State<'_, AppState>,
    env_id: String,
    entry_id: String,
    input: EntryInput,
) -> AppResult<()> {
    let env_key = resolve_env_key(&state, &env_id).await?;
    entries::update_entry(&state.pool, &env_key, &env_id, &entry_id, &input).await
}

#[tauri::command]
pub async fn import_entries(
    state: State<'_, AppState>,
    env_id: String,
    entries: Vec<EntryInput>,
) -> AppResult<usize> {
    let env_key = resolve_env_key(&state, &env_id).await?;
    entries::import_entries(&state.pool, &env_key, &env_id, &entries).await
}

#[tauri::command]
pub async fn archive_entry(
    state: State<'_, AppState>,
    env_id: String,
    entry_id: String,
) -> AppResult<()> {
    if !state.session.lock().map_err(|_| session_unavailable())?.is_unlocked() {
        return Err(AppError::VaultLocked);
    }
    entries::archive_entry(&state.pool, &env_id, &entry_id).await
}

#[tauri::command]
pub async fn list_archived_entries(
    state: State<'_, AppState>,
    env_id: String,
) -> AppResult<Vec<EntrySummary>> {
    if !state.session.lock().map_err(|_| session_unavailable())?.is_unlocked() {
        return Err(AppError::VaultLocked);
    }
    entries::list_archived_entries(&state.pool, &env_id).await
}

#[tauri::command]
pub async fn restore_entry(
    state: State<'_, AppState>,
    env_id: String,
    entry_id: String,
) -> AppResult<()> {
    if !state.session.lock().map_err(|_| session_unavailable())?.is_unlocked() {
        return Err(AppError::VaultLocked);
    }
    entries::restore_entry(&state.pool, &env_id, &entry_id).await
}

#[tauri::command]
pub async fn delete_entry(
    state: State<'_, AppState>,
    env_id: String,
    entry_id: String,
) -> AppResult<()> {
    if !state.session.lock().map_err(|_| session_unavailable())?.is_unlocked() {
        return Err(AppError::VaultLocked);
    }
    entries::delete_entry(&state.pool, &env_id, &entry_id).await
}
