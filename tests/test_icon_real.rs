// Icons out of real AppImages, through the real unsquashfs.
//
// The earlier icon tests used a fake extractor that planted a PNG at the exact
// name the desktop entry gave. That is the one shape of AppImage the old code
// handled. Real ones differ: `.DirIcon` is a symlink to the icon, `Icon=` may
// carry an extension, the icon may exist only under `usr/share/icons`, and some
// icons are SVG. The extractor writes a symlink as a symlink, and the old code
// skipped every symlink, so for those images no icon was ever found.
//
// Each test builds a payload with mksquashfs and skips (loudly) when the squashfs
// tools are not installed, as the other real-tool tests do.

mod common;

use common::{
    desktop_entry, mksquashfs_on_path, real_appimage, unsquashfs_on_path, Harness, Member,
    TINY_PNG, TINY_SVG,
};
use goshaim_core::inspector::AppImageInspector;
use goshaim_core::process::SystemRunner;
use goshaim_core::types::{InspectOptions, InspectionResult};

fn tools() -> bool {
    if mksquashfs_on_path() && unsquashfs_on_path() {
        return true;
    }
    eprintln!("SKIPPED: the squashfs tools are not on PATH");
    false
}

fn inspect(members: &[Member], upd_info: Option<&str>) -> InspectionResult {
    let h = Harness::new();
    let path = real_appimage(h.tmp.path(), "App.AppImage", upd_info, members);
    let runner = SystemRunner::new();
    let result = AppImageInspector::new(&runner).inspect(
        path.to_str().unwrap(),
        &InspectOptions::default(),
        &Harness::cancel(),
        None,
    );
    assert!(result.magic_valid, "error: {}", result.error);
    assert_eq!(
        result.metadata.name, "Quill",
        "metadata was not read: {:?}",
        result.warnings
    );
    result
}

fn staged_icon(result: &InspectionResult) -> Option<(String, Vec<u8>)> {
    if result.metadata.extracted_icon_path.is_empty() {
        return None;
    }
    let bytes = std::fs::read(&result.metadata.extracted_icon_path).ok()?;
    assert!(
        result
            .metadata
            .extracted_icon_path
            .ends_with(&format!("icon.{}", result.metadata.icon_format)),
        "the staged file is named for its format"
    );
    Some((result.metadata.icon_format.clone(), bytes))
}

#[test]
fn the_icon_named_by_the_desktop_entry_is_staged() {
    if !tools() {
        return;
    }
    let desktop = desktop_entry("Quill", "1.0", "Icon=quill");
    let result = inspect(
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("quill.png", TINY_PNG),
            Member::Link(".DirIcon", "quill.png"),
        ],
        None,
    );
    assert_eq!(
        staged_icon(&result),
        Some(("png".into(), TINY_PNG.to_vec()))
    );
    result.discard_staging();
}

#[test]
fn an_svg_icon_is_staged_as_svg() {
    if !tools() {
        return;
    }
    let desktop = desktop_entry("Quill", "1.0", "Icon=quill");
    let result = inspect(
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("quill.svg", TINY_SVG),
            Member::Link(".DirIcon", "quill.svg"),
        ],
        None,
    );
    assert_eq!(
        staged_icon(&result),
        Some(("svg".into(), TINY_SVG.to_vec()))
    );
    result.discard_staging();
}

#[test]
fn a_dir_icon_symlink_is_followed_when_the_entry_names_no_icon() {
    if !tools() {
        return;
    }
    // No `Icon=` line: the only pointer to the icon is the `.DirIcon` link.
    let desktop = desktop_entry("Quill", "1.0", "");
    let result = inspect(
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("quill.png", TINY_PNG),
            Member::Link(".DirIcon", "quill.png"),
        ],
        None,
    );
    assert_eq!(
        staged_icon(&result),
        Some(("png".into(), TINY_PNG.to_vec()))
    );
    result.discard_staging();
}

#[test]
fn a_dir_icon_that_leads_through_two_links_into_the_theme_folders_is_followed() {
    if !tools() {
        return;
    }
    let desktop = desktop_entry("Quill", "1.0", "Icon=quill");
    let result = inspect(
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("usr/share/icons/hicolor/256x256/apps/quill.png", TINY_PNG),
            Member::Link(
                "quill.png",
                "usr/share/icons/hicolor/256x256/apps/quill.png",
            ),
            Member::Link(".DirIcon", "quill.png"),
        ],
        None,
    );
    assert_eq!(
        staged_icon(&result),
        Some(("png".into(), TINY_PNG.to_vec()))
    );
    result.discard_staging();
}

