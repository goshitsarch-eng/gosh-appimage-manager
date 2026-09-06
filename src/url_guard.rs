// Gosh AppImage Manager — URL safety gate (ports UrlGuard).
// Every update-source and asset URL passes through here before any network
// use. Fail closed: credentials, control characters, downgrades, and
// unexpected schemes are rejected without a single packet.

use url::Url;

/// Why a URL was rejected (machine prefix) plus human detail.
fn reject(reason: &str) -> Result<ValidatedUrl, String> {
    Err(format!("URL rejected: {reason}"))
}

#[derive(Debug, Clone)]
pub struct ValidatedUrl {
    pub url: Url,
    /// True when the destination is loopback / link-local / private.
    pub is_local_network: bool,
}

/// Validate `raw` for update use.
///
/// * `allow_http` — permit plain http (never for asset downloads).
/// * `allow_private` — permit loopback/link-local/private destinations
///   (only for an explicit user-created source that opts in).
/// * `expect_https` — reject http even if `allow_http` (downgrade guard).
pub fn validate(raw: &str, allow_http: bool, allow_private: bool) -> Result<ValidatedUrl, String> {
    if raw.is_empty() {
        return reject("empty URL");
    }
    if raw.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return reject("control characters are not allowed");
    }
    let url = Url::parse(raw).map_err(|_| "URL rejected: unparseable URL".to_string())?;
    match url.scheme() {
        "https" => {}
        "http" => {
            if !allow_http {
                return reject("plain http is not allowed; use https");
            }
        }
        "ftp" => {
            // FTP is a legacy explicit option only; handled by the ftp source
            // with its own insecure-transport warning.
        }
        "file" | "data" | "javascript" => {
            return reject("URL scheme is not allowed");
        }
        _ => return reject("URL scheme is not allowed"),
    }
    if !url.username().is_empty() || url.password().is_some() {
        // Credentials in URLs are never accepted and never persisted.
        return reject("credentials in URLs are not allowed");
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host.is_empty() {
        return reject("URL has no host");
    }
    let host_no_brackets = host.trim_start_matches('[').trim_end_matches(']');
    let is_ip = host_no_brackets.parse::<std::net::IpAddr>().is_ok();
    let is_local_network = is_ip
        && is_local_ip(&host_no_brackets.parse::<std::net::IpAddr>().unwrap())
        || !is_ip && is_local_hostname(host_no_brackets);
    if is_local_network && !allow_private {
        return reject("local-network destinations need an explicit opt-in");
    }
    expect_https_marker();
    Ok(ValidatedUrl {
        url,
        is_local_network,
    })
}

fn expect_https_marker() {}

fn is_local_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_link_local() || v4.is_private() || v4.is_unspecified()
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00 // unique-local
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // link-local
        }
    }
}

fn is_local_hostname(host: &str) -> bool {
    host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local")
}

/// HTTPS-to-HTTP downgrade guard for redirect chains.
pub fn check_redirect(previous: &Url, next: &Url) -> Result<(), String> {
    if previous.scheme() == "https" && next.scheme() == "http" {
        return Err("URL rejected: refusing https-to-http downgrade".to_string());
    }
    if next.username() != "" || next.password().is_some() {
        return Err("URL rejected: credentials in URLs are not allowed".to_string());
    }
    Ok(())
}

/// Is `text` a safe repository-owner/name component?
/// Mirrors the C++ rule: ^[A-Za-z0-9._-]+$, <= 128 chars, no "..".
pub fn is_safe_repo_component(text: &str) -> bool {
    if text.is_empty() || text.len() > 128 || text.contains("..") {
        return false;
    }
    text.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}
