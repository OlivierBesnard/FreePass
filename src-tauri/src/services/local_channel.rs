//! Local loopback control channel for the browser extension (DESIGN §7,
//! THREAT F6/F7/F14). An axum server bound to `127.0.0.1:0` (loopback only,
//! never an external interface) that serves credentials to the paired extension.
//!
//! Defense in depth:
//! - **Loopback only**: bound to 127.0.0.1, so nothing off-host can reach it.
//! - **Pairing token**: every request must carry `Authorization: Bearer <token>`
//!   (a capability, not a vault secret — F14). The token alone cannot unlock.
//! - **Origin gate + CORS**: only `*-extension://` origins are answered, and the
//!   `Access-Control-Allow-Origin` header is echoed only for those — a web page
//!   (phishing, A2) cannot read responses cross-origin.
//! - **Unlocked-only**: credentials are served only while the vault is unlocked.
//! - **Strict domain match**: an entry is returned only when its stored URL is
//!   the same registrable domain as the page origin (F6) — never cross-origin.

use std::sync::{Arc, Mutex};

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::error::{AppError, AppResult};
use crate::services::{entries, vault};
use crate::state::VaultSession;

/// Extension URL schemes we will answer (and only these). A normal web page
/// origin (`https://…`) is never allowed (F6/F7).
const EXTENSION_SCHEMES: [&str; 3] = [
    "chrome-extension://",
    "moz-extension://",
    "safari-web-extension://",
];

/// Fixed loopback ports the channel tries to bind, in order. Fixed (not
/// ephemeral) so the extension can auto-discover the app after a restart
/// without the user re-entering a port. Falls back to an ephemeral port only
/// if all are busy.
pub const CANDIDATE_PORTS: [u16; 3] = [48100, 48101, 48102];

/// Minimal set of multi-label public suffixes so `foo.co.uk` ≠ `bar.co.uk`.
/// A full Public Suffix List is a Phase 8 hardening item; this covers the
/// common cases and never *broadens* matching beyond the registrable domain.
const MULTI_SUFFIXES: [&str; 12] = [
    "co.uk", "org.uk", "gov.uk", "ac.uk", "co.jp", "com.au", "net.au", "com.br",
    "co.nz", "co.za", "com.mx", "co.in",
];

#[derive(Clone)]
struct ChannelState {
    pool: SqlitePool,
    session: Arc<Mutex<VaultSession>>,
    token: Arc<str>,
}

/// A credential served to the extension for autofill.
#[derive(Serialize)]
pub struct Credential {
    pub title: String,
    pub url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Site favicon as a `data:` URL, if one was fetched (cosmetic).
    pub icon: Option<String>,
}

#[derive(Deserialize)]
struct CredentialsRequest {
    origin: String,
}

