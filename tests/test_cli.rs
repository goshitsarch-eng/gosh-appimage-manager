mod common;

use common::Harness;
use goshaim_core::cli::run_cli;
use goshaim_core::types::ExitCode;

use common::{args, plant_self_extraction, ran_self_extraction};

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
    assert!(doc.get("items").is_none(), "must use installed, not items");
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

/// An AppImage whose metadata cannot be read still integrates, and the CLI says
/// so on stderr. Silent success on a fallback name is what the QA run saw.
#[test]
fn integrate_reports_metadata_warnings_on_stderr() {
    let h = Harness::new();
    let mut c = h.controller();
    // No payload behind the header, and no canned extractor: extraction fails.
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let (code, _out, err) = run(
        &h,
        &mut c,
        &["--integrate", path.to_str().unwrap(), "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");
    assert!(err.contains("Warning:"), "stderr: {err}");
    assert_eq!(c.registry().apps().len(), 1);
}

/// A settings file that cannot be read means the defaults are in use, so no
/// mutating command runs on them. The refusal says why and exits non-zero.
#[test]
fn mutating_commands_refuse_when_the_settings_file_is_corrupt() {
    let h = Harness::new();
    let config = h.tmp.path().join(".config/gosh-appimage-manager");
    std::fs::create_dir_all(&config).unwrap();
    let settings = config.join("settings.json");
    std::fs::write(&settings, b"{ not json").unwrap();
    let mut c = h.controller();
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");

    let (code, _out, err) = run(
        &h,
        &mut c,
        &["--integrate", path.to_str().unwrap(), "--yes"],
        false,
    );
    assert_eq!(code as i32, ExitCode::Failure as i32, "stderr: {err}");
    assert!(err.contains("settings.json"), "stderr: {err}");
    assert!(c.registry().apps().is_empty(), "nothing was integrated");
    assert_eq!(std::fs::read(&settings).unwrap(), b"{ not json");

    // Reading still works, with a warning that the defaults are in use.
    let (code, _out, err) = run(&h, &mut c, &["--list-installed"], false);
    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");
    assert!(err.contains("Warning:"), "stderr: {err}");
}

/// A managed folder that exists but cannot be read is an error. An empty list
/// would read as "nothing here" when the folder was simply locked.
#[test]
fn list_discovered_refuses_an_unreadable_managed_folder() {
    use std::os::unix::fs::PermissionsExt;

    let h = Harness::new();
    let locked = h.tmp.path().join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&locked).is_ok() {
        // Root reads mode-000 folders; the permission check cannot be exercised.
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
        eprintln!("SKIPPED list_discovered_refuses_an_unreadable_managed_folder: running as root");
        return;
    }
    let mut c = h.controller();
    c.settings_mut()
        .set_managed_folder(locked.clone())
        .expect("the folder setting is saved");

    let (code, out, err) = run(&h, &mut c, &["--list-discovered", "--json"], false);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();

    assert_eq!(
        code as i32,
        ExitCode::Failure as i32,
        "stdout: {out} stderr: {err}"
    );
    assert!(err.contains(&locked.display().to_string()), "stderr: {err}");
    assert!(out.is_empty(), "no JSON for a failed listing: {out}");
}

/// `--integrate` takes the stored setting. With it on, the AppImage that safe
/// extraction could not read is run with its own --appimage-extract, and stderr
/// says so.
#[test]
fn integrate_on_the_cli_uses_the_unsafe_fallback_when_the_setting_is_on() {
    let h = Harness::new();
    let mut c = h.controller();
    let log = plant_self_extraction(&h);
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");

    let (code, _out, err) = run(
        &h,
        &mut c,
        &[
            "--integrate",
            path.to_str().unwrap(),
            "--yes",
            "--allow-unsafe",
        ],
        false,
    );

    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");
    assert!(ran_self_extraction(&log), "the fallback did not run");
    assert!(
        err.contains("Unsafe extraction fallback used"),
        "stderr: {err}"
    );
    assert_eq!(c.registry().apps()[0].name, "Unpacked");
}

