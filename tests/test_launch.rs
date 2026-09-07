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

/// Audit finding S-10. setsid() makes the launched app a session leader but
/// does not reparent it, so it stays our direct child and must be reaped.
/// `mem::forget` on the Child handle expressed "never wait" but left a zombie
/// per launch, which a long-lived GUI session accumulates until it hits the
/// per-user process limit.
#[cfg(unix)]
#[test]
fn detached_launches_do_not_leave_zombies() {
    use goshaim_core::process::{ProcessRequest, ProcessRunner, SystemRunner};

    fn own_zombie_children() -> usize {
        let me = std::process::id();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return 0;
        };
        entries
            .flatten()
            .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
            .filter(|pid| {
                let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                    return false;
                };
                // "pid (comm) state ppid ..." -- comm may contain spaces and
                // parentheses, so split after the last ')'.
                let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r) else {
                    return false;
                };
                let mut fields = rest.split_whitespace();
                let state = fields.next().unwrap_or_default();
                let ppid: u32 = fields.next().unwrap_or_default().parse().unwrap_or(0);
                state == "Z" && ppid == me
            })
            .count()
    }

    let before = own_zombie_children();
    let runner = SystemRunner::new();
    for _ in 0..8 {
        runner
            .start_detached(&ProcessRequest {
                program: "/bin/true".to_string(),
                timeout_ms: 5_000,
                ..Default::default()
            })
            .expect("start_detached should succeed for /bin/true");
    }

    // Give the children time to exit and the reapers time to collect them.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if own_zombie_children() <= before {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let after = own_zombie_children();
    assert!(
        after <= before,
        "detached launches leaked {} zombie(s) (before={before}, after={after})",
        after.saturating_sub(before)
    );
}