/// Handle to a running channel. Dropping or calling `stop()` shuts the server
/// down gracefully; `port`/`token` are surfaced to the UI for pairing.
pub struct ChannelHandle {
    pub port: u16,
    pub token: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl ChannelHandle {
    pub fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for ChannelHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Load the persisted pairing token, generating + storing one on first use so
/// it stays stable across restarts (the extension pairs once). The token is a
/// capability, not a vault secret (THREAT F14).
pub async fn get_or_create_token(pool: &SqlitePool) -> AppResult<String> {
    let existing: Option<String> = sqlx::query("SELECT channel_token FROM vault WHERE id = 1")
        .fetch_one(pool)
        .await?
        .get("channel_token");
    if let Some(token) = existing {
        if !token.is_empty() {
            return Ok(token);
        }
    }
    let token = random_token();
    sqlx::query("UPDATE vault SET channel_token = ? WHERE id = 1")
        .bind(&token)
        .execute(pool)
        .await?;
    Ok(token)
}

/// Bind the first available fixed candidate port; fall back to an ephemeral one.
async fn bind_loopback() -> AppResult<(TcpListener, u16)> {
    for &port in CANDIDATE_PORTS.iter() {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return Ok((listener, port));
        }
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

/// Start the channel on a fixed loopback port with the persistent pairing token.
/// Spawned on the Tauri tokio runtime; shuts down gracefully when the handle is
/// dropped/stopped.
pub async fn start(
    pool: SqlitePool,
    session: Arc<Mutex<VaultSession>>,
) -> AppResult<ChannelHandle> {
    let token = get_or_create_token(&pool).await?;
    let (listener, port) = bind_loopback().await?;

    let state = ChannelState {
        pool,
        session,
        token: Arc::from(token.as_str()),
    };
    let app = Router::new()
        .route("/health", get(health).options(preflight))
        .route("/credentials", post(credentials).options(preflight))
        .with_state(state);

    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
    });

    Ok(ChannelHandle { port, token, shutdown: Some(tx) })
}

// === Pure, testable predicates ===

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn is_extension_origin(o: &str) -> bool {
    EXTENSION_SCHEMES.iter().any(|s| o.starts_with(s))
}

/// Decide whether to answer a request based on its `Origin`, and what to echo
/// back for CORS:
/// - **`Ok(Some(o))`** — an extension origin (Firefox `moz-extension://`, or
///   Chrome when it doesn't bypass CORS): allowed, echo `o` as ACAO.
/// - **`Ok(None)`** — **no Origin header**: this is the *normal* Chrome case.
///   With `host_permissions` for `127.0.0.1`, Chrome bypasses CORS and sends the
///   extension's fetch with **no Origin** (just like a direct navigation). We
///   must allow it; no ACAO is echoed (Chrome reads the body regardless). The
///   bearer token — not the Origin — is the real authentication gate (F7/F14).
/// - **`Err(())`** — a web-page Origin (`https://…`): rejected. A real web page
///   (phishing, F6) always sends its Origin, so this still slams the door on
///   cross-origin reads.
fn classify_origin(origin: Option<&str>) -> Result<Option<&str>, ()> {
    match origin {
        None => Ok(None),
        Some(o) if is_extension_origin(o) => Ok(Some(o)),
        Some(_) => Err(()),
    }
}

fn bearer_ok(auth: Option<&str>, token: &str) -> bool {
    match auth {
        Some(a) => a == format!("Bearer {token}"),
        None => false,
    }
}

/// Extract the host from a URL or bare host string (drops scheme, path, port).
fn host_of(input: &str) -> String {
    let s = input.trim();
    let s = s.split("://").last().unwrap_or(s); // drop scheme
    let s = s.split('/').next().unwrap_or(s); // drop path
    let s = s.split('?').next().unwrap_or(s);
    let s = s.split(':').next().unwrap_or(s); // drop port
    s.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// Registrable domain of a host (strips `www.`, honours a small multi-label
/// suffix list). Used to compare a page origin to a stored entry URL.
fn registrable_domain(host: &str) -> String {
    let host = host.strip_prefix("www.").unwrap_or(host);
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    if labels.len() < 2 {
        return host.to_string();
    }
    let last_two = format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1]);
    let take = if MULTI_SUFFIXES.contains(&last_two.as_str()) { 3 } else { 2 };
    if labels.len() <= take {
        return labels.join(".");
    }
    labels[labels.len() - take..].join(".")
}

/// True iff a stored entry URL belongs to the same registrable domain as the
/// page origin. Never matches across registrable domains (F6).
fn domains_match(page_origin: &str, entry_url: &str) -> bool {
    let page = registrable_domain(&host_of(page_origin));
    let entry = registrable_domain(&host_of(entry_url));
    !page.is_empty() && page == entry
}

