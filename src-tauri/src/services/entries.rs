//! Entry CRUD (DESIGN §4-5). Secret fields are encrypted per-field under the
//! environment key before they touch SQLite and decrypted in memory on read
//! (CRYPTO_SPEC §4). `title`/`url` are clear metadata (assumed, F5). Search runs
//! against the local DB on clear columns only — never leaves the machine.

use std::collections::HashMap;

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::crypto::{self, Sealed, SecretKey};
use crate::error::{AppError, AppResult};
use crate::models::entry::{EntryDetail, EntryInput, EntrySummary};

/// Entry type handled in v1. `secret` / `env_var` come with the agent feature.
const ENTRY_TYPE_LOGIN: &str = "login";

/// The encryptable login fields, in a stable order. The favicon (`icon`) is a
/// separate encrypted field that is *not* part of this set: it is fetched
/// independently and must survive an entry edit (see `update_entry`).
const LOGIN_FIELDS: [&str; 3] = ["username", "password", "notes"];

/// Field name under which the site favicon (a `data:` URL) is stored, encrypted
/// under the env key with AAD bound to env_id + entry_id + "icon" like any
/// other field.
const ICON_FIELD: &str = "icon";

fn nonce_from(blob: Vec<u8>) -> AppResult<[u8; crypto::aead::NONCE_LEN]> {
    blob.try_into()
        .map_err(|_| AppError::Crypto(crypto::CryptoError::Decrypt))
}

fn field_value<'a>(input: &'a EntryInput, name: &str) -> Option<&'a str> {
    let v = match name {
        "username" => input.username.as_deref(),
        "password" => input.password.as_deref(),
        "notes" => input.notes.as_deref(),
        _ => None,
    };
    v.filter(|s| !s.is_empty())
}

/// List clear-metadata summaries for an environment, optionally filtered by a
/// local search over `title`/`url`. No secret material is read (F5).
pub async fn list_entries(
    pool: &SqlitePool,
    env_id: &str,
    search: Option<&str>,
) -> AppResult<Vec<EntrySummary>> {
    let query = search.map(str::trim).filter(|s| !s.is_empty());
    let rows = if let Some(q) = query {
        let like = format!("%{q}%");
        sqlx::query(
            "SELECT id, env_id, type, title, url, updated_at FROM entries \
             WHERE env_id = ? AND archived_at IS NULL \
             AND (title LIKE ? OR IFNULL(url, '') LIKE ?) \
             ORDER BY title COLLATE NOCASE",
        )
        .bind(env_id)
        .bind(&like)
        .bind(&like)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, env_id, type, title, url, updated_at FROM entries \
             WHERE env_id = ? AND archived_at IS NULL ORDER BY title COLLATE NOCASE",
        )
        .bind(env_id)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .iter()
        .map(|r| EntrySummary {
            id: r.get("id"),
            env_id: r.get("env_id"),
            kind: r.get("type"),
            title: r.get("title"),
            url: r.get("url"),
            updated_at: r.get("updated_at"),
            // Per-environment list: the caller already knows the environment.
            env_name: None,
        })
        .collect())
}

/// List clear-metadata summaries across ALL live environments (Phase 10: the
/// unified, by-site list), optionally filtered by a local search over
/// `title`/`url`. Like `list_entries`, this reads clear metadata only — no
/// envKey, no decryption, no secret material (F5). An environment (or its owning
/// project) being archived excludes its entries, mirroring the autofill scan in
/// `local_channel::credentials_for_origin` so a "put away" environment is
/// consistently inactive everywhere. Each row carries its owning environment's
/// clear `name` so the front can show it as an optional badge.
pub async fn list_all_entries(
    pool: &SqlitePool,
    search: Option<&str>,
) -> AppResult<Vec<EntrySummary>> {
    const BASE: &str = "SELECT e.id, e.env_id, e.type, e.title, e.url, e.updated_at, \
         env.name AS env_name \
         FROM entries e \
         JOIN environments env ON env.id = e.env_id \
         LEFT JOIN projects p ON p.id = env.project_id \
         WHERE e.archived_at IS NULL AND env.archived_at IS NULL \
         AND (env.project_id IS NULL OR p.archived_at IS NULL)";

    let query = search.map(str::trim).filter(|s| !s.is_empty());
    let rows = if let Some(q) = query {
        let like = format!("%{q}%");
        sqlx::query(&format!(
            "{BASE} AND (e.title LIKE ? OR IFNULL(e.url, '') LIKE ?) \
             ORDER BY e.title COLLATE NOCASE"
        ))
        .bind(&like)
        .bind(&like)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(&format!("{BASE} ORDER BY e.title COLLATE NOCASE"))
            .fetch_all(pool)
            .await?
    };

    Ok(rows
        .iter()
        .map(|r| EntrySummary {
            id: r.get("id"),
            env_id: r.get("env_id"),
            kind: r.get("type"),
            title: r.get("title"),
            url: r.get("url"),
            updated_at: r.get("updated_at"),
            env_name: Some(r.get("env_name")),
        })
        .collect())
}

