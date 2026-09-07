mod common;

use common::{args, write_fixture, Harness};
use goshaim_core::cli::run_cli;
use goshaim_core::types::{ExitCode, InstalledApp, IntegrateRequest};

const GITHUB_LATEST: &str = r#"{
  "tag_name": "2.0",
  "assets": [
    {"name": "Demo.AppImage", "browser_download_url": "https://github.com/gosh/demo/releases/download/2.0/Demo.AppImage", "size": 128}
  ]
}"#;

fn seed_app_with_github(
    c: &mut goshaim_core::controller::AppController,
    version: &str,
    managed_path: &str,
) -> InstalledApp {
    let mut app = InstalledApp::new_owned();
    app.uuid = goshaim_core::registry::ManagedRegistry::new_uuid();
    app.name = "Demo".to_string();
    app.version = version.to_string();
    app.managed_path = managed_path.to_string();
    app.update_manager = "github".to_string();
    app.update_config
        .insert("username".to_string(), "gosh".to_string());
    app.update_config
        .insert("repo".to_string(), "demo".to_string());
    app.update_config
        .insert("filename".to_string(), "Demo.AppImage".to_string());
    c.registry_mut().upsert(app.clone()).unwrap();
    app
}

#[test]
fn github_check_offers_newer_release() {
    let h = Harness::new();
    let mut c = h.controller();
    h.network
        .canned_body("api.github.com", GITHUB_LATEST.as_bytes());
    // Asset download body = synthetic fixture (validated, never executed).
    let fixture = goshaim_core::inspector::make_test_elf(
        goshaim_core::types::Architecture::X86_64,
        goshaim_core::types::AppImageType::Type2,
    );
    h.network.canned_body("releases/download", &fixture);

    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    seed_app_with_github(&mut c, "1.0", path.to_str().unwrap());

    let cancel = Harness::cancel();
    let offers = c.check_updates(&cancel);
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].available_version, "2.0");
    assert_eq!(offers[0].manager, "github");
    assert!(offers[0].url.contains("releases/download"));
}

#[test]
fn same_version_produces_no_offer() {
    let h = Harness::new();
    let mut c = h.controller();
    h.network
        .canned_body("api.github.com", GITHUB_LATEST.as_bytes());
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    seed_app_with_github(&mut c, "2.0", path.to_str().unwrap());

    let cancel = Harness::cancel();
    assert!(c.check_updates(&cancel).is_empty());
}

#[test]
fn unknown_manager_fails_closed() {
    let h = Harness::new();
    let mut c = h.controller();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let mut app = seed_app_with_github(&mut c, "1.0", path.to_str().unwrap());
    app.update_manager = "megadrive".to_string();
    c.registry_mut().upsert(app).unwrap();

    let cancel = Harness::cancel();
    assert!(c.check_updates(&cancel).is_empty());

    let mut error = String::new();
    let ok = c.set_update_source(
        c.registry().apps()[0].clone(),
        "megadrive",
        Default::default(),
        &mut error,
    );
    assert!(!ok);
    assert!(error.contains("Unknown update manager"));
}

#[test]
fn credentials_in_url_fail_without_network() {
    let h = Harness::new();
    let mut c = h.controller();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let mut app = seed_app_with_github(&mut c, "1.0", path.to_str().unwrap());
    app.update_manager = "static".to_string();
    app.update_config.clear();
    app.update_config.insert(
        "url".to_string(),
        "https://user:pass@example.com/Demo.AppImage".to_string(),
    );
    c.registry_mut().upsert(app).unwrap();

    let cancel = Harness::cancel();
    assert!(c.check_updates(&cancel).is_empty());
    // No request left the process: FakeNetwork recorded nothing.
    assert!(h.network.calls.lock().unwrap().is_empty());
}

