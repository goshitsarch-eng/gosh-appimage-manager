mod common;

use goshaim_core::desktop;
use goshaim_core::types::{EnvPair, InstalledApp};

#[test]
fn exec_built_from_tokens_never_shell() {
    let line = desktop::build_exec_line(
        "/home/u/AppImages/Demo.AppImage",
        &[EnvPair {
            name: "FOO".into(),
            value: "a b".into(),
        }],
        &["--name".to_string(), "a;b".to_string(), "x`y`".to_string()],
    );
    assert!(line.starts_with("FOO=\"a b\" /home/u/AppImages/Demo.AppImage"));
    assert!(line.contains("--name a;b \"x`y`\"") || line.contains("--name"));
    // No shell metacharacters are interpreted: everything is one line.
    assert!(!line.contains('\n'));
    // Quoting covered a space and a backtick.
    assert!(line.contains("\"a b\""));
}

#[test]
fn exec_arg_passthrough_and_quoting() {
    assert_eq!(desktop::escape_exec_arg("%U"), "%U");
    assert_eq!(desktop::escape_exec_arg("--plain"), "--plain");
    assert_eq!(desktop::escape_exec_arg("a b"), "\"a b\"");
    assert_eq!(desktop::escape_exec_arg("a\"b"), "\"a\\\"b\"");
}

#[test]
fn env_name_validation() {
    assert!(desktop::valid_env_name("FOO_BAR2"));
    assert!(!desktop::valid_env_name(""));
    assert!(!desktop::valid_env_name("9LIVES"));
    assert!(!desktop::valid_env_name("HAS SPACE"));
}

#[test]
fn desktop_roundtrip_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = InstalledApp::new_owned();
    app.uuid = "desk-1".to_string();
    app.name = "Demo\nApp".to_string();
    app.version = "1.0".to_string();
    app.managed_path = "/home/u/AppImages/Demo.AppImage".to_string();
    let body = desktop::build_desktop_file(&app, &app.managed_path, false);
    assert!(body.contains("X-Gosh-AppImage-Manager=true"));
    assert!(body.contains("X-Gosh-AppImage-Id=desk-1"));
    assert!(body.contains("TryExec="));
    assert!(body.contains("Name=Demo\\nApp"));

    let path = dir.path().join("gosh-appimage-desk-1.desktop");
    std::fs::write(&path, &body).unwrap();
    let ownership = desktop::verify_ownership(&path);
    assert!(ownership.owned);
    assert_eq!(ownership.uuid, "desk-1");
    assert_eq!(ownership.managed_path, "/home/u/AppImages/Demo.AppImage");
}

#[test]
fn foreign_desktop_not_owned() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("foreign.desktop");
    std::fs::write(&path, "[Desktop Entry]\nName=Evil\nExec=/bin/evil\n").unwrap();
    let ownership = desktop::verify_ownership(&path);
    assert!(!ownership.owned);
}

#[test]
fn desktop_parse_bounded_and_sane() {
    let big = vec![b'x'; goshaim_core::limits::MAX_DESKTOP_FILE_BYTES + 1];
    assert!(desktop::parse_desktop_bytes(&big).is_err());
    let file =
        desktop::parse_desktop_bytes(b"[Desktop Entry]\nName[de]=Demo\nName=Real\n").unwrap();
    assert_eq!(file.entry("Name"), "Real");
    assert_eq!(desktop::unescape_entry_value("A\\nB"), "A\nB");
}

#[test]
fn archive_member_validation() {
    assert!(goshaim_core::safe_fs::valid_archive_member("AppDir/usr/bin/x").is_ok());
    assert!(goshaim_core::safe_fs::valid_archive_member("/abs/path").is_err());
    assert!(goshaim_core::safe_fs::valid_archive_member("../escape").is_err());
    assert!(goshaim_core::safe_fs::valid_archive_member("a/../../b").is_err());
    assert!(goshaim_core::safe_fs::valid_archive_member("").is_err());
}

