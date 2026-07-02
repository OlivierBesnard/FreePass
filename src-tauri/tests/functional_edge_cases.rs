//! Regression tests for the functional bugs fixed per AUDIT.md (B1, B2, B5,
//! D1–D4). These started life as characterization tests that ASSERTED the buggy
//! behaviour; they now lock in the corrected behaviour, so a regression is a
//! failing test. No prod code lives here — only the backend service layer is
//! exercised end-to-end.
//!
//! Run: cargo test --manifest-path src-tauri/Cargo.toml --test functional_edge_cases

use freepass_lib::error::AppError;
use freepass_lib::models::entry::EntryInput;
use freepass_lib::services::{entries, environments, projects, vault};
use freepass_lib::db::init_pool_with_url;
use sqlx::{Row, SqlitePool};

async fn pool() -> SqlitePool {
    init_pool_with_url("sqlite::memory:").await.unwrap()
}

fn login(title: &str, url: Option<&str>) -> EntryInput {
    EntryInput {
        title: title.into(),
        url: url.map(Into::into),
        username: Some("u".into()),
        password: Some("p".into()),
        notes: None,
    }
}

// ===========================================================================
// B5 — backfill after the "Personnel" project is archived
// ===========================================================================

/// Archiving "Personnel" then restarting must NOT create a duplicate project:
/// the backfill un-archives the existing one (it still carries the live default
/// env) instead of minting a second (B5).
#[tokio::test]
async fn archiving_personnel_then_backfill_reroots_without_duplicating() {
    let pool = pool().await;
    vault::create_vault(&pool, b"pw").await.unwrap();
    projects::backfill_default_project(&pool).await.unwrap();

    let personnel = projects::list_projects(&pool).await.unwrap()[0].id.clone();
    projects::archive_project(&pool, &personnel).await.unwrap();

    // Next startup reruns the backfill.
    projects::backfill_default_project(&pool).await.unwrap();

    let total: i64 = sqlx::query("SELECT COUNT(*) AS n FROM projects WHERE name = 'Personnel'")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("n");
    assert_eq!(total, 1, "backfill must not duplicate the Personnel project");

    let live = projects::list_projects(&pool).await.unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(
        live[0].id, personnel,
        "the SAME Personnel project is un-archived, not replaced by a new one"
    );
}

/// The default environment's entries must survive archiving "Personnel": while
/// archived they are put away (hidden), and the next backfill re-roots them so
/// they reappear — never silently lost (B1/B5).
#[tokio::test]
async fn entries_survive_archiving_their_project_after_backfill_reroots() {
    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    projects::backfill_default_project(&pool).await.unwrap();

    let env_id = vault::default_environment_id(&pool).await.unwrap();
    let env_key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();
    entries::create_entry(&pool, &env_key, &env_id, &login("GitHub", Some("github.com")))
        .await
        .unwrap();
    assert_eq!(entries::list_all_entries(&pool, None).await.unwrap().len(), 1);

    // Archive Personnel: while archived, the entry is consistently hidden.
    let personnel = projects::list_projects(&pool).await.unwrap()[0].id.clone();
    projects::archive_project(&pool, &personnel).await.unwrap();
    assert!(
        entries::list_all_entries(&pool, None).await.unwrap().is_empty(),
        "while Personnel is archived, its entries are hidden (consistent)"
    );

    // Next startup: the backfill un-archives Personnel (it still owns the live
    // default env), so the entry becomes visible again.
    projects::backfill_default_project(&pool).await.unwrap();
    let all = entries::list_all_entries(&pool, None).await.unwrap();
    assert_eq!(all.len(), 1, "the entry reappears after re-rooting");
    assert_eq!(all[0].title, "GitHub");
    // Per-environment access always saw it (it was never destroyed).
    assert_eq!(entries::list_entries(&pool, &env_id, None).await.unwrap().len(), 1);
}

// ===========================================================================
// D1 — archiving the last / only environment is refused
// ===========================================================================

/// The backend now refuses to archive the ONLY live environment (D1); the front
/// guard is no longer the sole protection. The default env stays available.
#[tokio::test]
async fn backend_refuses_archiving_the_only_environment() {
    let pool = pool().await;
    vault::create_vault(&pool, b"pw").await.unwrap();
    let env_id = vault::default_environment_id(&pool).await.unwrap();

    assert!(
        matches!(
            environments::archive_environment(&pool, &env_id).await,
            Err(AppError::Conflict(_))
        ),
        "archiving the last environment must be refused (D1)"
    );
    // The default env is untouched and still resolvable.
    assert_eq!(vault::default_environment_id(&pool).await.unwrap(), env_id);
}

