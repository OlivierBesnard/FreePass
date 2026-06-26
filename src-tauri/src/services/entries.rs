//! Entry CRUD (DESIGN §4-5). Secret fields are encrypted per-field under the
//! environment key before they touch SQLite and decrypted in memory on read
//! (CRYPTO_SPEC §4). `title`/`url` are clear metadata (assumed, F5). Search runs
//! against the local DB on clear columns only — never leaves the machine.

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::crypto::{self, Sealed, SecretKey};
use crate::error::{AppError, AppResult};
use crate::models::entry::{EntryDetail, EntryInput, EntrySummary};

/// Entry type handled in v1. `secret` / `env_var` come with the agent feature.
const ENTRY_TYPE_LOGIN: &str = "login";

/// The encryptable login fields, in a stable order.
const LOGIN_FIELDS: [&str; 3] = ["username", "password", "notes"];

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

    sqlx::query("DELETE FROM entry_fields WHERE entry_id = ?")
        .bind(entry_id)
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
    async fn get_missing_entry_is_not_found() {
        let (pool, env_key, env_id) = setup().await;
        assert!(matches!(
            get_entry(&pool, &env_key, &env_id, "nope").await,
            Err(AppError::NotFound)
        ));
    }
}
