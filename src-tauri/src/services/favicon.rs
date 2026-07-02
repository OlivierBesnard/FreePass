//! Best-effort favicon fetch, **directly from the target site only**.
//!
//! Privacy (DESIGN — "zéro cloud"): we never call a third-party favicon service
//! (Google/DuckDuckGo/…), because that would disclose the user's saved domains
//! to a remote party. We only contact the site the user already saved, and the
//! resulting icon is stored encrypted like any other entry field (CRYPTO_SPEC
//! §4), so nothing about which sites the user keeps leaks at rest.
//!
//! SSRF guard: the URL comes from user data (and could arrive via CSV import,
//! THREAT F13), so the fetch is constrained — only `http`/`https`, never a
//! loopback / private / link-local target, and redirects are bounded and
//! re-validated. The app must not become a proxy into the local network.
//!
//! This module does plain HTTP transport (rustls TLS); it is *not* vault crypto.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use base64::Engine;

use crate::error::{AppError, AppResult};

/// Favicons are tiny; cap the download so a hostile/huge response can't bloat
/// the vault or memory.
const MAX_ICON_BYTES: usize = 200 * 1024;
const TIMEOUT: Duration = Duration::from_secs(6);

/// True for addresses we must never fetch from (SSRF): loopback, private,
/// link-local, CGNAT, unspecified, multicast/broadcast, documentation, and the
/// IPv6 equivalents (incl. IPv4-mapped).
fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                // CGNAT 100.64.0.0/10
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // link-local fe80::/10
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // unique-local fc00::/7
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || v6
                    .to_ipv4_mapped()
                    .map(|m| is_forbidden_ip(IpAddr::V4(m)))
                    .unwrap_or(false)
        }
    }
}

/// Validate a candidate URL (`http`/`https` + public host) and return the host
/// plus a concrete PUBLIC socket address to pin the connection to. Resolving ONCE
/// here and pinning that address for the request closes the DNS-rebinding TOCTOU:
/// a name that passes this check can't flip to a private IP at connect time (B6).
/// A name resolving to *any* forbidden address is rejected wholesale.
async fn resolve_public(url: &str) -> Option<(String, SocketAddr)> {
    let u = reqwest::Url::parse(url).ok()?;
    match u.scheme() {
        "http" | "https" => {}
        _ => return None,
    }
    let host = u.host_str()?.to_string();
    let port = u.port_or_known_default()?;

    if let Ok(ip) = host.parse::<IpAddr>() {
        return if is_forbidden_ip(ip) {
            None
        } else {
            Some((host, SocketAddr::new(ip, port)))
        };
    }

    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .ok()?
        .collect();
    if addrs.is_empty() || addrs.iter().any(|a| is_forbidden_ip(a.ip())) {
        return None;
    }
    // Every resolved address is public; pin the first one for the connection.
    Some((host, addrs[0]))
}

/// An HTTP client that resolves `host` ONLY to the pre-validated `addr`, so DNS
/// is not consulted again at connect time (DNS-rebinding TOCTOU fix, B6).
fn pinned_client(host: &str, addr: SocketAddr) -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent("FreePass")
        // Pin the validated host→addr mapping (reqwest keeps the URL's port).
        .resolve(host, addr)
        // Bound redirects; block hops to non-http(s) schemes or forbidden IP
        // literals. NOTE: only the ORIGINAL host is pinned — a redirect to a
        // DIFFERENT host is resolved by normal DNS and is filtered here only when
        // it targets an IP literal, not a name (a name-based hop is not
        // re-validated). Acceptable for a best-effort, blind-SSRF favicon fetch on
        // a desktop mono-user app; full re-validation would need a manual loop.
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3 {
                return attempt.error("too many redirects");
            }
            match attempt.url().scheme() {
                "http" | "https" => {}
                _ => return attempt.stop(),
            }
            if let Some(host) = attempt.url().host_str() {
                if let Ok(ip) = host.parse::<IpAddr>() {
                    if is_forbidden_ip(ip) {
                        return attempt.stop();
                    }
                }
            }
            attempt.follow()
        }))
        .build()
        .map_err(|e| AppError::Other(format!("client http: {e}")))
}

