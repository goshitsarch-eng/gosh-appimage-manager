// Audit finding C-10: settings failures were invisible.
//
// load() returned silently on any read or parse error, so a corrupt file reset
// every setting to its default with no notice -- and the next change
// overwrote the original, destroying the user's configuration. save()
// discarded every I/O error, so a setting that failed to persist looked
// exactly like one that saved and reappeared at its old value on next launch.

mod common;

use common::Harness;
use goshaim_core::settings::{Dirs, SettingsStore};

#[test]
fn a_corrupt_settings_file_is_reported_and_left_alone() {
    let h = Harness::new();
    let config = h.tmp.path().join(".config/gosh-appimage-manager");
    std::fs::create_dir_all(&config).unwrap();
    let path = config.join("settings.json");
    std::fs::write(&path, b"{ this is not json").unwrap();

    let store = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert!(
        store.load_error().is_some(),
        "a corrupt settings file must be reported, not silently ignored"
    );
    assert!(!store.loaded(), "nothing was successfully read");
    assert!(
        store.load_error().unwrap().contains("not valid JSON"),
        "the message should say what is wrong: {:?}",
        store.load_error()
    );

    // Defaults are in effect, and the original file is untouched until the
    // user actually changes something.
    assert!(!store.move_source());
    assert_eq!(std::fs::read(&path).unwrap(), b"{ this is not json");
}

#[test]
fn a_readable_settings_file_loads_without_error() {
    let h = Harness::new();
    let config = h.tmp.path().join(".config/gosh-appimage-manager");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("settings.json"),
        br#"{"MoveSource":true,"Appearance":"dark"}"#,
    )
    .unwrap();

    let store = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert!(store.load_error().is_none());
    assert!(store.loaded());
    assert!(store.move_source());
    assert_eq!(
        goshaim_core::types::appearance_name(store.appearance()),
        "dark"
    );
}

/// An absent file is not an error; it is the first run.
#[test]
fn a_missing_settings_file_is_not_an_error() {
    let h = Harness::new();
    let store = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert!(store.load_error().is_none());
    assert!(!store.loaded());
}

#[test]
fn a_failed_save_is_reported_to_the_caller() {
    let h = Harness::new();
    let mut store = SettingsStore::new(Dirs::under(h.tmp.path()));

    // A successful save first, so the failure is clearly the change and not
    // the setup.
    store.set_move_source(true).expect("saving should succeed");
    assert!(store.move_source());

    // Make the config path unwritable by putting a directory where the file
    // needs to go; the atomic rename cannot replace a non-empty directory.
    let config = h.tmp.path().join(".config/gosh-appimage-manager");
    std::fs::remove_file(config.join("settings.json")).unwrap();
    std::fs::create_dir_all(config.join("settings.json/blocker")).unwrap();

    let error = store
        .set_move_source(false)
        .expect_err("a save that cannot write must report it");
    assert!(!error.is_empty(), "the failure must carry a reason");
}

/// The unsafe fallback is opt-in. It is off in a fresh home, the owner can turn it
/// on, and the choice is kept across a restart.
#[test]
fn the_unsafe_extraction_fallback_is_off_by_default_and_can_be_turned_on() {
    let h = Harness::new();
    let mut store = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert!(!store.unsafe_extraction_fallback(), "off in a fresh home");

    store
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    assert!(store.unsafe_extraction_fallback());
    let restarted = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert!(restarted.unsafe_extraction_fallback(), "the choice is kept");

    store.set_unsafe_extraction_fallback(false).unwrap();
    assert!(!SettingsStore::new(Dirs::under(h.tmp.path())).unsafe_extraction_fallback());
}

fn config_dir(h: &Harness) -> std::path::PathBuf {
    let config = h.tmp.path().join(".config/gosh-appimage-manager");
    std::fs::create_dir_all(&config).unwrap();
    config
}

fn corrupt_backups(config: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(config)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("settings.json.corrupt-"))
        })
        .collect()
}

#[test]
fn the_first_save_after_a_corrupt_load_keeps_the_original_bytes() {
    let h = Harness::new();
    let config = config_dir(&h);
    let path = config.join("settings.json");
    let original = b"{ this is not json";
    std::fs::write(&path, original).unwrap();

    let mut store = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert!(store.load_error().is_some());
    store
        .set_move_source(true)
        .expect("the first change is saved");

    let backups = corrupt_backups(&config);
    assert_eq!(backups.len(), 1, "one backup: {backups:?}");
    assert_eq!(std::fs::read(&backups[0]).unwrap(), original);

    let fresh = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert!(fresh.load_error().is_none(), "{:?}", fresh.load_error());
    assert!(fresh.move_source(), "the new settings are on disk");

    // A later change must not move the (now valid) file again.
    let mut again = SettingsStore::new(Dirs::under(h.tmp.path()));
    again.set_move_source(false).unwrap();
    assert_eq!(corrupt_backups(&config).len(), 1);
}

