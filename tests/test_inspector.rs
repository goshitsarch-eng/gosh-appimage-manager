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

/// The stored setting is off by default. A caller asking for the fallback
/// anyway does not get it.
#[test]
fn unsafe_fallback_never_used_by_default() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let c = h.controller();
    let options = InspectOptions {
        allow_unsafe_extract: true,
        ..Default::default()
    };
    let result = c.inspect_with(path.to_str().unwrap(), &options, &Harness::cancel(), None);
    assert!(!result.extraction_used_unsafe_fallback);
}

/// Audit finding: the unsafe `--appimage-extract` fallback did not exist. The
/// branch pushed a warning claiming untrusted code was being executed while
/// nothing ran. The stored setting is now the one gate, so the fallback runs only
/// while it is on, and a file it cannot read says the fallback is off.
#[test]
fn unsafe_fallback_runs_only_when_the_stored_setting_is_on() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let text = path.to_string_lossy().into_owned();
    let calls = record_self_extraction(&h);

    // Setting off, and a caller asks anyway: nothing runs, and the user is told why.
    let mut c = h.controller();
    let asks = InspectOptions {
        allow_unsafe_extract: true,
        ..Default::default()
    };
    let result = c.inspect_with(&text, &asks, &Harness::cancel(), None);
    assert!(!result.extraction_used_unsafe_fallback);
    assert!(
        !ran_self_extraction(&calls),
        "nothing may run while the setting is off"
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("fallback is off")),
        "warnings: {:?}",
        result.warnings
    );

    // Setting on: the same request now runs it.
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let confirmed = InspectOptions {
        confirm_unsafe: true,
        ..Default::default()
    };
    let result = c.inspect_with(&text, &confirmed, &Harness::cancel(), None);
    assert!(result.extraction_used_unsafe_fallback);
    assert!(ran_self_extraction(&calls));
    result.discard_staging();
}

/// With the setting on, the AppImage is asked to unpack itself and the metadata
/// is read from what it produced.
#[test]
fn unsafe_fallback_reads_metadata_from_self_extraction_when_on() {
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

    let mut c = h.controller();
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let opts = InspectOptions {
        confirm_unsafe: true,
        ..Default::default()
    };
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

// ---- AppImage-layout payloads (the QA "no metadata" defect) ----------------

use common::{elf_layout, unsquashfs_on_path, LAYOUT_PAYLOAD};
use goshaim_core::inspector::AppImageInspector;
use goshaim_core::process::{ProcessResult, SystemRunner};
use std::sync::{Arc, Mutex};

const LAYOUT_DESKTOP: &[u8] = b"[Desktop Entry]\nType=Application\nName=Quill Notes\n\
X-AppImage-Version=1.2.0\nIcon=quill-notes\nExec=AppRun %U\nCategories=Office;\nTerminal=false\n";
const LAYOUT_LISTING: &[u8] = b"squashfs-root/AppRun\nsquashfs-root/quill-notes.desktop\n\
squashfs-root/quill-notes.png\n";

fn layout_icon() -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(std::iter::repeat_n(b'A', 64));
    png
}

/// Every unsquashfs call the inspector made, as argument vectors.
fn record_unsquashfs(h: &Harness) -> Arc<Mutex<Vec<Vec<String>>>> {
    let calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    h.runner.on_run(Box::new(move |req| {
        if req.program != "unsquashfs" {
            return None;
        }
        seen.lock().unwrap().push(req.args.clone());
        // Plant the members the extractor was asked for, as the real tool would.
        if let Some(i) = req.args.iter().position(|a| a == "-d") {
            let dest = std::path::Path::new(&req.args[i + 1]);
            std::fs::write(dest.join("quill-notes.desktop"), LAYOUT_DESKTOP).ok()?;
            std::fs::write(dest.join("quill-notes.png"), layout_icon()).ok()?;
        }
        Some(ProcessResult {
            program: req.program.clone(),
            exit_code: 0,
            stdout: LAYOUT_LISTING.to_vec(),
            ..Default::default()
        })
    }));
    calls
}

