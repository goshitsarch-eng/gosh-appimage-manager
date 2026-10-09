// Adopting an AppImage reads it; refreshing and healing give it its icon.
//
// Adoption used to write "a registry row and nothing else": a name taken from
// the file name, and nothing the file says about itself. So every adopted app
// showed a letter tile instead of its icon, and every update check on one
// failed with "No update method was found" because its embedded update string
// was never read. Refresh read the file but threw the staged icon away.
//
// These tests use real AppImages (a real squashfs payload read by the real
// unsquashfs) and skip loudly when the squashfs tools are absent. No AppImage
// is executed, and nothing is written outside a scratch home.

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::{
    desktop_entry, mksquashfs_on_path, real_appimage, unsquashfs_on_path, Harness, Member,
    TINY_PNG, TINY_SVG,
};
use goshaim_core::controller::AppController;
use goshaim_core::process::SystemRunner;
use goshaim_core::types::{
    Architecture, ConflictPolicy, CopyMode, IntegrateRequest, RemovalMode, RemovalRequest,
};

const UPD: &str = "gh-releases-zsync|example-org|quill|latest|Quill-*x86_64.AppImage.zsync";

fn tools() -> bool {
    if mksquashfs_on_path() && unsquashfs_on_path() {
        return true;
    }
    eprintln!("SKIPPED: the squashfs tools are not on PATH");
    false
}

/// A controller that reads files with the real extractor, on a scratch home.
fn controller(h: &Harness) -> AppController {
    AppController::with_seams(
        Box::new(SystemRunner::new()),
        Box::new(h.network.clone()),
        Box::new(h.table.clone()),
        Box::new(h.trash.clone()),
        h.dirs(),
    )
    .expect("controller")
}

/// A real AppImage in the managed folder, with an icon and update information.
fn quill_in_managed_folder(h: &Harness, icon: &[u8], icon_file: &str) -> String {
    let folder = h.tmp.path().join("AppImages");
    std::fs::create_dir_all(&folder).unwrap();
    let desktop = desktop_entry("Quill", "1.0.0", "Icon=quill");
    let path = real_appimage(
        &folder,
        "quill-download.AppImage",
        Some(UPD),
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File(icon_file, icon),
            Member::Link(".DirIcon", icon_file),
        ],
    );
    path.to_string_lossy().into_owned()
}

#[test]
fn an_adopted_app_is_read_for_its_name_version_icon_and_update_information() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);

    let app = c
        .adopt_external(&path)
        .expect("a readable AppImage is adopted");

    assert!(app.adopted && app.owned);
    assert_eq!(app.name, "Quill", "named by the file, not its file name");
    assert_eq!(app.version, "1.0.0");
    assert_eq!(app.architecture, Architecture::X86_64);
    assert!(!app.sha256.is_empty());
    assert_eq!(app.embedded_update, UPD);

    // The icon is kept in the manager's own folder, named for the app so that
    // removal can prove it is ours.
    assert!(!app.icon_path.is_empty(), "adoption found no icon");
    let icon = Path::new(&app.icon_path);
    assert_eq!(std::fs::read(icon).unwrap(), TINY_PNG);
    assert!(icon.starts_with(c.settings().app_icons_dir()));
    assert!(app.icon_path.contains(&app.uuid));

    // It is what the registry holds, not just what the call returned.
    let stored = c.registry().by_uuid(&app.uuid).unwrap();
    assert_eq!(stored.icon_path, app.icon_path);
    assert_eq!(stored.embedded_update, UPD);
}

#[test]
fn adopting_writes_nothing_to_the_menu_or_the_icon_theme() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);

    let app = c.adopt_external(&path).unwrap();

    assert!(app.desktop_path.is_empty(), "no menu entry is written");
    assert!(
        !c.settings().applications_dir().exists()
            || std::fs::read_dir(c.settings().applications_dir())
                .unwrap()
                .next()
                .is_none(),
        "the user's applications folder was touched"
    );
    assert!(
        !c.settings().icons_dir().exists(),
        "the user's icon theme was touched"
    );
}