#[test]
fn apply_downloads_validates_and_replaces() {
    let h = Harness::new();
    let mut c = h.controller();
    h.network
        .canned_body("api.github.com", GITHUB_LATEST.as_bytes());
    let fixture = goshaim_core::inspector::make_test_elf(
        goshaim_core::types::Architecture::X86_64,
        goshaim_core::types::AppImageType::Type2,
    );
    h.network.canned_body("releases/download", &fixture);

    // Integrate a real managed file first.
    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let incoming = write_fixture(&src, "Demo.AppImage");
    let cancel = Harness::cancel();
    let integrated = c.integrate(
        &IntegrateRequest {
            source_path: incoming.to_str().unwrap().to_string(),
            assume_yes: true,
            ..Default::default()
        },
        &cancel,
    );
    assert!(integrated.ok, "error: {}", integrated.error);
    let mut app = integrated.app.clone();
    app.version = "1.0".to_string();
    app.update_manager = "github".to_string();
    app.update_config
        .insert("username".to_string(), "gosh".to_string());
    app.update_config
        .insert("repo".to_string(), "demo".to_string());
    app.update_config
        .insert("filename".to_string(), "Demo.AppImage".to_string());
    c.registry_mut().upsert(app.clone()).unwrap();

    let result = c.apply_update(&app, false, &cancel);
    assert!(result.ok, "error: {}", result.error);
    assert_eq!(result.app.version, "2.0");
    assert!(c.registry().by_uuid(&app.uuid).unwrap().version == "2.0");
}

#[test]
fn running_app_blocks_update_unless_forced() {
    let h = Harness::new();
    let mut c = h.controller();
    h.network
        .canned_body("api.github.com", GITHUB_LATEST.as_bytes());
    let fixture = goshaim_core::inspector::make_test_elf(
        goshaim_core::types::Architecture::X86_64,
        goshaim_core::types::AppImageType::Type2,
    );
    h.network.canned_body("releases/download", &fixture);

    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let incoming = write_fixture(&src, "Demo.AppImage");
    let cancel = Harness::cancel();
    let integrated = c.integrate(
        &IntegrateRequest {
            source_path: incoming.to_str().unwrap().to_string(),
            assume_yes: true,
            ..Default::default()
        },
        &cancel,
    );
    assert!(integrated.ok);
    let mut app = integrated.app.clone();
    app.version = "1.0".to_string();
    app.update_manager = "github".to_string();
    app.update_config
        .insert("username".to_string(), "gosh".to_string());
    app.update_config
        .insert("repo".to_string(), "demo".to_string());
    app.update_config
        .insert("filename".to_string(), "Demo.AppImage".to_string());
    c.registry_mut().upsert(app.clone()).unwrap();

    h.table.mark_running(&app.managed_path);
    let blocked = c.apply_update(&app, false, &cancel);
    assert!(!blocked.ok);
    assert!(blocked.error.contains("running"));

    let forced = c.apply_update(&app, true, &cancel);
    assert!(forced.ok, "error: {}", forced.error);
}

#[test]
fn digest_mismatch_fails_update() {
    let h = Harness::new();
    let mut c = h.controller();
    let tampered = GITHUB_LATEST.replace("\"size\": 128", "\"size\": 128, \"digest\": \"00\"");
    h.network.canned_body("api.github.com", tampered.as_bytes());
    let fixture = goshaim_core::inspector::make_test_elf(
        goshaim_core::types::Architecture::X86_64,
        goshaim_core::types::AppImageType::Type2,
    );
    h.network.canned_body("releases/download", &fixture);

    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let incoming = write_fixture(&src, "Demo.AppImage");
    let cancel = Harness::cancel();
    let integrated = c.integrate(
        &IntegrateRequest {
            source_path: incoming.to_str().unwrap().to_string(),
            assume_yes: true,
            ..Default::default()
        },
        &cancel,
    );
    assert!(integrated.ok);
    let mut app = integrated.app.clone();
    app.version = "1.0".to_string();
    app.update_manager = "github".to_string();
    app.update_config
        .insert("username".to_string(), "gosh".to_string());
    app.update_config
        .insert("repo".to_string(), "demo".to_string());
    app.update_config
        .insert("filename".to_string(), "Demo.AppImage".to_string());
    c.registry_mut().upsert(app.clone()).unwrap();

    let result = c.apply_update(&app, false, &cancel);
    assert!(!result.ok);
    assert!(result.error.contains("digest"));
}

