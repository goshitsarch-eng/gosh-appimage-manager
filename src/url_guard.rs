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
    let is_local_network = match host_no_brackets.parse::<std::net::IpAddr>() {
        Ok(ip) => is_local_ip(&ip),
        Err(_) => is_local_hostname(host_no_brackets),
    };
    if is_local_network && !allow_private {
        return reject("local-network destinations need an explicit opt-in");
    }
    Ok(ValidatedUrl {
        url,
        is_local_network,
    })
}

/// Is `ip` a destination we refuse to reach without an explicit opt-in?
///
/// This is checked twice: once on the literal host in the URL, and again on
/// whatever the resolver actually returns (see `network::guard_resolved`).
/// The literal check alone is not a guard — any hostname can resolve anywhere.
pub fn is_local_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => is_local_v4(v4),
        std::net::IpAddr::V6(v6) => {
            // An IPv4 address wearing an IPv6 costume reaches exactly the same
            // host, so unwrap both transitional encodings before judging it.
            // `::ffff:127.0.0.1` and the NAT64 well-known prefix 64:ff9b::/96
            // both used to sail past a plain `is_loopback()` check.
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_local_v4(&v4);
            }
            if v6.segments()[..6] == [0x0064, 0xff9b, 0, 0, 0, 0] {
                let s = v6.segments();
                let v4 = std::net::Ipv4Addr::new(
                    (s[6] >> 8) as u8,
                    (s[6] & 0xff) as u8,
                    (s[7] >> 8) as u8,
                    (s[7] & 0xff) as u8,
                );
                return is_local_v4(&v4);
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00 // unique-local
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // link-local
        }
    }
}

fn is_local_v4(v4: &std::net::Ipv4Addr) -> bool {
    v4.is_loopback()
        || v4.is_link_local()
        || v4.is_private()
        || v4.is_unspecified()
        || v4.is_broadcast()
        || v4.is_documentation()
        // 100.64.0.0/10 carrier-grade NAT: not public, and routable to
        // infrastructure on many networks.
        || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 0x40)
        // 192.0.0.0/24 IETF protocol assignments.
        || (v4.octets()[0] == 192 && v4.octets()[1] == 0 && v4.octets()[2] == 0)
        // 198.18.0.0/15 benchmarking.
        || (v4.octets()[0] == 198 && (v4.octets()[1] & 0xfe) == 18)
}

fn is_local_hostname(host: &str) -> bool {
    // A fully-qualified name may carry a trailing dot; `localhost.` resolves
    // to loopback exactly as `localhost` does.
    let host = host.strip_suffix('.').unwrap_or(host);
    host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
}

/// Guard one hop of a redirect chain.
///
/// Applied to every hop before it is followed, not just to the final URL: by
/// the time the final URL is known the intermediate requests have already been
/// issued, which is the whole of an SSRF.
pub fn check_redirect(previous: &Url, next: &Url) -> Result<(), String> {
    if previous.scheme() == "https" && next.scheme() == "http" {
        return Err("URL rejected: refusing https-to-http downgrade".to_string());
    }
    if next.username() != "" || next.password().is_some() {
        return Err("URL rejected: credentials in URLs are not allowed".to_string());
    }
    match next.scheme() {
        "https" | "http" => {}
        _ => return Err("URL rejected: URL scheme is not allowed".to_string()),
    }
    let host = next.host_str().unwrap_or_default().to_ascii_lowercase();
    if host.is_empty() {
        return Err("URL rejected: URL has no host".to_string());
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let local = match bare.parse::<std::net::IpAddr>() {
        Ok(ip) => is_local_ip(&ip),
        Err(_) => is_local_hostname(bare),
    };
    if local {
        return Err("URL rejected: refusing redirect to a local-network destination".to_string());
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