#[test]
fn an_adopted_app_can_be_checked_for_updates() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);
    let app = c.adopt_external(&path).unwrap();
    h.network.canned_body(
        "api.github.com",
        br#"{"tag_name":"v1.2.0","assets":[
            {"name":"Quill-1.2.0-x86_64.AppImage.zsync","size":9,"browser_download_url":"https://github.com/example-org/quill/releases/download/v1.2.0/Quill-1.2.0-x86_64.AppImage.zsync"},
            {"name":"Quill-1.2.0-x86_64.AppImage","size":4096,"browser_download_url":"https://github.com/example-org/quill/releases/download/v1.2.0/Quill-1.2.0-x86_64.AppImage"}]}"#,
    );

    let checked = c.check_one_update(&app, &AtomicBool::new(false));

    assert!(
        checked.ok,
        "an adopted app could not be checked: {}",
        checked.error
    );
    assert!(goshaim_core::updates_service::offers_update(&app, &checked));
    assert!(checked.url.ends_with("Quill-1.2.0-x86_64.AppImage"));
}

#[test]
fn an_svg_icon_is_kept_as_svg() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_SVG, "quill.svg");
    let mut c = controller(&h);

    let app = c.adopt_external(&path).unwrap();

    assert!(app.icon_path.ends_with(".svg"), "icon: {}", app.icon_path);
    assert_eq!(std::fs::read(&app.icon_path).unwrap(), TINY_SVG);
}

#[test]
fn a_file_that_cannot_be_read_is_still_adopted_as_before() {
    let h = Harness::new();
    let folder = h.tmp.path().join("AppImages");
    std::fs::create_dir_all(&folder).unwrap();
    // Passes the file-name test for adoption, but is not an AppImage at all.
    let path = folder.join("Broken.AppImage");
    std::fs::write(&path, b"not an elf").unwrap();
    let mut c = controller(&h);

    let app = c
        .adopt_external(path.to_str().unwrap())
        .expect("adoption does not depend on the file being readable");

    assert_eq!(app.name, "Broken");
    assert!(app.icon_path.is_empty());
}

#[test]
fn removing_an_adopted_app_removes_the_icon_it_was_given() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);
    let app = c.adopt_external(&path).unwrap();
    assert!(Path::new(&app.icon_path).is_file());

    let result = c.remove_app(&RemovalRequest {
        path_or_uuid: app.uuid.clone(),
        mode: RemovalMode::Trash,
        assume_yes: true,
    });

    assert!(result.ok, "removal failed: {}", result.error);
    assert!(
        !Path::new(&app.icon_path).exists(),
        "the icon was left behind"
    );
    assert!(c.registry().by_uuid(&app.uuid).is_none());
}

#[test]
fn refreshing_an_app_installs_its_icon() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);
    // An adoption from before adoption read the file: a bare row.
    let bare = c
        .registry_mut()
        .adopt_external("quill-download".into(), path.clone(), false)
        .unwrap();
    assert!(bare.icon_path.is_empty());

    let name = c
        .refresh_metadata(&bare.uuid, &AtomicBool::new(false))
        .expect("refresh reads the file");

    assert_eq!(name, "Quill");
    let after = c.registry().by_uuid(&bare.uuid).unwrap();
    assert_eq!(std::fs::read(&after.icon_path).unwrap(), TINY_PNG);
}

