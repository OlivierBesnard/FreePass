//! Vault lifecycle: init, unlock, lock, change master password (DESIGN §5,
//! CRYPTO_SPEC §3). Pool-based and Tauri-agnostic so it is unit-testable with an
//! in-memory SQLite DB. The in-memory key *session* lives in `state.rs`; this
//! module only derives/wraps/unwraps and persists the wrapped material.
//!
//! Security invariants enforced here:
//! - masterKey / vaultKey / envKey / master password are NEVER persisted in the
//!   clear (only `salt`, `kdf_params`, and AEAD-sealed blobs hit the DB). F2.
//! - a wrong master password fails as a generic crypto error (no oracle). F5.
//! - changing the master password re-wraps the SAME vaultKey, so environments
//!   and entries are untouched. CRYPTO_SPEC §3.

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::crypto::{self, KdfParams, Sealed, SecretKey};
use crate::error::{AppError, AppResult};

/// Name of the environment created automatically at vault init. v1 exposes a
/// single default environment; the multi-environment UI comes later (DESIGN §4).
pub const DEFAULT_ENV_NAME: &str = "Personnel";

struct VaultRow {
    initialized_at: Option<String>,
    kdf_salt: Option<Vec<u8>>,
    kdf_params: Option<String>,
    vault_key_wrapped: Option<Vec<u8>>,
    vault_key_nonce: Option<Vec<u8>>,
}

async fn load_vault_row(pool: &SqlitePool) -> AppResult<VaultRow> {
    let row = sqlx::query(
        "SELECT initialized_at, kdf_salt, kdf_params, vault_key_wrapped, vault_key_nonce \
         FROM vault WHERE id = 1",
    )
    .fetch_one(pool)
    .await?;
    Ok(VaultRow {
        initialized_at: row.get("initialized_at"),
        kdf_salt: row.get("kdf_salt"),
        kdf_params: row.get("kdf_params"),
        vault_key_wrapped: row.get("vault_key_wrapped"),
        vault_key_nonce: row.get("vault_key_nonce"),
    })
}

/// True once a master password has been set (the vault has been created).
pub async fn is_initialized(pool: &SqlitePool) -> AppResult<bool> {
    Ok(load_vault_row(pool).await?.initialized_at.is_some())
}

/// Turn a stored 24-byte nonce blob into a fixed array, or a generic crypto error.
fn nonce_from(blob: Vec<u8>) -> AppResult<[u8; crypto::aead::NONCE_LEN]> {
    blob.try_into()
        .map_err(|_| AppError::Crypto(crypto::CryptoError::Decrypt))
}

/// Create the vault: derive the master key, generate + wrap the vault key, and
/// create the default environment with its own wrapped env key. Returns the
/// (zeroizing) vault key so the caller can open the session. F2/F4.
pub async fn create_vault(pool: &SqlitePool, password: &[u8]) -> AppResult<SecretKey> {
    if is_initialized(pool).await? {
        return Err(AppError::VaultAlreadyInitialized);
    }

    let salt = crypto::generate_salt();
    let params = KdfParams::default();
    let master_key = crypto::derive_master_key(password, &salt, &params)?;

    let vault_key = SecretKey::generate();
    let wrapped_vault = crypto::wrap_vault_key(&master_key, &vault_key)?;

    let params_json =
        serde_json::to_string(&params).map_err(|e| AppError::Other(e.to_string()))?;
    let now = Utc::now().to_rfc3339();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE vault SET kdf_salt = ?, kdf_params = ?, vault_key_wrapped = ?, \
         vault_key_nonce = ?, initialized_at = ?, updated_at = ? WHERE id = 1",
    )
    .bind(&salt[..])
    .bind(&params_json)
    .bind(&wrapped_vault.ciphertext)
    .bind(&wrapped_vault.nonce[..])
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    // The default environment has no project yet (the Phase 10 startup backfill
    // attaches it to the default project). Reuse the shared env-insert helper so
    // there is exactly one path that generates + wraps a fresh envKey.
    insert_environment(&mut tx, &vault_key, DEFAULT_ENV_NAME, None, &now).await?;
    tx.commit().await?;

    Ok(vault_key)
}

