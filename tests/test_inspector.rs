mod common;

use common::{write_fixture, write_fixture_arch, Harness};
use goshaim_core::types::{AppImageType, Architecture, InspectOptions};

#[test]
fn missing_file_errors_without_execution() {
    let h = Harness::new();
    let c = h.controller();
    let cancel = Harness::cancel();
    let result = c.inspect_file("/nonexistent/Demo.AppImage", &cancel, None);
    assert!(!result.magic_valid);
    assert!(!result.error.is_empty());
    assert!(!result.extraction_used_unsafe_fallback);
}

#[test]
fn directory_rejected() {
    let h = Harness::new();
    let c = h.controller();
    let cancel = Harness::cancel();
    let result = c.inspect_file(h.tmp.path().to_str().unwrap(), &cancel, None);
    assert!(!result.magic_valid);
    assert_eq!(result.error, "Not a regular file");
}

#[test]
fn empty_file_rejected() {
    let h = Harness::new();
    let empty = h.tmp.path().join("Empty.AppImage");
    std::fs::write(&empty, b"").unwrap();
    let c = h.controller();
    let cancel = Harness::cancel();
    let result = c.inspect_file(empty.to_str().unwrap(), &cancel, None);
    assert_eq!(result.error, "Empty file");
}

#[test]
fn valid_fixture_inspects_without_execution() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let c = h.controller();
    let cancel = Harness::cancel();
    let result = c.inspect_file(path.to_str().unwrap(), &cancel, None);
    assert!(result.error.is_empty(), "error: {}", result.error);
    assert!(result.magic_valid);
    assert_eq!(result.app_type, AppImageType::Type2);
    assert_eq!(result.architecture, Architecture::X86_64);
    assert!(result.architecture_supported);
    assert!(!result.identity.sha256.is_empty());
    assert!(!result.extraction_used_unsafe_fallback);
    // No canned extractor output: metadata extraction warns, magic stands.
    assert!(result.extraction_attempted);
}

#[test]
fn unsupported_arch_warns_but_reports() {
    let h = Harness::new();
    let path = write_fixture_arch(
        h.tmp.path(),
        "Legacy.AppImage",
        Architecture::I386,
        AppImageType::Type2,
    );
    let c = h.controller();
    let cancel = Harness::cancel();
    let result = c.inspect_file(path.to_str().unwrap(), &cancel, None);
    assert!(result.magic_valid);
    assert!(!result.architecture_supported);
    assert!(result
        .warnings
        .iter()
        .any(|w| w.contains("Unsupported architecture")));
}

#[test]
fn size_bound_enforced() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let c = h.controller();
    let cancel = Harness::cancel();
    let options = InspectOptions {
        max_bytes: 16, // smaller than the 128-byte fixture
        ..Default::default()
    };
    let result = c.inspect_with(path.to_str().unwrap(), &options, &cancel, None);
    assert!(result.error.contains("exceeds configured size bound"));
    assert!(!result.magic_valid);
}

#[test]
fn existing_managed_id_marks_adopted() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let c = h.controller();
    let cancel = Harness::cancel();
    let result = c.inspect_file(path.to_str().unwrap(), &cancel, Some("uuid-1"));
    assert!(result.already_managed);
    assert_eq!(result.existing_managed_id, "uuid-1");
}

#[test]
fn unsafe_fallback_never_used_by_default() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let c = h.controller();
    let cancel = Harness::cancel();
    let options = InspectOptions {
        allow_unsafe_extract: true,    // even when allowed...
        confirm_unsafe_extract: false, // ...without per-file confirm it stays off
        ..Default::default()
    };
    let result = c.inspect_with(path.to_str().unwrap(), &options, &cancel, None);
    assert!(!result.extraction_used_unsafe_fallback);
}
