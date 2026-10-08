//! Background update checks and the login entry that runs them. The owner's
//! rules: background checks start off; a login check needs background checks
//! on; the login entry runs `--fetch-updates --background`, which contacts no
//! update source while background checks are off; and startup brings an entry
//! left by an earlier build into line with the setting.

mod common;

use common::{args, Harness};
use goshaim_core::cli::run_cli;
use goshaim_core::controller::AppController;
use goshaim_core::types::{ExitCode, InstalledApp};

const GITHUB_LATEST: &str = r#"{
  "tag_name": "2.0",
  "assets": [
    {"name": "Demo.AppImage", "browser_download_url": "https://github.com/gosh/demo/releases/download/2.0/Demo.AppImage", "size": 128}
  ]
}"#;

/// The login entry as the earlier release wrote it: the check ran with no
/// gate, so its Exec has no `--background`.
const EARLIER_BUILD_ENTRY: &str = "[Desktop Entry]\nType=Application\nName=Gosh AppImage Manager update checks\nExec=/usr/bin/gosh-appimage-manager --fetch-updates\nIcon=com.goshapps.AppImageManager\nTerminal=false\nCategories=Utility;\nX-GNOME-Autostart-enabled=true\n";

fn run(c: &mut AppController, a: &[&str]) -> (ExitCode, String, String) {
    let argv = args(a);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdin: &[u8] = b"";
    let code = run_cli(c, &argv, false, &mut out, &mut err, &mut stdin);
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

/// A GitHub-sourced installation that the canned release would update.
fn seed_app(c: &mut AppController, managed_path: &str) -> InstalledApp {
    let mut app = InstalledApp::new_owned();
    app.uuid = goshaim_core::registry::ManagedRegistry::new_uuid();
    app.name = "Demo".to_string();
    app.version = "1.0".to_string();
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

fn contacted(h: &Harness) -> Vec<String> {
    h.network.calls.lock().unwrap().clone()
}

#[test]
fn a_fresh_home_has_background_checks_off() {
    let h = Harness::new();
    let c = h.controller();
    assert!(
        !c.settings().background_update_checks(),
        "the default is off; that is the owner's decision"
    );
}

#[test]
fn a_login_check_cannot_be_added_while_background_checks_are_off() {
    let h = Harness::new();
    let c = h.controller();
    let error = c
        .sync_autostart(true)
        .expect_err("the core refuses a login check while background checks are off");
    assert!(error.contains("Check in the background"), "{error}");
    assert!(
        !c.autostart_desktop_path().exists(),
        "no entry is written when the request is refused"
    );
}

#[test]
fn a_login_check_is_written_with_the_background_flag_when_background_checks_are_on() {
    let h = Harness::new();
    let mut c = h.controller();
    c.settings_mut().set_background_update_checks(true).unwrap();
    c.sync_autostart(true)
        .expect("allowed once background checks are on");
    let body = std::fs::read_to_string(c.autostart_desktop_path()).unwrap();
    let exec = body
        .lines()
        .find_map(|line| line.strip_prefix("Exec="))
        .expect("an Exec line");
    assert!(exec.ends_with("--fetch-updates --background"), "{exec}");
}

#[test]
fn turning_the_login_check_off_removes_its_entry() {
    let h = Harness::new();
    let mut c = h.controller();
    c.settings_mut().set_background_update_checks(true).unwrap();
    c.sync_autostart(true).unwrap();
    assert!(c.autostart_desktop_path().exists());
    c.sync_autostart(false).unwrap();
    assert!(!c.autostart_desktop_path().exists());
}

#[test]
fn background_checks_stored_on_survive_a_restart_and_off_is_stored_too() {
    let h = Harness::new();
    {
        let mut c = h.controller();
        c.settings_mut().set_background_update_checks(true).unwrap();
    }
    assert!(h.controller().settings().background_update_checks());
    {
        let mut c = h.controller();
        c.settings_mut()
            .set_background_update_checks(false)
            .unwrap();
    }
    assert!(!h.controller().settings().background_update_checks());
}

#[test]
fn the_login_flag_stops_a_check_before_any_source_is_contacted_when_background_is_off() {
    let h = Harness::new();
    let mut c = h.controller();
    let fixture = h.tmp.path().join("Demo.AppImage");
    seed_app(&mut c, fixture.to_str().unwrap());
    h.network
        .canned_body("api.github.com", GITHUB_LATEST.as_bytes());

    let (code, out, err) = run(&mut c, &["--fetch-updates", "--background"]);

    assert_eq!(code as i32, ExitCode::Ok as i32);
    assert!(out.is_empty(), "stdout stays empty: {out:?}");
    assert!(
        err.contains("Background update checks are off"),
        "one line says why nothing ran: {err:?}"
    );
    assert!(
        contacted(&h).is_empty(),
        "no update source was contacted: {:?}",
        contacted(&h)
    );
}

#[test]
fn the_login_flag_still_scans_when_background_checks_are_on() {
    let h = Harness::new();
    let mut c = h.controller();
    c.settings_mut().set_background_update_checks(true).unwrap();
    let fixture = h.tmp.path().join("Demo.AppImage");
    seed_app(&mut c, fixture.to_str().unwrap());
    h.network
        .canned_body("api.github.com", GITHUB_LATEST.as_bytes());

    let (code, _out, err) = run(&mut c, &["--fetch-updates", "--background"]);

    assert_eq!(code as i32, ExitCode::Ok as i32);
    assert!(err.contains("1 update(s) available"), "{err:?}");
    assert!(contacted(&h)
        .iter()
        .any(|url| url.contains("api.github.com")));
}

#[test]
fn a_manual_fetch_updates_without_the_flag_scans_even_when_background_is_off() {
    let h = Harness::new();
    let mut c = h.controller();
    let fixture = h.tmp.path().join("Demo.AppImage");
    seed_app(&mut c, fixture.to_str().unwrap());
    h.network
        .canned_body("api.github.com", GITHUB_LATEST.as_bytes());

    let (code, _out, err) = run(&mut c, &["--fetch-updates"]);

    assert_eq!(code as i32, ExitCode::Ok as i32);
    assert!(err.contains("1 update(s) available"), "{err:?}");
    assert!(contacted(&h)
        .iter()
        .any(|url| url.contains("api.github.com")));
}

#[test]
fn startup_removes_an_earlier_login_entry_when_background_checks_are_off() {
    let h = Harness::new();
    let c = h.controller();
    let path = c.autostart_desktop_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, EARLIER_BUILD_ENTRY).unwrap();

    c.reconcile_autostart().expect("reconciles");

    assert!(
        !path.exists(),
        "an ungated entry must not outlive the setting"
    );
}

#[test]
fn startup_rewrites_an_earlier_login_entry_to_carry_the_background_flag() {
    let h = Harness::new();
    let mut c = h.controller();
    c.settings_mut().set_background_update_checks(true).unwrap();
    let path = c.autostart_desktop_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, EARLIER_BUILD_ENTRY).unwrap();

    c.reconcile_autostart().expect("reconciles");

    let body = std::fs::read_to_string(&path).unwrap();
    assert!(
        body.lines()
            .any(|line| line.starts_with("Exec=") && line.ends_with("--fetch-updates --background")),
        "{body}"
    );
    // A second startup finds the entry in line and leaves it alone.
    c.reconcile_autostart().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), body);
}

#[test]
fn startup_leaves_a_login_file_that_is_not_ours_alone() {
    let h = Harness::new();
    let c = h.controller();
    let path = c.autostart_desktop_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let theirs = "[Desktop Entry]\nName=Someone else\nExec=/usr/bin/other --fetch-updates\n";
    std::fs::write(&path, theirs).unwrap();

    c.reconcile_autostart().unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), theirs);
}
