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
