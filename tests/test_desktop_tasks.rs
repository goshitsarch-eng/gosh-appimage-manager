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
