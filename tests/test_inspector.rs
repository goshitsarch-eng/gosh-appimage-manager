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

/// Audit finding: the unsafe `--appimage-extract` fallback did not exist. The
/// branch pushed a warning claiming untrusted code was being executed while
/// nothing ran, so the Settings toggle, the confirmation dialog and
/// `--probe-inspect`'s "proof of no execution" all referred to a feature that
/// was never built.
#[test]
fn unsafe_fallback_runs_only_with_setting_and_per_file_confirmation() {
    use goshaim_core::types::InspectOptions;

    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    // Safe extraction finds nothing (no extractor tool answers), so the
    // fallback is the only way metadata could be read.
    let text = path.to_string_lossy().into_owned();

    // Both off: nothing runs.
    let c = h.controller();
    let result = c.inspect_with(&text, &InspectOptions::default(), &Harness::cancel(), None);
    assert!(!result.extraction_used_unsafe_fallback);
    assert!(h.runner.spawned.lock().unwrap().is_empty());

    // Setting on but this file not confirmed: still nothing runs, and the
    // user is told why.
    let opts = InspectOptions {
        allow_unsafe_extract: true,
        confirm_unsafe_extract: false,
        ..Default::default()
    };
    let result = c.inspect_with(&text, &opts, &Harness::cancel(), None);
    assert!(
        !result.extraction_used_unsafe_fallback,
        "enabling the setting alone must not execute anything"
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("per-file confirmation")),
        "warnings: {:?}",
        result.warnings
    );
}

/// With both the setting and the per-file confirmation, the AppImage is asked
/// to unpack itself and the metadata is read from what it produced.
#[test]
fn confirmed_unsafe_fallback_reads_metadata_from_self_extraction() {
    use goshaim_core::types::InspectOptions;

    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");

    // Stand in for the AppImage runtime: on `--appimage-extract`, write a
    // squashfs-root tree into the working directory it was given.
    h.runner.on_run(Box::new(|req| {
        if !req.args.iter().any(|a| a == "--appimage-extract") {
            return None;
        }
        let root = std::path::Path::new(&req.work_dir).join("squashfs-root");
        std::fs::create_dir_all(&root).ok()?;
        std::fs::write(
            root.join("demo.desktop"),
            b"[Desktop Entry]\nName=Unpacked\nX-AppImage-Version=9.9\nIcon=demo\nExec=demo\n",
        )
        .ok()?;
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&[0u8; 32]);
        std::fs::write(root.join("demo.png"), &png).ok()?;
        Some(goshaim_core::process::ProcessResult {
            program: req.program.clone(),
            exit_code: 0,
            ..Default::default()
        })
    }));

    let opts = InspectOptions {
        allow_unsafe_extract: true,
        confirm_unsafe_extract: true,
        ..Default::default()
    };
    let c = h.controller();
    let result = c.inspect_with(&path.to_string_lossy(), &opts, &Harness::cancel(), None);

    assert!(
        result.extraction_used_unsafe_fallback,
        "the fallback should report that it executed the AppImage"
    );
    assert_eq!(result.metadata.name, "Unpacked");
    assert_eq!(result.metadata.version, "9.9");
    assert_eq!(result.extractor_used, "--appimage-extract");
    assert!(
        result.warnings.iter().any(|w| w.contains("was executed")),
        "the user must be told the binary ran: {:?}",
        result.warnings
    );
    assert!(
        std::path::Path::new(&result.metadata.extracted_icon_path).is_file(),
        "the icon should be staged from the extracted tree"
    );
    result.discard_staging();
}
