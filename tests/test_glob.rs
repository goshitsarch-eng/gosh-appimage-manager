// Regression tests for the asset-name glob matcher (audit finding S-4).
//
// The matcher is reachable from fully untrusted input: the pattern is the
// `filename` update-source config, which `config_from_embedded` copies out of
// an AppImage's `.upd_info` section, and the text is an asset name from a
// remote release document. The previous recursive implementation backtracked
// exponentially (measured: ~8x per two added pattern characters, 3.15s at a
// 15-character pattern in release mode), which hung `--fetch-updates` — the
// autostart background check.

mod common;

use common::Harness;
use goshaim_core::types::InstalledApp;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn github_app(pattern: &str) -> InstalledApp {
    let mut app = InstalledApp::new_owned();
    app.uuid = "u1".into();
    app.name = "Victim".into();
    app.version = "v1".into();
    app.update_manager = "github".into();
    app.update_config.insert("username".into(), "x".into());
    app.update_config.insert("repo".into(), "y".into());
    app.update_config.insert("filename".into(), pattern.into());
    app
}

/// The shape that used to blow up must now finish promptly.
#[test]
fn pathological_pattern_does_not_blow_up() {
    let h = Harness::new();
    let c = h.controller();
    // 50 characters that can never satisfy the trailing 'b'.
    let asset = "a".repeat(50);
    let body = format!(
        r#"{{"tag_name":"v9","assets":[{{"name":"{asset}.zip","browser_download_url":"https://github.com/x/y/releases/download/v9/a.AppImage","size":1}}]}}"#
    );
    h.network.canned_body("api.github.com", body.as_bytes());

    // 24 star-groups; the old matcher needed longer than the age of the
    // session to answer this.
    let pattern = format!("{}b", "*a".repeat(24));
    let cancel = AtomicBool::new(false);
    let start = Instant::now();
    let result = c.update_service().check(&github_app(&pattern), &cancel);
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "glob matching took {elapsed:?}; the matcher is backtracking again"
    );
    // No asset matches, so the check reports that rather than an offer.
    assert!(!result.available, "no asset should have matched");
}

/// Speed is worthless if the answers changed. These pin the semantics.
#[test]
fn glob_semantics_are_unchanged() {
    let h = Harness::new();
    let c = h.controller();
    // (pattern, asset name, should the asset be selected?)
    let cases: &[(&str, &str, bool)] = &[
        ("*.AppImage", "App-1.2.3-x86_64.AppImage", true),
        ("*.AppImage", "App-1.2.3-x86_64.zip", false),
        ("App-*-x86_64.AppImage", "App-1.2.3-x86_64.AppImage", true),
        ("App-*-x86_64.AppImage", "App-1.2.3-aarch64.AppImage", false),
        ("App-?.?.?.AppImage", "App-1.2.3.AppImage", true),
        ("App-?.?.?.AppImage", "App-1.22.3.AppImage", false),
        ("*", "anything at all", true),
        ("**", "anything at all", true),
        ("*x*", "prefix-x-suffix", true),
        ("*x*", "no letter here", false),
        ("exact.AppImage", "exact.AppImage", true),
        ("exact.AppImage", "exact.AppImage.sig", false),
        // A trailing star must accept the empty remainder.
        ("App*", "App", true),
        // A star must be able to match nothing in the middle, too.
        ("A*B", "AB", true),
        // Backtracking case: the first star has to give ground.
        ("*ab", "aaab", true),
        ("*ab", "aaba", false),
    ];
    for (pattern, asset, expected) in cases {
        let body = format!(
            r#"{{"tag_name":"v9","assets":[{{"name":"{asset}","browser_download_url":"https://github.com/x/y/releases/download/v9/a.AppImage","size":1}}]}}"#
        );
        h.network.canned_body("api.github.com", body.as_bytes());
        let result = c
            .update_service()
            .check(&github_app(pattern), &AtomicBool::new(false));
        assert_eq!(
            result.available, *expected,
            "pattern {pattern:?} against asset {asset:?}"
        );
    }
}

/// Oversized patterns and asset names are refused outright rather than matched.
#[test]
fn oversized_glob_inputs_fail_closed() {
    let h = Harness::new();
    let c = h.controller();
    let body = r#"{"tag_name":"v9","assets":[{"name":"app.AppImage","browser_download_url":"https://github.com/x/y/releases/download/v9/a.AppImage","size":1}]}"#;
    h.network.canned_body("api.github.com", body.as_bytes());

    let huge = format!(
        "{}*",
        "a".repeat(goshaim_core::limits::MAX_GLOB_PATTERN_LENGTH)
    );
    let result = c
        .update_service()
        .check(&github_app(&huge), &AtomicBool::new(false));
    assert!(!result.available, "an oversized pattern must not match");
}