#[test]
fn an_integrated_apps_icon_and_entry_are_repaired_by_a_refresh() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let dir = h.tmp.path().join("Downloads");
    std::fs::create_dir_all(&dir).unwrap();
    let desktop = desktop_entry("Quill", "1.0.0", "Icon=quill");
    let source = real_appimage(
        &dir,
        "Quill.AppImage",
        Some(UPD),
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("quill.png", TINY_PNG),
        ],
    );
    let mut c = controller(&h);
    let done = c.integrate(
        &IntegrateRequest {
            source_path: source.to_string_lossy().into_owned(),
            conflict: ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: CopyMode::Copy,
            assume_yes: true,
            confirm_unsafe: false,
        },
        &AtomicBool::new(false),
    );
    assert!(done.ok, "integrate failed: {}", done.error);
    let integrated = done.app;
    assert!(Path::new(&integrated.icon_path).is_file());

    // Lose the icon, as a hand-cleaned theme folder would.
    std::fs::remove_file(&integrated.icon_path).unwrap();
    assert_eq!(c.apps_missing_icons().len(), 1);

    let changed = c
        .heal_icon(&integrated.uuid, &AtomicBool::new(false))
        .unwrap();

    assert!(changed);
    assert!(Path::new(&integrated.icon_path).is_file(), "icon restored");
    let entry = std::fs::read_to_string(&integrated.desktop_path).unwrap();
    assert!(
        entry.contains(&format!("Icon={}", integrated.icon_path)),
        "the entry names the icon:\n{entry}"
    );
    assert!(c.apps_missing_icons().is_empty());
}

#[test]
fn healing_fills_in_a_never_read_row_once_and_then_leaves_it_alone() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);
    let bare = c
        .registry_mut()
        .adopt_external("quill-download".into(), path, false)
        .unwrap();
    assert_eq!(c.apps_missing_icons().len(), 1);

    assert!(c.heal_icon(&bare.uuid, &AtomicBool::new(false)).unwrap());

    let healed = c.registry().by_uuid(&bare.uuid).unwrap();
    assert_eq!(healed.name, "Quill");
    assert_eq!(healed.embedded_update, UPD);
    assert_eq!(healed.architecture, Architecture::X86_64);
    assert!(Path::new(&healed.icon_path).is_file());
    assert!(c.apps_missing_icons().is_empty());
    assert!(
        !c.heal_icon(&bare.uuid, &AtomicBool::new(false)).unwrap(),
        "nothing changes the second time"
    );
}

#[test]
fn healing_keeps_an_edit_made_while_the_file_was_being_read() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);
    let bare = c
        .registry_mut()
        .adopt_external("quill-download".into(), path, false)
        .unwrap();
    // Another call (a second controller, as the bridge makes one per call)
    // changes the app's launch arguments after this one started.
    let mut other = controller(&h);
    other
        .set_arguments_and_environment(&bare.uuid, vec!["--flag".to_string()], vec![])
        .unwrap();

    c.heal_icon(&bare.uuid, &AtomicBool::new(false)).unwrap();

    let after = c.registry().by_uuid(&bare.uuid).unwrap();
    assert_eq!(after.arguments, vec!["--flag".to_string()]);
    assert!(Path::new(&after.icon_path).is_file());
}

#[test]
fn an_app_with_no_icon_in_its_file_is_not_reported_as_changed() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let folder = h.tmp.path().join("AppImages");
    std::fs::create_dir_all(&folder).unwrap();
    let desktop = desktop_entry("Plain", "2.0", "");
    let path = real_appimage(
        &folder,
        "Plain.AppImage",
        None,
        &[Member::File("plain.desktop", desktop.as_bytes())],
    );
    let mut c = controller(&h);

    let app = c.adopt_external(path.to_str().unwrap()).unwrap();
    assert_eq!(app.name, "Plain");
    assert!(app.icon_path.is_empty());

    assert!(!c.heal_icon(&app.uuid, &AtomicBool::new(false)).unwrap());
}

