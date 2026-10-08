mod common;

use common::Harness;

#[test]
fn legacy_json_registry_imports_once() {
    let h = Harness::new();
    let data_dir = h.dirs().app_data_dir();
    std::fs::create_dir_all(&data_dir).unwrap();
    let legacy = serde_json::json!({
        "schema_version": 1,
        "apps": [
            {
                "uuid": "legacy-1",
                "name": "Legacy",
                "version": "0.9",
                "managed_path": "/home/u/AppImages/Legacy.AppImage",
                "desktop_id": "gosh-appimage-legacy-1.desktop",
                "type": "type-2",
                "architecture": "x86_64",
                "size": 1024,
                "arguments": ["--x"],
                "environment": {"FOO": "bar"},
                "update_manager": "github",
                "update_config": {"username": "gosh", "repo": "demo"},
                "owned": true
            },
            {"uuid": "", "name": "Skipped", "managed_path": ""},
            {"uuid": "legacy-1", "name": "Dupe", "managed_path": "/x.AppImage"}
        ]
    });
    std::fs::write(data_dir.join("registry.json"), legacy.to_string()).unwrap();

    let c = h.controller();
    let apps = c.registry().apps();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].uuid, "legacy-1");
    assert_eq!(apps[0].name, "Legacy");
    assert_eq!(apps[0].arguments, vec!["--x".to_string()]);
    assert_eq!(apps[0].environment.len(), 1);
    assert_eq!(apps[0].update_manager, "github");
    // Persisted to sqlite and reloaded without the legacy file.
    std::fs::remove_file(data_dir.join("registry.json")).unwrap();
    let c2 = h.controller();
    assert_eq!(c2.registry().apps().len(), 1);
}

#[test]
fn legacy_json_rejects_bad_schema() {
    let h = Harness::new();
    let data_dir = h.dirs().app_data_dir();
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(
        data_dir.join("registry.json"),
        r#"{"schema_version": 99, "apps": []}"#,
    )
    .unwrap();
    let registry_path = h.dirs().app_data_dir().join("registry.sqlite");
    let mut registry = goshaim_core::registry::ManagedRegistry::open(&registry_path).unwrap();
    assert!(registry
        .import_legacy_json(&data_dir.join("registry.json"))
        .is_err());
    assert!(registry.apps().is_empty());
}

/// The registry as the release before `integrated_at` wrote it: the same
/// tables, without the column.
const PREVIOUS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS apps(
    uuid TEXT PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    version TEXT NOT NULL DEFAULT '',
    comment TEXT NOT NULL DEFAULT '',
    managed_path TEXT NOT NULL DEFAULT '',
    desktop_id TEXT NOT NULL DEFAULT '',
    desktop_path TEXT NOT NULL DEFAULT '',
    icon_path TEXT NOT NULL DEFAULT '',
    sha256 TEXT NOT NULL DEFAULT '',
    app_type INTEGER NOT NULL DEFAULT 0,
    architecture INTEGER NOT NULL DEFAULT 0,
    size INTEGER NOT NULL DEFAULT 0,
    arguments TEXT NOT NULL DEFAULT '[]',
    default_arguments TEXT NOT NULL DEFAULT '[]',
    environment TEXT NOT NULL DEFAULT '[]',
    update_manager TEXT NOT NULL DEFAULT '',
    update_config TEXT NOT NULL DEFAULT '{}',
    embedded_update TEXT NOT NULL DEFAULT '',
    last_update_check TEXT NOT NULL DEFAULT '',
    available_version TEXT NOT NULL DEFAULT '',
    available_url TEXT NOT NULL DEFAULT '',
    available_size INTEGER NOT NULL DEFAULT 0,
    update_available INTEGER NOT NULL DEFAULT 0,
    digest TEXT NOT NULL DEFAULT '',
    reduced_verification INTEGER NOT NULL DEFAULT 0,
    external_folder INTEGER NOT NULL DEFAULT 0,
    owned INTEGER NOT NULL DEFAULT 1,
    adopted INTEGER NOT NULL DEFAULT 0,
    website TEXT NOT NULL DEFAULT '',
    terminal INTEGER NOT NULL DEFAULT 0,
    actions TEXT NOT NULL DEFAULT '[]'
);";