#[test]
fn an_icon_name_with_an_extension_is_taken_as_the_file() {
    if !tools() {
        return;
    }
    let desktop = desktop_entry("Quill", "1.0", "Icon=quill.png");
    let result = inspect(
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("quill.png", TINY_PNG),
        ],
        None,
    );
    assert_eq!(
        staged_icon(&result),
        Some(("png".into(), TINY_PNG.to_vec()))
    );
    result.discard_staging();
}

#[test]
fn an_icon_that_exists_only_in_the_theme_folders_is_found_at_the_best_size() {
    if !tools() {
        return;
    }
    let small = [TINY_PNG, b"small"].concat();
    let desktop = desktop_entry("Quill", "1.0", "Icon=quill");
    let result = inspect(
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("usr/share/icons/hicolor/16x16/apps/quill.png", &small),
            Member::File("usr/share/icons/hicolor/256x256/apps/quill.png", TINY_PNG),
            Member::File("usr/share/icons/hicolor/48x48/apps/quill.png", &small),
        ],
        None,
    );
    assert_eq!(
        staged_icon(&result),
        Some(("png".into(), TINY_PNG.to_vec())),
        "the 256 px icon, not the first one listed"
    );
    result.discard_staging();
}

#[test]
fn the_top_level_icon_beats_a_themed_copy_that_sorts_before_it() {
    if !tools() {
        return;
    }
    // `usr/...` sorts before `zed.png`, so a first-match search picked the 16 px copy.
    let tiny_marker = [TINY_PNG, b"themed"].concat();
    let desktop = desktop_entry("Quill", "1.0", "Icon=zed");
    let result = inspect(
        &[
            Member::File("zed.desktop", desktop.as_bytes()),
            Member::File("zed.png", TINY_PNG),
            Member::File("usr/share/icons/hicolor/16x16/apps/zed.png", &tiny_marker),
        ],
        None,
    );
    assert_eq!(
        staged_icon(&result),
        Some(("png".into(), TINY_PNG.to_vec()))
    );
    result.discard_staging();
}

#[test]
fn a_file_that_is_not_an_image_is_not_taken_for_the_icon() {
    if !tools() {
        return;
    }
    let desktop = desktop_entry("Quill", "1.0", "Icon=quill");
    let result = inspect(
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            // Named like an icon, but it is text.
            Member::File("quill.png", b"this is not a picture"),
            Member::File("usr/share/icons/hicolor/128x128/apps/quill.png", TINY_PNG),
        ],
        None,
    );
    assert_eq!(
        staged_icon(&result),
        Some(("png".into(), TINY_PNG.to_vec())),
        "the next candidate is used"
    );
    result.discard_staging();
}

#[test]
fn links_that_leave_the_archive_or_loop_yield_no_icon_and_read_nothing_outside() {
    if !tools() {
        return;
    }
    let desktop = desktop_entry("Quill", "1.0", "");
    for link in [
        "/etc/passwd",
        "../../../etc/passwd",
        ".DirIcon",
        "a/../../x",
    ] {
        let result = inspect(
            &[
                Member::File("quill.desktop", desktop.as_bytes()),
                Member::Link(".DirIcon", link),
            ],
            None,
        );
        assert_eq!(staged_icon(&result), None, "link to {link}");
        result.discard_staging();
    }
}

#[test]
fn an_appimage_without_an_icon_still_gets_its_metadata() {
    if !tools() {
        return;
    }
    let desktop = desktop_entry("Quill", "3.1", "Icon=missing");
    let result = inspect(&[Member::File("quill.desktop", desktop.as_bytes())], None);
    assert_eq!(result.metadata.version, "3.1");
    assert_eq!(staged_icon(&result), None);
}

#[test]
fn embedded_update_information_is_read_alongside_the_icon() {
    if !tools() {
        return;
    }
    let desktop = desktop_entry("Quill", "1.0", "Icon=quill");
    let upd = "gh-releases-zsync|example-org|quill|latest|Quill-*x86_64.AppImage.zsync";
    let result = inspect(
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("quill.png", TINY_PNG),
        ],
        Some(upd),
    );
    assert_eq!(result.update_info.raw, upd);
    assert_eq!(result.update_info.manager_hint, "github");
    assert_eq!(
        result
            .update_info
            .fields
            .get("filename")
            .map(String::as_str),
        Some("Quill-*x86_64.AppImage.zsync")
    );
    result.discard_staging();
}