#[test]
fn a_refresh_that_finds_a_different_format_replaces_the_old_icon_file() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);
    let app = c.adopt_external(&path).unwrap();
    let old_icon = app.icon_path.clone();
    assert!(old_icon.ends_with(".png"));
    // The same file now ships an SVG instead (an updated build, say).
    let desktop = desktop_entry("Quill", "1.1.0", "Icon=quill");
    let rebuilt = real_appimage(
        h.tmp.path().join("AppImages").as_path(),
        "rebuilt.AppImage",
        Some(UPD),
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("quill.svg", TINY_SVG),
        ],
    );
    std::fs::copy(&rebuilt, &path).unwrap();

    c.refresh_metadata(&app.uuid, &AtomicBool::new(false))
        .unwrap();

    let after = c.registry().by_uuid(&app.uuid).unwrap();
    assert!(
        after.icon_path.ends_with(".svg"),
        "icon: {}",
        after.icon_path
    );
    assert!(
        !Path::new(&old_icon).exists(),
        "the old PNG was left behind"
    );
}

#[test]
fn an_icon_path_outside_our_folders_is_never_deleted_by_a_refresh() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);
    let mut bare = c
        .registry_mut()
        .adopt_external("quill-download".into(), path, false)
        .unwrap();
    // A record that names someone else's file, with this app's id in its name.
    let theirs = h.tmp.path().join(format!("keep-{}.png", bare.uuid));
    std::fs::write(&theirs, b"someone else's file").unwrap();
    bare.icon_path = theirs.to_string_lossy().into_owned();
    c.registry_mut().upsert(bare.clone()).unwrap();

    c.refresh_metadata(&bare.uuid, &AtomicBool::new(false))
        .unwrap();

    assert!(
        theirs.is_file(),
        "a file outside our icon folders was deleted"
    );
    let after = c.registry().by_uuid(&bare.uuid).unwrap();
    assert!(after
        .icon_path
        .starts_with(c.settings().app_icons_dir().to_str().unwrap()));
}

#[test]
fn healing_an_already_read_app_keeps_its_recorded_checksum() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let path = quill_in_managed_folder(&h, TINY_PNG, "quill.png");
    let mut c = controller(&h);
    let app = c.adopt_external(&path).unwrap();
    assert!(!app.sha256.is_empty());
    // Lose the icon file; the row was fully read when it was adopted.
    std::fs::remove_file(&app.icon_path).unwrap();

    assert!(c.heal_icon(&app.uuid, &AtomicBool::new(false)).unwrap());

    let after = c.registry().by_uuid(&app.uuid).unwrap();
    assert_eq!(
        after.sha256, app.sha256,
        "the checksum is not blanked or redone"
    );
    assert!(Path::new(&after.icon_path).is_file());
}

#[test]
fn healing_never_rewrites_a_menu_entry_that_is_no_longer_ours() {
    if !tools() {
        return;
    }
    let h = Harness::new();
    let dir = h.tmp.path().join("Downloads");
    std::fs::create_dir_all(&dir).unwrap();
    let desktop = desktop_entry("Quill", "1.0.0", "Icon=quill");
    let source = real_appimage(
        &dir,
        "Quill.AppImage",
        None,
        &[
            Member::File("quill.desktop", desktop.as_bytes()),
            Member::File("quill.png", TINY_PNG),
        ],
    );
    let mut c = controller(&h);
    let done = c.integrate(
        &IntegrateRequest {
            source_path: source.to_string_lossy().into_owned(),
            conflict: ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: CopyMode::Copy,
            assume_yes: true,
            confirm_unsafe: false,
        },
        &AtomicBool::new(false),
    );
    assert!(done.ok, "integrate failed: {}", done.error);
    let app = done.app;
    // Someone replaces the entry with their own file, and the icon goes missing.
    let theirs = "[Desktop Entry]\nType=Application\nName=My own launcher\nExec=/opt/mine\n";
    std::fs::write(&app.desktop_path, theirs).unwrap();
    std::fs::remove_file(&app.icon_path).unwrap();

    assert!(c.heal_icon(&app.uuid, &AtomicBool::new(false)).unwrap());

    assert_eq!(
        std::fs::read_to_string(&app.desktop_path).unwrap(),
        theirs,
        "a menu entry without our markers was overwritten"
    );
    assert!(
        Path::new(&app.icon_path).is_file(),
        "the icon is still restored"
    );
}