#[test]
fn invalid_utf8_settings_are_reported_as_utf8() {
    let h = Harness::new();
    let config = config_dir(&h);
    std::fs::write(
        config.join("settings.json"),
        b"{\"DebugLogging\": \"\xff\xfe\"}",
    )
    .unwrap();

    let store = SettingsStore::new(Dirs::under(h.tmp.path()));
    let error = store.load_error().expect("reported");
    assert!(error.contains("UTF-8"), "message: {error}");
}

/// Keys this version does not know belong to a newer build or a user's own
/// edits. They survive a save, and the known settings still change.
#[test]
fn unknown_settings_keys_survive_a_save() {
    let h = Harness::new();
    let config = config_dir(&h);
    let path = config.join("settings.json");
    std::fs::write(
        &path,
        br#"{"ManagedFolder": "/srv/apps", "FutureSetting": {"a": [1, 2]}, "MoveSource": false}"#,
    )
    .unwrap();

    let mut store = SettingsStore::new(Dirs::under(h.tmp.path()));
    store.set_move_source(true).unwrap();

    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).expect("valid JSON");
    assert_eq!(saved["FutureSetting"], serde_json::json!({"a": [1, 2]}));
    assert_eq!(saved["MoveSource"], serde_json::json!(true));
    assert_eq!(saved["ManagedFolder"], serde_json::json!("/srv/apps"));
}

/// A file that exists but cannot be read is not a file we may replace.
#[test]
fn an_unreadable_settings_file_is_not_overwritten() {
    use std::os::unix::fs::PermissionsExt;

    let h = Harness::new();
    let config = config_dir(&h);
    let path = config.join("settings.json");
    std::fs::write(&path, b"{\"MoveSource\": true}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&path).is_ok() {
        // Root reads mode-000 files; the permission check cannot be exercised.
        eprintln!("SKIPPED an_unreadable_settings_file_is_not_overwritten: running as root");
        return;
    }

    let mut store = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert!(store.load_error().is_some());
    assert!(
        store.set_move_source(false).is_err(),
        "the save must be refused"
    );

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"{\"MoveSource\": true}");
    assert!(corrupt_backups(&config).is_empty());
}

/// The managed folder choice is written when it is made and read back by a
/// new process (a new store), not only held in memory.
#[test]
fn the_managed_folder_choice_survives_a_restart() {
    let h = Harness::new();
    let chosen = h.tmp.path().join("my-apps");
    std::fs::create_dir_all(&chosen).unwrap();
    let mut store = SettingsStore::new(Dirs::under(h.tmp.path()));
    store.set_managed_folder(chosen.clone()).unwrap();

    let restarted = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert_eq!(restarted.managed_folder(), chosen.as_path());
}

/// Audit R6-02: a file is not a managed folder. It is refused with the reason,
/// and nothing is saved.
#[test]
fn a_regular_file_is_refused_as_the_managed_folder_and_not_saved() {
    let h = Harness::new();
    let config_file = h
        .tmp
        .path()
        .join(".config/gosh-appimage-manager/settings.json");
    let mut store = SettingsStore::new(Dirs::under(h.tmp.path()));
    let in_use = store.managed_folder().to_path_buf();

    let a_file = h.tmp.path().join("not-a-folder");
    std::fs::write(&a_file, b"x").unwrap();
    let error = store
        .set_managed_folder(a_file)
        .expect_err("a regular file is refused");
    assert!(
        error.contains("must be an existing directory"),
        "the reason is named: {error}"
    );
    assert_eq!(store.managed_folder(), in_use.as_path());
    assert!(!config_file.exists(), "nothing is saved for a refused file");
}

/// Audit R6-02: a missing path and a relative path are refused the same way.
#[test]
fn a_missing_or_relative_managed_folder_is_refused_and_not_saved() {
    let h = Harness::new();
    let config_file = h
        .tmp
        .path()
        .join(".config/gosh-appimage-manager/settings.json");
    let mut store = SettingsStore::new(Dirs::under(h.tmp.path()));
    let in_use = store.managed_folder().to_path_buf();

    let missing = h.tmp.path().join("never-created");
    let error = store
        .set_managed_folder(missing)
        .expect_err("a missing folder is refused");
    assert!(
        error.contains("must be an existing directory"),
        "the reason is named: {error}"
    );

    let error = store
        .set_managed_folder(std::path::PathBuf::from("relative/AppImages"))
        .expect_err("a relative path is refused");
    assert!(error.contains("absolute"), "the reason is named: {error}");

    assert_eq!(store.managed_folder(), in_use.as_path());
    assert!(
        !config_file.exists(),
        "nothing is saved for a refused folder"
    );
}

/// Audit R6-02 and R6-05 (core side): an existing absolute directory is saved,
/// and a new store reads it back.
#[test]
fn an_existing_absolute_directory_is_saved_and_survives_a_restart() {
    let h = Harness::new();
    let folder = h.tmp.path().join("AppImages-archive");
    std::fs::create_dir_all(&folder).unwrap();
    let mut store = SettingsStore::new(Dirs::under(h.tmp.path()));
    store
        .set_managed_folder(folder.clone())
        .expect("an existing absolute directory is accepted");

    let restarted = SettingsStore::new(Dirs::under(h.tmp.path()));
    assert_eq!(restarted.managed_folder(), folder.as_path());
}