/// The QA repro: a 4096-byte ELF followed by the squashfs. unsquashfs must be
/// given the payload offset, or it looks for the superblock at byte 0.
#[test]
fn squashfs_listing_and_extraction_pass_the_payload_offset() {
    let h = Harness::new();
    let mut bytes = elf_layout(0, &[], &[(0, 4096)]);
    bytes.resize(4096, 0);
    bytes.extend_from_slice(LAYOUT_PAYLOAD);
    let path = h.tmp.path().join("Quill.AppImage");
    std::fs::write(&path, &bytes).unwrap();
    let calls = record_unsquashfs(&h);

    let c = h.controller();
    let result = c.inspect_file(path.to_str().unwrap(), &Harness::cancel(), None);

    assert_eq!(result.payload_offset, 4096);
    let calls = calls.lock().unwrap();
    assert!(!calls.is_empty(), "unsquashfs was never run");
    for args in calls.iter() {
        assert!(
            args.windows(2).any(|w| w[0] == "-o" && w[1] == "4096"),
            "unsquashfs must get -o 4096: {args:?}"
        );
    }
    assert_eq!(result.metadata.name, "Quill Notes", "{:?}", result.warnings);
    assert_eq!(result.metadata.version, "1.2.0");
    assert!(!result.metadata.extracted_icon_path.is_empty());
    result.discard_staging();
}

/// Real tools end to end: an AppImage-shaped file (ELF header and section table,
/// squashfs appended exactly where the table ends) gives the desktop entry's
/// name, version and icon through the real unsquashfs.
#[test]
fn appimage_layout_yields_the_desktop_name_and_icon_with_real_unsquashfs() {
    if !unsquashfs_on_path() {
        eprintln!(
            "SKIPPED appimage_layout_yields_the_desktop_name_and_icon: unsquashfs is not on PATH"
        );
        return;
    }
    let h = Harness::new();
    let mut bytes = elf_layout(256, &[(0, 0, 0), (3, 64, 16)], &[]);
    let end_of_elf = bytes.len();
    bytes.extend_from_slice(LAYOUT_PAYLOAD);
    let path = h.tmp.path().join("Quill.AppImage");
    std::fs::write(&path, &bytes).unwrap();

    let runner = SystemRunner::new();
    let inspector = AppImageInspector::new(&runner);
    let result = inspector.inspect(
        path.to_str().unwrap(),
        &InspectOptions::default(),
        &Harness::cancel(),
        None,
    );

    assert!(result.magic_valid, "error: {}", result.error);
    assert_eq!(result.payload_offset, end_of_elf as i64);
    assert_eq!(
        result.metadata.name, "Quill Notes",
        "warnings: {:?}",
        result.warnings
    );
    assert_eq!(result.metadata.version, "1.2.0");
    assert_eq!(result.metadata.icon_format, "png");
    let icon = std::fs::read(&result.metadata.extracted_icon_path)
        .expect("the desktop entry's icon is staged");
    assert_eq!(icon, layout_icon());
    result.discard_staging();
}

/// An offset outside the file is refused before any extractor runs.
#[test]
fn payload_offset_outside_the_file_is_refused_before_any_extractor_runs() {
    for claimed_end in [1_000_000u64, 8192] {
        let h = Harness::new();
        // The header says the payload ends at `claimed_end`; the file is 8192 bytes.
        let mut bytes = elf_layout(0, &[], &[(0, claimed_end)]);
        bytes.resize(8192, 0);
        let path = h.tmp.path().join("Broken.AppImage");
        std::fs::write(&path, &bytes).unwrap();
        let calls = record_unsquashfs(&h);

        let c = h.controller();
        let result = c.inspect_file(path.to_str().unwrap(), &Harness::cancel(), None);

        assert!(
            calls.lock().unwrap().is_empty(),
            "an extractor ran for offset {claimed_end}"
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("outside the file")),
            "offset {claimed_end}: warnings {:?}",
            result.warnings
        );
        assert!(result.metadata.name.is_empty());
    }
}

/// Archive members are read with a bound. A file over the bound is refused, and a
/// file exactly at the bound is read whole.
#[test]
fn bounded_reads_refuse_oversize_files_and_keep_exact_ones() {
    let h = Harness::new();
    let file = h.tmp.path().join("member.bin");
    std::fs::write(&file, vec![7u8; 2048]).unwrap();
    assert_eq!(
        goshaim_core::safe_fs::read_bounded(&file, 2048)
            .unwrap()
            .len(),
        2048
    );
    let error = goshaim_core::safe_fs::read_bounded(&file, 2047).unwrap_err();
    assert!(error.contains("size bound"), "error: {error}");
}