/// With the setting off, `--integrate` does not run the AppImage and says why.
#[test]
fn integrate_on_the_cli_refuses_the_unsafe_fallback_while_the_setting_is_off() {
    let h = Harness::new();
    let mut c = h.controller();
    let log = plant_self_extraction(&h);
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");

    let (code, _out, err) = run(
        &h,
        &mut c,
        &["--integrate", path.to_str().unwrap(), "--yes"],
        false,
    );

    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");
    assert!(
        !ran_self_extraction(&log),
        "the AppImage ran while the setting is off"
    );
    assert!(
        err.contains("Warning:") && err.contains("fallback is off"),
        "stderr: {err}"
    );
}

/// Without `--allow-unsafe`, an AppImage that needs the fallback is not integrated.
/// The exit code says the run needs confirmation, and stderr names the flag.
#[test]
fn integrate_on_the_cli_stops_at_a_pending_read_and_names_the_flag() {
    let h = Harness::new();
    let mut c = h.controller();
    let log = plant_self_extraction(&h);
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");

    let (code, _out, err) = run(
        &h,
        &mut c,
        &["--integrate", path.to_str().unwrap(), "--yes"],
        false,
    );

    assert_eq!(
        code as i32,
        ExitCode::NeedsConfirmation as i32,
        "stderr: {err}"
    );
    assert!(err.contains("--allow-unsafe"), "stderr: {err}");
    assert!(
        !ran_self_extraction(&log),
        "the AppImage ran before confirmation"
    );
    assert!(c.registry().apps().is_empty(), "nothing may be installed");
    assert!(path.exists(), "the source must stay where it is");
}

/// With `--allow-unsafe`, the fallback runs for this file and the file is installed.
#[test]
fn integrate_on_the_cli_runs_the_fallback_for_that_file_with_allow_unsafe() {
    let h = Harness::new();
    let mut c = h.controller();
    let log = plant_self_extraction(&h);
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");

    let (code, _out, err) = run(
        &h,
        &mut c,
        &[
            "--integrate",
            path.to_str().unwrap(),
            "--yes",
            "--allow-unsafe",
        ],
        false,
    );

    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");
    assert!(ran_self_extraction(&log), "the fallback did not run");
    assert_eq!(c.registry().apps()[0].name, "Unpacked");
}

/// `--allow-unsafe` does not override a setting that is off.
#[test]
fn the_allow_unsafe_flag_does_not_override_a_setting_that_is_off() {
    let h = Harness::new();
    let mut c = h.controller();
    let log = plant_self_extraction(&h);
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");

    let (code, _out, err) = run(
        &h,
        &mut c,
        &[
            "--integrate",
            path.to_str().unwrap(),
            "--yes",
            "--allow-unsafe",
        ],
        false,
    );

    assert_eq!(code as i32, ExitCode::Ok as i32, "stderr: {err}");
    assert!(
        !ran_self_extraction(&log),
        "the AppImage ran while the setting is off"
    );
    assert!(err.contains("fallback is off"), "stderr: {err}");
}

/// The line that tells the user how to allow a pending file. It must be a command a
/// script can run: it repeats the user's flags and adds --allow-unsafe and --yes.
fn suggestion_line(stderr: &str) -> String {
    stderr
        .lines()
        .find(|line| line.contains("gosh-appimage-manager --integrate"))
        .unwrap_or("")
        .to_string()
}

#[test]
fn a_pending_suggestion_names_both_flags_a_script_needs() {
    let h = Harness::new();
    let mut c = h.controller();
    let _log = plant_self_extraction(&h);
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let path_text = path.to_str().unwrap();

    // A script passes --yes, so the suggestion must repeat it.
    let (code, _out, err) = run(&h, &mut c, &["--integrate", path_text, "--yes"], false);
    assert_eq!(
        code as i32,
        ExitCode::NeedsConfirmation as i32,
        "stderr: {err}"
    );
    let line = suggestion_line(&err);
    assert!(
        line.contains("--allow-unsafe") && line.contains("--yes"),
        "suggestion: {line}"
    );

    // A terminal user answers the prompt without --yes, so the suggestion adds it.
    let argv = args(&["--integrate", path_text]);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdin: &[u8] = b"y\n";
    let code = run_cli(&mut c, &argv, true, &mut out, &mut err, &mut stdin);
    let err = String::from_utf8_lossy(&err).into_owned();
    assert_eq!(
        code as i32,
        ExitCode::NeedsConfirmation as i32,
        "stderr: {err}"
    );
    let line = suggestion_line(&err);
    assert!(
        line.contains("--allow-unsafe") && line.contains("--yes"),
        "suggestion: {line}"
    );
}
