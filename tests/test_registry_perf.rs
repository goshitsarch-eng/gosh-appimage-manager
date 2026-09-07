// Audit findings P-4 and P-5, measured rather than asserted.
//
// P-4: save() opened a fresh connection, re-ran the schema batch, then
// DELETEd every row and re-INSERTed the whole table -- and upsert/remove_uuid
// both called it. Changing one field rewrote the library; removing N apps did
// N full-table rewrites.
//
// P-5: by_path called canonical_bounded (an lstat plus readlink hops) for
// every row on every lookup, and the library scan calls by_path once per
// discovered file, so a scan cost O(files x apps) syscalls.

mod common;

use common::Harness;
use goshaim_core::registry::ManagedRegistry;
use goshaim_core::types::InstalledApp;
use std::time::Instant;

fn seed(registry: &mut ManagedRegistry, dir: &std::path::Path, count: usize) {
    for i in 0..count {
        let path = dir.join(format!("App{i}.AppImage"));
        std::fs::write(&path, b"x").unwrap();
        let mut app = InstalledApp::new_owned();
        app.uuid = format!("uuid-{i}");
        app.name = format!("App{i}");
        app.managed_path = path.to_string_lossy().into_owned();
        registry.upsert(app).unwrap();
    }
}

/// A single-row update must not scale with the size of the library.
#[test]
fn single_row_update_does_not_scale_with_library_size() {
    let h = Harness::new();
    let c = h.controller();
    let dir = h.tmp.path().join("apps");
    std::fs::create_dir_all(&dir).unwrap();
    let mut registry = ManagedRegistry::open(&c.settings().registry_path()).unwrap();

    seed(&mut registry, &dir, 400);
    assert_eq!(registry.apps().len(), 400);

    // Time 100 single-field updates against the full library.
    let mut app = registry.by_uuid("uuid-7").unwrap();
    let start = Instant::now();
    for i in 0..100 {
        app.version = format!("v{i}");
        registry.upsert(app.clone()).unwrap();
    }
    let elapsed = start.elapsed();
    eprintln!("400-row library, 100 single-row updates: {elapsed:?}");

    // A full-table rewrite per call would be ~40,000 inserts. Targeted writes
    // are 100. The bound is generous so this measures the shape, not the box.
    assert!(
        elapsed.as_millis() < 4000,
        "single-row updates look like full-table rewrites: {elapsed:?}"
    );
    assert_eq!(registry.by_uuid("uuid-7").unwrap().version, "v99");
    assert_eq!(registry.apps().len(), 400, "no rows lost or duplicated");
}

/// Repeated path lookups must not re-stat every row each time.
#[test]
fn repeated_path_lookups_are_not_quadratic() {
    let h = Harness::new();
    let c = h.controller();
    let dir = h.tmp.path().join("apps");
    std::fs::create_dir_all(&dir).unwrap();
    let mut registry = ManagedRegistry::open(&c.settings().registry_path()).unwrap();
    seed(&mut registry, &dir, 300);

    // Exact-path lookups: the common case, and now a plain string match.
    let start = Instant::now();
    for i in 0..300 {
        let path = dir.join(format!("App{i}.AppImage"));
        assert!(registry.by_path(&path.to_string_lossy()).is_some());
    }
    let exact = start.elapsed();
    eprintln!("300 exact-path lookups over a 300-row library: {exact:?}");
    assert!(
        exact.as_millis() < 2000,
        "exact lookups too slow: {exact:?}"
    );

    // A miss still resolves correctly, and a symlinked path still matches the
    // row it points at.
    assert!(registry.by_path("/nowhere/App.AppImage").is_none());
    let link = h.tmp.path().join("link.AppImage");
    std::os::unix::fs::symlink(dir.join("App5.AppImage"), &link).unwrap();
    let found = registry
        .by_path(&link.to_string_lossy())
        .expect("a symlink must still resolve to the row it points at");
    assert_eq!(found.uuid, "uuid-5");
}

/// Removing every app must not rewrite the table once per removal.
#[test]
fn bulk_removal_is_linear() {
    let h = Harness::new();
    let c = h.controller();
    let dir = h.tmp.path().join("apps");
    std::fs::create_dir_all(&dir).unwrap();
    let mut registry = ManagedRegistry::open(&c.settings().registry_path()).unwrap();
    seed(&mut registry, &dir, 300);

    let start = Instant::now();
    for i in 0..300 {
        registry.remove_uuid(&format!("uuid-{i}")).unwrap();
    }
    let elapsed = start.elapsed();
    eprintln!("300 removals from a 300-row library: {elapsed:?}");
    assert!(
        elapsed.as_millis() < 4000,
        "bulk removal too slow: {elapsed:?}"
    );
    assert!(registry.apps().is_empty());

    // And the rows really are gone from disk, not just from memory.
    let reopened = ManagedRegistry::open(&c.settings().registry_path()).unwrap();
    assert!(reopened.apps().is_empty());
}
