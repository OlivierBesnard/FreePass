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
    archive_entry, create_entry, default_environment, delete_entry, get_entry, import_entries,
    list_entries, update_entry,
};
use crate::commands::vault::{
    change_master_password, create_vault, local_channel_info, lock, unlock, vault_status,
};
use crate::db::init_pool;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
                handle.manage(AppState::new(pool, app_data_dir_for_state));
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
            delete_entry,
            import_entries,
            generate_password,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