/// Collect decrypted credentials whose stored URL matches `page_origin`, across
/// ALL non-archived environments (Phase 10: multi-env autofill, criterion #7).
/// Errors (with `VaultLocked`) if the vault is not unlocked (F7). The strict
/// registrable-domain match (F6) is unchanged — only the set of environments
/// scanned widened from the single default env to every live env.
pub async fn credentials_for_origin(
    pool: &SqlitePool,
    session: &Arc<Mutex<VaultSession>>,
    page_origin: &str,
) -> AppResult<Vec<Credential>> {
    let vault_key = {
        let s = session
            .lock()
            .map_err(|_| AppError::Other("session indisponible".into()))?;
        if !s.is_unlocked() {
            return Err(AppError::VaultLocked);
        }
        s.vault_key().ok_or(AppError::VaultLocked)?.clone()
    };

    // Every live environment. Archived == not served — and archiving the owning
    // PROJECT also takes its environments out of autofill, so a project "put
    // away" in the UI is consistently inactive for the extension too (F7). A
    // LEFT JOIN keeps envs whose project_id is still NULL (pre-backfill window).
    let env_ids: Vec<String> = sqlx::query(
        "SELECT e.id FROM environments e \
         LEFT JOIN projects p ON p.id = e.project_id \
         WHERE e.archived_at IS NULL AND (e.project_id IS NULL OR p.archived_at IS NULL)",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| r.get::<String, _>("id"))
    .collect();

    let mut out = Vec::new();
    for env_id in env_ids {
        // Each environment is decrypted under its OWN envKey (CRYPTO_SPEC §3); a
        // key from one env never decrypts another (AAD-bound, F8).
        let env_key = vault::load_env_key(pool, &vault_key, &env_id).await?;
        let summaries = entries::list_entries(pool, &env_id, None).await?;
        for summary in summaries {
            let Some(url) = &summary.url else { continue };
            if domains_match(page_origin, url) {
                let detail = entries::get_entry(pool, &env_key, &env_id, &summary.id).await?;
                out.push(Credential {
                    title: detail.title,
                    url: detail.url,
                    username: detail.username,
                    password: detail.password,
                    icon: detail.icon,
                });
            }
        }
    }
    Ok(out)
}

// === HTTP layer ===

fn json_response(status: StatusCode, allow_origin: Option<&str>, body: String) -> Response {
    let mut builder = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("vary", "Origin");
    if let Some(origin) = allow_origin {
        builder = builder
            .header("access-control-allow-origin", origin)
            // Chrome's Private Network Access: an extension talking to 127.0.0.1
            // is blocked unless this header is granted.
            .header("access-control-allow-private-network", "true");
    }
    builder.body(Body::from(body)).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap()
    })
}

async fn preflight(headers: HeaderMap) -> Response {
    // A CORS preflight always carries an Origin; only extension origins get one.
    // (Chrome's host_permissions fetch bypasses CORS and sends no preflight.)
    let origin = header(&headers, "origin");
    let Some(echo) = origin.filter(|o| is_extension_origin(o)) else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::empty())
            .unwrap();
    };
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("access-control-allow-origin", echo)
        .header("access-control-allow-methods", "POST, GET, OPTIONS")
        .header("access-control-allow-headers", "authorization, content-type")
        // Grant Private Network Access so Chrome lets the extension reach 127.0.0.1.
        .header("access-control-allow-private-network", "true")
        .header("vary", "Origin")
        .body(Body::empty())
        .unwrap()
}

