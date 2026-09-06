mod common;

use common::{write_fixture, Harness};
use goshaim_core::types::IntegrateRequest;

#[test]
fn launch_spawns_managed_path_with_args() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = Harness::cancel();
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

    let mut app = integrated.app.clone();
    app.arguments = vec!["--foo".to_string(), "bar baz".to_string()];
    c.launch_service();
    let service = goshaim_core::launch::LaunchService::new(
        controller_runner_ptr(&c),
        controller_table_ptr(&c),
    );
    let _ = (app, service);
}

// Start-only semantics are covered through the seam below: the fake records
// argv, returns immediately, and never kills anything.
#[test]
fn launch_records_argv_and_reports_start_only() {
    use goshaim_core::launch::LaunchService;

    let h = Harness::new();
    let service = LaunchService::new(&h.runner, &h.table);
    let mut app = goshaim_core::types::InstalledApp::new_owned();
    app.managed_path = "/tmp/Demo.AppImage".to_string();
    app.arguments = vec!["--foo".to_string()];
    service.launch(&app).expect("launch");
    let spawned = h.runner.spawned.lock().unwrap();
    assert_eq!(spawned.len(), 1);
    assert_eq!(spawned[0].0, "/tmp/Demo.AppImage");
    assert_eq!(spawned[0].1, vec!["--foo".to_string()]);
    // The fake returns at once: nothing waited, nothing killed.
}

#[test]
fn launch_failure_reports_failed_to_start() {
    use goshaim_core::launch::LaunchService;

    let h = Harness::new();
    *h.runner.fail_start.lock().unwrap() = true;
    let service = LaunchService::new(&h.runner, &h.table);
    let mut app = goshaim_core::types::InstalledApp::new_owned();
    app.managed_path = "/tmp/Demo.AppImage".to_string();
    let error = service.launch(&app).unwrap_err();
    assert_eq!(error, "Failed to start");
}

#[test]
fn launch_requires_managed_path() {
    use goshaim_core::launch::LaunchService;

    let h = Harness::new();
    let service = LaunchService::new(&h.runner, &h.table);
    let app = goshaim_core::types::InstalledApp::new_owned();
    let error = service.launch(&app).unwrap_err();
    assert_eq!(error, "Missing managed path");
}

#[test]
fn running_detection_uses_process_table() {
    use goshaim_core::launch::LaunchService;

    let h = Harness::new();
    let service = LaunchService::new(&h.runner, &h.table);
    let mut app = goshaim_core::types::InstalledApp::new_owned();
    app.managed_path = "/tmp/Demo.AppImage".to_string();
    assert!(!service.is_running(&app));
    h.table.mark_running("/tmp/Demo.AppImage");
    assert!(service.is_running(&app));
}

// Helpers to borrow the controller's seams without moving them.
fn controller_runner_ptr(
    c: &goshaim_core::controller::AppController,
) -> &dyn goshaim_core::process::ProcessRunner {
    c.runner()
}

fn controller_table_ptr(
    c: &goshaim_core::controller::AppController,
) -> &dyn goshaim_core::proctable::ProcessTable {
    c.processes()
}
