// Audit findings G-14 and related: the per-item detail actions the brief
// requires had no implementation behind them. These cover the core operations
// the new detail page drives, so they are tested without a display server.

mod common;

use common::{write_fixture, Harness};
use goshaim_core::types::{ConflictPolicy, CopyMode, EnvPair, IntegrateRequest};
use std::sync::atomic::AtomicBool;

fn integrate_one(h: &Harness) -> (goshaim_core::controller::AppController, String) {
    let mut c = h.controller();
    let source = write_fixture(h.tmp.path(), "Demo.AppImage");
    let result = c.integrate(
        &IntegrateRequest {
            source_path: source.to_string_lossy().into_owned(),
            conflict: ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: CopyMode::Copy,
            assume_yes: true,
        },
        &AtomicBool::new(false),
    );
    assert!(result.ok, "setup integrate failed: {}", result.error);
    let uuid = result.app.uuid.clone();
    (c, uuid)
}

/// Arguments are stored and written as separate tokens, and reach the entry.
#[test]
fn arguments_and_environment_are_saved_and_written_to_the_entry() {
    let h = Harness::new();
    let (mut c, uuid) = integrate_one(&h);

    c.set_arguments_and_environment(
        &uuid,
        vec!["--flag".into(), "two words".into()],
        vec![EnvPair {
            name: "MY_VAR".into(),
            value: "some value".into(),
        }],
    )
    .expect("saving valid arguments should succeed");

    let app = c.registry().by_uuid(&uuid).unwrap();
    assert_eq!(
        app.arguments,
        vec!["--flag".to_string(), "two words".to_string()]
    );
    assert_eq!(app.environment.len(), 1);

    let entry = std::fs::read_to_string(&app.desktop_path).unwrap();
    assert!(
        entry.contains("MY_VAR=\"some value\""),
        "environment should be written into Exec:\n{entry}"
    );
    assert!(
        entry.contains("--flag \"two words\""),
        "arguments should be separate quoted tokens:\n{entry}"
    );
}

/// Invalid environment names are refused rather than silently dropped.
#[test]
fn invalid_environment_names_are_refused() {
    let h = Harness::new();
    let (mut c, uuid) = integrate_one(&h);
    let before = c.registry().by_uuid(&uuid).unwrap();

    for bad in ["2LEADING", "has space", "has-dash", ""] {
        let error = c
            .set_arguments_and_environment(
                &uuid,
                vec![],
                vec![EnvPair {
                    name: bad.into(),
                    value: "x".into(),
                }],
            )
            .unwrap_err();
        assert!(
            error.contains("environment variable name"),
            "for {bad:?} got: {error}"
        );
    }
    // Nothing was written on the way to any of those errors.
    let after = c.registry().by_uuid(&uuid).unwrap();
    assert_eq!(after.environment, before.environment);
    assert_eq!(after.arguments, before.arguments);
}

#[test]
fn argument_limits_are_enforced() {
    let h = Harness::new();
    let (mut c, uuid) = integrate_one(&h);
    let too_many: Vec<String> = (0..(goshaim_core::limits::MAX_ARGUMENTS + 1))
        .map(|i| format!("--a{i}"))
        .collect();
    assert!(c
        .set_arguments_and_environment(&uuid, too_many, vec![])
        .unwrap_err()
        .contains("Too many arguments"));

    let overlong = "x".repeat(goshaim_core::limits::MAX_ARGUMENT_LENGTH + 1);
    assert!(c
        .set_arguments_and_environment(&uuid, vec![overlong], vec![])
        .unwrap_err()
        .contains("not usable"));
}

/// Refreshing re-reads the file and rewrites the entry we own.
#[test]
fn refresh_metadata_rereads_the_file_and_rewrites_the_entry() {
    let h = Harness::new();
    // Extraction reports a name and version the first integrate did not see.
    h.runner
        .canned("unsquashfs", 0, b"squashfs-root/demo.desktop\n");
    h.runner.on_run(Box::new(|req| {
        let dest = req
            .args
            .iter()
            .position(|a| a == "-d")
            .and_then(|i| req.args.get(i + 1))
            .map(std::path::PathBuf::from)?;
        let _ = std::fs::create_dir_all(&dest);
        let _ = std::fs::write(
            dest.join("demo.desktop"),
            b"[Desktop Entry]\nName=Refreshed\nX-AppImage-Version=4.2\nExec=demo\n",
        );
        None
    }));
    let (mut c, uuid) = integrate_one(&h);

    let name = c
        .refresh_metadata(&uuid, &AtomicBool::new(false))
        .expect("refresh should succeed");
    assert_eq!(name, "Refreshed");

    let app = c.registry().by_uuid(&uuid).unwrap();
    assert_eq!(app.version, "4.2");
    let entry = std::fs::read_to_string(&app.desktop_path).unwrap();
    assert!(entry.contains("Name=Refreshed"), "\n{entry}");
    assert!(entry.contains("X-AppImage-Version=4.2"), "\n{entry}");
}

#[test]
fn refresh_metadata_reports_a_missing_or_invalid_file() {
    let h = Harness::new();
    let (mut c, uuid) = integrate_one(&h);
    let app = c.registry().by_uuid(&uuid).unwrap();
    std::fs::write(&app.managed_path, b"no longer an appimage").unwrap();

    let error = c
        .refresh_metadata(&uuid, &AtomicBool::new(false))
        .unwrap_err();
    assert!(!error.is_empty(), "the failure must carry a reason");
    assert!(c
        .refresh_metadata("no-such-uuid", &AtomicBool::new(false))
        .is_err());
}