/// load_env_key fails closed for an ARCHIVED environment (the crypto layer never
/// hands out a key for a put-away env). We create a second env so the first is
/// not the last (D1 would otherwise refuse the archive).
#[tokio::test]
async fn load_env_key_rejects_an_archived_environment() {
    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    projects::backfill_default_project(&pool).await.unwrap();
    let project = projects::list_projects(&pool).await.unwrap()[0].id.clone();

    let env2 = environments::create_environment(&pool, &vk, &project, "Prod")
        .await
        .unwrap();
    environments::archive_environment(&pool, &env2.id).await.unwrap();

    assert!(matches!(
        vault::load_env_key(&pool, &vk, &env2.id).await,
        Err(AppError::EnvironmentNotFound)
    ));
}

// ===========================================================================
// B1 — default_environment_id skips an env whose PROJECT is archived
// ===========================================================================

/// default_environment_id must ignore a live env whose owning project is
/// archived: otherwise the "Add" flow would target it and every new entry would
/// be a ghost (B1). With no other rooted env, it fails closed instead.
#[tokio::test]
async fn default_environment_id_skips_an_env_under_an_archived_project() {
    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    projects::backfill_default_project(&pool).await.unwrap();

    // The default env is attached to "Personnel".
    let _default_env = vault::default_environment_id(&pool).await.unwrap();
    let personnel = projects::list_projects(&pool).await.unwrap()[0].id.clone();

    // Archive the PROJECT (the env row stays live) — WITHOUT rerunning the backfill.
    projects::archive_project(&pool, &personnel).await.unwrap();

    // No env is rooted in a live project anymore => no default (fail-closed).
    assert!(
        matches!(
            vault::default_environment_id(&pool).await,
            Err(AppError::EnvironmentNotFound)
        ),
        "an env under an archived project must not be returned as default (B1)"
    );

    // And a fresh vault key never leaks a ghost write: there is no default env to
    // create into, so the UI disables "Add" (defaultEnvId is undefined).
    let _ = vk;
}

// ===========================================================================
// D2 — restore_entry into an archived environment/project is refused
// ===========================================================================

/// Restoring an entry into an archived environment is refused (D2): it would
/// otherwise come back live yet invisible (a ghost).
#[tokio::test]
async fn restore_entry_into_an_archived_environment_is_refused() {
    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    projects::backfill_default_project(&pool).await.unwrap();
    let project = projects::list_projects(&pool).await.unwrap()[0].id.clone();

    // A second env holding an archived entry (so we may archive it — D1).
    let prod = environments::create_environment(&pool, &vk, &project, "Prod")
        .await
        .unwrap();
    let key = vault::load_env_key(&pool, &vk, &prod.id).await.unwrap();
    let id = entries::create_entry(&pool, &key, &prod.id, &login("X", Some("x.com")))
        .await
        .unwrap();
    entries::archive_entry(&pool, &prod.id, &id).await.unwrap();

    // Archive the whole environment (allowed: the default env keeps the project live).
    environments::archive_environment(&pool, &prod.id).await.unwrap();

    // D2: restoring the entry into the archived env is refused.
    assert!(matches!(
        entries::restore_entry(&pool, &prod.id, &id).await,
        Err(AppError::Conflict(_))
    ));
    assert!(entries::list_all_entries(&pool, None).await.unwrap().is_empty());
}

// ===========================================================================
// D3 — set_icon on an archived entry is refused
// ===========================================================================

