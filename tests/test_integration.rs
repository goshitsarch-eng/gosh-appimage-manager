mod common;

use common::{plant_self_extraction, ran_self_extraction, write_fixture, Harness};
use goshaim_core::types::{ConflictPolicy, CopyMode, IntegrateRequest};
use std::sync::atomic::AtomicBool;

fn req(path: &str) -> IntegrateRequest {
    IntegrateRequest {
        source_path: path.to_string(),
        conflict: ConflictPolicy::Unspecified,
        replace_uuid: String::new(),
        copy_mode: CopyMode::Copy,
        assume_yes: true,
        confirm_unsafe: false,
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

/// The integration records the folder its source was in, and a reopened
/// registry reads the same folder back from disk.
#[test]
fn integration_stores_the_source_folder_and_reads_it_back() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let downloads = h.tmp.path().join("Downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let path = write_fixture(&downloads, "Quill.AppImage");
    let result = c.integrate(&req(path.to_str().unwrap()), &cancel);
    assert!(result.ok, "error: {}", result.error);
    let folder = downloads.to_str().unwrap();
    assert_eq!(result.app.integrated_folder, folder);
    drop(c);

    let registry_path = h.dirs().app_data_dir().join("registry.sqlite");
    let reopened = goshaim_core::registry::ManagedRegistry::open(&registry_path).unwrap();
    assert_eq!(
        reopened
            .by_uuid(&result.app.uuid)
            .unwrap()
            .integrated_folder,
        folder
    );
}

/// A replace records the integration that replaced the file: its date is the
/// time of the replace, and its folder is the replacing source's folder. The
/// date and folder of the install it replaces do not carry over.
#[test]
fn a_replace_records_the_replacing_integration() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let downloads = h.tmp.path().join("Downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let first = c.integrate(
        &req(write_fixture(&downloads, "Quill.AppImage")
            .to_str()
            .unwrap()),
        &cancel,
    );
    assert!(first.ok, "error: {}", first.error);
    // The install being replaced was integrated long ago, from another folder.
    let mut old = c.registry().by_uuid(&first.app.uuid).unwrap();
    old.integrated_at = 1_000_000_000;
    old.integrated_folder = "/somewhere/else".to_string();
    c.registry_mut().upsert(old).unwrap();

    let desktop = h.tmp.path().join("Desktop");
    std::fs::create_dir_all(&desktop).unwrap();
    let mut replace = req(write_fixture(&desktop, "Quill.AppImage").to_str().unwrap());
    replace.conflict = ConflictPolicy::Replace;
    replace.replace_uuid = first.app.uuid.clone();
    let before = goshaim_core::tasks::now_unix();
    let second = c.integrate(&replace, &cancel);
    assert!(second.ok, "error: {}", second.error);
    assert!(
        second.app.integrated_at >= before,
        "the date is the replacing integration's: {} < {before}",
        second.app.integrated_at
    );
    assert_eq!(second.app.integrated_folder, desktop.to_str().unwrap());
    let stored = c.registry().by_uuid(&second.app.uuid).unwrap();
    assert_eq!(stored.integrated_at, second.app.integrated_at);
    assert_eq!(stored.integrated_folder, desktop.to_str().unwrap());
}

/// A replace of an app with no recorded date records the replacing integration
/// too: the same rule as any other replace.
#[test]
fn a_replace_of_an_undated_app_records_the_replacing_integration() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let downloads = h.tmp.path().join("Downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let first = c.integrate(
        &req(write_fixture(&downloads, "Quill.AppImage")
            .to_str()
            .unwrap()),
        &cancel,
    );
    assert!(first.ok, "error: {}", first.error);
    // An entry from before the date and folder were recorded.
    let mut undated = c.registry().by_uuid(&first.app.uuid).unwrap();
    undated.integrated_at = 0;
    undated.integrated_folder = String::new();
    c.registry_mut().upsert(undated).unwrap();

    let desktop = h.tmp.path().join("Desktop");
    std::fs::create_dir_all(&desktop).unwrap();
    let mut replace = req(write_fixture(&desktop, "Quill.AppImage").to_str().unwrap());
    replace.conflict = ConflictPolicy::Replace;
    replace.replace_uuid = first.app.uuid.clone();
    let before = goshaim_core::tasks::now_unix();
    let second = c.integrate(&replace, &cancel);
    assert!(second.ok, "error: {}", second.error);
    assert!(second.app.integrated_at >= before && second.app.integrated_at > 0);
    assert_eq!(second.app.integrated_folder, desktop.to_str().unwrap());
}

