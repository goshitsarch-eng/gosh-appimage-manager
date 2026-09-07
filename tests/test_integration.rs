mod common;

use common::{write_fixture, Harness};
use goshaim_core::types::{ConflictPolicy, CopyMode, IntegrateRequest};
use std::sync::atomic::AtomicBool;

fn req(path: &str) -> IntegrateRequest {
    IntegrateRequest {
        source_path: path.to_string(),
        conflict: ConflictPolicy::Unspecified,
        replace_uuid: String::new(),
        copy_mode: CopyMode::Copy,
        assume_yes: true,
    }
}

#[test]
fn copy_mode_keeps_source_move_trashes_source() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let path = write_fixture(&src, "Demo.AppImage");

    let mut move_req = req(path.to_str().unwrap());
    move_req.copy_mode = CopyMode::Move;
    let result = c.integrate(&move_req, &cancel);
    assert!(result.ok, "error: {}", result.error);
    assert!(result.source_removed);
    assert_eq!(h.trash.trashed.lock().unwrap().len(), 1);

    // Copy mode leaves the source alone.
    let path2 = write_fixture(&src, "Second.AppImage");
    let result = c.integrate(&req(path2.to_str().unwrap()), &cancel);
    assert!(result.ok, "error: {}", result.error);
    assert!(!result.source_removed);
    assert!(path2.exists());
}

#[test]
fn move_trash_failure_keeps_install_partial() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let path = write_fixture(&src, "Demo.AppImage");
    *h.trash.fail_next.lock().unwrap() = true;

    let mut move_req = req(path.to_str().unwrap());
    move_req.copy_mode = CopyMode::Move;
    let result = c.integrate(&move_req, &cancel);
    assert!(!result.ok);
    assert!(result.partial);
    assert!(result.error.contains("Trash failed; leaving source intact"));
    // Install is kept; source is intact.
    assert_eq!(c.registry().apps().len(), 1);
    assert!(path.exists());
}

#[test]
fn staged_copy_is_executable_and_verified() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let result = c.integrate(&req(path.to_str().unwrap()), &cancel);
    assert!(result.ok, "error: {}", result.error);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&result.app.managed_path)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
    assert_eq!(result.app.sha256.len(), 32);
    assert_eq!(result.app.size, 128);
}

#[test]
fn no_temp_leftovers_after_success() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let result = c.integrate(&req(path.to_str().unwrap()), &cancel);
    assert!(result.ok, "error: {}", result.error);
    let managed = c.settings().managed_folder().to_path_buf();
    let leftovers: Vec<_> = std::fs::read_dir(&managed)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".gosh-"))
        .collect();
    assert!(leftovers.is_empty(), "leftovers: {leftovers:?}");
}

/// A successful replace reuses the identity and customisation of the row it
/// replaces, and does not multiply registry rows.
///
/// Rollback at an injected failure point is covered separately, in
/// tests/test_rollback.rs, which drives the IntegrateFailPoint seam directly.
/// This test previously carried that name while conceding in its own comment
/// that it could not reach the seam.
#[test]
fn replace_reuses_identity_and_customisation() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let first = c.integrate(&req(path.to_str().unwrap()), &cancel);
    assert!(first.ok, "error: {}", first.error);

    // Give the installed row some customisation to preserve.
    let mut customised = c.registry().by_uuid(&first.app.uuid).unwrap();
    customised.arguments = vec!["--flag".to_string()];
    customised.update_manager = "github".to_string();
    c.registry_mut().upsert(customised).unwrap();

    let src2 = h.tmp.path().join("src2");
    std::fs::create_dir_all(&src2).unwrap();
    let path2 = write_fixture(&src2, "Demo.AppImage");
    let mut replace = req(path2.to_str().unwrap());
    replace.conflict = ConflictPolicy::Replace;
    replace.replace_uuid = first.app.uuid.clone();

    let second = c.integrate(&replace, &cancel);
    assert!(second.ok, "error: {}", second.error);
    assert_eq!(second.app.uuid, first.app.uuid, "identity must be reused");
    assert_eq!(
        second.app.arguments,
        vec!["--flag".to_string()],
        "custom arguments must survive a replace"
    );
    assert_eq!(second.app.update_manager, "github");
    assert_eq!(c.registry().apps().len(), 1, "replace must not add a row");
}