#[test]
fn set_and_unset_source_roundtrip() {
    let h = Harness::new();
    let mut c = h.controller();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let app = seed_app_with_github(&mut c, "1.0", path.to_str().unwrap());

    // Invalid repo component rejected.
    let mut error = String::new();
    let mut bad = std::collections::BTreeMap::new();
    bad.insert("username".to_string(), "bad/../x".to_string());
    bad.insert("repo".to_string(), "demo".to_string());
    bad.insert("filename".to_string(), "Demo.AppImage".to_string());
    assert!(!c.set_update_source(app.clone(), "github", bad, &mut error));
    assert!(!error.is_empty());

    // Unset clears manager + config.
    assert!(c.unset_update_source(app.clone(), &mut error));
    let stored = c.registry().by_uuid(&app.uuid).unwrap();
    assert!(stored.update_manager.is_empty());

    // CLI --set-update-source key=value parsing.
    let argv = args(&[
        "--set-update-source",
        path.to_str().unwrap(),
        "--manager",
        "github",
        "username=gosh",
        "repo=demo",
        "filename=Demo.AppImage",
    ]);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdin: &[u8] = b"";
    // Path is not integrated (only seeded by uuid with another path)... seed path differs;
    // by_path lookup: seeded managed_path == fixture path, but fixture not integrated —
    // by_path matches the registry row regardless of file existence. Good.
    let code = run_cli(&mut c, &argv, false, &mut out, &mut err, &mut stdin);
    assert_eq!(
        code as i32,
        ExitCode::Ok as i32,
        "stderr: {}",
        String::from_utf8_lossy(&err)
    );
}

#[test]
fn list_updates_json_schema() {
    let h = Harness::new();
    let mut c = h.controller();
    h.network
        .canned_body("api.github.com", GITHUB_LATEST.as_bytes());
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    seed_app_with_github(&mut c, "1.0", path.to_str().unwrap());

    let argv = args(&["--list-updates", "--json"]);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdin: &[u8] = b"";
    let code = run_cli(&mut c, &argv, false, &mut out, &mut err, &mut stdin);
    assert_eq!(code as i32, ExitCode::Ok as i32);
    let doc: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out).trim()).unwrap();
    assert_eq!(doc.get("schema_version").and_then(|v| v.as_i64()), Some(1));
    let updates = doc.get("updates").and_then(|v| v.as_array()).unwrap();
    assert_eq!(updates.len(), 1);
    for key in [
        "name",
        "path",
        "desktop_id",
        "current_version",
        "available_version",
        "download_size",
        "manager",
        "embedded_source",
        "running",
    ] {
        assert!(updates[0].get(key).is_some(), "missing {key}");
    }
}

/// Audit finding S-7. GitHub returns `digest: "sha256:<hex>"`; the comparison
/// was against the raw field, so a correctly digested asset failed every time.
#[test]
fn digest_formats_are_parsed_before_comparison() {
    use goshaim_core::updates_service::parse_expected_sha256;
    let hex = "a".repeat(64);
    // GitHub's algorithm-prefixed form and GitLab's bare hex both verify.
    assert_eq!(
        parse_expected_sha256(&format!("sha256:{hex}")),
        Some(hex.clone())
    );
    assert_eq!(
        parse_expected_sha256(&format!("SHA256:{}", hex.to_uppercase())),
        Some(hex.clone())
    );
    assert_eq!(parse_expected_sha256(&hex), Some(hex.clone()));
    assert_eq!(
        parse_expected_sha256(&format!("  sha256:{hex}  ")),
        Some(hex.clone())
    );
    // Anything we cannot check must report that, not quietly pass.
    assert_eq!(parse_expected_sha256(""), None);
    assert_eq!(
        parse_expected_sha256(&"a".repeat(32)),
        None,
        "an MD5 is not a SHA-256"
    );
    assert_eq!(
        parse_expected_sha256(&format!("md5:{}", "a".repeat(32))),
        None
    );
    assert_eq!(
        parse_expected_sha256(&format!("sha512:{}", "a".repeat(128))),
        None
    );
    assert_eq!(
        parse_expected_sha256(&format!("sha256:{}", "z".repeat(64))),
        None,
        "non-hex"
    );
}

