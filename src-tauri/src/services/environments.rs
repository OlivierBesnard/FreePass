//! Environment CRUD (DESIGN §4, PLAN Phase 10). Creating an environment mints a
//! fresh envKey (OsRng) sealed under the unlocked vaultKey — REUSING the single
//! `vault::insert_environment` path (zero new crypto primitive, CRYPTO_SPEC §3).
//! Rename/archive are metadata-only. Pool-based and Tauri-agnostic for testing.

use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::crypto::SecretKey;
use crate::error::{AppError, AppResult};
use crate::models::entry::EnvironmentInfo;
use crate::services::{projects, vault};

/// Validate an environment name like a project name / entry title: trimmed,
/// non-empty. Returns the trimmed value.
fn require_name(name: &str) -> AppResult<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Conflict("le nom est requis".into()));
    }
    Ok(trimmed)
}

fn row_to_env(r: &sqlx::sqlite::SqliteRow) -> EnvironmentInfo {
    EnvironmentInfo {
        id: r.get("id"),
        name: r.get("name"),
        project_id: r.get("project_id"),
    }
}

/// Create an environment under a project, minting a fresh envKey (OsRng) sealed
/// under `vault_key`. Requires the caller to have already proven the vault is
/// unlocked (it passes the recovered `vault_key`). The `project_id` must
/// reference an existing, non-archived project (else `NotFound`).
pub async fn create_environment(
    pool: &SqlitePool,
    vault_key: &SecretKey,
    project_id: &str,
    name: &str,
) -> AppResult<EnvironmentInfo> {
    let name = require_name(name)?;
    if !projects::project_exists(pool, project_id).await? {
        return Err(AppError::NotFound);
    }

    let now = Utc::now().to_rfc3339();
    let mut tx = pool.begin().await?;
    let env_id =
        vault::insert_environment(&mut tx, vault_key, name, Some(project_id), &now).await?;
    tx.commit().await?;

    Ok(EnvironmentInfo {
        id: env_id,
        name: name.to_string(),
        project_id: Some(project_id.to_string()),
    })
}

/// List non-archived environments of a project, ordered by name (case-insensitive).
pub async fn list_environments(
    pool: &SqlitePool,
    project_id: &str,
) -> AppResult<Vec<EnvironmentInfo>> {
    let rows = sqlx::query(
        "SELECT id, name, project_id FROM environments \
         WHERE project_id = ? AND archived_at IS NULL ORDER BY name COLLATE NOCASE",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_env).collect())
}

/// Rename an environment (metadata only — envKey untouched). Bumps `updated_at`.
pub async fn rename_environment(pool: &SqlitePool, env_id: &str, name: &str) -> AppResult<()> {
    let name = require_name(name)?;
    let now = Utc::now().to_rfc3339();
    let res = sqlx::query(
        "UPDATE environments SET name = ?, updated_at = ? \
         WHERE id = ? AND archived_at IS NULL",
    )
    .bind(name)
    .bind(&now)
    .bind(env_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::EnvironmentNotFound);
    }
    Ok(())
}

