//! Project CRUD commands — the IPC surface (PLAN Phase 10 contract). All
//! commands fail closed with `VaultLocked` when the vault is not unlocked
//! (criterion #8), even though projects are clear metadata: a locked vault
//! exposes nothing about its contents.

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::models::project::ProjectInfo;
use crate::services::projects;
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

#[tauri::command]
pub async fn create_project(state: State<'_, AppState>, name: String) -> AppResult<ProjectInfo> {
    require_unlocked(&state)?;
    projects::create_project(&state.pool, &name).await
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<ProjectInfo>> {
    require_unlocked(&state)?;
    projects::list_projects(&state.pool).await
}

#[tauri::command]
pub async fn rename_project(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
) -> AppResult<()> {
    require_unlocked(&state)?;
    projects::rename_project(&state.pool, &project_id, &name).await
}

#[tauri::command]
pub async fn archive_project(state: State<'_, AppState>, project_id: String) -> AppResult<()> {
    require_unlocked(&state)?;
    projects::archive_project(&state.pool, &project_id).await
}