/// The same finding, end to end: a real GitHub-shaped digest must now verify
/// and let the update through.
#[test]
fn github_prefixed_digest_verifies_and_applies() {
    let h = Harness::new();
    let mut c = h.controller();
    let live = common::write_fixture(h.tmp.path(), "V.AppImage");
    let payload = goshaim_core::inspector::make_test_elf(
        goshaim_core::types::Architecture::X86_64,
        goshaim_core::types::AppImageType::Type2,
    );
    let real = {
        use sha2::Digest;
        hex::encode(sha2::Sha256::digest(&payload))
    };
    let body = format!(
        r#"{{"tag_name":"v2","assets":[{{"name":"app.AppImage","browser_download_url":"https://github.com/x/y/releases/download/v2/app.AppImage","size":128,"digest":"sha256:{real}"}}]}}"#
    );
    h.network.canned_body("api.github.com", body.as_bytes());
    h.network
        .canned_body("github.com/x/y/releases/download", &payload);

    let app = seed_arch_app(&mut c, &live);
    let result = c.apply_update(&app, false, &std::sync::atomic::AtomicBool::new(false));
    assert!(
        result.ok,
        "correct digest must verify, got: {}",
        result.error
    );
}

/// Audit finding S-8. Parsing an architecture is not the same as being able to
/// run it: an x86_64 install used to accept an aarch64 payload and break.
#[test]
fn foreign_architecture_update_is_refused() {
    let h = Harness::new();
    let mut c = h.controller();
    let live = common::write_fixture(h.tmp.path(), "V.AppImage");
    let before = std::fs::read(&live).unwrap();
    let foreign = goshaim_core::inspector::make_test_elf(
        goshaim_core::types::Architecture::AArch64,
        goshaim_core::types::AppImageType::Type2,
    );
    let body = r#"{"tag_name":"v2","assets":[{"name":"app.AppImage","browser_download_url":"https://github.com/x/y/releases/download/v2/app.AppImage","size":128}]}"#;
    h.network.canned_body("api.github.com", body.as_bytes());
    h.network
        .canned_body("github.com/x/y/releases/download", &foreign);

    let app = seed_arch_app(&mut c, &live);
    let result = c.apply_update(&app, false, &std::sync::atomic::AtomicBool::new(false));
    assert!(
        !result.ok,
        "an aarch64 payload must not replace an x86_64 install"
    );
    assert!(
        result.error.contains("aarch64") && result.error.contains("x86_64"),
        "the error should name both architectures, got: {}",
        result.error
    );
    assert_eq!(
        std::fs::read(&live).unwrap(),
        before,
        "the working installation must be left untouched"
    );
}

fn seed_arch_app(
    c: &mut goshaim_core::controller::AppController,
    live: &std::path::Path,
) -> goshaim_core::types::InstalledApp {
    let mut app = goshaim_core::types::InstalledApp::new_owned();
    app.uuid = "arch-app".into();
    app.name = "Victim".into();
    app.version = "v1".into();
    app.architecture = goshaim_core::types::Architecture::X86_64;
    app.managed_path = live.to_string_lossy().into_owned();
    app.update_manager = "github".into();
    app.update_config.insert("username".into(), "x".into());
    app.update_config.insert("repo".into(), "y".into());
    app.update_config
        .insert("filename".into(), "app.AppImage".into());
    c.registry_mut().upsert(app.clone()).unwrap();
    app
}

