// Audit finding: icon extraction was broken end to end.
//
// The inspector recorded an *archive member name* in extracted_icon_path and
// then deleted the directory that member had been extracted into, and
// IntegrationService::stage_icon returned None unconditionally. Verified
// before the fix by integrating a real file: the installed entry read
// `Icon=application-x-executable` and no icon was ever written.
//
// The extractor is a process seam, so these tests drive it with a fake runner
// that lists and "extracts" a synthetic archive -- no real unsquashfs needed
// and no AppImage is ever executed.

mod common;

use common::Harness;
use goshaim_core::types::{ConflictPolicy, CopyMode, IntegrateRequest};
use std::sync::atomic::AtomicBool;

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Teach the fake runner to behave like `unsquashfs -l` and `unsquashfs -d`
/// over an archive containing a desktop entry and an icon.
fn stage_archive(h: &Harness, desktop_body: &str, icon: Option<(&str, Vec<u8>)>) {
    let mut listing = String::from("squashfs-root/demo.desktop\n");
    if let Some((name, _)) = &icon {
        listing.push_str(&format!("squashfs-root/{name}\n"));
    }
    h.runner.canned("unsquashfs", 0, listing.as_bytes());

    // The real tool writes into -d <dest>; the fake runner cannot, so the
    // harness plants the same files the extraction would have produced.
    let desktop_body = desktop_body.to_string();
    let icon = icon.map(|(n, b)| (n.to_string(), b));
    h.runner.on_run(Box::new(move |req| {
        // Only the extract invocation carries -d.
        let dest = req
            .args
            .iter()
            .position(|a| a == "-d")
            .and_then(|i| req.args.get(i + 1))
            .map(std::path::PathBuf::from)?;
        let _ = std::fs::create_dir_all(&dest);
        for member in req.args.iter().filter(|a| !a.starts_with('-')) {
            if member.ends_with(".desktop") {
                let _ = std::fs::write(dest.join(member), desktop_body.as_bytes());
            } else if let Some((name, bytes)) = &icon {
                if member == name || member == ".DirIcon" {
                    if let Some(parent) = dest.join(member).parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(dest.join(member), bytes);
                }
            }
        }
        None
    }));
}

fn integrate(h: &Harness) -> goshaim_core::types::IntegrateResult {
    let mut c = h.controller();
    let source = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    c.integrate(
        &IntegrateRequest {
            source_path: source.to_string_lossy().into_owned(),
            conflict: ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: CopyMode::Copy,
            assume_yes: true,
        },
        &AtomicBool::new(false),
    )
}

#[test]
fn referenced_icon_is_extracted_and_installed() {
    let h = Harness::new();
    let mut png = PNG_MAGIC.to_vec();
    png.extend_from_slice(&[0u8; 64]);
    stage_archive(
        &h,
        "[Desktop Entry]\nName=Demo\nIcon=demo\nExec=demo --flag %U\nCategories=Network;Utility;\nMimeType=text/plain;\n",
        Some(("demo.png", png.clone())),
    );

    let result = integrate(&h);
    assert!(result.ok, "integrate failed: {}", result.error);
    assert!(
        !result.app.icon_path.is_empty(),
        "an icon should have been installed"
    );
    let installed = std::path::PathBuf::from(&result.app.icon_path);
    assert!(installed.is_file(), "icon file missing at {installed:?}");
    assert_eq!(
        std::fs::read(&installed).unwrap(),
        png,
        "the installed icon should be the bytes from the archive"
    );
    assert!(
        installed.extension().is_some_and(|e| e == "png"),
        "icon should keep its format: {installed:?}"
    );

    // The entry points at the installed icon, not the generic fallback.
    let entry = std::fs::read_to_string(&result.app.desktop_path).unwrap();
    assert!(
        entry.contains(&format!("Icon={}", installed.display())),
        "entry should reference the installed icon:\n{entry}"
    );
    assert!(
        !entry.contains("Icon=application-x-executable"),
        "entry fell back to the generic icon:\n{entry}"
    );
}

/// The staging directory the inspector creates must not be left behind. A
/// caller that inspects without integrating owns it explicitly; integration
/// clears it as part of finishing.
#[test]
fn icon_staging_is_owned_and_removable() {
    let h = Harness::new();
    let mut png = PNG_MAGIC.to_vec();
    png.extend_from_slice(&[0u8; 16]);
    stage_archive(
        &h,
        "[Desktop Entry]\nName=Demo\nIcon=demo\nExec=demo\n",
        Some(("demo.png", png)),
    );
    let c = h.controller();
    let source = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let inspected = c.inspect_file(&source.to_string_lossy(), &AtomicBool::new(false), None);

    assert!(
        !inspected.icon_staging_dir.is_empty(),
        "an extracted icon should report the directory holding it"
    );
    let dir = std::path::PathBuf::from(&inspected.icon_staging_dir);
    assert!(dir.is_dir(), "staging directory should exist while in use");
    assert!(
        std::path::Path::new(&inspected.metadata.extracted_icon_path).is_file(),
        "the staged icon should be a real readable file, not an archive member name"
    );

    inspected.discard_staging();
    assert!(!dir.exists(), "discard_staging must remove the directory");

    // Integration does the same on the way out.
    let mut c = h.controller();
    let result = c.integrate(
        &IntegrateRequest {
            source_path: source.to_string_lossy().into_owned(),
            conflict: ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: CopyMode::Copy,
            assume_yes: true,
        },
        &AtomicBool::new(false),
    );
    assert!(result.ok, "integrate failed: {}", result.error);
    assert!(
        std::path::Path::new(&result.app.icon_path).is_file(),
        "the icon should survive in the icon theme directory"
    );
}

/// `.DirIcon` carries no extension, so the format is sniffed from content.
#[test]
fn diricon_fallback_is_named_by_its_content() {
    let h = Harness::new();
    let svg = b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec();
    stage_archive(
        &h,
        "[Desktop Entry]\nName=Demo\nExec=demo\n",
        Some((".DirIcon", svg.clone())),
    );
    let result = integrate(&h);
    assert!(result.ok, "integrate failed: {}", result.error);
    let installed = std::path::PathBuf::from(&result.app.icon_path);
    assert!(installed.is_file(), "no icon installed");
    assert!(
        installed.extension().is_some_and(|e| e == "svg"),
        "an SVG .DirIcon should be installed as .svg, got {installed:?}"
    );
}

/// Desktop metadata beyond name/version now reaches the written entry.
#[test]
fn categories_mime_types_and_arguments_are_carried_over() {
    let h = Harness::new();
    stage_archive(
        &h,
        "[Desktop Entry]\nName=Demo\nExec=demo --flag \"two words\" %U\nCategories=Network;Utility;\nMimeType=text/plain;application/pdf;\nStartupWMClass=Demo\n",
        None,
    );
    let result = integrate(&h);
    assert!(result.ok, "integrate failed: {}", result.error);

    assert_eq!(
        result.app.default_arguments,
        vec!["--flag".to_string(), "two words".to_string()],
        "field codes are dropped and quoting is honoured"
    );
    let entry = std::fs::read_to_string(&result.app.desktop_path).unwrap();
    assert!(entry.contains("Categories=Network;Utility;"), "\n{entry}");
    assert!(
        entry.contains("MimeType=text/plain;application/pdf;"),
        "\n{entry}"
    );
    assert!(entry.contains("StartupWMClass=Demo"), "\n{entry}");
}
