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
use sqlx::SqlitePool;
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

/// Start the channel on an ephemeral loopback port. Spawned on the Tauri tokio
/// runtime; shuts down gracefully when the handle is dropped/stopped.
pub async fn start(
    pool: SqlitePool,
    session: Arc<Mutex<VaultSession>>,
) -> AppResult<ChannelHandle> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let token = random_token();

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

fn is_allowed_origin(origin: Option<&str>) -> bool {
    match origin {
        Some(o) => EXTENSION_SCHEMES.iter().any(|s| o.starts_with(s)),
        None => false,
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

/// Collect decrypted credentials whose stored URL matches `page_origin`. Errors
/// (with `VaultLocked`) if the vault is not unlocked.
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

    let env_id = vault::default_environment_id(pool).await?;
    let env_key = vault::load_env_key(pool, &vault_key, &env_id).await?;
    let summaries = entries::list_entries(pool, &env_id, None).await?;

    let mut out = Vec::new();
    for summary in summaries {
        let Some(url) = &summary.url else { continue };
        if domains_match(page_origin, url) {
            let detail = entries::get_entry(pool, &env_key, &env_id, &summary.id).await?;
            out.push(Credential {
                title: detail.title,
                url: detail.url,
                username: detail.username,
                password: detail.password,
            });
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
        builder = builder.header("access-control-allow-origin", origin);
    }
    builder.body(Body::from(body)).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap()
    })
}

async fn preflight(headers: HeaderMap) -> Response {
    let origin = header(&headers, "origin");
    if !is_allowed_origin(origin) {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::empty())
            .unwrap();
    }
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("access-control-allow-origin", origin.unwrap())
        .header("access-control-allow-methods", "POST, GET, OPTIONS")
        .header("access-control-allow-headers", "authorization, content-type")
        .header("vary", "Origin")
        .body(Body::empty())
        .unwrap()
}

async fn health(State(state): State<ChannelState>, headers: HeaderMap) -> Response {
    let origin = header(&headers, "origin");
    if !is_allowed_origin(origin) {
        return json_response(StatusCode::FORBIDDEN, None, r#"{"error":"origin"}"#.into());
    }
    let unlocked = state
        .session
        .lock()
        .map(|s| s.is_unlocked())
        .unwrap_or(false);
    let body = format!(r#"{{"status":"ok","unlocked":{unlocked}}}"#);
    json_response(StatusCode::OK, origin, body)
}

async fn credentials(
    State(state): State<ChannelState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let origin = header(&headers, "origin");
    if !is_allowed_origin(origin) {
        return json_response(StatusCode::FORBIDDEN, None, r#"{"error":"origin"}"#.into());
    }
    let allow = origin.unwrap();

    if !bearer_ok(header(&headers, "authorization"), &state.token) {
        return json_response(
            StatusCode::UNAUTHORIZED,
            Some(allow),
            r#"{"error":"unauthorized"}"#.into(),
        );
    }

    let req: CredentialsRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                Some(allow),
                r#"{"error":"bad_request"}"#.into(),
            )
        }
    };

    match credentials_for_origin(&state.pool, &state.session, &req.origin).await {
        Ok(creds) => {
            let body = serde_json::to_string(&creds).unwrap_or_else(|_| "[]".into());
            json_response(StatusCode::OK, Some(allow), body)
        }
        // Locked or any error => serve nothing, never leak why.
        Err(_) => json_response(StatusCode::FORBIDDEN, Some(allow), r#"{"error":"locked"}"#.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_with_url;
    use crate::models::entry::EntryInput;

    #[test]
    fn only_extension_origins_are_allowed() {
        assert!(is_allowed_origin(Some("chrome-extension://abcdef")));
        assert!(is_allowed_origin(Some("moz-extension://abcdef")));
        assert!(!is_allowed_origin(Some("https://evil.com")));
        assert!(!is_allowed_origin(Some("http://localhost")));
        assert!(!is_allowed_origin(None));
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
        let body = resp.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (status, body)
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

        handle.stop();
    }
}