/// Generate a fresh random envKey (OsRng), seal it under `vault_key` bound to the
/// new env id (CRYPTO_SPEC §3, anti-swap F8), and insert the environment row in
/// the given transaction. The ONLY place that creates an environment, shared by
/// vault init and `environments::create_environment` — zero new crypto primitive.
/// Returns the new env id. The envKey never leaves this function in the clear.
pub(crate) async fn insert_environment(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    vault_key: &SecretKey,
    name: &str,
    project_id: Option<&str>,
    now: &str,
) -> AppResult<String> {
    let env_id = Uuid::new_v4().to_string();
    let env_key = SecretKey::generate();
    let wrapped_env = crypto::wrap_env_key(vault_key, &env_id, &env_key)?;

    sqlx::query(
        "INSERT INTO environments \
         (id, name, env_key_wrapped, env_key_nonce, project_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&env_id)
    .bind(name)
    .bind(&wrapped_env.ciphertext)
    .bind(&wrapped_env.nonce[..])
    .bind(project_id)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    Ok(env_id)
}

/// Derive the master key and recover the vault key. A wrong password fails as a
/// generic crypto error (THREAT F5). Refuses sub-floor stored params (THREAT F4).
pub async fn unlock(pool: &SqlitePool, password: &[u8]) -> AppResult<SecretKey> {
    let row = load_vault_row(pool).await?;
    let (salt, params_json, wrapped, nonce) = match (
        row.initialized_at,
        row.kdf_salt,
        row.kdf_params,
        row.vault_key_wrapped,
        row.vault_key_nonce,
    ) {
        (Some(_), Some(s), Some(p), Some(w), Some(n)) => (s, p, w, n),
        _ => return Err(AppError::VaultNotInitialized),
    };

    let params: KdfParams =
        serde_json::from_str(&params_json).map_err(|e| AppError::Other(e.to_string()))?;
    let master_key = crypto::derive_master_key(password, &salt, &params)?;

    let sealed = Sealed { nonce: nonce_from(nonce)?, ciphertext: wrapped };
    let vault_key = crypto::unwrap_vault_key(&master_key, &sealed)?;
    Ok(vault_key)
}

/// Re-derive the master key from `new_password` with a fresh salt and re-wrap the
/// SAME vault key. Entries and environments are untouched (their envKeys are
/// sealed under the unchanged vaultKey). Verifies `current_password` first.
pub async fn change_master_password(
    pool: &SqlitePool,
    current_password: &[u8],
    new_password: &[u8],
) -> AppResult<()> {
    // Unlock with the current password — this both verifies it and yields the
    // vault key we will re-wrap. Wrong current password => generic crypto error.
    let vault_key = unlock(pool, current_password).await?;

    let new_salt = crypto::generate_salt();
    let params = KdfParams::default();
    let new_master = crypto::derive_master_key(new_password, &new_salt, &params)?;
    let rewrapped = crypto::wrap_vault_key(&new_master, &vault_key)?;

    let params_json =
        serde_json::to_string(&params).map_err(|e| AppError::Other(e.to_string()))?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE vault SET kdf_salt = ?, kdf_params = ?, vault_key_wrapped = ?, \
         vault_key_nonce = ?, updated_at = ? WHERE id = 1",
    )
    .bind(&new_salt[..])
    .bind(&params_json)
    .bind(&rewrapped.ciphertext)
    .bind(&rewrapped.nonce[..])
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

/// The id of the default environment: the oldest environment that is itself
/// live AND rooted in a live project (or still orphan, pre-backfill). An
/// environment whose owning PROJECT is archived is NOT eligible — otherwise the
/// "Add" flow would target it and every new entry would be a ghost, invisible in
/// the unified list and in autofill (B1). This mirrors the archived-project
/// predicate used by `list_all_entries` and the autofill scan.
pub async fn default_environment_id(pool: &SqlitePool) -> AppResult<String> {
    let row = sqlx::query(
        "SELECT e.id FROM environments e \
         LEFT JOIN projects p ON p.id = e.project_id \
         WHERE e.archived_at IS NULL AND (e.project_id IS NULL OR p.archived_at IS NULL) \
         ORDER BY e.created_at LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::EnvironmentNotFound)?;
    Ok(row.get("id"))
}

/// Recover an environment key by unwrapping it under the (already unlocked) vault
/// key. Used by entry encryption (Phase 3) and the local channel (Phase 6).
pub async fn load_env_key(
    pool: &SqlitePool,
    vault_key: &SecretKey,
    env_id: &str,
) -> AppResult<SecretKey> {
    let row = sqlx::query(
        "SELECT env_key_wrapped, env_key_nonce FROM environments \
         WHERE id = ? AND archived_at IS NULL",
    )
    .bind(env_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::EnvironmentNotFound)?;

    let ciphertext: Vec<u8> = row.get("env_key_wrapped");
    let nonce: Vec<u8> = row.get("env_key_nonce");
    let sealed = Sealed { nonce: nonce_from(nonce)?, ciphertext };
    Ok(crypto::unwrap_env_key(vault_key, env_id, &sealed)?)
}

/// Best-effort zeroize of a password `String` taken from the IPC boundary. The
/// JS-side copy is outside our control (assumed limitation); we wipe the Rust one.
pub fn zeroize_password(mut password: String) {
    password.zeroize();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_with_url;

    async fn pool() -> SqlitePool {
        init_pool_with_url("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn create_then_unlock_with_correct_password_recovers_same_vault_key() {
        let pool = pool().await;
        let vk1 = create_vault(&pool, b"master-pw").await.unwrap();
        let vk2 = unlock(&pool, b"master-pw").await.unwrap();
        assert_eq!(vk1.expose(), vk2.expose());
    }

    #[tokio::test]
    async fn unlock_with_wrong_password_is_a_generic_crypto_error() {
        let pool = pool().await;
        create_vault(&pool, b"right").await.unwrap();
        // Avoid `unwrap_err` here: it would require `SecretKey: Debug`, which we
        // deliberately do not implement (a key must never be printable).
        let err = match unlock(&pool, b"wrong").await {
            Err(e) => e,
            Ok(_) => panic!("a wrong password must not unlock the vault"),
        };
        assert!(matches!(err, AppError::Crypto(_)), "got {err:?}");
        // Anti-oracle (F5): the message must be the single generic one.
        assert_eq!(err.to_string(), "opération de coffre invalide");
    }

    #[tokio::test]
    async fn creating_twice_is_rejected() {
        let pool = pool().await;
        create_vault(&pool, b"pw").await.unwrap();
        assert!(matches!(
            create_vault(&pool, b"pw").await,
            Err(AppError::VaultAlreadyInitialized)
        ));
    }

    #[tokio::test]
    async fn unlock_before_init_is_rejected() {
        let pool = pool().await;
        assert!(matches!(
            unlock(&pool, b"pw").await,
            Err(AppError::VaultNotInitialized)
        ));
    }

    #[tokio::test]
    async fn default_environment_envkey_unwraps_under_the_vault_key() {
        let pool = pool().await;
        let vk = create_vault(&pool, b"pw").await.unwrap();
        let env_id = default_environment_id(&pool).await.unwrap();
        // Correct env id + vault key => unwraps.
        load_env_key(&pool, &vk, &env_id).await.unwrap();
        // Wrong env id => AAD mismatch => crypto error (anti-swap, F8).
        assert!(load_env_key(&pool, &vk, "not-the-env").await.is_err());
    }

    #[tokio::test]
    async fn change_master_password_keeps_vault_key_and_invalidates_old() {
        let pool = pool().await;
        let vk_before = create_vault(&pool, b"old").await.unwrap();
        let env_id = default_environment_id(&pool).await.unwrap();

        change_master_password(&pool, b"old", b"new").await.unwrap();

        // New password unlocks the SAME vault key.
        let vk_after = unlock(&pool, b"new").await.unwrap();
        assert_eq!(vk_before.expose(), vk_after.expose());
        // Old password no longer works.
        assert!(unlock(&pool, b"old").await.is_err());
        // Environments untouched: the env key still unwraps under the same vault key.
        load_env_key(&pool, &vk_after, &env_id).await.unwrap();
    }

    #[tokio::test]
    async fn change_master_password_requires_the_correct_current_password() {
        let pool = pool().await;
        create_vault(&pool, b"old").await.unwrap();
        assert!(change_master_password(&pool, b"bad", b"new").await.is_err());
        // The old password must still work after a failed change.
        unlock(&pool, b"old").await.unwrap();
    }

    #[tokio::test]
    async fn no_clear_key_or_password_material_on_disk() {
        // F1/F2: scan every blob/text column for the vault key bytes and the
        // password — none must appear in the clear.
        let pool = pool().await;
        let vk = create_vault(&pool, b"super-secret-pw").await.unwrap();

        let vrow = sqlx::query(
            "SELECT kdf_salt, vault_key_wrapped, vault_key_nonce FROM vault WHERE id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let wrapped: Vec<u8> = vrow.get("vault_key_wrapped");
        assert_ne!(wrapped.as_slice(), vk.expose(), "vault key stored in clear!");
        assert!(!contains(&wrapped, b"super-secret-pw"), "password in vault blob!");

        let erow = sqlx::query("SELECT env_key_wrapped FROM environments LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let env_wrapped: Vec<u8> = erow.get("env_key_wrapped");
        assert!(!contains(&env_wrapped, vk.expose()), "vault key in env blob!");
        assert!(!contains(&env_wrapped, b"super-secret-pw"), "password in env blob!");
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }
}
