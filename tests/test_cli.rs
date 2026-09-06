mod common;

use common::Harness;
use goshaim_core::cli::run_cli;
use goshaim_core::types::ExitCode;

use common::args;

fn run(
    _h: &Harness,
    c: &mut goshaim_core::controller::AppController,
    a: &[&str],
    tty: bool,
) -> (ExitCode, String, String) {
    let argv = args(a);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdin: &[u8] = b"";
    let code = run_cli(c, &argv, tty, &mut out, &mut err, &mut stdin);
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

#[test]
fn list_managers_names_all_six() {
    let h = Harness::new();
    let mut c = h.controller();
    let (code, out, _) = run(&h, &mut c, &["--list-update-managers"], false);
    assert_eq!(code as i32, ExitCode::Ok as i32);
    for name in ["static", "github", "gitlab", "codeberg", "forgejo", "ftp"] {
        assert!(out.lines().any(|l| l == name), "missing {name} in {out:?}");
    }
}

#[test]
fn list_installed_json_schema() {
    let h = Harness::new();
    let mut c = h.controller();
    // Seed one owned app.
    let mut app = goshaim_core::types::InstalledApp::new_owned();
    app.uuid = "cli-1".to_string();
    app.name = "Demo".to_string();
    app.managed_path = h
        .tmp
        .path()
        .join("Demo.AppImage")
        .to_string_lossy()
        .into_owned();
    app.desktop_id = "gosh-appimage-cli-1.desktop".to_string();
    app.version = "1.0".to_string();
    c.registry_mut().upsert(app).unwrap();

    let (code, out, _err) = run(&h, &mut c, &["--list-installed", "--json"], false);
    assert_eq!(code as i32, ExitCode::Ok as i32);
    let doc: serde_json::Value = serde_json::from_str(out.trim()).expect("valid JSON");
    assert_eq!(doc.get("schema_version").and_then(|v| v.as_i64()), Some(1));
    let installed = doc
        .get("installed")
        .and_then(|v| v.as_array())
        .expect("installed array");
    assert!(!doc.get("items").is_some(), "must use installed, not items");
    assert_eq!(installed.len(), 1);
    let row = &installed[0];
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
        "uuid",
        "owned",
    ] {
        assert!(row.get(key).is_some(), "missing key {key}");
    }
    assert_eq!(row.get("name").and_then(|v| v.as_str()), Some("Demo"));
}

#[test]
fn integrate_needs_confirmation_without_tty() {
    let h = Harness::new();
    let mut c = h.controller();
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let (code, _out, err) = run(&h, &mut c, &["--integrate", path.to_str().unwrap()], false);
    assert_eq!(code as i32, ExitCode::NeedsConfirmation as i32);
    assert!(err.contains("pass --yes"));
    assert!(c.registry().apps().is_empty());
}

#[test]
fn integrate_with_yes_succeeds() {
    let h = Harness::new();
    let mut c = h.controller();
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let (code, _out, err) = run(
        &h,
        &mut c,
        &["--integrate", path.to_str().unwrap(), "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");
    assert!(err.contains("Integrated"));
    assert_eq!(c.registry().apps().len(), 1);
    let app = &c.registry().apps()[0];
    assert!(app.owned);
    assert!(std::path::Path::new(&app.managed_path).exists());
    assert!(std::path::Path::new(&app.desktop_path).exists());
}

#[test]
fn replace_owned_succeeds_unowned_refused() {
    let h = Harness::new();
    let mut c = h.controller();
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let (code, _, err) = run(
        &h,
        &mut c,
        &["--integrate", path.to_str().unwrap(), "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");

    // Replace without a target resolves the single filename match.
    let path2 = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let (code, _, err) = run(
        &h,
        &mut c,
        &["--integrate", path2.to_str().unwrap(), "--replace", "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");

    // Replace with a bogus uuid is refused.
    let path3 = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let (code, _, _) = run(
        &h,
        &mut c,
        &[
            "--integrate",
            path3.to_str().unwrap(),
            "--replace",
            "--replace-uuid",
            "nope",
            "--yes",
        ],
        false,
    );
    assert_eq!(code as i32, ExitCode::NotIntegrated as i32);
}

#[test]
fn second_integrate_without_policy_is_validation() {
    let h = Harness::new();
    let mut c = h.controller();
    // Integrate from two different source dirs with the same file name.
    let src1 = h.tmp.path().join("src1");
    let src2 = h.tmp.path().join("src2");
    std::fs::create_dir_all(&src1).unwrap();
    std::fs::create_dir_all(&src2).unwrap();
    let p1 = common::write_fixture(&src1, "Demo.AppImage");
    let p2 = common::write_fixture(&src2, "Demo.AppImage");
    let (code, _, _) = run(
        &h,
        &mut c,
        &["--integrate", p1.to_str().unwrap(), "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::Ok as i32);
    let (code, _, err) = run(
        &h,
        &mut c,
        &["--integrate", p2.to_str().unwrap(), "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::Validation as i32);
    assert!(
        err.contains("keep-both") || err.contains("replace"),
        "stderr: {err}"
    );

    // keep-both resolves the conflict.
    let (code, _, _) = run(
        &h,
        &mut c,
        &["--integrate", p2.to_str().unwrap(), "--keep-both", "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::Ok as i32);
    assert_eq!(c.registry().apps().len(), 2);
}

#[test]
fn update_unknown_path_is_not_integrated() {
    let h = Harness::new();
    let mut c = h.controller();
    let (code, _, _) = run(
        &h,
        &mut c,
        &["--update", "/nope/Demo.AppImage", "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::NotIntegrated as i32);
}

#[test]
fn fetch_updates_notice_goes_to_stderr_stdout_empty() {
    let h = Harness::new();
    let mut c = h.controller();
    let (code, out, err) = run(&h, &mut c, &["--fetch-updates"], false);
    assert_eq!(code as i32, ExitCode::Ok as i32);
    assert!(out.is_empty(), "stdout must stay empty, got {out:?}");
    assert!(err.contains("0 update(s) available"));
}

#[test]
fn remove_needs_confirmation_and_trashes() {
    let h = Harness::new();
    let mut c = h.controller();
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let (code, _, _) = run(
        &h,
        &mut c,
        &["--integrate", path.to_str().unwrap(), "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::Ok as i32);
    let managed = c.registry().apps()[0].managed_path.clone();

    let (code, _, _) = run(&h, &mut c, &["--remove", &managed], false);
    assert_eq!(code as i32, ExitCode::NeedsConfirmation as i32);

    let (code, _, err) = run(&h, &mut c, &["--remove", &managed, "--yes"], false);
    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");
    assert!(c.registry().apps().is_empty());
    assert_eq!(h.trash.trashed.lock().unwrap().len(), 1);
}

#[test]
fn self_test_passes_offline() {
    let h = Harness::new();
    let mut c = h.controller();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = goshaim_core::cli::run_self_test(&mut c, &mut out, &mut err);
    assert_eq!(
        code as i32,
        ExitCode::Ok as i32,
        "stderr: {}",
        String::from_utf8_lossy(&err)
    );
    assert!(String::from_utf8_lossy(&out).contains("SELF_TEST_OK"));
}

#[test]
fn unknown_command_is_usage() {
    let h = Harness::new();
    let mut c = h.controller();
    let (code, _, err) = run(&h, &mut c, &["--frobnicate"], false);
    assert_eq!(code as i32, ExitCode::Usage as i32);
    assert!(err.contains("Unknown command"));
}