/// Audit finding S-12. The payload path must not buffer the whole AppImage:
/// the size bound defaults to 8 GiB, so an oversized body has to be refused
/// while streaming, not after allocating it.
#[test]
fn oversized_download_is_refused_without_buffering_it() {
    use goshaim_core::network::stream_to_file;
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("staged.AppImage");
    let payload = vec![0u8; 512 * 1024];

    let err = stream_to_file(
        payload.as_slice(),
        &dest,
        64 * 1024,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(err.contains("exceeds size bound"), "got: {err}");
    assert!(
        !dest.exists(),
        "a refused download must not leave a partial file"
    );

    // Within the bound it lands on disk with private permissions.
    let n = stream_to_file(
        payload.as_slice(),
        &dest,
        1024 * 1024,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(n, payload.len() as u64);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "staging must not be readable by other users");
    }
}

/// Cancellation now reaches the download, which it could not when the body was
/// fetched in one call before anything was written.
#[test]
fn cancelled_download_removes_the_partial_file() {
    use goshaim_core::network::stream_to_file;
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("staged.AppImage");
    let cancel = std::sync::atomic::AtomicBool::new(true);
    let err = stream_to_file(vec![0u8; 4096].as_slice(), &dest, 1 << 20, &cancel).unwrap_err();
    assert_eq!(err, "Cancelled");
    assert!(
        !dest.exists(),
        "a cancelled download must not leave a partial file"
    );
}

/// Audit finding C-4. The forge sources report `available: true` for any
/// matching asset; only list_updates compared versions. Applying directly --
/// which is what `--update <path>` does -- therefore replaced a working
/// installation with the identical version.
#[test]
fn applying_when_already_current_is_refused() {
    let h = Harness::new();
    let mut c = h.controller();
    let live = common::write_fixture(h.tmp.path(), "V.AppImage");
    let before = std::fs::read(&live).unwrap();
    let payload = goshaim_core::inspector::make_test_elf(
        goshaim_core::types::Architecture::X86_64,
        goshaim_core::types::AppImageType::Type2,
    );
    // The remote advertises exactly the version already installed.
    let body = r#"{"tag_name":"v1","assets":[{"name":"app.AppImage","browser_download_url":"https://github.com/x/y/releases/download/v1/app.AppImage","size":128}]}"#;
    h.network.canned_body("api.github.com", body.as_bytes());
    h.network
        .canned_body("github.com/x/y/releases/download", &payload);

    let app = seed_arch_app(&mut c, &live);
    let result = c.apply_update(&app, false, &std::sync::atomic::AtomicBool::new(false));
    assert!(
        !result.ok,
        "must not replace an install with its own version"
    );
    assert!(
        result.error.contains("Already at the latest version"),
        "got: {}",
        result.error
    );
    assert_eq!(
        std::fs::read(&live).unwrap(),
        before,
        "file must be untouched"
    );
}

/// Audit finding C-6. A failed check must be reported as a failure, not folded
/// into "no updates available" -- which reads to a user as "you are current".
#[test]
fn failed_checks_are_reported_not_silently_dropped() {
    let h = Harness::new();
    let mut c = h.controller();
    let live = common::write_fixture(h.tmp.path(), "V.AppImage");
    // No canned body for api.github.com: the check fails, as it would with the
    // network down or a certificate expired.
    let app = seed_arch_app(&mut c, &live);

    let scan = c.scan_updates(&std::sync::atomic::AtomicBool::new(false));
    assert!(scan.offers.is_empty(), "no offer can be produced");
    assert_eq!(scan.checked, 1, "the app should have been checked");
    assert_eq!(scan.failures.len(), 1, "the failure must be reported");
    assert_eq!(scan.failures[0].uuid, app.uuid);
    assert!(
        !scan.failures[0].error.is_empty(),
        "the failure must carry a reason"
    );

    // An app with no update source is skipped, not counted as a failure.
    let mut plain = goshaim_core::types::InstalledApp::new_owned();
    plain.uuid = "plain".into();
    plain.managed_path = live.to_string_lossy().into_owned();
    c.registry_mut().upsert(plain).unwrap();
    let scan = c.scan_updates(&std::sync::atomic::AtomicBool::new(false));
    assert_eq!(scan.skipped, 1);
    assert_eq!(scan.failures.len(), 1);
}
