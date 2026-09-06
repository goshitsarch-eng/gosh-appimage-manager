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