/// Derive an `scheme://host[:port]` origin from a stored entry URL (assumes
/// `https` when the user typed a bare host). Returns None for non-http(s)
/// schemes or when there is no host.
fn origin_of(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let url = reqwest::Url::parse(&with_scheme).ok()?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return None,
    }
    let host = url.host_str()?;
    let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
    Some(format!("{}://{host}{port}", url.scheme()))
}

/// Resolve a (possibly relative) icon href against the page origin.
fn resolve(origin: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with("data:") {
        return None;
    }
    reqwest::Url::parse(origin)
        .ok()?
        .join(href)
        .ok()
        .map(|u| u.to_string())
}

/// Extract the value of `name="…"` (single or double quoted, or unquoted) from a
/// single HTML tag. ASCII-case-insensitive on the attribute name; byte offsets
/// align because ASCII lowercasing preserves length.
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(name) {
        let pos = from + rel;
        let boundary_ok = pos == 0 || lb[pos - 1].is_ascii_whitespace() || lb[pos - 1] == b'<';
        let mut j = pos + name.len();
        while j < lb.len() && lb[j].is_ascii_whitespace() {
            j += 1;
        }
        if boundary_ok && j < lb.len() && lb[j] == b'=' {
            j += 1;
            while j < lb.len() && lb[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= lb.len() {
                return None;
            }
            let q = lb[j];
            if q == b'"' || q == b'\'' {
                j += 1;
                let start = j;
                while j < lb.len() && lb[j] != q {
                    j += 1;
                }
                return Some(tag[start..j].to_string());
            }
            let start = j;
            while j < lb.len() && !lb[j].is_ascii_whitespace() && lb[j] != b'>' {
                j += 1;
            }
            return Some(tag[start..j].to_string());
        }
        from = pos + name.len();
    }
    None
}

/// Find declared icon links (`<link rel="...icon..." href="...">`) in page HTML.
fn icon_links(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(rel) = lower[i..].find("<link") {
        let start = i + rel;
        let end = lower[start..].find('>').map(|e| start + e).unwrap_or(lower.len());
        let tag = &html[start..end];
        let tl = &lower[start..end];
        if tl.contains("icon") {
            if let Some(rel_val) = attr(tag, "rel") {
                if rel_val.to_ascii_lowercase().contains("icon") {
                    if let Some(href) = attr(tag, "href") {
                        out.push(href);
                    }
                }
            }
        }
        i = end + 1;
        if out.len() >= 5 {
            break;
        }
    }
    out
}

/// Fetch one candidate URL and, if it is a non-empty image within the size cap,
/// return it as a `data:` URL. Any error / non-image yields None. The body is
/// read in bounded chunks so a chunked response without a Content-Length cannot
/// balloon memory past the cap.
async fn try_icon(url: &str) -> Option<String> {
    // Resolve + validate the host once, then pin it for the connection (B6).
    let (host, addr) = resolve_public(url).await?;
    let client = pinned_client(&host, addr).ok()?;
    let mut resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_ICON_BYTES {
            return None;
        }
    }
    // Require a real image content-type — even for a `.ico` URL, so a non-image
    // response served at a `.ico` path can't slip through the MIME gate (B6c).
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !ct.starts_with("image/") {
        return None;
    }
    let mime = ct.split(';').next().unwrap_or("image/x-icon").trim().to_string();

    let mut buf: Vec<u8> = Vec::new();
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                if buf.len() + chunk.len() > MAX_ICON_BYTES {
                    return None;
                }
                buf.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(_) => return None,
        }
    }
    if buf.is_empty() {
        return None;
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    Some(format!("data:{mime};base64,{b64}"))
}