async fn health(State(state): State<ChannelState>, headers: HeaderMap) -> Response {
    let echo = match classify_origin(header(&headers, "origin")) {
        Ok(e) => e,
        Err(()) => return json_response(StatusCode::FORBIDDEN, None, r#"{"error":"origin"}"#.into()),
    };
    // Require the pairing token here too: with the channel now always-on on a
    // guessable fixed port, an unauthenticated `/health` would let any local
    // process read the lock state as an oracle (THREAT F7). The paired extension
    // already holds the token.
    if !bearer_ok(header(&headers, "authorization"), &state.token) {
        return json_response(
            StatusCode::UNAUTHORIZED,
            echo,
            r#"{"error":"unauthorized"}"#.into(),
        );
    }
    let unlocked = state
        .session
        .lock()
        .map(|s| s.is_unlocked())
        .unwrap_or(false);
    let body = format!(r#"{{"status":"ok","unlocked":{unlocked}}}"#);
    json_response(StatusCode::OK, echo, body)
}

async fn credentials(
    State(state): State<ChannelState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let echo = match classify_origin(header(&headers, "origin")) {
        Ok(e) => e,
        Err(()) => return json_response(StatusCode::FORBIDDEN, None, r#"{"error":"origin"}"#.into()),
    };

    if !bearer_ok(header(&headers, "authorization"), &state.token) {
        return json_response(
            StatusCode::UNAUTHORIZED,
            echo,
            r#"{"error":"unauthorized"}"#.into(),
        );
    }

    let req: CredentialsRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                echo,
                r#"{"error":"bad_request"}"#.into(),
            )
        }
    };

    match credentials_for_origin(&state.pool, &state.session, &req.origin).await {
        Ok(creds) => {
            let body = serde_json::to_string(&creds).unwrap_or_else(|_| "[]".into());
            json_response(StatusCode::OK, echo, body)
        }
        // Locked or any error => serve nothing, never leak why.
        Err(_) => json_response(StatusCode::FORBIDDEN, echo, r#"{"error":"locked"}"#.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_with_url;
    use crate::models::entry::EntryInput;

    #[test]
    fn origin_classification_allows_extensions_and_no_origin_but_rejects_web() {
        // Extension origin (Firefox, or Chrome without the CORS bypass): echoed.
        assert_eq!(classify_origin(Some("chrome-extension://abcdef")), Ok(Some("chrome-extension://abcdef")));
        assert_eq!(classify_origin(Some("moz-extension://abcdef")), Ok(Some("moz-extension://abcdef")));
        // No Origin = the normal Chrome host_permissions fetch: allowed, no echo.
        assert_eq!(classify_origin(None), Ok(None));
        // A web page always sends its Origin → rejected (F6).
        assert_eq!(classify_origin(Some("https://evil.com")), Err(()));
        assert_eq!(classify_origin(Some("http://localhost")), Err(()));
    }

    #[test]
    fn bearer_must_match_exactly() {
        assert!(bearer_ok(Some("Bearer abc"), "abc"));
        assert!(!bearer_ok(Some("Bearer abc"), "xyz"));
        assert!(!bearer_ok(Some("abc"), "abc"));
        assert!(!bearer_ok(None, "abc"));
    }

    #[test]
    fn registrable_domain_handles_subdomains_and_multi_suffixes() {
        assert_eq!(registrable_domain(&host_of("https://accounts.google.com/login")), "google.com");
        assert_eq!(registrable_domain(&host_of("www.github.com")), "github.com");
        assert_eq!(registrable_domain(&host_of("foo.co.uk")), "foo.co.uk");
    }

    #[test]
    fn domains_match_is_strict() {
        assert!(domains_match("https://accounts.google.com", "google.com"));
        assert!(domains_match("github.com", "https://github.com/login"));
        assert!(!domains_match("https://paypa1.com", "paypal.com")); // typosquat
        assert!(!domains_match("https://evil.com", "github.com"));
        assert!(!domains_match("foo.co.uk", "bar.co.uk")); // same suffix, different domain
    }

    #[tokio::test]
    async fn credentials_served_only_when_unlocked_and_matching() {
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        let vk = vault::create_vault(&pool, b"pw").await.unwrap();
        let env_id = vault::default_environment_id(&pool).await.unwrap();
        let env_key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();
        entries::create_entry(
            &pool,
            &env_key,
            &env_id,
            &EntryInput {
                title: "GitHub".into(),
                url: Some("github.com".into()),
                username: Some("alice".into()),
                password: Some("hunter2".into()),
                notes: None,
            },
        )
        .await
        .unwrap();

        // Unlocked session containing the vault key.
        let session = Arc::new(Mutex::new(VaultSession::default()));
        session.lock().unwrap().unlock(vk);

        let hits = credentials_for_origin(&pool, &session, "https://github.com/login").await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].password.as_deref(), Some("hunter2"));

        // A different domain returns nothing (no cross-origin leak).
        let none = credentials_for_origin(&pool, &session, "https://evil.com").await.unwrap();
        assert!(none.is_empty());

        // Locked session is refused.
        session.lock().unwrap().lock();
        assert!(matches!(
            credentials_for_origin(&pool, &session, "https://github.com").await,
            Err(AppError::VaultLocked)
        ));
    }

    #[tokio::test]
    async fn credentials_are_served_from_a_second_environment() {
        // Phase 10 criterion #7: an entry created in a NON-default environment
        // must be autofilled. The scan widened to all live environments.
        use crate::services::{environments, projects};

        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        let vk = vault::create_vault(&pool, b"pw").await.unwrap();
        projects::backfill_default_project(&pool).await.unwrap();
        let project = &projects::list_projects(&pool).await.unwrap()[0].id.clone();

        // A fresh second environment with its own envKey.
        let env2 = environments::create_environment(&pool, &vk, project, "Prod")
            .await
            .unwrap();
        let env2_key = vault::load_env_key(&pool, &vk, &env2.id).await.unwrap();
        entries::create_entry(
            &pool,
            &env2_key,
            &env2.id,
            &EntryInput {
                title: "GitLab".into(),
                url: Some("gitlab.com".into()),
                username: Some("bob".into()),
                password: Some("s3cr3t".into()),
                notes: None,
            },
        )
        .await
        .unwrap();

        let session = Arc::new(Mutex::new(VaultSession::default()));
        session.lock().unwrap().unlock(vk);

        let hits = credentials_for_origin(&pool, &session, "https://gitlab.com/login")
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "second-env credential must be autofilled");
        assert_eq!(hits[0].password.as_deref(), Some("s3cr3t"));

        // Archiving that environment removes it from the scan (F7: archived = not served).
        environments::archive_environment(&pool, &env2.id).await.unwrap();
        let none = credentials_for_origin(&pool, &session, "https://gitlab.com/login")
            .await
            .unwrap();
        assert!(none.is_empty(), "archived environment must not be served");
    }

    /// Helper: a vault with two live environments, each holding a login for the
    /// SAME domain. Returns (pool, session, env_a_id, env_b_id).
    async fn vault_with_two_envs_same_domain() -> (
        SqlitePool,
        Arc<Mutex<VaultSession>>,
        String,
        String,
    ) {
        use crate::services::{environments, projects};
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        let vk = vault::create_vault(&pool, b"pw").await.unwrap();
        projects::backfill_default_project(&pool).await.unwrap();
        let project = projects::list_projects(&pool).await.unwrap()[0].id.clone();

        let env_a = vault::default_environment_id(&pool).await.unwrap();
        let key_a = vault::load_env_key(&pool, &vk, &env_a).await.unwrap();
        let env_b = environments::create_environment(&pool, &vk, &project, "Prod")
            .await
            .unwrap();
        let key_b = vault::load_env_key(&pool, &vk, &env_b.id).await.unwrap();

        let mk = |user: &str, pass: &str| EntryInput {
            title: "Example".into(),
            url: Some("example.com".into()),
            username: Some(user.into()),
            password: Some(pass.into()),
            notes: None,
        };
        entries::create_entry(&pool, &key_a, &env_a, &mk("alice", "pw-a")).await.unwrap();
        entries::create_entry(&pool, &key_b, &env_b.id, &mk("bob", "pw-b")).await.unwrap();

        let session = Arc::new(Mutex::new(VaultSession::default()));
        session.lock().unwrap().unlock(vk);
        (pool, session, env_a, env_b.id)
    }

    #[tokio::test]
    async fn autofill_aggregates_the_same_domain_across_two_environments() {
        // Criterion #7: one domain present in TWO environments => BOTH credentials
        // are served (the scan unions across all live environments).
        let (pool, session, _a, _b) = vault_with_two_envs_same_domain().await;
        let hits = credentials_for_origin(&pool, &session, "https://example.com/login")
            .await
            .unwrap();
        assert_eq!(hits.len(), 2, "both environments' credentials must be served");
        let mut passwords: Vec<_> = hits.iter().filter_map(|c| c.password.clone()).collect();
        passwords.sort();
        assert_eq!(passwords, vec!["pw-a", "pw-b"]);
    }

    #[tokio::test]
    async fn autofill_returns_nothing_for_a_domain_in_no_environment() {
        // A domain matching no entry in any environment => empty (no leak).
        let (pool, session, _a, _b) = vault_with_two_envs_same_domain().await;
        let none = credentials_for_origin(&pool, &session, "https://unrelated.org")
            .await
            .unwrap();
        assert!(none.is_empty(), "an unknown domain must yield nothing");
    }

    #[tokio::test]
    async fn autofill_rejects_a_typosquat_even_with_multiple_environments() {
        // F6 across multi-env: a look-alike domain must match NEITHER environment.
        let (pool, session, _a, _b) = vault_with_two_envs_same_domain().await;
        let none = credentials_for_origin(&pool, &session, "https://examp1e.com/login")
            .await
            .unwrap();
        assert!(none.is_empty(), "typosquat must not match any environment (F6)");
        // A different registrable domain that merely contains the string is rejected too.
        let none2 = credentials_for_origin(&pool, &session, "https://example.com.evil.net")
            .await
            .unwrap();
        assert!(none2.is_empty(), "substring/suffix attack must not match (F6)");
    }

    #[tokio::test]
    async fn autofill_serves_nothing_when_locked_even_with_multiple_environments() {
        // F7: with several environments holding matching credentials, a locked
        // vault must still serve nothing.
        let (pool, session, _a, _b) = vault_with_two_envs_same_domain().await;
        session.lock().unwrap().lock();
        assert!(matches!(
            credentials_for_origin(&pool, &session, "https://example.com/login").await,
            Err(AppError::VaultLocked)
        ));
    }

    #[tokio::test]
    async fn archiving_a_project_removes_its_environments_from_autofill() {
        // Archiving a PROJECT takes all its environments out of autofill, so a
        // project "put away" in the UI is consistently inactive for the extension
        // too (F7). The scan LEFT JOINs `projects` and excludes envs whose owning
        // project is archived — without mutating/cascading the environment rows.
        let (pool, session, _env_a, _env_b) = vault_with_two_envs_same_domain().await;
        use crate::services::projects;
        let project = projects::list_projects(&pool).await.unwrap()[0].id.clone();

        // Both environments belong to the default project; before archiving, both
        // credentials are served.
        let before = credentials_for_origin(&pool, &session, "https://example.com/login")
            .await
            .unwrap();
        assert_eq!(before.len(), 2);

        projects::archive_project(&pool, &project).await.unwrap();

        let after = credentials_for_origin(&pool, &session, "https://example.com/login")
            .await
            .unwrap();
        assert!(
            after.is_empty(),
            "archiving the owning project must remove its envs from autofill, got {}",
            after.len()
        );
    }

    #[tokio::test]
    async fn autofill_skips_archived_env_but_still_serves_the_live_one() {
        // Archiving ONE of two environments removes only its credential; the
        // surviving environment's credential is still served (F7 partial).
        let (pool, session, _env_a, env_b) = vault_with_two_envs_same_domain().await;
        use crate::services::environments;
        environments::archive_environment(&pool, &env_b).await.unwrap();
        let hits = credentials_for_origin(&pool, &session, "https://example.com/login")
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "only the live environment's credential remains");
        assert_eq!(hits[0].password.as_deref(), Some("pw-a"));
    }

    #[tokio::test]
    async fn pairing_token_is_persistent_across_calls() {
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        vault::create_vault(&pool, b"pw").await.unwrap();
        let t1 = get_or_create_token(&pool).await.unwrap();
        let t2 = get_or_create_token(&pool).await.unwrap();
        assert_eq!(t1, t2, "token must be stable across restarts");
        assert!(t1.len() >= 32);
    }

    // Minimal raw HTTP/1.1 client for the live-socket test.
    async fn http(
        port: u16,
        method: &str,
        path: &str,
        extra_headers: &[(&str, &str)],
        body: &str,
    ) -> (u16, String) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
        for (k, v) in extra_headers {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        req.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).await.unwrap();
        let status: u16 = resp
            .lines()
            .next()
            .and_then(|l| l.split(' ').nth(1))
            .and_then(|s| s.parse().ok())
            .unwrap();
        // Return the full response (headers + body) so tests can assert headers.
        (status, resp)
    }

    #[tokio::test]
    async fn live_server_enforces_token_and_origin() {
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        let vk = vault::create_vault(&pool, b"pw").await.unwrap();
        let env_id = vault::default_environment_id(&pool).await.unwrap();
        let env_key = vault::load_env_key(&pool, &vk, &env_id).await.unwrap();
        entries::create_entry(
            &pool,
            &env_key,
            &env_id,
            &EntryInput {
                title: "GitHub".into(),
                url: Some("github.com".into()),
                username: Some("alice".into()),
                password: Some("hunter2".into()),
                notes: None,
            },
        )
        .await
        .unwrap();

        let session = Arc::new(Mutex::new(VaultSession::default()));
        session.lock().unwrap().unlock(vk);

        let handle = start(pool.clone(), session.clone()).await.unwrap();
        let port = handle.port;
        let ext_origin = "chrome-extension://abc";
        let auth = format!("Bearer {}", handle.token);
        let req_body = r#"{"origin":"https://github.com/login"}"#;

        // No token => 401.
        let (s, _) = http(port, "POST", "/credentials", &[("Origin", ext_origin)], req_body).await;
        assert_eq!(s, 401, "missing token must be rejected");

        // Web origin (phishing) => 403, even with the token.
        let (s, _) = http(
            port,
            "POST",
            "/credentials",
            &[("Origin", "https://evil.com"), ("Authorization", &auth)],
            req_body,
        )
        .await;
        assert_eq!(s, 403, "non-extension origin must be rejected");

        // Token + extension origin + matching domain => 200 with the credential.
        let (s, body) = http(
            port,
            "POST",
            "/credentials",
            &[("Origin", ext_origin), ("Authorization", &auth), ("Content-Type", "application/json")],
            req_body,
        )
        .await;
        assert_eq!(s, 200);
        assert!(body.contains("hunter2"), "expected credential, got: {body}");

        // THE REAL CHROME CASE: no Origin header at all (Chrome omits it for an
        // extension host_permissions fetch). Token alone must be enough => 200.
        let (s, body) = http(
            port,
            "POST",
            "/credentials",
            &[("Authorization", &auth), ("Content-Type", "application/json")],
            req_body,
        )
        .await;
        assert_eq!(s, 200, "a no-Origin request (Chrome) with a valid token must be served");
        assert!(body.contains("hunter2"), "expected credential for no-Origin request, got: {body}");

        // No Origin and no token => still 401 (token is the real gate).
        let (s, _) = http(port, "GET", "/health", &[], "").await;
        assert_eq!(s, 401, "no-Origin + no-token must be rejected");

        // /health requires the token (an unauthenticated probe is refused so a
        // local process can't read the lock-state oracle — THREAT F7).
        let (s, _) = http(port, "GET", "/health", &[("Origin", ext_origin)], "").await;
        assert_eq!(s, 401, "unauthenticated /health must be rejected");

        // With the token, /health is reachable (used to discover the port).
        let (s, body) =
            http(port, "GET", "/health", &[("Origin", ext_origin), ("Authorization", &auth)], "").await;
        assert_eq!(s, 200);
        assert!(body.contains("unlocked"));

        // Preflight must grant Private Network Access (Chrome blocks an
        // extension -> 127.0.0.1 request otherwise).
        let (s, body) = http(
            port,
            "OPTIONS",
            "/credentials",
            &[
                ("Origin", ext_origin),
                ("Access-Control-Request-Private-Network", "true"),
            ],
            "",
        )
        .await;
        assert_eq!(s, 204);
        assert!(
            body.to_lowercase().contains("access-control-allow-private-network"),
            "preflight must grant private network access"
        );

        handle.stop();
    }

    #[tokio::test]
    async fn locked_vault_is_discoverable_but_serves_no_credentials() {
        // The channel stays up while the vault is locked: /health must answer
        // (so the extension can discover the app and show "locked"), but
        // /credentials must be refused — no secret is served (THREAT F7/F14).
        let pool = init_pool_with_url("sqlite::memory:").await.unwrap();
        vault::create_vault(&pool, b"pw").await.unwrap();

        // Locked session (never unlocked).
        let session = Arc::new(Mutex::new(VaultSession::default()));
        let handle = start(pool.clone(), session.clone()).await.unwrap();
        let port = handle.port;
        let ext_origin = "chrome-extension://abc";
        let auth = format!("Bearer {}", handle.token);

        // /health (with the token) is reachable and reports the locked state.
        let (s, body) =
            http(port, "GET", "/health", &[("Origin", ext_origin), ("Authorization", &auth)], "").await;
        assert_eq!(s, 200, "locked app must still be discoverable");
        assert!(body.contains("\"unlocked\":false"), "health must report locked, got: {body}");

        // /credentials is refused even with a valid token + extension origin.
        let (s, _) = http(
            port,
            "POST",
            "/credentials",
            &[("Origin", ext_origin), ("Authorization", &auth), ("Content-Type", "application/json")],
            r#"{"origin":"https://github.com/login"}"#,
        )
        .await;
        assert_eq!(s, 403, "a locked vault must never serve credentials");

        handle.stop();
    }
}
