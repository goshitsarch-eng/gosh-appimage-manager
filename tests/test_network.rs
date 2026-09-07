mod common;

use goshaim_core::network::NetworkClient;
use goshaim_core::url_guard;

#[test]
fn rejects_credentials_control_and_schemes() {
    assert!(url_guard::validate("https://user:pass@example.com/x", false, false).is_err());
    assert!(url_guard::validate("https://example.com/\x01x", false, false).is_err());
    assert!(url_guard::validate("file:///etc/passwd", false, false).is_err());
    assert!(url_guard::validate("data:text/plain,hi", false, false).is_err());
    assert!(url_guard::validate("javascript:alert(1)", false, false).is_err());
    assert!(url_guard::validate("http://example.com/x", false, false).is_err());
    assert!(url_guard::validate("", false, false).is_err());
}

#[test]
fn allows_https_and_explicit_http_or_ftp() {
    assert!(url_guard::validate("https://example.com/x.AppImage", false, false).is_ok());
    assert!(url_guard::validate("http://example.com/x", true, false).is_ok());
    assert!(url_guard::validate("ftp://example.com/x", true, false).is_ok());
}

#[test]
fn local_networks_need_opt_in() {
    assert!(url_guard::validate("https://127.0.0.1/x", false, false).is_err());
    assert!(url_guard::validate("https://192.168.1.10/x", false, false).is_err());
    assert!(url_guard::validate("https://localhost/x", false, false).is_err());
    assert!(url_guard::validate("https://127.0.0.1/x", false, true).is_ok());
    assert!(url_guard::validate("https://[::1]/x", false, true).is_ok());
}

#[test]
fn downgrade_guard() {
    let prev = url::Url::parse("https://example.com/a").unwrap();
    let next = url::Url::parse("http://example.com/a").unwrap();
    assert!(url_guard::check_redirect(&prev, &next).is_err());
    let ok = url::Url::parse("https://example.com/b").unwrap();
    assert!(url_guard::check_redirect(&prev, &ok).is_ok());
}

#[test]
fn repo_component_rules() {
    assert!(url_guard::is_safe_repo_component("gosh"));
    assert!(url_guard::is_safe_repo_component("my.repo-name_2"));
    assert!(!url_guard::is_safe_repo_component(""));
    assert!(!url_guard::is_safe_repo_component("a/b"));
    assert!(!url_guard::is_safe_repo_component("a..b"));
    assert!(!url_guard::is_safe_repo_component("a b"));
    assert!(!url_guard::is_safe_repo_component(&"a".repeat(129)));
}

#[test]
fn oversized_body_fails_closed() {
    let big = vec![b'x'; goshaim_core::limits::MAX_JSON_BODY_BYTES + 1];
    let net = goshaim_core::network::FakeNetwork::new().canned_body("example.com", &big);
    let result = net.get(
        "https://example.com/releases",
        &[],
        goshaim_core::network::Local::Denied,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceeds size bound"));
}

#[test]
fn ftp_source_rejects_credentials_without_network() {
    use goshaim_core::updates_sources::{FtpSource, UpdateSource};
    let source = FtpSource;
    let mut config = std::collections::BTreeMap::new();
    config.insert(
        "url".to_string(),
        "ftp://user:pass@example.com/f.AppImage".to_string(),
    );
    assert!(source.validate_config(&config).is_err());
    config.insert(
        "url".to_string(),
        "ftp://example.com/f.AppImage".to_string(),
    );
    assert!(source.validate_config(&config).is_ok());
}

#[test]
fn github_validation_matrix() {
    use goshaim_core::updates_sources::{GithubSource, UpdateSource};
    let source = GithubSource;
    let good = |u: &str, r: &str| {
        let mut config = std::collections::BTreeMap::new();
        config.insert("username".to_string(), u.to_string());
        config.insert("repo".to_string(), r.to_string());
        config.insert("filename".to_string(), "f.AppImage".to_string());
        source.validate_config(&config).is_ok()
    };
    assert!(good("gosh", "demo"));
    assert!(!good("bad/user", "demo"));
    assert!(!good("gosh", ".."));
    assert!(!good("gosh", &"r".repeat(200)));
}

/// Audit finding S-9. The literal-host guard missed every transitional IPv4
/// encoding and the shared/CGNAT range, so an attacker only had to spell the
/// address differently. Measured before the fix: all of these were allowed.
#[test]
fn transitional_and_shared_addresses_are_local() {
    for hostile in [
        "https://[::ffff:127.0.0.1]/x",       // IPv4-mapped loopback
        "https://[::ffff:169.254.169.254]/x", // IPv4-mapped cloud metadata
        "https://[64:ff9b::7f00:1]/x",        // NAT64-embedded loopback
        "https://[64:ff9b::a9fe:a9fe]/x",     // NAT64-embedded metadata
        "https://100.64.0.1/x",               // carrier-grade NAT
        "https://192.0.0.1/x",                // IETF protocol assignments
        "https://198.19.0.1/x",               // benchmarking range
        "https://255.255.255.255/x",          // broadcast
        "https://localhost./x",               // trailing-dot loopback name
        "https://db.internal./x",             // trailing-dot internal name
    ] {
        assert!(
            goshaim_core::url_guard::validate(hostile, false, false).is_err(),
            "{hostile} should need an explicit local-network opt-in"
        );
    }
    // Ordinary public destinations are unaffected.
    for benign in [
        "https://api.github.com/repos/x/y/releases/latest",
        "https://8.8.8.8/x",
        "https://[2606:4700:4700::1111]/x",
        "https://101.64.0.1/x", // just outside 100.64.0.0/10
    ] {
        assert!(
            goshaim_core::url_guard::validate(benign, false, false).is_ok(),
            "{benign} should still be allowed"
        );
    }
}

/// Audit finding S-2. A redirect hop is guarded before it is followed, so the
/// internal request is never issued at all.
#[test]
fn redirect_into_a_local_destination_is_refused() {
    let public = url::Url::parse("https://updates.example.com/app").unwrap();
    for hop in [
        "https://127.0.0.1/admin",
        "https://10.0.0.5/admin",
        "https://[::ffff:169.254.169.254]/latest/meta-data/",
        "https://localhost./admin",
    ] {
        let next = url::Url::parse(hop).unwrap();
        assert!(
            goshaim_core::url_guard::check_redirect(&public, &next).is_err(),
            "redirect to {hop} must be refused"
        );
    }
    // A normal cross-host redirect still works.
    let ok = url::Url::parse("https://cdn.example.net/app.AppImage").unwrap();
    assert!(goshaim_core::url_guard::check_redirect(&public, &ok).is_ok());
}

/// Audit finding S-3. The downgrade guard applies to redirect hops on every
/// method, not only to `get`'s final URL.
#[test]
fn redirect_downgrade_and_scheme_change_are_refused() {
    let https = url::Url::parse("https://updates.example.com/app").unwrap();
    for hop in [
        "http://updates.example.com/app",
        "ftp://updates.example.com/app",
    ] {
        let next = url::Url::parse(hop).unwrap();
        assert!(
            goshaim_core::url_guard::check_redirect(&https, &next).is_err(),
            "redirect to {hop} must be refused"
        );
    }
}
