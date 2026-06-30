pub mod commands;
pub mod crypto;
pub mod db;
pub mod error;
pub mod models;
pub mod services;
pub mod state;

use tauri::Manager;

use crate::commands::db::db_health_check;
use crate::commands::generator::generate_password;
use crate::commands::entries::{
    archive_entry, create_entry, default_environment, delete_entry, entry_icons, get_entry,
    import_entries, list_archived_entries, list_entries, refresh_entry_icon, restore_entry,
    update_entry,
};
use crate::commands::environments::{
    archive_environment, create_environment, list_environments, rename_environment,
};
use crate::commands::projects::{
    archive_project, create_project, list_projects, rename_project,
};
use crate::commands::vault::{
    change_master_password, create_vault, local_channel_info, lock, unlock, vault_status,
};
use crate::db::init_pool;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Resolve the OS app-data dir (e.g. %APPDATA%\com.freepass.desktop)
            // and open the SQLite pool, running migrations automatically.
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;

            let handle = app.handle().clone();
            let app_data_dir_for_state = app_data_dir.clone();
            tauri::async_runtime::block_on(async move {
                let pool = init_pool(&app_data_dir).await?;
                let state = AppState::new(pool, app_data_dir_for_state);
                // Start the loopback channel up-front when a vault already exists,
                // so the extension can always discover the app — even at the lock
                // screen. Credentials stay gated on unlock; `/health` only reveals
                // the lock state to extension origins (THREAT F7/F14). Best-effort:
                // a bind failure must not prevent the app from starting.
                if crate::services::vault::is_initialized(&state.pool).await? {
                    // Phase 10: ensure the default "Personnel" project exists and
                    // every orphan environment is attached to it. Idempotent and
                    // crypto-free; runs on every startup of an existing vault.
                    crate::services::projects::backfill_default_project(&state.pool).await?;
                    if let Ok(channel) =
                        crate::services::local_channel::start(state.pool.clone(), state.session.clone())
                            .await
                    {
                        if let Ok(mut guard) = state.channel.lock() {
                            *guard = Some(channel);
                        }
                    }
                }
                handle.manage(state);
                Ok::<(), crate::error::AppError>(())
            })
            .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            db_health_check,
            vault_status,
            create_vault,
            unlock,
            lock,
            change_master_password,
            local_channel_info,
            default_environment,
            list_entries,
            get_entry,
            create_entry,
            update_entry,
            archive_entry,
            list_archived_entries,
            restore_entry,
            delete_entry,
            import_entries,
            refresh_entry_icon,
            entry_icons,
            generate_password,
            create_project,
            list_projects,
            rename_project,
            archive_project,
            create_environment,
            list_environments,
            rename_environment,
            archive_environment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