/// set_icon refuses an archived entry (D3): load_icons filters archived entries,
/// so the write would be dead. Nothing is written.
#[tokio::test]
async fn set_icon_on_archived_entry_is_refused() {
    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let env_id = vault::default_environment_id(&pool).await.unwrap();
    let key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();

    let id = entries::create_entry(&pool, &key, &env_id, &login("X", Some("x.com")))
        .await
        .unwrap();
    entries::archive_entry(&pool, &env_id, &id).await.unwrap();

    assert!(matches!(
        entries::set_icon(&pool, &key, &env_id, &id, Some("data:image/png;base64,AAAA")).await,
        Err(AppError::NotFound)
    ));

    let n: i64 = sqlx::query(
        "SELECT COUNT(*) AS n FROM entry_fields WHERE entry_id = ? AND field_name = 'icon'",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap()
    .get("n");
    assert_eq!(n, 0, "no icon field is written for an archived entry");
}

// ===========================================================================
// B2 — IP-literal hosts match only themselves through the autofill matcher
// ===========================================================================

/// An IP-literal host must match ONLY itself: `10.0.3.4` and `192.168.3.4` share
/// their last two octets but are different hosts and must not cross-match (B2/F6).
#[tokio::test]
async fn ip_literal_hosts_match_only_themselves() {
    use freepass_lib::services::local_channel;
    use std::sync::{Arc, Mutex};

    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let env_id = vault::default_environment_id(&pool).await.unwrap();
    let key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();

    entries::create_entry(&pool, &key, &env_id, &login("A", Some("http://10.0.3.4")))
        .await
        .unwrap();

    let session = Arc::new(Mutex::new(freepass_lib::state::VaultSession::default()));
    session.lock().unwrap().unlock(vk);

    // Different IP host sharing the last two octets => NO match.
    let cross = local_channel::credentials_for_origin(&pool, &session, "http://192.168.3.4")
        .await
        .unwrap();
    assert!(cross.is_empty(), "different IP hosts must not cross-match (B2)");

    // The exact same IP host => match.
    let same = local_channel::credentials_for_origin(&pool, &session, "http://10.0.3.4/login")
        .await
        .unwrap();
    assert_eq!(same.len(), 1, "the exact IP host matches itself");
}

/// CONTROL: two 2-label internal hosts (`wiki.corp` vs `jenkins.corp`) do NOT
/// cross-match — registrable_domain keeps both labels, so they differ.
#[tokio::test]
async fn two_label_internal_hosts_do_not_cross_match() {
    use freepass_lib::services::local_channel;
    use std::sync::{Arc, Mutex};

    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let env_id = vault::default_environment_id(&pool).await.unwrap();
    let key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();

    entries::create_entry(&pool, &key, &env_id, &login("Wiki", Some("http://wiki.corp")))
        .await
        .unwrap();

    let session = Arc::new(Mutex::new(freepass_lib::state::VaultSession::default()));
    session.lock().unwrap().unlock(vk);

    let hits = local_channel::credentials_for_origin(&pool, &session, "http://jenkins.corp")
        .await
        .unwrap();
    assert!(hits.is_empty(), "distinct 2-label hosts must not cross-match");
}

/// EXPECTED (eTLD+1 semantics): sibling subdomains of the SAME registrable
/// domain match — `app.dev.local` and `admin.dev.local` both collapse to
/// `dev.local`, exactly like `mail.google.com` / `accounts.google.com`.
#[tokio::test]
async fn sibling_subdomains_of_the_same_registrable_domain_match() {
    use freepass_lib::services::local_channel;
    use std::sync::{Arc, Mutex};

    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let env_id = vault::default_environment_id(&pool).await.unwrap();
    let key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();

    entries::create_entry(&pool, &key, &env_id, &login("App", Some("http://app.dev.local")))
        .await
        .unwrap();

    let session = Arc::new(Mutex::new(freepass_lib::state::VaultSession::default()));
    session.lock().unwrap().unlock(vk);

    let hits = local_channel::credentials_for_origin(&pool, &session, "http://admin.dev.local")
        .await
        .unwrap();
    assert_eq!(
        hits.len(),
        1,
        "sibling subdomains of the same registrable domain match (correct eTLD+1 scoping)"
    );
}

// ===========================================================================
// D4 + updated_at coherence across the lifecycle
// ===========================================================================

/// update_entry must move updated_at strictly forward of created_at (a future
/// recency sort relies on it).
#[tokio::test]
async fn update_entry_moves_updated_at_forward_of_created_at() {
    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let env_id = vault::default_environment_id(&pool).await.unwrap();
    let key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();
    let id = entries::create_entry(&pool, &key, &env_id, &login("X", Some("x.com")))
        .await
        .unwrap();

    let row = sqlx::query("SELECT created_at, updated_at FROM entries WHERE id = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let created: String = row.get("created_at");
    let updated0: String = row.get("updated_at");
    assert_eq!(created, updated0, "at creation, created_at == updated_at");

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    entries::update_entry(&pool, &key, &env_id, &id, &login("X2", Some("x.com")))
        .await
        .unwrap();
    let updated1: String = sqlx::query("SELECT updated_at FROM entries WHERE id = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("updated_at");
    assert!(updated1 > created, "update must push updated_at past created_at (got {updated1} vs {created})");
}

/// import_entries now stamps every row with a DISTINCT timestamp (D4), so a
/// future recency sort can tell import order apart.
#[tokio::test]
async fn import_stamps_rows_with_distinct_timestamps() {
    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let env_id = vault::default_environment_id(&pool).await.unwrap();
    let key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();

    entries::import_entries(
        &pool,
        &key,
        &env_id,
        &[login("A", Some("a.com")), login("B", Some("b.com")), login("C", Some("c.com"))],
    )
    .await
    .unwrap();

    let stamps: Vec<String> = sqlx::query("SELECT updated_at FROM entries")
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(|r| r.get::<String, _>("updated_at"))
        .collect();
    assert_eq!(stamps.len(), 3);
    let unique: std::collections::HashSet<&String> = stamps.iter().collect();
    assert_eq!(unique.len(), 3, "each imported row gets a distinct timestamp (D4)");
}

/// archive + restore keep updated_at monotonic through the lifecycle.
#[tokio::test]
async fn updated_at_is_bumped_by_archive_and_restore() {
    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let env_id = vault::default_environment_id(&pool).await.unwrap();
    let key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();
    let id = entries::create_entry(&pool, &key, &env_id, &login("X", Some("x.com")))
        .await
        .unwrap();

    let read_updated = |eid: String| {
        let pool = pool.clone();
        async move {
            sqlx::query("SELECT updated_at FROM entries WHERE id = ?")
                .bind(&eid)
                .fetch_one(&pool)
                .await
                .unwrap()
                .get::<String, _>("updated_at")
        }
    };
    let created = read_updated(id.clone()).await;

    entries::archive_entry(&pool, &env_id, &id).await.unwrap();
    let after_archive = read_updated(id.clone()).await;
    entries::restore_entry(&pool, &env_id, &id).await.unwrap();
    let after_restore = read_updated(id.clone()).await;

    assert!(after_archive >= created, "archive should not move updated_at backwards");
    assert!(after_restore >= after_archive, "restore should not move updated_at backwards");
}

// ===========================================================================
// HTTP method / body edge cases on the live loopback channel
// ===========================================================================

/// A GET on the POST-only /credentials route returns 405 (never a credential).
#[tokio::test]
async fn get_on_post_only_credentials_route_is_405() {
    use freepass_lib::services::local_channel;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let session = Arc::new(Mutex::new(freepass_lib::state::VaultSession::default()));
    session.lock().unwrap().unlock(vk);

    let handle = local_channel::start(pool.clone(), session.clone()).await.unwrap();
    let port = handle.port;
    let auth = format!("Bearer {}", handle.token);

    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let req = format!(
        "GET /credentials HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\
         Origin: chrome-extension://abc\r\nAuthorization: {auth}\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).await.unwrap();
    let status: u16 = resp
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap();
    assert_eq!(status, 405, "GET on a POST-only route must be Method Not Allowed");
    assert!(!resp.contains("password"), "405 must not leak any credential body");
    handle.stop();
}

/// A well-formed request whose JSON body is missing `origin` is a clean 400.
#[tokio::test]
async fn malformed_credentials_body_is_400_not_500() {
    use freepass_lib::services::local_channel;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let session = Arc::new(Mutex::new(freepass_lib::state::VaultSession::default()));
    session.lock().unwrap().unlock(vk);

    let handle = local_channel::start(pool.clone(), session.clone()).await.unwrap();
    let port = handle.port;
    let auth = format!("Bearer {}", handle.token);
    let body = r#"{"not_origin":"x"}"#;

    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let req = format!(
        "POST /credentials HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\
         Origin: chrome-extension://abc\r\nAuthorization: {auth}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).await.unwrap();
    let status: u16 = resp
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap();
    assert_eq!(status, 400, "a body missing `origin` must be a clean 400");
    handle.stop();
}

/// import_entries creates rows even when password/username are absent and does
/// NOT dedupe exact duplicates — purely additive.
#[tokio::test]
async fn import_creates_passwordless_rows_and_keeps_exact_duplicates() {
    let pool = pool().await;
    let vk = vault::create_vault(&pool, b"pw").await.unwrap();
    let env_id = vault::default_environment_id(&pool).await.unwrap();
    let key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();

    let passwordless = EntryInput {
        title: "SiteOnly".into(),
        url: Some("site.com".into()),
        username: None,
        password: None,
        notes: None,
    };
    let dup = login("Dup", Some("dup.com"));
    let dup2 = login("Dup", Some("dup.com"));

    let n = entries::import_entries(&pool, &key, &env_id, &[passwordless, dup, dup2])
        .await
        .unwrap();
    assert_eq!(n, 3, "passwordless + exact-duplicate rows are all imported (additive)");

    let list = entries::list_entries(&pool, &env_id, None).await.unwrap();
    let site = list.iter().find(|e| e.title == "SiteOnly").unwrap();
    let got = entries::get_entry(&pool, &key, &env_id, &site.id).await.unwrap();
    assert_eq!(got.password, None, "passwordless import stores no password field");
    assert_eq!(list.iter().filter(|e| e.title == "Dup").count(), 2);
}
