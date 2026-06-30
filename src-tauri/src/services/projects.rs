//! Project CRUD + startup backfill (DESIGN §4, PLAN Phase 10). A project is a
//! clear-metadata grouping layer above environments — there is NO crypto here
//! (no envKey, no vaultKey): projects only carry clear names and timestamps.
//!
//! Mono-user invariant (DESIGN §6): `project_id` is an object identifier, never
//! a principal. Pool-based and Tauri-agnostic so it is unit-testable.

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::project::ProjectInfo;

/// Name of the default project created by the backfill. Distinct from the default
/// *environment* (`vault::DEFAULT_ENV_NAME`, also "Personnel"): the project is the
/// new grouping object, the environment is the existing key-bearing object.
pub const DEFAULT_PROJECT_NAME: &str = "Personnel";

/// Validate a project/environment name the same way entries validate their title:
/// trimmed, non-empty. Returns the trimmed value on success.
fn require_name(name: &str) -> AppResult<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Conflict("le nom est requis".into()));
    }
    Ok(trimmed)
}

fn row_to_project(r: &sqlx::sqlite::SqliteRow) -> ProjectInfo {
    ProjectInfo {
        id: r.get("id"),
        name: r.get("name"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

/// Create a project (clear metadata). No crypto, no key material.
pub async fn create_project(pool: &SqlitePool, name: &str) -> AppResult<ProjectInfo> {
    let name = require_name(name)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO projects (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(ProjectInfo {
        id,
        name: name.to_string(),
        created_at: now.clone(),
        updated_at: now,
    })
}

/// List non-archived projects, ordered by name (case-insensitive).
pub async fn list_projects(pool: &SqlitePool) -> AppResult<Vec<ProjectInfo>> {
    let rows = sqlx::query(
        "SELECT id, name, created_at, updated_at FROM projects \
         WHERE archived_at IS NULL ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_project).collect())
}

/// Rename a project (metadata only). Bumps `updated_at`.
pub async fn rename_project(pool: &SqlitePool, project_id: &str, name: &str) -> AppResult<()> {
    let name = require_name(name)?;
    let now = Utc::now().to_rfc3339();
    let res = sqlx::query(
        "UPDATE projects SET name = ?, updated_at = ? \
         WHERE id = ? AND archived_at IS NULL",
    )
    .bind(name)
    .bind(&now)
    .bind(project_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// Soft-delete (archive) a project. Reversible at the data layer (sets
/// `archived_at`). Environments keep their `project_id`; this is metadata only.
pub async fn archive_project(pool: &SqlitePool, project_id: &str) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let res = sqlx::query(
        "UPDATE projects SET archived_at = ?, updated_at = ? \
         WHERE id = ? AND archived_at IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(project_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// True iff a project exists and is not archived. Used to validate
/// `create_environment(project_id, ...)`.
pub async fn project_exists(pool: &SqlitePool, project_id: &str) -> AppResult<bool> {
    let found = sqlx::query("SELECT 1 FROM projects WHERE id = ? AND archived_at IS NULL")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .is_some();
    Ok(found)
}

/// Idempotent startup backfill (PLAN Phase 10, criterion #3): guarantee a default
/// "Personnel" project exists and attach every orphan environment
/// (`project_id IS NULL`) to it. No re-encryption: only the clear `project_id`
/// column is written. Safe to run on every startup.
pub async fn backfill_default_project(pool: &SqlitePool) -> AppResult<()> {
    // Reuse an existing non-archived "Personnel" project if present, so a second
    // startup never creates a duplicate.
    let existing = sqlx::query(
        "SELECT id FROM projects WHERE name = ? AND archived_at IS NULL \
         ORDER BY created_at LIMIT 1",
    )
    .bind(DEFAULT_PROJECT_NAME)
    .fetch_optional(pool)
    .await?;

    let project_id = match existing {
        Some(row) => row.get::<String, _>("id"),
        None => create_project(pool, DEFAULT_PROJECT_NAME).await?.id,
    };

    // Attach orphan environments only. Environments already pointing at a project
    // are left untouched (idempotent on re-run).
    sqlx::query("UPDATE environments SET project_id = ? WHERE project_id IS NULL")
        .bind(&project_id)
        .execute(pool)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_with_url;
    use crate::services::vault;

    async fn pool() -> SqlitePool {
        init_pool_with_url("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn create_list_rename_archive_roundtrip() {
        let pool = pool().await;
        let p = create_project(&pool, "  Travail  ").await.unwrap();
        assert_eq!(p.name, "Travail", "name is trimmed");

        let listed = list_projects(&pool).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, p.id);

        rename_project(&pool, &p.id, "Perso").await.unwrap();
        assert_eq!(list_projects(&pool).await.unwrap()[0].name, "Perso");

        archive_project(&pool, &p.id).await.unwrap();
        assert!(list_projects(&pool).await.unwrap().is_empty(), "archived projects are hidden");
    }

    #[tokio::test]
    async fn list_is_sorted_case_insensitively() {
        let pool = pool().await;
        create_project(&pool, "zeta").await.unwrap();
        create_project(&pool, "Alpha").await.unwrap();
        create_project(&pool, "beta").await.unwrap();
        let names: Vec<String> = list_projects(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["Alpha", "beta", "zeta"]);
    }

    #[tokio::test]
    async fn empty_name_is_rejected() {
        let pool = pool().await;
        assert!(matches!(
            create_project(&pool, "   ").await,
            Err(AppError::Conflict(_))
        ));
    }

    #[tokio::test]
    async fn rename_or_archive_unknown_project_is_not_found() {
        let pool = pool().await;
        assert!(matches!(
            rename_project(&pool, "nope", "X").await,
            Err(AppError::NotFound)
        ));
        assert!(matches!(
            archive_project(&pool, "nope").await,
            Err(AppError::NotFound)
        ));
    }

    #[tokio::test]
    async fn backfill_creates_default_project_and_attaches_orphan_envs() {
        let pool = pool().await;
        // create_vault inserts the default environment with NULL project_id.
        vault::create_vault(&pool, b"pw").await.unwrap();
        let env_id = vault::default_environment_id(&pool).await.unwrap();

        backfill_default_project(&pool).await.unwrap();

        let projects = list_projects(&pool).await.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, DEFAULT_PROJECT_NAME);

        let attached: Option<String> = sqlx::query("SELECT project_id FROM environments WHERE id = ?")
            .bind(&env_id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("project_id");
        assert_eq!(attached.as_deref(), Some(projects[0].id.as_str()));
    }

    #[tokio::test]
    async fn backfill_is_idempotent() {
        let pool = pool().await;
        vault::create_vault(&pool, b"pw").await.unwrap();

        backfill_default_project(&pool).await.unwrap();
        backfill_default_project(&pool).await.unwrap();
        backfill_default_project(&pool).await.unwrap();

        // Exactly one "Personnel" project, no duplicates.
        let n: i64 = sqlx::query("SELECT COUNT(*) AS n FROM projects WHERE name = ?")
            .bind(DEFAULT_PROJECT_NAME)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("n");
        assert_eq!(n, 1, "second/third startup must not create a duplicate project");
    }

    // === Phase 10: backfill — adversarial / edge cases ===

    /// Read the project_id attached to an environment row.
    async fn env_project(pool: &SqlitePool, env_id: &str) -> Option<String> {
        sqlx::query("SELECT project_id FROM environments WHERE id = ?")
            .bind(env_id)
            .fetch_one(pool)
            .await
            .unwrap()
            .get("project_id")
    }

    #[tokio::test]
    async fn backfill_does_not_re_attach_an_already_attached_environment() {
        // An environment already pointing at a *different* project must keep that
        // project after the backfill runs (idempotent: orphans only).
        let pool = pool().await;
        vault::create_vault(&pool, b"pw").await.unwrap();
        let env_id = vault::default_environment_id(&pool).await.unwrap();

        // Attach the env to a custom project by hand (simulate a user-created one).
        let custom = create_project(&pool, "Travail").await.unwrap();
        sqlx::query("UPDATE environments SET project_id = ? WHERE id = ?")
            .bind(&custom.id)
            .bind(&env_id)
            .execute(&pool)
            .await
            .unwrap();

        backfill_default_project(&pool).await.unwrap();

        assert_eq!(
            env_project(&pool, &env_id).await.as_deref(),
            Some(custom.id.as_str()),
            "an already-attached environment must not be re-parented to Personnel"
        );
    }

    #[tokio::test]
    async fn backfill_reuses_a_preexisting_personnel_project() {
        // If a non-archived "Personnel" project already exists, the backfill must
        // reuse it (attach orphans to it) rather than creating a duplicate.
        let pool = pool().await;
        vault::create_vault(&pool, b"pw").await.unwrap();
        let env_id = vault::default_environment_id(&pool).await.unwrap();
        let pre = create_project(&pool, DEFAULT_PROJECT_NAME).await.unwrap();

        backfill_default_project(&pool).await.unwrap();

        let n: i64 = sqlx::query("SELECT COUNT(*) AS n FROM projects WHERE name = ?")
            .bind(DEFAULT_PROJECT_NAME)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("n");
        assert_eq!(n, 1, "must reuse the existing Personnel project, not duplicate it");
        assert_eq!(
            env_project(&pool, &env_id).await.as_deref(),
            Some(pre.id.as_str()),
            "the orphan env must be attached to the pre-existing Personnel project"
        );
    }

    #[tokio::test]
    async fn backfill_attaches_every_orphan_environment_to_the_same_project() {
        // Multiple orphan environments all land on the one default project.
        let pool = pool().await;
        let vk = vault::create_vault(&pool, b"pw").await.unwrap();
        // create_vault made env #1 (orphan). Add two more orphan envs directly via
        // the shared insert helper (project_id = None).
        let now = Utc::now().to_rfc3339();
        let mut tx = pool.begin().await.unwrap();
        let e2 = vault::insert_environment(&mut tx, &vk, "Two", None, &now).await.unwrap();
        let e3 = vault::insert_environment(&mut tx, &vk, "Three", None, &now).await.unwrap();
        tx.commit().await.unwrap();

        backfill_default_project(&pool).await.unwrap();

        let projects = list_projects(&pool).await.unwrap();
        assert_eq!(projects.len(), 1);
        let pid = projects[0].id.as_str();
        for env in [
            vault::default_environment_id(&pool).await.unwrap(),
            e2,
            e3,
        ] {
            assert_eq!(
                env_project(&pool, &env).await.as_deref(),
                Some(pid),
                "every orphan environment must attach to the default project"
            );
        }
    }

    #[tokio::test]
    async fn default_environment_is_orphan_until_backfill_runs() {
        // Migration 004 is additive: a freshly created vault's default env has a
        // NULL project_id until the startup backfill attaches it (criterion #1/#3).
        let pool = pool().await;
        vault::create_vault(&pool, b"pw").await.unwrap();
        let env_id = vault::default_environment_id(&pool).await.unwrap();
        assert_eq!(
            env_project(&pool, &env_id).await,
            None,
            "the default env must be orphan (project_id NULL) before backfill"
        );

        backfill_default_project(&pool).await.unwrap();
        assert!(
            env_project(&pool, &env_id).await.is_some(),
            "after backfill it must be attached"
        );
    }
}
