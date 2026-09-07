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
