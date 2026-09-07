// Audit finding C-1: integration rollback.
//
// The IntegrateFailPoint seam existed but had no caller anywhere in src/ or
// tests/, so every injected-failure branch was unreachable and no test
// exercised rollback at any point in the transaction. test_integration.rs's
// `fail_point_rolls_back_without_touching_live` concedes in its own comment
// that it could not reach the seam and asserts something else instead.
//
// These tests drive the seam directly. Before the fix, a failure at
// RegistrySave left the committed AppImage in the managed folder *and* the new
// .desktop in the applications directory, with no registry row -- an orphan
// invisible to the app and unremovable through it, whose menu entry pointed at
// an untracked binary.

mod common;

use common::{write_fixture, Harness};
use goshaim_core::integration::IntegrationService;
use goshaim_core::registry::ManagedRegistry;
use goshaim_core::types::{
    ConflictPolicy, CopyMode, IntegrateFailPoint, IntegrateRequest, IntegrateResult,
};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

struct Fixture {
    _h: Harness,
    managed: PathBuf,
    apps_dir: PathBuf,
    registry_path: PathBuf,
}

fn names_in(dir: &std::path::Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn integrate_failing_at(point: IntegrateFailPoint) -> (Fixture, IntegrateResult, usize) {
    let h = Harness::new();
    let c = h.controller();
    let source = write_fixture(h.tmp.path(), "Demo.AppImage");
    let fixture = Fixture {
        managed: c.settings().managed_folder().to_path_buf(),
        apps_dir: c.settings().applications_dir(),
        registry_path: c.settings().registry_path(),
        _h: h,
    };

    let mut registry = ManagedRegistry::open(&fixture.registry_path).unwrap();
    let mut service = IntegrationService::new(c.settings(), c.runner(), c.trash());
    service.set_fail_point(point);
    let result = service.integrate(
        &mut registry,
        &IntegrateRequest {
            source_path: source.to_string_lossy().into_owned(),
            conflict: ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: CopyMode::Copy,
            assume_yes: true,
        },
        &AtomicBool::new(false),
    );
    let rows = registry.apps().len();
    (fixture, result, rows)
}

/// Every failure point must leave the filesystem as it was found.
#[test]
fn failure_at_any_point_leaves_nothing_behind() {
    for point in [
        IntegrateFailPoint::AfterStage,
        IntegrateFailPoint::DesktopWrite,
        IntegrateFailPoint::DesktopInstall,
        IntegrateFailPoint::RegistrySave,
    ] {
        let (fx, result, rows) = integrate_failing_at(point);
        assert!(!result.ok, "{point:?}: the injected failure should fail");
        assert_eq!(rows, 0, "{point:?}: no registry row may survive");

        let managed = names_in(&fx.managed);
        assert!(
            managed.is_empty(),
            "{point:?}: managed folder must be empty, found {managed:?}"
        );
        let apps = names_in(&fx.apps_dir);
        assert!(
            apps.is_empty(),
            "{point:?}: applications dir must be empty, found {apps:?}"
        );
    }
}

/// The rollback report has to be truthful: it used to come back empty even
/// when artifacts had been left on disk.
#[test]
fn rollback_reports_what_it_removed() {
    let (_fx, result, _) = integrate_failing_at(IntegrateFailPoint::RegistrySave);
    assert!(!result.ok);
    assert!(
        result
            .rolled_back
            .iter()
            .any(|p| p.ends_with("Demo.AppImage")),
        "the committed AppImage should be named in the rollback list, got {:?}",
        result.rolled_back
    );
    assert!(
        result.rolled_back.iter().any(|p| p.ends_with(".desktop")),
        "the installed desktop entry should be named in the rollback list, got {:?}",
        result.rolled_back
    );
}

/// A failed replace must put the previous version back, byte for byte, and
/// keep its registry row.
#[test]
fn failed_replace_restores_the_previous_installation() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);

    let first_src = write_fixture(h.tmp.path(), "Demo.AppImage");
    let first = c.integrate(
        &IntegrateRequest {
            source_path: first_src.to_string_lossy().into_owned(),
            conflict: ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: CopyMode::Copy,
            assume_yes: true,
        },
        &cancel,
    );
    assert!(first.ok, "setup integrate failed: {}", first.error);
    let live = PathBuf::from(&first.app.managed_path);
    let live_bytes = std::fs::read(&live).unwrap();
    let desktop_before = std::fs::read(&first.app.desktop_path).unwrap();

    // A different payload, so a successful replace would be visible.
    let second_dir = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&second_dir).unwrap();
    let second_src = common::write_fixture_arch(
        &second_dir,
        "Demo.AppImage",
        goshaim_core::types::Architecture::X86_64,
        goshaim_core::types::AppImageType::Type1,
    );

    let mut registry = ManagedRegistry::open(&c.settings().registry_path()).unwrap();
    let mut service = IntegrationService::new(c.settings(), c.runner(), c.trash());
    service.set_fail_point(IntegrateFailPoint::RegistrySave);
    let result = service.integrate(
        &mut registry,
        &IntegrateRequest {
            source_path: second_src.to_string_lossy().into_owned(),
            conflict: ConflictPolicy::Replace,
            replace_uuid: first.app.uuid.clone(),
            copy_mode: CopyMode::Copy,
            assume_yes: true,
        },
        &cancel,
    );
    assert!(!result.ok, "the injected failure should fail the replace");

    assert_eq!(
        std::fs::read(&live).unwrap(),
        live_bytes,
        "the previous AppImage must be restored byte for byte"
    );
    assert_eq!(
        std::fs::read(&first.app.desktop_path).unwrap(),
        desktop_before,
        "the previous desktop entry must be restored"
    );
    assert_eq!(
        registry.apps().len(),
        1,
        "the existing registry row must survive a failed replace"
    );
    let leftovers: Vec<String> = names_in(c.settings().managed_folder())
        .into_iter()
        .filter(|n| n.starts_with(".gosh-"))
        .collect();
    assert!(leftovers.is_empty(), "temp leftovers: {leftovers:?}");
}

/// Audit finding P-7. Rollback material is made with a hard link where the
/// filesystem allows it: the same bytes under a second name, which survives
/// the rename that replaces the original and costs neither space nor I/O.
/// Copying a multi-gigabyte AppImage to make a backup that is discarded
/// seconds later is pure waste.
#[test]
fn backup_material_survives_a_replacing_rename() {
    use goshaim_core::safe_fs::backup_copy;

    let tmp = tempfile::tempdir().unwrap();
    let live = tmp.path().join("App.AppImage");
    let backup = tmp.path().join(".gosh-bak-App");
    let original = b"the original payload".to_vec();
    std::fs::write(&live, &original).unwrap();

    backup_copy(&live, &backup).expect("making a backup should succeed");
    assert_eq!(std::fs::read(&backup).unwrap(), original);

    // Replace `live` the way the update path does.
    let staged = tmp.path().join(".gosh-upd-App");
    std::fs::write(&staged, b"the new payload").unwrap();
    std::fs::rename(&staged, &live).unwrap();

    // The backup must still hold the previous content: renaming replaces the
    // directory entry, not the inode the link points at.
    assert_eq!(
        std::fs::read(&backup).unwrap(),
        original,
        "the backup must not follow the replacement"
    );
    assert_eq!(std::fs::read(&live).unwrap(), b"the new payload".to_vec());

    // And restoring it puts the original back.
    std::fs::rename(&backup, &live).unwrap();
    assert_eq!(std::fs::read(&live).unwrap(), original);
}