/// The name the user sees keeps the spaces and words of the file name. The
/// managed copy's file name is sanitised, but that must not leak into the
/// registry name or the desktop entry.
#[test]
fn display_name_keeps_the_spaces_of_the_source_file_name() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    for (file, shown) in [
        ("Gamma Editor.AppImage", "Gamma Editor"),
        ("Quill Notes 1.2.0.AppImage", "Quill Notes 1.2.0"),
    ] {
        let path = write_fixture(&src, file);
        let result = c.integrate(&req(path.to_str().unwrap()), &cancel);
        assert!(result.ok, "error: {}", result.error);
        assert_eq!(result.app.name, shown);
        assert!(
            result
                .app
                .managed_path
                .ends_with(&format!("{}.AppImage", shown.replace(' ', "_"))),
            "the managed copy keeps the sanitised file name: {}",
            result.app.managed_path
        );
        let desktop = std::fs::read_to_string(&result.app.desktop_path).unwrap();
        assert!(
            desktop.lines().any(|line| line == format!("Name={shown}")),
            "desktop entry: {desktop}"
        );
    }
}

#[test]
fn display_name_drops_only_the_appimage_suffix() {
    use goshaim_core::desktop::display_name_from_file;
    assert_eq!(
        display_name_from_file("Gamma Editor.AppImage"),
        "Gamma Editor"
    );
    assert_eq!(
        display_name_from_file("Quill Notes 1.2.0.appimage"),
        "Quill Notes 1.2.0"
    );
    assert_eq!(
        display_name_from_file("Notes.AppImage.AppImage"),
        "Notes.AppImage"
    );
    assert_eq!(display_name_from_file("Plain"), "Plain");
    assert_eq!(display_name_from_file(".AppImage"), "AppImage");
}

/// The stored setting decides for an integration too. With it on, an AppImage whose
/// metadata safe extraction cannot read is run with its own --appimage-extract, and
/// the result says so.
#[test]
fn an_integration_uses_the_unsafe_fallback_when_the_setting_is_on() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let log = plant_self_extraction(&h);
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let path = write_fixture(&src, "Demo.AppImage");

    let mut request = req(path.to_str().unwrap());
    request.confirm_unsafe = true;
    let result = c.integrate(&request, &cancel);

    assert!(result.ok, "error: {}", result.error);
    assert!(ran_self_extraction(&log), "the fallback did not run");
    assert_eq!(
        result.app.name, "Unpacked",
        "warnings: {:?}",
        result.warnings
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("Unsafe extraction fallback used") && w.contains("executed")),
        "the result must say the AppImage ran: {:?}",
        result.warnings
    );
}

/// With the setting off, the same integration does not run the AppImage. It
/// completes from the file name, and the result says the fallback is off.
#[test]
fn an_integration_refuses_the_unsafe_fallback_while_the_setting_is_off() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let log = plant_self_extraction(&h);
    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let path = write_fixture(&src, "Demo.AppImage");

    let result = c.integrate(&req(path.to_str().unwrap()), &cancel);

    assert!(result.ok, "error: {}", result.error);
    assert!(
        !ran_self_extraction(&log),
        "the AppImage ran while the setting is off"
    );
    assert_eq!(result.app.name, "Demo");
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("fallback is off")),
        "warnings: {:?}",
        result.warnings
    );
}

/// A file that needs the fallback and is not confirmed is not integrated: nothing
/// is installed, the source stays where it is, and MoveSource does not run.
#[test]
fn a_pending_integration_installs_nothing_and_keeps_the_source() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let log = plant_self_extraction(&h);
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    c.settings_mut()
        .set_move_source(true)
        .expect("MoveSource can be set");
    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let path = write_fixture(&src, "Demo.AppImage");
    let mut request = req(path.to_str().unwrap());
    request.copy_mode = CopyMode::Move;

    let result = c.integrate(&request, &cancel);

    assert!(!result.ok, "a pending file must not report success");
    assert!(result.fallback_pending, "error: {}", result.error);
    assert!(
        !ran_self_extraction(&log),
        "the AppImage ran before confirmation"
    );
    assert!(c.registry().apps().is_empty(), "nothing may be registered");
    assert!(path.exists(), "the source must stay where it is");
    assert!(!result.source_removed);
    assert!(
        h.trash.trashed.lock().unwrap().is_empty(),
        "MoveSource must not run"
    );
    assert!(!h
        .tmp
        .path()
        .join("AppImages")
        .join("Demo.AppImage")
        .exists());
}

/// Once the user confirms the file, it is integrated as before.
#[test]
fn a_confirmed_integration_installs_it() {
    let h = Harness::new();
    let mut c = h.controller();
    let cancel = AtomicBool::new(false);
    let log = plant_self_extraction(&h);
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let src = h.tmp.path().join("incoming");
    std::fs::create_dir_all(&src).unwrap();
    let path = write_fixture(&src, "Demo.AppImage");
    let mut request = req(path.to_str().unwrap());
    request.confirm_unsafe = true;

    let result = c.integrate(&request, &cancel);

    assert!(result.ok, "error: {}", result.error);
    assert!(!result.fallback_pending);
    assert!(ran_self_extraction(&log));
    assert_eq!(c.registry().apps().len(), 1);
}