/// A payload whose files carry SELinux labels (`security.selinux`), as images built
/// on SELinux hosts do. Extracting those labels as a normal user fails, and
/// unsquashfs reports that as exit status 2 after it has written the file. The
/// metadata must still be read.
#[test]
fn a_payload_with_selinux_labels_still_yields_its_metadata() {
    if !unsquashfs_on_path() {
        eprintln!("SKIPPED a_payload_with_selinux_labels_still_yields_its_metadata: unsquashfs is not on PATH");
        return;
    }
    let h = Harness::new();
    let mut bytes = elf_layout(0, &[], &[(0, 4096)]);
    bytes.resize(4096, 0);
    bytes.extend_from_slice(include_bytes!(
        "fixtures/appimage-layout-selinux-payload.sqsh"
    ));
    let path = h.tmp.path().join("Probe.AppImage");
    std::fs::write(&path, &bytes).unwrap();

    let runner = SystemRunner::new();
    let result = AppImageInspector::new(&runner).inspect(
        path.to_str().unwrap(),
        &InspectOptions::default(),
        &Harness::cancel(),
        None,
    );

    assert_eq!(
        result.metadata.name, "Probe App",
        "warnings: {:?}",
        result.warnings
    );
    assert_eq!(result.metadata.version, "1.2.3");
    result.discard_staging();
}

// ---- The unsafe --appimage-extract fallback: opt-in, last resort, warned -----

use common::{plant_self_extraction as record_self_extraction, ran_self_extraction};

/// The stored setting is off by default. A caller that asks for the fallback
/// anyway is refused, and the user is told why.
#[test]
fn the_unsafe_fallback_is_refused_with_a_clear_message_while_the_setting_is_off() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let calls = record_self_extraction(&h);
    let c = h.controller();
    let options = InspectOptions {
        allow_unsafe_extract: true,
        ..Default::default()
    };
    let result = c.inspect_with(path.to_str().unwrap(), &options, &Harness::cancel(), None);

    assert!(!ran_self_extraction(&calls), "the AppImage was run");
    assert!(!result.extraction_used_unsafe_fallback);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("fallback is off") && w.contains("Settings")),
        "warnings: {:?}",
        result.warnings
    );
}

/// The stored setting, not the caller, decides. With the setting on, the fallback
/// runs, and the result says that it did.
#[test]
fn the_fallback_runs_when_the_stored_setting_is_on_and_says_so() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let calls = record_self_extraction(&h);
    let mut c = h.controller();
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let options = InspectOptions {
        confirm_unsafe: true,
        ..Default::default()
    };
    let result = c.inspect_with(path.to_str().unwrap(), &options, &Harness::cancel(), None);

    assert!(ran_self_extraction(&calls), "the fallback did not run");
    assert!(result.extraction_used_unsafe_fallback);
    assert_eq!(result.metadata.name, "Unpacked");
    assert_eq!(result.extractor_used, "--appimage-extract");
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("Unsafe extraction fallback used") && w.contains("executed")),
        "the result must say the AppImage ran: {:?}",
        result.warnings
    );
    result.discard_staging();
}

/// The fallback is the last resort: when safe extraction gives a name, the
/// AppImage is never run, even with the setting on.
#[test]
fn the_fallback_is_the_last_resort_after_safe_extraction() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let calls = record_unsquashfs(&h);
    let mut c = h.controller();
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let options = InspectOptions {
        allow_unsafe_extract: true,
        ..Default::default()
    };
    let result = c.inspect_with(path.to_str().unwrap(), &options, &Harness::cancel(), None);

    assert_eq!(result.metadata.name, "Quill Notes", "{:?}", result.warnings);
    assert!(
        !ran_self_extraction(&calls),
        "the AppImage ran although safe extraction worked"
    );
    assert!(!result.extraction_used_unsafe_fallback);
    result.discard_staging();
}

// ---- Per-file confirmation for the unsafe fallback ---------------------------

/// With the stored setting on, a file the safe read could not handle is not run
/// until the user confirms it. The result is pending and says what to confirm.
#[test]
fn an_unconfirmed_fallback_is_pending_and_runs_nothing_while_the_setting_is_on() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let log = record_self_extraction(&h);
    let mut c = h.controller();
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");

    let result = c.inspect_with(
        path.to_str().unwrap(),
        &InspectOptions::default(),
        &Harness::cancel(),
        None,
    );

    assert!(
        !ran_self_extraction(&log),
        "nothing may run before the file is confirmed"
    );
    assert!(result.fallback_pending, "warnings: {:?}", result.warnings);
    assert!(!result.extraction_used_unsafe_fallback);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("Confirm") && w.contains("this file")),
        "warnings: {:?}",
        result.warnings
    );
}