/// Fetch one entry and decrypt its fields in memory.
pub async fn get_entry(
    pool: &SqlitePool,
    env_key: &SecretKey,
    env_id: &str,
    entry_id: &str,
) -> AppResult<EntryDetail> {
    let row = sqlx::query(
        "SELECT type, title, url, created_at, updated_at FROM entries \
         WHERE id = ? AND env_id = ? AND archived_at IS NULL",
    )
    .bind(entry_id)
    .bind(env_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let mut detail = EntryDetail {
        id: entry_id.to_string(),
        env_id: env_id.to_string(),
        kind: row.get("type"),
        title: row.get("title"),
        url: row.get("url"),
        username: None,
        password: None,
        notes: None,
        icon: None,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    let fields = sqlx::query(
        "SELECT field_name, nonce, ciphertext FROM entry_fields WHERE entry_id = ?",
    )
    .bind(entry_id)
    .fetch_all(pool)
    .await?;

    for f in fields {
        let name: String = f.get("field_name");
        let sealed = Sealed {
            nonce: nonce_from(f.get("nonce"))?,
            ciphertext: f.get("ciphertext"),
        };
        let plain = crypto::decrypt_field(env_key, env_id, entry_id, &name, &sealed)?;
        let value = String::from_utf8(plain.to_vec())
            .map_err(|_| AppError::Other("champ non-UTF8".into()))?;
        match name.as_str() {
            "username" => detail.username = Some(value),
            "password" => detail.password = Some(value),
            "notes" => detail.notes = Some(value),
            "icon" => detail.icon = Some(value),
            _ => {}
        }
    }

    Ok(detail)
}

/// Create a login entry, encrypting each present field under the env key.
pub async fn create_entry(
    pool: &SqlitePool,
    env_key: &SecretKey,
    env_id: &str,
    input: &EntryInput,
) -> AppResult<String> {
    if input.title.trim().is_empty() {
        return Err(AppError::Conflict("le titre est requis".into()));
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO entries (id, env_id, type, title, url, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(env_id)
    .bind(ENTRY_TYPE_LOGIN)
    .bind(input.title.trim())
    .bind(&input.url)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    for name in LOGIN_FIELDS {
        if let Some(value) = field_value(input, name) {
            let sealed = crypto::encrypt_field(env_key, env_id, &id, name, value.as_bytes())?;
            sqlx::query(
                "INSERT INTO entry_fields (entry_id, field_name, nonce, ciphertext) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(name)
            .bind(&sealed.nonce[..])
            .bind(&sealed.ciphertext)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(id)
}

/// Update a login entry: replace metadata and re-encrypt all fields (fresh
/// nonces, F10). Fields absent/empty in `input` are removed.
pub async fn update_entry(
    pool: &SqlitePool,
    env_key: &SecretKey,
    env_id: &str,
    entry_id: &str,
    input: &EntryInput,
) -> AppResult<()> {
    if input.title.trim().is_empty() {
        return Err(AppError::Conflict("le titre est requis".into()));
    }
    let now = Utc::now().to_rfc3339();

    let mut tx = pool.begin().await?;
    let res = sqlx::query(
        "UPDATE entries SET title = ?, url = ?, updated_at = ? \
         WHERE id = ? AND env_id = ? AND archived_at IS NULL",
    )
    .bind(input.title.trim())
    .bind(&input.url)
    .bind(&now)
    .bind(entry_id)
    .bind(env_id)
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    // Replace only the login fields; the favicon (`icon`) is fetched
    // independently and must survive an edit, so it is left untouched.
    sqlx::query("DELETE FROM entry_fields WHERE entry_id = ? AND field_name <> ?")
        .bind(entry_id)
        .bind(ICON_FIELD)
        .execute(&mut *tx)
        .await?;
    for name in LOGIN_FIELDS {
        if let Some(value) = field_value(input, name) {
            let sealed = crypto::encrypt_field(env_key, env_id, entry_id, name, value.as_bytes())?;
            sqlx::query(
                "INSERT INTO entry_fields (entry_id, field_name, nonce, ciphertext) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(entry_id)
            .bind(name)
            .bind(&sealed.nonce[..])
            .bind(&sealed.ciphertext)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// List archived (soft-deleted) entries — the "trash". Clear metadata only.
pub async fn list_archived_entries(
    pool: &SqlitePool,
    env_id: &str,
) -> AppResult<Vec<EntrySummary>> {
    let rows = sqlx::query(
        "SELECT id, env_id, type, title, url, updated_at FROM entries \
         WHERE env_id = ? AND archived_at IS NOT NULL ORDER BY archived_at DESC",
    )
    .bind(env_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| EntrySummary {
            id: r.get("id"),
            env_id: r.get("env_id"),
            kind: r.get("type"),
            title: r.get("title"),
            url: r.get("url"),
            updated_at: r.get("updated_at"),
            // Trash is scoped to one environment: the caller knows it.
            env_name: None,
        })
        .collect())
}

/// Restore an archived entry (un-archive). Its encrypted fields are untouched.
pub async fn restore_entry(pool: &SqlitePool, env_id: &str, entry_id: &str) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let res = sqlx::query(
        "UPDATE entries SET archived_at = NULL, updated_at = ? \
         WHERE id = ? AND env_id = ? AND archived_at IS NOT NULL",
    )
    .bind(&now)
    .bind(entry_id)
    .bind(env_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// Soft-delete (archive) an entry.
pub async fn archive_entry(pool: &SqlitePool, env_id: &str, entry_id: &str) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let res = sqlx::query(
        "UPDATE entries SET archived_at = ?, updated_at = ? \
         WHERE id = ? AND env_id = ? AND archived_at IS NULL",
    )
    .bind(&now)
    .bind(&now)
    .bind(entry_id)
    .bind(env_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// Hard-delete (purge) an entry — only allowed once it has been archived. Its
/// encrypted fields cascade away via the FK.
pub async fn delete_entry(pool: &SqlitePool, env_id: &str, entry_id: &str) -> AppResult<()> {
    let res = sqlx::query(
        "DELETE FROM entries WHERE id = ? AND env_id = ? AND archived_at IS NOT NULL",
    )
    .bind(entry_id)
    .bind(env_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "l'entrée doit être archivée avant suppression".into(),
        ));
    }
    Ok(())
}

/// Bulk-import login entries in a single transaction (CSV import, F13). Rows
/// with an empty title are skipped. Returns the number of entries created.
pub async fn import_entries(
    pool: &SqlitePool,
    env_key: &SecretKey,
    env_id: &str,
    inputs: &[EntryInput],
) -> AppResult<usize> {
    let now = Utc::now().to_rfc3339();
    let mut created = 0usize;

    let mut tx = pool.begin().await?;
    for input in inputs {
        if input.title.trim().is_empty() {
            continue;
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO entries (id, env_id, type, title, url, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(env_id)
        .bind(ENTRY_TYPE_LOGIN)
        .bind(input.title.trim())
        .bind(&input.url)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        for name in LOGIN_FIELDS {
            if let Some(value) = field_value(input, name) {
                let sealed = crypto::encrypt_field(env_key, env_id, &id, name, value.as_bytes())?;
                sqlx::query(
                    "INSERT INTO entry_fields (entry_id, field_name, nonce, ciphertext) \
                     VALUES (?, ?, ?, ?)",
                )
                .bind(&id)
                .bind(name)
                .bind(&sealed.nonce[..])
                .bind(&sealed.ciphertext)
                .execute(&mut *tx)
                .await?;
            }
        }
        created += 1;
    }
    tx.commit().await?;
    Ok(created)
}

/// Store (or replace) the encrypted favicon for an entry. The icon is a `data:`
/// URL string, sealed under the env key with AAD bound to env_id + entry_id +
/// "icon" — exactly like the other fields. No-op safety: callers fetch the icon
/// best-effort, so a `None` data URL clears any stored icon.
pub async fn set_icon(
    pool: &SqlitePool,
    env_key: &SecretKey,
    env_id: &str,
    entry_id: &str,
    data_url: Option<&str>,
) -> AppResult<()> {
    // Make sure the entry exists in this environment before writing a field.
    let exists = sqlx::query("SELECT 1 FROM entries WHERE id = ? AND env_id = ?")
        .bind(entry_id)
        .bind(env_id)
        .fetch_optional(pool)
        .await?
        .is_some();
    if !exists {
        return Err(AppError::NotFound);
    }

    match data_url.filter(|s| !s.is_empty()) {
        Some(value) => {
            let sealed = crypto::encrypt_field(env_key, env_id, entry_id, ICON_FIELD, value.as_bytes())?;
            sqlx::query(
                "INSERT OR REPLACE INTO entry_fields (entry_id, field_name, nonce, ciphertext) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(entry_id)
            .bind(ICON_FIELD)
            .bind(&sealed.nonce[..])
            .bind(&sealed.ciphertext)
            .execute(pool)
            .await?;
        }
        None => {
            sqlx::query("DELETE FROM entry_fields WHERE entry_id = ? AND field_name = ?")
                .bind(entry_id)
                .bind(ICON_FIELD)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// Decrypt every stored favicon for a (non-archived) environment, keyed by entry
/// id. Used to overlay icons on the list view without decrypting secret fields.
pub async fn load_icons(
    pool: &SqlitePool,
    env_key: &SecretKey,
    env_id: &str,
) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query(
        "SELECT f.entry_id, f.nonce, f.ciphertext FROM entry_fields f \
         JOIN entries e ON e.id = f.entry_id \
         WHERE f.field_name = ? AND e.env_id = ? AND e.archived_at IS NULL",
    )
    .bind(ICON_FIELD)
    .bind(env_id)
    .fetch_all(pool)
    .await?;

    let mut out = HashMap::with_capacity(rows.len());
    for r in rows {
        let entry_id: String = r.get("entry_id");
        let sealed = Sealed {
            nonce: nonce_from(r.get("nonce"))?,
            ciphertext: r.get("ciphertext"),
        };
        let plain = crypto::decrypt_field(env_key, env_id, &entry_id, ICON_FIELD, &sealed)?;
        if let Ok(value) = String::from_utf8(plain.to_vec()) {
            out.insert(entry_id, value);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_with_url;
    use crate::services::vault;

    async fn setup() -> (SqlitePool, SecretKey, String) {
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        let vk = vault::create_vault(&pool, b"pw").await.unwrap();
        let env_id = vault::default_environment_id(&pool).await.unwrap();
        let env_key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();
        (pool, env_key, env_id)
    }

    fn login(title: &str, url: Option<&str>, user: &str, pass: &str) -> EntryInput {
        EntryInput {
            title: title.into(),
            url: url.map(Into::into),
            username: Some(user.into()),
            password: Some(pass.into()),
            notes: None,
        }
    }

    #[tokio::test]
    async fn create_then_get_roundtrips_decrypted_fields() {
        let (pool, env_key, env_id) = setup().await;
        let id = create_entry(&pool, &env_key, &env_id, &login("GitHub", Some("github.com"), "alice", "hunter2"))
            .await
            .unwrap();
        let got = get_entry(&pool, &env_key, &env_id, &id).await.unwrap();
        assert_eq!(got.title, "GitHub");
        assert_eq!(got.url.as_deref(), Some("github.com"));
        assert_eq!(got.username.as_deref(), Some("alice"));
        assert_eq!(got.password.as_deref(), Some("hunter2"));
        assert_eq!(got.notes, None);
    }

    #[tokio::test]
    async fn list_returns_clear_metadata_and_filters_by_search() {
        let (pool, env_key, env_id) = setup().await;
        create_entry(&pool, &env_key, &env_id, &login("GitHub", Some("github.com"), "a", "x")).await.unwrap();
        create_entry(&pool, &env_key, &env_id, &login("GitLab", Some("gitlab.com"), "b", "y")).await.unwrap();
        create_entry(&pool, &env_key, &env_id, &login("Bank", Some("bank.fr"), "c", "z")).await.unwrap();

        let all = list_entries(&pool, &env_id, None).await.unwrap();
        assert_eq!(all.len(), 3);

        let git = list_entries(&pool, &env_id, Some("git")).await.unwrap();
        assert_eq!(git.len(), 2, "search should match GitHub + GitLab on title");

        let bank = list_entries(&pool, &env_id, Some("bank.fr")).await.unwrap();
        assert_eq!(bank.len(), 1, "search should match on url too");
    }

    #[tokio::test]
    async fn update_replaces_fields() {
        let (pool, env_key, env_id) = setup().await;
        let id = create_entry(&pool, &env_key, &env_id, &login("X", None, "old", "oldpw")).await.unwrap();
        let mut input = login("X renamed", Some("x.com"), "new", "newpw");
        input.notes = Some("a note".into());
        update_entry(&pool, &env_key, &env_id, &id, &input).await.unwrap();

        let got = get_entry(&pool, &env_key, &env_id, &id).await.unwrap();
        assert_eq!(got.title, "X renamed");
        assert_eq!(got.username.as_deref(), Some("new"));
        assert_eq!(got.password.as_deref(), Some("newpw"));
        assert_eq!(got.notes.as_deref(), Some("a note"));
    }

    #[tokio::test]
    async fn archive_hides_from_list_and_purge_requires_archive_first() {
        let (pool, env_key, env_id) = setup().await;
        let id = create_entry(&pool, &env_key, &env_id, &login("X", None, "u", "p")).await.unwrap();

        // Cannot purge a live entry.
        assert!(matches!(
            delete_entry(&pool, &env_id, &id).await,
            Err(AppError::Conflict(_))
        ));

        archive_entry(&pool, &env_id, &id).await.unwrap();
        assert_eq!(list_entries(&pool, &env_id, None).await.unwrap().len(), 0);

        // Now purge works, and the fields cascade away.
        delete_entry(&pool, &env_id, &id).await.unwrap();
        let n: i64 = sqlx::query("SELECT COUNT(*) AS n FROM entry_fields WHERE entry_id = ?")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("n");
        assert_eq!(n, 0, "entry_fields must cascade-delete");
    }

    #[tokio::test]
    async fn secret_fields_are_unreadable_on_disk_but_title_is_clear() {
        // F1/F5: the password must not appear in the clear in entry_fields; the
        // title is deliberately clear metadata.
        let (pool, env_key, env_id) = setup().await;
        let id = create_entry(&pool, &env_key, &env_id, &login("MyBank", Some("bank.fr"), "alice", "S3cr3t-Pw"))
            .await
            .unwrap();

        let frow = sqlx::query("SELECT ciphertext FROM entry_fields WHERE entry_id = ? AND field_name = 'password'")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let ct: Vec<u8> = frow.get("ciphertext");
        assert!(
            !ct.windows(9).any(|w| w == b"S3cr3t-Pw"),
            "password leaked in the clear in entry_fields"
        );

        let erow = sqlx::query("SELECT title FROM entries WHERE id = ?")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let title: String = erow.get("title");
        assert_eq!(title, "MyBank", "title is clear metadata by design");
    }

    #[tokio::test]
    async fn import_creates_entries_and_skips_titleless_rows() {
        let (pool, env_key, env_id) = setup().await;
        let rows = vec![
            login("GitHub", Some("github.com"), "a", "x"),
            EntryInput { title: "  ".into(), url: None, username: None, password: Some("p".into()), notes: None },
            login("GitLab", Some("gitlab.com"), "b", "y"),
        ];
        let n = import_entries(&pool, &env_key, &env_id, &rows).await.unwrap();
        assert_eq!(n, 2, "the empty-title row must be skipped");

        let list = list_entries(&pool, &env_id, None).await.unwrap();
        assert_eq!(list.len(), 2);
        // Imported secrets decrypt correctly.
        let gh = list.iter().find(|e| e.title == "GitHub").unwrap();
        let got = get_entry(&pool, &env_key, &env_id, &gh.id).await.unwrap();
        assert_eq!(got.password.as_deref(), Some("x"));
    }

    #[tokio::test]
    async fn archived_entries_can_be_listed_and_restored() {
        let (pool, env_key, env_id) = setup().await;
        let id = create_entry(&pool, &env_key, &env_id, &login("X", None, "u", "p")).await.unwrap();

        archive_entry(&pool, &env_id, &id).await.unwrap();
        assert_eq!(list_entries(&pool, &env_id, None).await.unwrap().len(), 0);
        let archived = list_archived_entries(&pool, &env_id).await.unwrap();
        assert_eq!(archived.len(), 1, "archived entry should be in the trash");

        restore_entry(&pool, &env_id, &id).await.unwrap();
        assert_eq!(list_entries(&pool, &env_id, None).await.unwrap().len(), 1);
        assert_eq!(list_archived_entries(&pool, &env_id).await.unwrap().len(), 0);
        // Restored entry still decrypts.
        let got = get_entry(&pool, &env_key, &env_id, &id).await.unwrap();
        assert_eq!(got.password.as_deref(), Some("p"));
    }

    #[tokio::test]
    async fn restoring_a_live_entry_is_not_found() {
        let (pool, env_key, env_id) = setup().await;
        let id = create_entry(&pool, &env_key, &env_id, &login("X", None, "u", "p")).await.unwrap();
        assert!(matches!(
            restore_entry(&pool, &env_id, &id).await,
            Err(AppError::NotFound)
        ));
    }

    #[tokio::test]
    async fn icon_is_stored_encrypted_survives_edits_and_clears() {
        let (pool, env_key, env_id) = setup().await;
        let id = create_entry(&pool, &env_key, &env_id, &login("GitHub", Some("github.com"), "a", "x"))
            .await
            .unwrap();

        let data_url = "data:image/png;base64,AAAA";
        set_icon(&pool, &env_key, &env_id, &id, Some(data_url)).await.unwrap();

        // Round-trips on the detail and via the list-overlay loader.
        let got = get_entry(&pool, &env_key, &env_id, &id).await.unwrap();
        assert_eq!(got.icon.as_deref(), Some(data_url));
        let icons = load_icons(&pool, &env_key, &env_id).await.unwrap();
        assert_eq!(icons.get(&id).map(String::as_str), Some(data_url));

        // Stored encrypted: the data URL must not appear in the clear on disk.
        let ct: Vec<u8> = sqlx::query("SELECT ciphertext FROM entry_fields WHERE entry_id = ? AND field_name = 'icon'")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("ciphertext");
        assert!(!ct.windows(4).any(|w| w == b"AAAA"), "icon leaked in the clear");

        // Editing the entry must NOT drop the icon.
        update_entry(&pool, &env_key, &env_id, &id, &login("GitHub", Some("github.com"), "a2", "x2"))
            .await
            .unwrap();
        let after = get_entry(&pool, &env_key, &env_id, &id).await.unwrap();
        assert_eq!(after.icon.as_deref(), Some(data_url), "icon must survive an edit");
        assert_eq!(after.username.as_deref(), Some("a2"));

        // Passing None clears it.
        set_icon(&pool, &env_key, &env_id, &id, None).await.unwrap();
        assert_eq!(get_entry(&pool, &env_key, &env_id, &id).await.unwrap().icon, None);
    }

    #[tokio::test]
    async fn set_icon_on_missing_entry_is_not_found() {
        let (pool, env_key, env_id) = setup().await;
        assert!(matches!(
            set_icon(&pool, &env_key, &env_id, "nope", Some("data:image/png;base64,AA")).await,
            Err(AppError::NotFound)
        ));
    }

    #[tokio::test]
    async fn get_missing_entry_is_not_found() {
        let (pool, env_key, env_id) = setup().await;
        assert!(matches!(
            get_entry(&pool, &env_key, &env_id, "nope").await,
            Err(AppError::NotFound)
        ));
    }

    // === Phase 10: isolation between environments (adversarial) ===

    use crate::services::{environments, projects};

    /// Two live environments under one project, each with its own envKey.
    /// Returns (pool, vaultKey, (env_a_id, env_a_key), (env_b_id, env_b_key)).
    async fn two_envs() -> (
        SqlitePool,
        SecretKey,
        (String, SecretKey),
        (String, SecretKey),
    ) {
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        let vk = vault::create_vault(&pool, b"pw").await.unwrap();
        projects::backfill_default_project(&pool).await.unwrap();
        let project = projects::list_projects(&pool).await.unwrap()[0].id.clone();
        // env A is the default environment; env B is a freshly minted one.
        let env_a_id = vault::default_environment_id(&pool).await.unwrap();
        let env_a_key = vault::load_env_key(&pool, &vk, &env_a_id).await.unwrap();
        let env_b = environments::create_environment(&pool, &vk, &project, "Prod")
            .await
            .unwrap();
        let env_b_key = vault::load_env_key(&pool, &vk, &env_b.id).await.unwrap();
        (pool, vk, (env_a_id, env_a_key), (env_b.id, env_b_key))
    }

    #[tokio::test]
    async fn list_entries_is_scoped_to_its_environment() {
        // An entry created in env A must NOT appear when listing env B.
        let (pool, _vk, (env_a, key_a), (env_b, _key_b)) = two_envs().await;
        create_entry(&pool, &key_a, &env_a, &login("OnlyInA", Some("a.com"), "u", "p"))
            .await
            .unwrap();

        let in_a = list_entries(&pool, &env_a, None).await.unwrap();
        assert_eq!(in_a.len(), 1, "the entry must be visible in its own env");

        let in_b = list_entries(&pool, &env_b, None).await.unwrap();
        assert!(in_b.is_empty(), "env B must not see env A's entry (isolation)");

        // Search in env B must not surface env A's entry either.
        let search_b = list_entries(&pool, &env_b, Some("OnlyInA")).await.unwrap();
        assert!(search_b.is_empty(), "scoped search must not cross environments");
    }

    #[tokio::test]
    async fn get_entry_with_the_wrong_env_id_is_not_found() {
        // get_entry filters by env_id in the WHERE clause: querying env B for an
        // entry that lives in env A must be NotFound (not a decrypt error / leak).
        let (pool, _vk, (env_a, key_a), (env_b, key_b)) = two_envs().await;
        let id = create_entry(&pool, &key_a, &env_a, &login("Secret", Some("a.com"), "u", "p"))
            .await
            .unwrap();

        // Correct env id + key => fine.
        get_entry(&pool, &key_a, &env_a, &id).await.unwrap();
        // Wrong env id (even with env B's own key) => NotFound, never a partial read.
        assert!(matches!(
            get_entry(&pool, &key_b, &env_b, &id).await,
            Err(AppError::NotFound)
        ));
    }

    #[tokio::test]
    async fn entry_does_not_decrypt_under_another_environments_key() {
        // F8 at the entry level, end-to-end through the service layer: env A's
        // ciphertext must NOT decrypt under env B's envKey. We bypass get_entry's
        // env_id WHERE filter by reading the raw field and decrypting with the
        // WRONG key/env to prove the AEAD (not just the SQL filter) rejects it.
        let (pool, _vk, (env_a, key_a), (env_b, key_b)) = two_envs().await;
        let id = create_entry(&pool, &key_a, &env_a, &login("X", Some("a.com"), "u", "topsecret"))
            .await
            .unwrap();

        let f = sqlx::query(
            "SELECT nonce, ciphertext FROM entry_fields WHERE entry_id = ? AND field_name = 'password'",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let sealed = Sealed {
            nonce: nonce_from(f.get("nonce")).unwrap(),
            ciphertext: f.get("ciphertext"),
        };

        // Correct env key + env id => decrypts.
        let ok = crypto::decrypt_field(&key_a, &env_a, &id, "password", &sealed);
        assert_eq!(&ok.unwrap()[..], b"topsecret");

        // Env B's key (different key) => MAC failure, no plaintext.
        assert!(
            crypto::decrypt_field(&key_b, &env_a, &id, "password", &sealed).is_err(),
            "env B's key must not decrypt env A's field (F8 / key isolation)"
        );
        // Even env A's key but B's env_id in the AAD => MAC failure (anti cross-env swap).
        assert!(
            crypto::decrypt_field(&key_a, &env_b, &id, "password", &sealed).is_err(),
            "field bound to env A's id must not open under env B's id (F8 AAD)"
        );
    }

    #[tokio::test]
    async fn icons_loader_is_scoped_to_its_environment() {
        // load_icons joins on entries.env_id; an icon stored in env A must not be
        // returned when loading env B's icons.
        let (pool, _vk, (env_a, key_a), (env_b, key_b)) = two_envs().await;
        let id = create_entry(&pool, &key_a, &env_a, &login("WithIcon", Some("a.com"), "u", "p"))
            .await
            .unwrap();
        set_icon(&pool, &key_a, &env_a, &id, Some("data:image/png;base64,ZZZZ"))
            .await
            .unwrap();

        assert_eq!(load_icons(&pool, &key_a, &env_a).await.unwrap().len(), 1);
        assert!(
            load_icons(&pool, &key_b, &env_b).await.unwrap().is_empty(),
            "env B must not load env A's icons"
        );
    }

    #[tokio::test]
    async fn mutations_cannot_target_an_entry_through_the_wrong_environment() {
        // An attacker (or a bug) passing entry A's id but env B's id must not be
        // able to update / archive / delete / set-icon that entry across the env
        // boundary. Every mutation filters by env_id.
        let (pool, _vk, (env_a, key_a), (env_b, key_b)) = two_envs().await;
        let id = create_entry(&pool, &key_a, &env_a, &login("X", Some("a.com"), "u", "p"))
            .await
            .unwrap();

        // update via wrong env => NotFound, and the entry is left intact.
        assert!(matches!(
            update_entry(&pool, &key_b, &env_b, &id, &login("hijacked", None, "z", "z")).await,
            Err(AppError::NotFound)
        ));
        assert_eq!(
            get_entry(&pool, &key_a, &env_a, &id).await.unwrap().title,
            "X",
            "cross-env update must not have mutated the entry"
        );

        // archive / restore / delete / set_icon via wrong env => refused.
        assert!(matches!(
            archive_entry(&pool, &env_b, &id).await,
            Err(AppError::NotFound)
        ));
        assert!(matches!(
            set_icon(&pool, &key_b, &env_b, &id, Some("data:image/png;base64,QQ")).await,
            Err(AppError::NotFound)
        ));
        // Entry is still live and listable only in env A.
        assert_eq!(list_entries(&pool, &env_a, None).await.unwrap().len(), 1);
        assert_eq!(list_entries(&pool, &env_b, None).await.unwrap().len(), 0);
    }

    // === Phase 10: unified cross-environment list (list_all_entries) ===

    #[tokio::test]
    async fn list_all_entries_unions_live_environments_with_env_name() {
        // Entries in TWO different environments must all appear, each tagged with
        // its owning environment's clear name.
        let (pool, _vk, (env_a, key_a), (env_b, key_b)) = two_envs().await;
        create_entry(&pool, &key_a, &env_a, &login("InA", Some("a.com"), "u", "p"))
            .await
            .unwrap();
        create_entry(&pool, &key_b, &env_b, &login("InB", Some("b.com"), "u", "p"))
            .await
            .unwrap();

        let all = list_all_entries(&pool, None).await.unwrap();
        assert_eq!(all.len(), 2, "both environments' entries must be listed");

        let a = all.iter().find(|e| e.title == "InA").unwrap();
        let b = all.iter().find(|e| e.title == "InB").unwrap();
        // env A is the default environment ("Personnel" default env name), env B is "Prod".
        assert_eq!(a.env_id, env_a);
        assert_eq!(b.env_id, env_b);
        assert_eq!(b.env_name.as_deref(), Some("Prod"), "env_name must be the env's clear name");
        assert!(a.env_name.is_some(), "every unified row carries its env_name");
    }

    #[tokio::test]
    async fn list_all_entries_excludes_archived_environment() {
        // An entry living in an archived environment must NOT surface in the list.
        let (pool, _vk, (env_a, key_a), (env_b, key_b)) = two_envs().await;
        create_entry(&pool, &key_a, &env_a, &login("Live", Some("a.com"), "u", "p"))
            .await
            .unwrap();
        create_entry(&pool, &key_b, &env_b, &login("Hidden", Some("b.com"), "u", "p"))
            .await
            .unwrap();

        environments::archive_environment(&pool, &env_b).await.unwrap();

        let all = list_all_entries(&pool, None).await.unwrap();
        assert_eq!(all.len(), 1, "archived env's entry must be excluded");
        assert_eq!(all[0].title, "Live");
    }

    #[tokio::test]
    async fn list_all_entries_excludes_entries_of_an_archived_project() {
        // Archiving the owning PROJECT removes its environments' entries from the
        // unified list, mirroring the autofill scan (F7 consistency).
        let (pool, _vk, (env_a, key_a), (env_b, key_b)) = two_envs().await;
        create_entry(&pool, &key_a, &env_a, &login("X", Some("a.com"), "u", "p"))
            .await
            .unwrap();
        create_entry(&pool, &key_b, &env_b, &login("Y", Some("b.com"), "u", "p"))
            .await
            .unwrap();

        // Both envs belong to the single default project.
        let project = projects::list_projects(&pool).await.unwrap()[0].id.clone();
        projects::archive_project(&pool, &project).await.unwrap();

        let all = list_all_entries(&pool, None).await.unwrap();
        assert!(all.is_empty(), "archiving the owning project must hide its entries");
    }

    #[tokio::test]
    async fn list_all_entries_filters_by_search_on_title_and_url() {
        let (pool, _vk, (env_a, key_a), (env_b, key_b)) = two_envs().await;
        create_entry(&pool, &key_a, &env_a, &login("GitHub", Some("github.com"), "u", "p"))
            .await
            .unwrap();
        create_entry(&pool, &key_b, &env_b, &login("GitLab", Some("gitlab.com"), "u", "p"))
            .await
            .unwrap();
        create_entry(&pool, &key_a, &env_a, &login("Bank", Some("bank.fr"), "u", "p"))
            .await
            .unwrap();

        // Title match spans environments.
        let git = list_all_entries(&pool, Some("git")).await.unwrap();
        assert_eq!(git.len(), 2, "search must match GitHub + GitLab across envs");
        // URL match.
        let bank = list_all_entries(&pool, Some("bank.fr")).await.unwrap();
        assert_eq!(bank.len(), 1, "search must match on url too");
        // Whitespace-only search is treated as no filter.
        let all = list_all_entries(&pool, Some("   ")).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn list_all_entries_leaks_no_secret_metadata_only() {
        // F5: the unified list must expose clear metadata only — never the
        // decrypted username/password/notes. EntrySummary structurally cannot
        // carry secrets; assert the row shape stays metadata + env_name.
        let (pool, _vk, (env_a, key_a), _b) = two_envs().await;
        create_entry(&pool, &key_a, &env_a, &login("MyBank", Some("bank.fr"), "alice", "S3cr3t"))
            .await
            .unwrap();

        let all = list_all_entries(&pool, None).await.unwrap();
        assert_eq!(all.len(), 1);
        let serialized = serde_json::to_string(&all).unwrap();
        assert!(!serialized.contains("alice"), "username must not appear in the list");
        assert!(!serialized.contains("S3cr3t"), "password must not appear in the list");
        assert!(serialized.contains("MyBank"), "title is clear metadata");
        assert!(serialized.contains("env_name"), "env_name is part of the summary");
    }

    #[tokio::test]
    async fn same_id_in_two_envs_each_decrypts_under_its_own_key() {
        // Defensive: even if (improbably) the same title/url exist in both envs,
        // each entry's secret is independent and only opens under its own envKey.
        let (pool, _vk, (env_a, key_a), (env_b, key_b)) = two_envs().await;
        let id_a = create_entry(&pool, &key_a, &env_a, &login("Dup", Some("dup.com"), "ua", "pa"))
            .await
            .unwrap();
        let id_b = create_entry(&pool, &key_b, &env_b, &login("Dup", Some("dup.com"), "ub", "pb"))
            .await
            .unwrap();

        assert_eq!(
            get_entry(&pool, &key_a, &env_a, &id_a).await.unwrap().password.as_deref(),
            Some("pa")
        );
        assert_eq!(
            get_entry(&pool, &key_b, &env_b, &id_b).await.unwrap().password.as_deref(),
            Some("pb")
        );
    }
}