/// Soft-delete (archive) an environment. Its envKey and entries stay on disk
/// (reversible at the data layer); the archived environment is excluded from
/// lists and from the autofill scan (F7 stays intact — archived = not served).
///
/// Refuses to archive the LAST live environment of its project (D1): doing so
/// would leave the project (or, for the orphan default env, the whole vault)
/// with no environment, so `default_environment_id` would fail and the coffre
/// would be unusable. The front already hides this; enforce it here so a direct
/// IPC call cannot reach the broken state. To remove a project's only
/// environment, archive the project instead.
pub async fn archive_environment(pool: &SqlitePool, env_id: &str) -> AppResult<()> {
    let project_id: Option<String> = sqlx::query(
        "SELECT project_id FROM environments WHERE id = ? AND archived_at IS NULL",
    )
    .bind(env_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::EnvironmentNotFound)?
    .get("project_id");

    let live_siblings: i64 = match &project_id {
        Some(pid) => sqlx::query(
            "SELECT COUNT(*) AS n FROM environments \
             WHERE project_id = ? AND archived_at IS NULL",
        )
        .bind(pid)
        .fetch_one(pool)
        .await?
        .get("n"),
        None => sqlx::query(
            "SELECT COUNT(*) AS n FROM environments \
             WHERE project_id IS NULL AND archived_at IS NULL",
        )
        .fetch_one(pool)
        .await?
        .get("n"),
    };
    if live_siblings <= 1 {
        return Err(AppError::Conflict(
            "impossible d'archiver le dernier environnement du projet".into(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    let res = sqlx::query(
        "UPDATE environments SET archived_at = ?, updated_at = ? \
         WHERE id = ? AND archived_at IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(env_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::EnvironmentNotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_with_url;

    /// Fresh vault + a project to hang environments off. Returns the recovered
    /// vault key and the project id.
    async fn setup() -> (SqlitePool, SecretKey, String) {
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        let vk = vault::create_vault(&pool, b"pw").await.unwrap();
        let project = projects::create_project(&pool, "Travail").await.unwrap();
        (pool, vk, project.id)
    }

    #[tokio::test]
    async fn create_environment_mints_a_distinct_unwrappable_envkey() {
        let (pool, vk, project_id) = setup().await;

        let env = create_environment(&pool, &vk, &project_id, "Staging")
            .await
            .unwrap();
        assert_eq!(env.name, "Staging");
        assert_eq!(env.project_id.as_deref(), Some(project_id.as_str()));

        // The fresh envKey unwraps under the vault key with the CORRECT env id...
        let key_new = vault::load_env_key(&pool, &vk, &env.id).await.unwrap();
        // ...and is distinct from the default environment's envKey.
        let default_id = vault::default_environment_id(&pool).await.unwrap();
        let key_default = vault::load_env_key(&pool, &vk, &default_id).await.unwrap();
        assert_ne!(
            key_new.expose(),
            key_default.expose(),
            "each environment must own a fresh, independent envKey"
        );

        // Anti-swap (F8): the wrapped blob is bound to its own env id via AAD, so
        // it must NOT unwrap under a different env id.
        let row = sqlx::query("SELECT env_key_wrapped, env_key_nonce FROM environments WHERE id = ?")
            .bind(&env.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let wrapped: Vec<u8> = row.get("env_key_wrapped");
        let nonce: Vec<u8> = row.get("env_key_nonce");
        let nonce: [u8; crate::crypto::aead::NONCE_LEN] = nonce.try_into().unwrap();
        let sealed = crate::crypto::Sealed { nonce, ciphertext: wrapped };
        assert!(
            crate::crypto::unwrap_env_key(&vk, &default_id, &sealed).is_err(),
            "a wrapped envKey must not unwrap under another env id (anti-swap F8)"
        );
    }

    #[tokio::test]
    async fn create_environment_rejects_unknown_or_archived_project() {
        let (pool, vk, project_id) = setup().await;
        assert!(matches!(
            create_environment(&pool, &vk, "nope", "X").await,
            Err(AppError::NotFound)
        ));

        projects::archive_project(&pool, &project_id).await.unwrap();
        assert!(matches!(
            create_environment(&pool, &vk, &project_id, "X").await,
            Err(AppError::NotFound)
        ));
    }

    #[tokio::test]
    async fn list_rename_archive_environments() {
        let (pool, vk, project_id) = setup().await;
        let env = create_environment(&pool, &vk, &project_id, "Dev").await.unwrap();
        // A keeper so `env` is not the project's LAST environment (D1 guard).
        let keeper = create_environment(&pool, &vk, &project_id, "Prod").await.unwrap();

        let listed = list_environments(&pool, &project_id).await.unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|e| e.id == env.id));

        rename_environment(&pool, &env.id, "Développement").await.unwrap();
        assert_eq!(
            list_environments(&pool, &project_id)
                .await
                .unwrap()
                .iter()
                .find(|e| e.id == env.id)
                .unwrap()
                .name,
            "Développement"
        );

        archive_environment(&pool, &env.id).await.unwrap();
        let after = list_environments(&pool, &project_id).await.unwrap();
        assert!(
            after.iter().all(|e| e.id != env.id),
            "archived environment is hidden"
        );
        assert!(after.iter().any(|e| e.id == keeper.id), "the keeper remains");
        // load_env_key filters archived, so the archived env is gone from that path.
        assert!(matches!(
            vault::load_env_key(&pool, &vk, &env.id).await,
            Err(AppError::EnvironmentNotFound)
        ));
    }

    #[tokio::test]
    async fn cannot_archive_the_last_environment_of_a_project() {
        // D1: archiving the only live environment of a project is refused with a
        // clean Conflict, so the project never ends up with zero environments.
        let (pool, vk, project_id) = setup().await;
        let only = create_environment(&pool, &vk, &project_id, "Solo").await.unwrap();
        assert!(matches!(
            archive_environment(&pool, &only.id).await,
            Err(AppError::Conflict(_))
        ));
        // It is still live and listed.
        assert_eq!(list_environments(&pool, &project_id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn empty_name_is_rejected() {
        let (pool, vk, project_id) = setup().await;
        assert!(matches!(
            create_environment(&pool, &vk, &project_id, "  ").await,
            Err(AppError::Conflict(_))
        ));
    }

    #[tokio::test]
    async fn rename_or_archive_unknown_environment_is_not_found() {
        let (pool, _vk, _project_id) = setup().await;
        assert!(matches!(
            rename_environment(&pool, "nope", "X").await,
            Err(AppError::EnvironmentNotFound)
        ));
        assert!(matches!(
            archive_environment(&pool, "nope").await,
            Err(AppError::EnvironmentNotFound)
        ));
    }

    // === Phase 10: additional edge / adversarial cases ===

    #[tokio::test]
    async fn list_environments_is_scoped_to_its_project() {
        // Environments of project P1 must not appear under project P2 (object-level
        // scoping, like entries scope by env_id).
        let (pool, vk, p1) = setup().await;
        let p2 = projects::create_project(&pool, "Autre").await.unwrap();
        create_environment(&pool, &vk, &p1, "Dev-P1").await.unwrap();
        let e2 = create_environment(&pool, &vk, &p2.id, "Dev-P2").await.unwrap();

        let in_p2 = list_environments(&pool, &p2.id).await.unwrap();
        assert_eq!(in_p2.len(), 1, "P2 must list only its own environments");
        assert_eq!(in_p2[0].id, e2.id);
        // P1 still has only the env created under it (not P2's).
        let in_p1 = list_environments(&pool, &p1).await.unwrap();
        assert_eq!(in_p1.len(), 1);
        assert!(in_p1.iter().all(|e| e.id != e2.id));
    }

    #[tokio::test]
    async fn archived_environment_is_dropped_from_lists_and_from_load_env_key() {
        // F7 at the data layer: archiving must remove the env from list_environments
        // AND make its envKey unreachable via load_env_key (the autofill path).
        let (pool, vk, project_id) = setup().await;
        let env = create_environment(&pool, &vk, &project_id, "Prod").await.unwrap();
        // A keeper so `env` is not the LAST environment (D1 guard would refuse).
        create_environment(&pool, &vk, &project_id, "Keep").await.unwrap();
        // Its key loads while live.
        vault::load_env_key(&pool, &vk, &env.id).await.unwrap();

        archive_environment(&pool, &env.id).await.unwrap();
        assert!(list_environments(&pool, &project_id)
            .await
            .unwrap()
            .iter()
            .all(|e| e.id != env.id));
        assert!(matches!(
            vault::load_env_key(&pool, &vk, &env.id).await,
            Err(AppError::EnvironmentNotFound)
        ));
    }

    #[tokio::test]
    async fn unicode_environment_name_is_preserved() {
        // Limit case: non-ASCII names round-trip through create and rename.
        let (pool, vk, project_id) = setup().await;
        let env = create_environment(&pool, &vk, &project_id, "Préprod 🌱 测试")
            .await
            .unwrap();
        assert_eq!(env.name, "Préprod 🌱 测试");
        rename_environment(&pool, &env.id, "Recette ✅").await.unwrap();
        let listed = list_environments(&pool, &project_id).await.unwrap();
        assert_eq!(listed[0].name, "Recette ✅");
    }

    #[tokio::test]
    async fn each_new_environment_uses_a_fresh_nonce_and_distinct_wrapped_blob() {
        // F10: two environments minted under the same vaultKey must NOT reuse the
        // wrapping nonce, and their wrapped blobs must differ.
        let (pool, vk, project_id) = setup().await;
        let e1 = create_environment(&pool, &vk, &project_id, "A").await.unwrap();
        let e2 = create_environment(&pool, &vk, &project_id, "B").await.unwrap();

        let read = |id: String| {
            let pool = pool.clone();
            async move {
                let r = sqlx::query(
                    "SELECT env_key_nonce, env_key_wrapped FROM environments WHERE id = ?",
                )
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
                let nonce: Vec<u8> = r.get("env_key_nonce");
                let wrapped: Vec<u8> = r.get("env_key_wrapped");
                (nonce, wrapped)
            }
        };
        let (n1, w1) = read(e1.id).await;
        let (n2, w2) = read(e2.id).await;
        assert_eq!(n1.len(), crate::crypto::aead::NONCE_LEN, "nonce must be 24 bytes");
        assert_ne!(n1, n2, "env wrapping nonces must never repeat (F10)");
        assert_ne!(w1, w2, "each env must have a distinct wrapped key blob");
    }
}
