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
    let result = net.get("https://example.com/releases", &[]);
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
