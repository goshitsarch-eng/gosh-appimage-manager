//! What an update reports while it runs, what a single check touches, and what
//! a task records about its own timing and versions.

mod common;

use common::{write_fixture, Harness};
use goshaim_core::tasks::TaskQueue;
use goshaim_core::types::{
    AppImageType, ApplyEvent, Architecture, InstalledApp, IntegrateRequest, TaskKind, UpdatePhase,
};

const GITHUB_LATEST: &str = r#"{
  "tag_name": "2.0",
  "assets": [
    {"name": "Demo.AppImage", "browser_download_url": "https://github.com/gosh/demo/releases/download/2.0/Demo.AppImage", "size": 128}
  ]
}"#;

/// Integrate a real managed file, then give it an older version and a GitHub
/// source, so the release on the canned network is an update.
fn integrated_app_behind_release(
    h: &Harness,
    c: &mut goshaim_core::controller::AppController,
) -> InstalledApp {
    h.network
        .canned_body("api.github.com", GITHUB_LATEST.as_bytes());
    let fixture = goshaim_core::inspector::make_test_elf(Architecture::X86_64, AppImageType::Type2);
    h.network.canned_body("releases/download", &fixture);
    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let incoming = write_fixture(&src, "Demo.AppImage");
    let cancel = Harness::cancel();
    let integrated = c.integrate(
        &IntegrateRequest {
            source_path: incoming.to_str().unwrap().to_string(),
            assume_yes: true,
            confirm_unsafe: false,
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
    app
}

#[test]
fn an_update_reports_its_versions_then_download_bytes_then_verify_then_swap_in() {
    let h = Harness::new();
    let mut c = h.controller();
    let app = integrated_app_behind_release(&h, &mut c);
    let fixture_len =
        goshaim_core::inspector::make_test_elf(Architecture::X86_64, AppImageType::Type2).len()
            as u64;
    let cancel = Harness::cancel();

    let mut events = Vec::new();
    let result =
        c.apply_update_with_progress(&app, false, &cancel, &mut |event| events.push(event));
    assert!(result.ok, "error: {}", result.error);

    // The versions and the size the source advertised come first.
    assert_eq!(
        events.first(),
        Some(&ApplyEvent::Planned {
            from_version: "1.0".to_string(),
            to_version: "2.0".to_string(),
            download_size: 128,
        })
    );

    // Then the stages, in order: download, verify, swap in.
    let phases: Vec<UpdatePhase> = events
        .iter()
        .filter_map(|event| match event {
            ApplyEvent::Phase { phase, .. } => Some(*phase),
            ApplyEvent::Planned { .. } => None,
        })
        .collect();
    let verify = phases
        .iter()
        .position(|p| *p == UpdatePhase::Verify)
        .expect("verify is reported");
    let swap = phases
        .iter()
        .position(|p| *p == UpdatePhase::SwapIn)
        .expect("swap in is reported");
    assert!(phases[..verify].iter().all(|p| *p == UpdatePhase::Download));
    assert!(verify < swap, "verify comes before swap in: {phases:?}");

    // The download's bytes never go backwards and end at the full size.
    let downloads: Vec<(u64, u64)> = events
        .iter()
        .filter_map(|event| match event {
            ApplyEvent::Phase {
                phase: UpdatePhase::Download,
                done,
                total,
            } => Some((*done, *total)),
            _ => None,
        })
        .collect();
    assert!(!downloads.is_empty(), "download bytes are reported");
    assert!(
        downloads.windows(2).all(|w| w[0].0 <= w[1].0),
        "{downloads:?}"
    );
    assert_eq!(*downloads.last().unwrap(), (fixture_len, fixture_len));

    // The update itself did land.
    assert_eq!(c.registry().by_uuid(&app.uuid).unwrap().version, "2.0");
}

#[test]
fn checking_one_app_offers_the_release_and_changes_nothing_installed() {
    let h = Harness::new();
    let mut c = h.controller();
    let app = integrated_app_behind_release(&h, &mut c);
    let before = std::fs::read(&app.managed_path).unwrap();
    let cancel = Harness::cancel();

    let checked = c.check_one_update(&app, &cancel);

    assert!(checked.ok, "error: {}", checked.error);
    assert!(checked.available);
    assert_eq!(checked.version, "2.0");
    // The installed file and its registry version are as they were.
    assert_eq!(std::fs::read(&app.managed_path).unwrap(), before);
    assert_eq!(c.registry().by_uuid(&app.uuid).unwrap().version, "1.0");
    // The check asked for metadata only; no download was made.
    let calls = h.network.calls.lock().unwrap().clone();
    assert!(
        calls.iter().all(|url| !url.contains("releases/download")),
        "a check must not download: {calls:?}"
    );
}

#[test]
fn a_task_records_when_it_started_and_ended_and_the_versions_it_moves() {
    let mut queue = TaskQueue::new();
    let id = queue.begin(TaskKind::Update, "Updating", "Demo", false);
    let running = queue.get(&id).unwrap();
    assert!(running.started_at > 0, "a started task has a start time");
    assert_eq!(running.finished_at, 0, "a running task has no end time");

    queue.set_versions(&id, "3.1.0", "3.2.0");
    queue.set_phase(&id, UpdatePhase::Verify, 0, 0);
    queue.finish(&id, Ok(()), false);

    let done = queue.get(&id).unwrap();
    assert!(done.finished_at > 0 && done.finished_at >= done.started_at);
    assert_eq!(done.from_version, "3.1.0");
    assert_eq!(done.to_version, "3.2.0");
    assert_eq!(done.phase_index, 2);
    assert_eq!(done.phase, "Verify");
}

#[test]
fn a_task_shows_the_bytes_of_its_download_stage() {
    let mut queue = TaskQueue::new();
    let id = queue.begin(TaskKind::Update, "Updating", "Demo", false);
    queue.set_phase(&id, UpdatePhase::Download, 43_201_331, 69_206_016);
    let item = queue.get(&id).unwrap();
    assert_eq!(item.phase_index, 1);
    assert_eq!(item.phase, "Download");
    assert_eq!(
        (item.bytes_done, item.bytes_total),
        (43_201_331, 69_206_016)
    );
}