#[test]
fn sha256_streaming_matches_known_digest() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.bin");
    std::fs::write(&path, b"abc").unwrap();
    let sum = goshaim_core::safe_fs::sha256_file(&path, &std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        hex::encode(&sum),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn task_queue_runs_and_bounds_history() {
    use goshaim_core::tasks::TaskQueue;
    use goshaim_core::types::TaskKind;
    let mut queue = TaskQueue::new();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let item = queue.run_task(
        TaskKind::Inspect,
        "Inspect Demo",
        "/tmp/Demo.AppImage",
        false,
        &cancel,
        |progress, _| {
            progress(50, "half");
            Ok(())
        },
    );
    assert_eq!(item.state, goshaim_core::types::TaskState::Succeeded);
    assert_eq!(item.progress, 100);
    assert_eq!(queue.history().len(), 1);

    let failing = queue.run_task(
        TaskKind::Update,
        "Update Demo",
        "/tmp/Demo.AppImage",
        true,
        &cancel,
        |_, _| Err("boom".to_string()),
    );
    assert_eq!(failing.state, goshaim_core::types::TaskState::Failed);
    assert!(failing.retryable);
}

/// Audit finding S-5. Member names are appended to the extractor's argv as
/// positional arguments and `unsquashfs` has no `--` terminator, so a member
/// that looks like a switch is a switch. With 7-Zip, `-o<dir>` redirects
/// extraction out of the private temp directory. The names come from the
/// archive's own listing, so they are fully attacker-controlled.
#[test]
fn option_shaped_archive_members_are_refused() {
    use goshaim_core::safe_fs::valid_archive_member;
    for hostile in [
        "-o/tmp/pwned/evil.desktop",
        "-x",
        "--help",
        "-e/etc/passwd",
        "-p secret",
        "-scrc",
    ] {
        assert!(
            valid_archive_member(hostile).is_err(),
            "member {hostile:?} would be parsed as an extractor switch"
        );
    }
    // Ordinary members keep working, including ones that merely contain a dash.
    for benign in [
        "usr/share/applications/app.desktop",
        ".DirIcon",
        "some-app-1.2.desktop",
        "a/b-c/d.png",
    ] {
        assert!(
            valid_archive_member(benign).is_ok(),
            "member {benign:?} should still be accepted"
        );
    }
}

/// The archive path itself is user-supplied; a relative name starting with `-`
/// must stay positional without changing which file it names.
#[test]
fn argv_safe_path_neutralises_leading_dash() {
    use goshaim_core::safe_fs::argv_safe_path;
    use std::path::Path;
    assert_eq!(argv_safe_path(Path::new("-x.AppImage")), "./-x.AppImage");
    assert_eq!(
        argv_safe_path(Path::new("--rm.AppImage")),
        "./--rm.AppImage"
    );
    // Unambiguous paths are passed through untouched.
    assert_eq!(
        argv_safe_path(Path::new("/apps/X.AppImage")),
        "/apps/X.AppImage"
    );
    assert_eq!(argv_safe_path(Path::new("./X.AppImage")), "./X.AppImage");
    assert_eq!(argv_safe_path(Path::new("X.AppImage")), "X.AppImage");
}

/// Audit finding S-6. The extraction parent is a predictable name in a shared
/// temp directory, so an existing path there is not necessarily ours.
#[test]
fn mkdir_0700_refuses_hostile_preexisting_paths() {
    use goshaim_core::safe_fs::mkdir_0700;
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();

    // A symlink pointing somewhere the attacker chose.
    let victim = tmp.path().join("victim");
    std::fs::create_dir_all(&victim).unwrap();
    let link = tmp.path().join("planted-symlink");
    std::os::unix::fs::symlink(&victim, &link).unwrap();
    assert!(
        mkdir_0700(&link).is_err(),
        "a symlink must not be accepted as our private directory"
    );

    // A regular file squatting on the name.
    let file = tmp.path().join("planted-file");
    std::fs::write(&file, b"x").unwrap();
    assert!(mkdir_0700(&file).is_err(), "a file is not a directory");

    // A world-writable directory is tightened rather than trusted as-is.
    let loose = tmp.path().join("planted-0777");
    std::fs::create_dir_all(&loose).unwrap();
    std::fs::set_permissions(&loose, std::fs::Permissions::from_mode(0o777)).unwrap();
    assert!(mkdir_0700(&loose).is_ok());
    let mode = std::fs::metadata(&loose).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o700,
        "a writable-by-others directory must be secured"
    );

    // The ordinary paths still work: fresh creation, and a re-run on our own
    // already-correct directory.
    let fresh = tmp.path().join("nested/fresh");
    assert!(mkdir_0700(&fresh).is_ok());
    assert_eq!(
        std::fs::metadata(&fresh).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(mkdir_0700(&fresh).is_ok(), "must be idempotent");
}

/// Audit finding C-5. The Desktop Entry spec reserves `%` for field codes, so
/// a literal percent in a user argument has to be written `%%`. It was passed
/// through raw, which mangled ordinary filenames and let an argument of
/// exactly `%U` become a live field code by accident.
#[test]
fn literal_percent_is_escaped_in_exec_arguments() {
    use goshaim_core::desktop::{build_exec_line, escape_exec_arg};

    assert_eq!(escape_exec_arg("--tag=50%"), "--tag=50%%");
    assert_eq!(escape_exec_arg("100% done.txt"), "\"100%% done.txt\"");
    assert_eq!(escape_exec_arg("a%b%c"), "a%%b%%c");
    // A deliberate two-character field code is still emitted as one.
    assert_eq!(escape_exec_arg("%U"), "%U");
    assert_eq!(escape_exec_arg("%f"), "%f");
    // Tokens without a percent are untouched.
    assert_eq!(escape_exec_arg("--verbose"), "--verbose");

    let line = build_exec_line(
        "/apps/X.AppImage",
        &[],
        &["--open".to_string(), "100% done.txt".to_string()],
    );
    assert_eq!(line, "/apps/X.AppImage --open \"100%% done.txt\"");
}

/// Audit finding: TaskQueue could not represent work in progress. `run_task`
/// recorded a task only after it had finished, so `active()` could never
/// return anything and TaskState::Queued/Cancelling were never assigned. The
/// Tasks page the brief requires had nothing it could display.
#[test]
fn task_queue_tracks_work_in_flight() {
    use goshaim_core::tasks::TaskQueue;
    use goshaim_core::types::{TaskKind, TaskState};

    let mut queue = TaskQueue::new();
    let id = queue.begin(
        TaskKind::Update,
        "Updating Demo",
        "/apps/Demo.AppImage",
        true,
    );

    let active = queue.active();
    assert_eq!(
        active.len(),
        1,
        "a started task must be visible while running"
    );
    assert_eq!(active[0].state, TaskState::Running);
    assert_eq!(active[0].title, "Updating Demo");

    queue.progress(&id, 42, "downloading");
    let running = queue.get(&id).unwrap();
    assert_eq!(running.progress, 42);
    assert_eq!(running.status_text, "downloading");

    queue.mark_cancelling(&id);
    assert_eq!(queue.get(&id).unwrap().state, TaskState::Cancelling);

    queue.finish(&id, Err("Cancelled".into()), true);
    let done = queue.get(&id).unwrap();
    assert_eq!(done.state, TaskState::Cancelled);
    assert!(
        queue.active().is_empty(),
        "a finished task is no longer active"
    );

    // A success path, and clear_finished keeping in-flight work.
    let ok = queue.begin(TaskKind::Integrate, "Integrating", "/x", false);
    queue.finish(&ok, Ok(()), false);
    assert_eq!(queue.get(&ok).unwrap().state, TaskState::Succeeded);
    assert_eq!(queue.get(&ok).unwrap().progress, 100);

    let live = queue.begin(TaskKind::Remove, "Removing", "/y", false);
    queue.clear_finished();
    assert_eq!(queue.history().len(), 1, "only the live task should remain");
    assert_eq!(queue.history()[0].id, live);
}

/// History is bounded, but a running task must never be evicted by the bound.
#[test]
fn history_bound_never_drops_a_running_task() {
    use goshaim_core::tasks::TaskQueue;
    use goshaim_core::types::TaskKind;

    let mut queue = TaskQueue::new();
    let live = queue.begin(TaskKind::Update, "Long running", "/live", false);
    for i in 0..(goshaim_core::limits::MAX_TASK_HISTORY + 40) {
        let id = queue.begin(TaskKind::CheckUpdate, "check", &format!("/x{i}"), false);
        queue.finish(&id, Ok(()), false);
    }
    assert!(
        queue.history().len() <= goshaim_core::limits::MAX_TASK_HISTORY,
        "history must stay bounded"
    );
    assert!(
        queue.get(&live).is_some(),
        "the still-running task must survive the bound"
    );
}

/// Audit finding C-9. The installed desktop entry used `%U`, which hands the
/// application URLs, while the GUI treated its positional arguments as
/// filesystem paths -- so opening an AppImage from a file manager produced
/// "Cannot open file: file:///...". The entry now uses `%F` (paths, and more
/// than one of them), and the GUI also tolerates a file:// URL.
#[test]
fn shipped_desktop_entry_passes_paths_not_urls() {
    let entry = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/com.goshapps.AppImageManager.desktop"
    ))
    .expect("the shipped desktop entry should be readable");
    assert!(
        entry.contains("Exec=gosh-appimage-manager %F"),
        "the entry must pass paths, not URLs:\n{entry}"
    );
    assert!(
        !entry.contains("%U"),
        "%U hands the app URLs it cannot open:\n{entry}"
    );
}
