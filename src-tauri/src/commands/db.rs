use sqlx::Row;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::state::AppState;

/// Returns "OK" if the database pool answers a `SELECT 1`.
/// Used by the frontend to confirm the backend booted and migrations ran.
#[tauri::command]
pub async fn db_health_check(state: State<'_, AppState>) -> AppResult<String> {
    health_check(&state.pool).await
}

pub async fn health_check(pool: &SqlitePool) -> AppResult<String> {
    let row = sqlx::query("SELECT 1 AS one").fetch_one(pool).await?;
    let value: i64 = row.get("one");
    debug_assert_eq!(value, 1);
    Ok("OK".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_with_url;

    #[tokio::test]
    async fn health_check_returns_ok_with_a_live_pool() {
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        let result = health_check(&pool).await.unwrap();
        assert_eq!(result, "OK");
    }

    #[tokio::test]
    async fn health_check_errors_when_pool_is_closed() {
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        pool.close().await;
        let result = health_check(&pool).await;
        assert!(result.is_err(), "expected error on closed pool");
    }
}