/// The same file runs once the user has confirmed it.
#[test]
fn a_confirmed_fallback_runs_when_the_setting_is_on() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let log = record_self_extraction(&h);
    let mut c = h.controller();
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let confirmed = InspectOptions {
        confirm_unsafe: true,
        ..Default::default()
    };

    let result = c.inspect_with(path.to_str().unwrap(), &confirmed, &Harness::cancel(), None);

    assert!(ran_self_extraction(&log), "the confirmed file did not run");
    assert!(result.extraction_used_unsafe_fallback);
    assert!(!result.fallback_pending);
    assert_eq!(result.metadata.name, "Unpacked");
    result.discard_staging();
}

/// The setting off refuses even a confirmed file, and says the fallback is off.
#[test]
fn the_setting_off_refuses_even_a_confirmed_fallback() {
    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Demo.AppImage");
    let log = record_self_extraction(&h);
    let c = h.controller();
    let confirmed = InspectOptions {
        confirm_unsafe: true,
        ..Default::default()
    };

    let result = c.inspect_with(path.to_str().unwrap(), &confirmed, &Harness::cancel(), None);

    assert!(!ran_self_extraction(&log));
    assert!(!result.fallback_pending);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("fallback is off")),
        "warnings: {:?}",
        result.warnings
    );
}

/// Audit QA3-009: a cancelled inspection must not come back as a valid result. The
/// archive reader here is busy until the cancel flag is set, the way a long read
/// is. It then returns the whole listing, so the metadata is complete by the time
/// the inspection notices the cancel. The inspection must still come back
/// cancelled: not a valid AppImage, an error that says so, and no metadata.
#[test]
fn a_cancelled_inspection_is_not_returned_as_a_valid_result() {
    use goshaim_core::inspector::AppImageInspector;
    use goshaim_core::process::ProcessResult;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::{Duration, Instant};

    let h = Harness::new();
    let path = write_fixture(h.tmp.path(), "Big.AppImage");
    let cancel = Arc::new(AtomicBool::new(false));
    let (entered_tx, entered_rx) = mpsc::channel();
    let entered = Mutex::new(Some(entered_tx));
    let reader_cancel = Arc::clone(&cancel);
    h.runner.on_run(Box::new(move |req| {
        if req.program != "unsquashfs" {
            return None;
        }
        if req.args.iter().any(|a| a == "-l") {
            if let Some(tx) = entered.lock().unwrap().take() {
                let _ = tx.send(());
            }
            // The fake reader is busy until the operation is cancelled.
            let deadline = Instant::now() + Duration::from_secs(10);
            while !reader_cancel.load(Ordering::Relaxed) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            return Some(ProcessResult {
                program: req.program.clone(),
                exit_code: 0,
                stdout: LAYOUT_LISTING.to_vec(),
                ..Default::default()
            });
        }
        // Extraction: write the desktop entry the listing names.
        if let Some(i) = req.args.iter().position(|a| a == "-d") {
            let dest = std::path::Path::new(&req.args[i + 1]);
            std::fs::write(dest.join("quill-notes.desktop"), LAYOUT_DESKTOP).ok()?;
        }
        Some(ProcessResult {
            program: req.program.clone(),
            exit_code: 0,
            ..Default::default()
        })
    }));

    let runner = h.runner.clone();
    let worker_cancel = Arc::clone(&cancel);
    let path_text = path.to_string_lossy().into_owned();
    let worker = std::thread::spawn(move || {
        AppImageInspector::new(&runner).inspect(
            &path_text,
            &InspectOptions::default(),
            &worker_cancel,
            None,
        )
    });
    entered_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the archive reader started");
    cancel.store(true, Ordering::Relaxed);
    let result = worker.join().expect("the inspection returns");

    assert!(
        !result.magic_valid,
        "a cancelled inspection is not a valid AppImage"
    );
    assert!(
        result.error.to_lowercase().contains("cancel"),
        "the error says the inspection was cancelled: {:?}",
        result.error
    );
    assert!(
        result.metadata.name.is_empty(),
        "a cancelled inspection carries no metadata: {:?}",
        result.metadata.name
    );
    assert!(result.icon_staging_dir.is_empty());
}
