mod common;

use common::{write_fixture, Harness};
use goshaim_core::types::{IntegrateRequest, RemovalMode, RemovalRequest};

#[test]
fn trash_removal_cleans_owned_artifacts() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = common::Harness::cancel();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let integrated = c.integrate(
        &IntegrateRequest {
            source_path: path.to_str().unwrap().to_string(),
            assume_yes: true,
            ..Default::default()
        },
        &cancel,
    );
    assert!(integrated.ok, "error: {}", integrated.error);
    let desktop = integrated.app.desktop_path.clone();

    let outcome = c.remove_app(&RemovalRequest {
        path_or_uuid: integrated.app.uuid.clone(),
        mode: RemovalMode::Trash,
        assume_yes: true,
    });
    assert!(outcome.ok, "error: {}", outcome.error);
    assert!(c.registry().apps().is_empty());
    assert!(!std::path::Path::new(&desktop).exists());
    assert_eq!(h.trash.trashed.lock().unwrap().len(), 1);
}

#[test]
fn trash_failure_leaves_everything_intact() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = common::Harness::cancel();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let integrated = c.integrate(
        &IntegrateRequest {
            source_path: path.to_str().unwrap().to_string(),
            assume_yes: true,
            ..Default::default()
        },
        &cancel,
    );
    assert!(integrated.ok);
    *h.trash.fail_next.lock().unwrap() = true;

    let outcome = c.remove_app(&RemovalRequest {
        path_or_uuid: integrated.app.uuid.clone(),
        mode: RemovalMode::Trash,
        assume_yes: true,
    });
    assert!(!outcome.ok);
    assert!(outcome.error.contains("Trash failed"));
    // Nothing touched: registry row, managed file, desktop all remain.
    assert_eq!(c.registry().apps().len(), 1);
    assert!(std::path::Path::new(&integrated.app.managed_path).exists());
    assert!(std::path::Path::new(&integrated.app.desktop_path).exists());
}

#[test]
fn unowned_app_cannot_be_removed() {
    let h = Harness::new();
    let mut c = h.controller();
    let mut app = goshaim_core::types::InstalledApp::new_owned();
    app.uuid = "external-1".to_string();
    app.name = "External".to_string();
    app.managed_path = "/opt/foreign.AppImage".to_string();
    app.owned = false;
    c.registry_mut().upsert(app).unwrap();

    let outcome = c.remove_app(&RemovalRequest {
        path_or_uuid: "external-1".to_string(),
        mode: RemovalMode::Trash,
        assume_yes: true,
    });
    assert!(!outcome.ok);
    assert_eq!(outcome.error, "Not an owned managed AppImage");
}

#[test]
fn permanent_delete_refuses_protected_paths() {
    assert!(goshaim_core::removal::is_forbidden_permanent_target(
        std::path::Path::new("/"),
        std::path::Path::new("/home/u"),
    ));
    assert!(goshaim_core::removal::is_forbidden_permanent_target(
        std::path::Path::new("/home/u"),
        std::path::Path::new("/home/u"),
    ));
    assert!(goshaim_core::removal::is_forbidden_permanent_target(
        std::path::Path::new("/top.AppImage"),
        std::path::Path::new("/home/u"),
    ));
    assert!(!goshaim_core::removal::is_forbidden_permanent_target(
        std::path::Path::new("/home/u/AppImages/Demo.AppImage"),
        std::path::Path::new("/home/u"),
    ));
}

#[test]
fn permanent_delete_removes_file_and_artifacts() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = common::Harness::cancel();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let integrated = c.integrate(
        &IntegrateRequest {
            source_path: path.to_str().unwrap().to_string(),
            assume_yes: true,
            ..Default::default()
        },
        &cancel,
    );
    assert!(integrated.ok, "error: {}", integrated.error);

    let outcome = c.remove_app(&RemovalRequest {
        path_or_uuid: integrated.app.uuid.clone(),
        mode: RemovalMode::Permanent,
        assume_yes: true,
    });
    assert!(outcome.ok, "error: {}", outcome.error);
    assert!(c.registry().apps().is_empty());
    assert!(!std::path::Path::new(&integrated.app.managed_path).exists());
    assert!(h.trash.trashed.lock().unwrap().is_empty());
}

#[test]
fn remove_all_only_touches_owned() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = common::Harness::cancel();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let integrated = c.integrate(
        &IntegrateRequest {
            source_path: path.to_str().unwrap().to_string(),
            assume_yes: true,
            ..Default::default()
        },
        &cancel,
    );
    assert!(integrated.ok);
    // CLI --remove-all path via run_cli.
    let argv = common::args(&["--remove-all", "--yes"]);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdin: &[u8] = b"";
    let code = goshaim_core::cli::run_cli(&mut c, &argv, false, &mut out, &mut err, &mut stdin);
    assert_eq!(code as i32, goshaim_core::types::ExitCode::Ok as i32);
    assert!(c.registry().apps().is_empty());
}

/// Audit finding C-8. The protected-path guard must refuse filesystem roots,
/// home directories, and anything sitting directly inside them.
#[test]
fn protected_target_rules_cover_roots_and_home_top_level() {
    use goshaim_core::removal::is_forbidden_permanent_target;
    use std::path::Path;
    let home = Path::new("/home/alice");

    for forbidden in [
        "/",
        "/home",
        "/root",
        "/home/alice",
        "/home/bob",     // directly inside /home
        "/root/thing",   // directly inside /root
        "/home/alice/x", // top level of this user's home
        "/etc",          // directly inside /
    ] {
        assert!(
            is_forbidden_permanent_target(Path::new(forbidden), home),
            "{forbidden} must be refused"
        );
    }
    // A managed AppImage well inside the home is deletable.
    assert!(!is_forbidden_permanent_target(
        Path::new("/home/alice/AppImages/App.AppImage"),
        home
    ));
    assert!(!is_forbidden_permanent_target(
        Path::new("/opt/apps/App.AppImage"),
        home
    ));
}
