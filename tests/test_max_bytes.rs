mod common;

use common::Harness;
use goshaim_core::limits::{max_appimage_mb, parse_max_appimage_mb, DEFAULT_MAX_APPIMAGE_BYTES};
use goshaim_core::safe_fs::copy_bounded;
use std::sync::atomic::AtomicBool;

#[test]
fn parse_accepts_plain_megabytes_and_rejects_garbage() {
    assert_eq!(
        parse_max_appimage_mb("8192").unwrap(),
        DEFAULT_MAX_APPIMAGE_BYTES
    );
    assert_eq!(parse_max_appimage_mb(" 1 ").unwrap(), 1024 * 1024);
    assert!(parse_max_appimage_mb("").is_err());
    assert!(parse_max_appimage_mb("abc").is_err());
    assert!(parse_max_appimage_mb("8.5").is_err());
    assert!(parse_max_appimage_mb("0").is_err());
    assert!(parse_max_appimage_mb("999999").is_err());
    assert_eq!(max_appimage_mb(DEFAULT_MAX_APPIMAGE_BYTES), 8192);
}

#[test]
fn setting_persists_and_reloads() {
    let h = Harness::new();
    let mut c = h.controller();
    let two_gb = 2048 * 1024 * 1024;
    c.settings_mut()
        .set_max_appimage_bytes(two_gb)
        .expect("persist max bytes");
    assert_eq!(c.settings().max_appimage_bytes(), two_gb);

    // Reload from the same isolated home: the value survives.
    let c2 = h.controller();
    assert_eq!(c2.settings().max_appimage_bytes(), two_gb);

    // Absurd values clamp instead of corrupting the store.
    c.settings_mut()
        .set_max_appimage_bytes(i64::MAX)
        .expect("clamp huge value");
    assert_eq!(
        c.settings().max_appimage_bytes(),
        goshaim_core::limits::ABSOLUTE_MAX_APPIMAGE_BYTES
    );
}

#[test]
fn oversized_file_is_refused_with_bound_message() {
    let h = Harness::new();
    let c = h.controller();
    let cancel = AtomicBool::new(false);
    let big = h.tmp.path().join("big.AppImage");
    std::fs::write(&big, vec![0u8; 2 * 1024 * 1024]).unwrap();
    let dest = h.tmp.path().join("dest.AppImage");
    let err = copy_bounded(&big, &dest, 1024 * 1024, &cancel).unwrap_err();
    assert!(
        err.contains("exceeds") && err.contains("bound"),
        "got: {err:?}"
    );
    assert!(!dest.exists(), "refused copy must leave no destination");
    let _ = c.settings().max_appimage_bytes();
}
