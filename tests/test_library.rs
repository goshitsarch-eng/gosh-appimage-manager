// Audit finding: external discovery and adoption were implemented in
// library.rs but reachable from no CLI command and no GUI affordance, and the
// `manage_outside_folder` setting was persisted and read by nothing. An
// AppImage integrated by another tool could never be adopted.

mod common;

use common::{write_fixture, Harness};

fn write_foreign_entry(dir: &std::path::Path, name: &str, target: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    // A desktop entry written by something that is not us: no ownership markers.
    std::fs::write(
        dir.join(name),
        format!(
            "[Desktop Entry]\nType=Application\nName=Foreign\nExec={} %U\nTryExec={}\n",
            target.display(),
            target.display()
        ),
    )
    .unwrap();
}

#[test]
fn managed_folder_is_always_scanned() {
    let h = Harness::new();
    let c = h.controller();
    let managed = c.settings().managed_folder().to_path_buf();
    std::fs::create_dir_all(&managed).unwrap();
    write_fixture(&managed, "Inside.AppImage");

    let found = c.discover();
    assert_eq!(found.len(), 1, "found: {found:?}");
    assert_eq!(found[0].name, "Inside");
    assert!(!found[0].managed, "not in the registry yet");
    assert_eq!(
        found[0].origin,
        goshaim_core::library::Origin::ManagedFolder
    );
}

#[test]
fn external_entries_are_found_only_when_the_setting_is_on() {
    let h = Harness::new();
    let mut c = h.controller();
    let outside = h.tmp.path().join("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    let target = write_fixture(&outside, "Foreign.AppImage");
    write_foreign_entry(&c.settings().applications_dir(), "foreign.desktop", &target);

    // Off by default: only the managed folder is scanned.
    assert!(c.discover().is_empty(), "external discovery must be opt-in");

    c.settings_mut().set_manage_outside_folder(true);
    let found = c.discover();
    assert_eq!(found.len(), 1, "found: {found:?}");
    assert_eq!(found[0].path, target.to_string_lossy());
    assert_eq!(
        found[0].origin,
        goshaim_core::library::Origin::ExternalDesktopEntry
    );
    assert!(
        found[0].desktop_path.ends_with("foreign.desktop"),
        "the entry that named it should be reported"
    );
}

/// Our own entries are already in the registry; rediscovering them as
/// "external" would invite adopting the same app twice.
#[test]
fn our_own_entries_are_not_reported_as_external() {
    let h = Harness::new();
    let mut c = h.controller();
    c.settings_mut().set_manage_outside_folder(true);
    let source = write_fixture(h.tmp.path(), "Demo.AppImage");
    let result = c.integrate(
        &goshaim_core::types::IntegrateRequest {
            source_path: source.to_string_lossy().into_owned(),
            conflict: goshaim_core::types::ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: goshaim_core::types::CopyMode::Copy,
            assume_yes: true,
        },
        &std::sync::atomic::AtomicBool::new(false),
    );
    assert!(result.ok, "{}", result.error);

    let found = c.discover();
    assert_eq!(found.len(), 1, "found: {found:?}");
    assert!(
        found[0].managed,
        "the integrated app is known to the registry"
    );
    assert_eq!(
        found[0].origin,
        goshaim_core::library::Origin::ManagedFolder,
        "our own entry must not also be reported as an external find"
    );
}

/// Adoption registers the file and changes nothing on disk.
#[test]
fn adoption_registers_without_touching_anything() {
    let h = Harness::new();
    let mut c = h.controller();
    let outside = h.tmp.path().join("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    let target = write_fixture(&outside, "Foreign.AppImage");
    let entry_dir = c.settings().applications_dir();
    write_foreign_entry(&entry_dir, "foreign.desktop", &target);
    let entry_before = std::fs::read(entry_dir.join("foreign.desktop")).unwrap();
    let bytes_before = std::fs::read(&target).unwrap();

    let app = c.adopt_external(&target.to_string_lossy()).unwrap();
    assert!(app.adopted, "the row should record that it was adopted");
    assert!(app.owned, "an adopted app is manageable");
    assert!(app.external_folder, "it lives outside the managed folder");
    assert!(
        app.desktop_path.is_empty() && app.icon_path.is_empty(),
        "adoption must not claim artifacts it did not create"
    );

    assert_eq!(std::fs::read(&target).unwrap(), bytes_before);
    assert_eq!(
        std::fs::read(entry_dir.join("foreign.desktop")).unwrap(),
        entry_before,
        "the foreign desktop entry must be left exactly as it was"
    );
    assert_eq!(c.registry().apps().len(), 1);
}

#[test]
fn adoption_refuses_duplicates_and_non_appimages() {
    let h = Harness::new();
    let mut c = h.controller();
    let target = write_fixture(h.tmp.path(), "Foreign.AppImage");
    assert!(c.adopt_external(&target.to_string_lossy()).is_ok());
    assert!(
        c.adopt_external(&target.to_string_lossy()).is_err(),
        "adopting the same file twice must be refused"
    );

    let plain = h.tmp.path().join("notes.txt");
    std::fs::write(&plain, b"not an appimage").unwrap();
    assert!(c.adopt_external(&plain.to_string_lossy()).is_err());
    assert!(c.adopt_external("/nope/missing.AppImage").is_err());
}
