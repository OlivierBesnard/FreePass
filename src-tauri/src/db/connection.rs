use std::path::Path;
use std::str::FromStr;

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};

/// Migrations are embedded at compile time and run automatically at startup
/// (never by hand). The single vault DB file lives in the OS app-data dir.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// File name of the encrypted vault database inside the app-data directory.
const VAULT_DB_FILE: &str = "vault.sqlite";

/// Build a SQLite URL pointing at `<app_data_dir>/vault.sqlite`, creating the
/// directory if it does not exist.
pub fn resolve_db_url(app_data_dir: &Path) -> AppResult<String> {
    std::fs::create_dir_all(app_data_dir)?;
    let db_path = app_data_dir.join(VAULT_DB_FILE);
    let path_str = db_path
        .to_str()
        .ok_or_else(|| AppError::InvalidPath(db_path.clone()))?;
    Ok(format!("sqlite://{}?mode=rwc", path_str.replace('\\', "/")))
}

/// Create a pool from a SQLite URL and run all pending migrations.
///
/// Used by tests with `sqlite::memory:` and by production with the file URL
/// produced by [`resolve_db_url`].
pub async fn init_pool_with_url(url: &str) -> AppResult<SqlitePool> {
    let connect_options = SqliteConnectOptions::from_str(url)?
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await?;

    MIGRATOR.run(&pool).await?;

    Ok(pool)
}

/// Initialize the production pool: `vault.sqlite` in the OS app-data dir,
/// foreign keys on, WAL journal mode, all migrations applied.
pub async fn init_pool(app_data_dir: &Path) -> AppResult<SqlitePool> {
    let url = resolve_db_url(app_data_dir)?;
    init_pool_with_url(&url).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    fn tempdir() -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("freepass-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    async fn fresh_memory_pool() -> SqlitePool {
        init_pool_with_url("sqlite::memory:")
            .await
            .expect("pool init")
    }

    #[tokio::test]
    async fn init_pool_runs_migrations_and_creates_the_vault_table() {
        let pool = fresh_memory_pool().await;

        let rows =
            sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();
        let names: Vec<String> = rows.iter().map(|r| r.get::<String, _>("name")).collect();

        assert!(
            names.iter().any(|n| n == "vault"),
            "missing table: vault, got {names:?}"
        );
    }

    #[tokio::test]
    async fn vault_table_has_exactly_one_row_after_migration() {
        let pool = fresh_memory_pool().await;
        let row = sqlx::query("SELECT COUNT(*) AS n FROM vault")
            .fetch_one(&pool)
            .await
            .unwrap();
        let n: i64 = row.get("n");
        assert_eq!(n, 1, "vault must hold exactly one metadata row");
    }

    #[tokio::test]
    async fn vault_crypto_metadata_starts_empty() {
        // Phase 0: crypto columns exist but are NULL until the vault is
        // initialized in Phase 2. No crypto logic here.
        let pool = fresh_memory_pool().await;
        let row = sqlx::query(
            "SELECT kdf_salt, vault_key_wrapped FROM vault WHERE id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let salt: Option<Vec<u8>> = row.get("kdf_salt");
        let wrapped: Option<Vec<u8>> = row.get("vault_key_wrapped");
        assert!(salt.is_none(), "kdf_salt should be NULL before init");
        assert!(wrapped.is_none(), "vault_key_wrapped should be NULL before init");
    }

    #[tokio::test]
    async fn init_pool_enables_foreign_keys() {
        let pool = fresh_memory_pool().await;
        let row = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        let on: i64 = row.get(0);
        assert_eq!(on, 1, "foreign_keys pragma should be ON");
    }

    #[tokio::test]
    async fn init_pool_uses_wal_journal_mode() {
        let dir = tempdir();
        let url = resolve_db_url(&dir).unwrap();
        let pool = init_pool_with_url(&url).await.unwrap();
        let row = sqlx::query("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .unwrap();
        let mode: String = row.get(0);
        assert_eq!(mode.to_lowercase(), "wal");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn migrations_are_idempotent_on_a_second_run() {
        let dir = tempdir();
        let url = resolve_db_url(&dir).unwrap();
        let pool = init_pool_with_url(&url).await.unwrap();
        // Re-running migrations on the same DB must be a no-op (and keep one row).
        MIGRATOR.run(&pool).await.expect("re-run migrations");
        let row = sqlx::query("SELECT COUNT(*) AS n FROM vault")
            .fetch_one(&pool)
            .await
            .unwrap();
        let n: i64 = row.get("n");
        assert_eq!(n, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn resolve_db_url_creates_the_app_data_dir() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("freepass-test-{}", uuid::Uuid::new_v4()));
        assert!(!dir.exists());
        let url = resolve_db_url(&dir).unwrap();
        assert!(dir.exists(), "app data dir should be created");
        assert!(url.starts_with("sqlite://"));
        assert!(url.ends_with("vault.sqlite?mode=rwc"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