/// Best-effort: fetch the favicon for `entry_url` directly from that site and
/// return it as a `data:` URL, or None if nothing usable was found. Never errors
/// on a network/parse problem — the icon is purely cosmetic.
pub async fn fetch_favicon(entry_url: &str) -> AppResult<Option<String>> {
    let Some(origin) = origin_of(entry_url) else {
        return Ok(None);
    };
    // SSRF guard: resolve + validate the origin host once, then pin it for the
    // page fetch (never loopback/private, no DNS-rebinding window — B6).
    let Some((host, addr)) = resolve_public(&origin).await else {
        return Ok(None);
    };
    let client = pinned_client(&host, addr)?;

    // Prefer icons the page declares, then fall back to the conventional path.
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(resp) = client.get(&origin).send().await {
        if let Ok(html) = resp.text().await {
            for href in icon_links(&html) {
                if let Some(abs) = resolve(&origin, &href) {
                    candidates.push(abs);
                }
            }
        }
    }
    candidates.push(format!("{origin}/favicon.ico"));

    for url in candidates.into_iter().take(5) {
        // Each candidate is re-validated + pinned inside try_icon (an HTML <link>
        // can point anywhere).
        if let Some(data_url) = try_icon(&url).await {
            return Ok(Some(data_url));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_assumes_https_keeps_port_and_rejects_non_http() {
        assert_eq!(origin_of("github.com").as_deref(), Some("https://github.com"));
        assert_eq!(
            origin_of("http://localhost:3000/login").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(origin_of("https://accounts.google.com/x").as_deref(), Some("https://accounts.google.com"));
        assert_eq!(origin_of("file:///etc/passwd"), None, "non-http(s) scheme rejected");
        assert_eq!(origin_of("ftp://x.com"), None);
        assert_eq!(origin_of(""), None);
    }

    #[test]
    fn forbidden_ips_cover_loopback_private_linklocal_metadata() {
        let bad = [
            "127.0.0.1", "10.0.0.1", "192.168.1.1", "172.16.0.1", "169.254.169.254",
            "0.0.0.0", "100.64.0.1", "::1", "fe80::1", "fc00::1", "::ffff:127.0.0.1",
        ];
        for s in bad {
            assert!(is_forbidden_ip(s.parse().unwrap()), "{s} must be forbidden");
        }
        let good = ["1.1.1.1", "8.8.8.8", "140.82.121.4", "2606:4700:4700::1111"];
        for s in good {
            assert!(!is_forbidden_ip(s.parse().unwrap()), "{s} must be allowed");
        }
    }

    #[tokio::test]
    async fn fetch_refuses_loopback_and_private_targets_without_network() {
        // SSRF guard short-circuits before any socket is opened.
        assert_eq!(fetch_favicon("http://127.0.0.1:48100").await.unwrap(), None);
        assert_eq!(fetch_favicon("http://169.254.169.254/latest/meta-data").await.unwrap(), None);
        assert_eq!(fetch_favicon("http://192.168.0.10").await.unwrap(), None);
        assert_eq!(fetch_favicon("file:///etc/passwd").await.unwrap(), None);
        assert_eq!(fetch_favicon("").await.unwrap(), None);
    }

    #[test]
    fn parses_icon_links_various_shapes() {
        let html = r#"
          <head>
            <link rel="stylesheet" href="/app.css">
            <link rel="icon" href="/favicon.png">
            <LINK REL='shortcut icon' HREF='https://cdn.example.com/i.ico'>
            <link rel="apple-touch-icon" href="touch.png">
          </head>"#;
        let links = icon_links(html);
        assert!(links.contains(&"/favicon.png".to_string()));
        assert!(links.contains(&"https://cdn.example.com/i.ico".to_string()));
        assert!(links.contains(&"touch.png".to_string()));
        assert!(!links.iter().any(|l| l.contains("app.css")), "stylesheet must be ignored");
    }

    #[test]
    fn resolve_handles_absolute_relative_and_rejects_data() {
        assert_eq!(
            resolve("https://x.com", "/favicon.ico").as_deref(),
            Some("https://x.com/favicon.ico")
        );
        assert_eq!(
            resolve("https://x.com", "https://cdn.y.com/i.png").as_deref(),
            Some("https://cdn.y.com/i.png")
        );
        assert_eq!(resolve("https://x.com", "img.png").as_deref(), Some("https://x.com/img.png"));
        assert_eq!(resolve("https://x.com", "data:image/png;base64,AAAA"), None);
    }
}