#[test]
fn a_registry_from_before_the_integration_date_keeps_its_rows() {
    let h = Harness::new();
    let path = h.tmp.path().join("previous-registry.sqlite");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(PREVIOUS_SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO apps(uuid, name, version, managed_path, owned)
             VALUES ('old-1', 'Old Demo', '1.4', '/home/someone/AppImages/Old.AppImage', 1)",
            [],
        )
        .unwrap();
    }

    let registry = goshaim_core::registry::ManagedRegistry::open(&path)
        .expect("a registry from the previous release opens");
    let app = registry
        .by_uuid("old-1")
        .expect("the row survives the migration");
    assert_eq!(
        (app.name.as_str(), app.version.as_str()),
        ("Old Demo", "1.4")
    );
    assert_eq!(app.integrated_at, 0, "no date is invented for an old row");
    drop(registry);

    // Opening it again changes nothing: the migration is idempotent.
    let again = goshaim_core::registry::ManagedRegistry::open(&path).expect("opens again");
    assert_eq!(again.by_uuid("old-1").unwrap().name, "Old Demo");
    assert_eq!(again.apps().len(), 1);
}

#[test]
fn an_integration_date_is_stored_and_read_back() {
    let h = Harness::new();
    let path = h.tmp.path().join("dated-registry.sqlite");
    let mut registry = goshaim_core::registry::ManagedRegistry::open(&path).unwrap();
    let mut app = goshaim_core::types::InstalledApp::new_owned();
    app.uuid = goshaim_core::registry::ManagedRegistry::new_uuid();
    app.name = "Dated".to_string();
    app.managed_path = "/home/someone/AppImages/Dated.AppImage".to_string();
    app.integrated_at = 1_788_000_000;
    registry.upsert(app.clone()).unwrap();
    drop(registry);

    let reopened = goshaim_core::registry::ManagedRegistry::open(&path).unwrap();
    assert_eq!(
        reopened.by_uuid(&app.uuid).unwrap().integrated_at,
        1_788_000_000
    );
}

/// The registry the last build wrote: it has the integration date, but not the
/// source folder. Its rows keep their dates and read as no folder.
#[test]
fn a_registry_from_before_the_integration_folder_keeps_its_rows() {
    let h = Harness::new();
    let path = h.tmp.path().join("last-build-registry.sqlite");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(PREVIOUS_SCHEMA).unwrap();
        conn.execute_batch("ALTER TABLE apps ADD COLUMN integrated_at INTEGER NOT NULL DEFAULT 0;")
            .unwrap();
        conn.execute(
            "INSERT INTO apps(uuid, name, version, managed_path, owned, integrated_at)
             VALUES ('dated-1', 'Dated Demo', '2.0',
                     '/home/someone/AppImages/Dated.AppImage', 1, 1788000000)",
            [],
        )
        .unwrap();
    }

    let registry = goshaim_core::registry::ManagedRegistry::open(&path)
        .expect("a registry from the last build opens");
    let app = registry
        .by_uuid("dated-1")
        .expect("the row survives the migration");
    assert_eq!(app.name, "Dated Demo");
    assert_eq!(app.integrated_at, 1_788_000_000, "the date is kept");
    assert_eq!(
        app.integrated_folder, "",
        "no folder is invented for an old row"
    );
    drop(registry);

    // Opening it again changes nothing: the migration is idempotent.
    let again = goshaim_core::registry::ManagedRegistry::open(&path).expect("opens again");
    assert_eq!(again.by_uuid("dated-1").unwrap().integrated_folder, "");
    assert_eq!(again.apps().len(), 1);
}
