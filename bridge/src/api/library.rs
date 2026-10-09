//! Library operations: the list, one app's details, launch, reveal, arguments
//! and environment, update sources, adopt, metadata refresh, and removal.

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, OnceLock};

use goshaim_core::controller::AppController;
use goshaim_core::types::{EnvPair, InstalledApp, RemovalMode, RemovalRequest, TaskKind};

use crate::api::common::{classify, controller, guard, CoreError, ErrorKind, OperationGuard};
use crate::api::dto::{AppDto, DiscoveredDto, EnvVarDto, KeyValueDto, OutcomeDto};

/// Everything the Library page shows, in one call.
#[derive(Debug, Clone)]
pub struct LibraryDto {
    pub apps: Vec<AppDto>,
    pub discovered: Vec<DiscoveredDto>,
}

/// The registered apps, which of them are running, and what discovery finds.
pub fn list_library() -> Result<LibraryDto, CoreError> {
    guard(|| {
        let controller = controller()?;
        let apps = controller.registry().apps();
        let running = controller.running_uuids(&apps);
        let apps = apps
            .iter()
            .map(|app| AppDto::from_core(app, running.contains(&app.uuid)))
            .collect();
        // An unreadable managed folder is an error the Library shows, not an
        // empty list that reads as "no apps".
        let discovered = controller
            .discover()
            .map_err(classify)?
            .iter()
            .map(DiscoveredDto::from_core)
            .collect();
        Ok(LibraryDto { apps, discovered })
    })
}

/// One app with its current running state.
pub fn get_app(uuid: String) -> Result<AppDto, CoreError> {
    guard(move || {
        let controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        let running = controller.is_running(&app);
        Ok(AppDto::from_core(&app, running))
    })
}

/// Start the app, detached. The manager never waits for it or stops it.
pub fn launch_app(uuid: String) -> Result<(), CoreError> {
    guard(move || {
        let controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        controller.launch_service().launch(&app).map_err(classify)
    })
}

/// Open the file manager at the app's folder.
pub fn reveal_app(uuid: String) -> Result<(), CoreError> {
    guard(move || {
        let controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        controller
            .reveal_in_file_manager(&app.managed_path)
            .map_err(classify)
    })
}

/// Replace the app's launch arguments and environment, and rewrite its entry.
pub fn save_arguments_and_environment(
    uuid: String,
    arguments: Vec<String>,
    environment: Vec<EnvVarDto>,
) -> Result<AppDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        let pairs: Vec<EnvPair> = environment
            .into_iter()
            .map(|var| EnvPair {
                name: var.name,
                value: var.value,
            })
            .collect();
        controller
            .set_arguments_and_environment(&uuid, arguments, pairs)
            .map_err(classify)?;
        let app = find_app(&controller, &uuid)?;
        let running = controller.is_running(&app);
        Ok(AppDto::from_core(&app, running))
    })
}

/// Choose how the app checks for updates. The config is the manager's keys.
pub fn set_update_source(
    uuid: String,
    manager: String,
    config: Vec<KeyValueDto>,
) -> Result<AppDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        set_update_source_on(&mut controller, uuid, manager, config)
    })
}

/// The body of `set_update_source`, run on a controller the caller provides, so a
/// test can run it on scratch seams. A refusal carries the core's message in full.
fn set_update_source_on(
    controller: &mut AppController,
    uuid: String,
    manager: String,
    config: Vec<KeyValueDto>,
) -> Result<AppDto, CoreError> {
    let app = find_app(controller, &uuid)?;
    let config: BTreeMap<String, String> = config
        .into_iter()
        .map(|pair| (pair.key, pair.value))
        .collect();
    let mut error = String::new();
    if !controller.set_update_source(app, &manager, config, &mut error) {
        if error.is_empty() {
            return Err(CoreError::new(
                ErrorKind::Validation,
                "The update source could not be saved.",
            ));
        }
        return Err(classify(error));
    }
    let app = find_app(controller, &uuid)?;
    let running = controller.is_running(&app);
    Ok(AppDto::from_core(&app, running))
}

/// Remove the app's update source, so it is no longer checked.
pub fn unset_update_source(uuid: String) -> Result<AppDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        let mut error = String::new();
        if !controller.unset_update_source(app, &mut error) {
            return Err(classify(error));
        }
        let app = find_app(&controller, &uuid)?;
        let running = controller.is_running(&app);
        Ok(AppDto::from_core(&app, running))
    })
}

/// Register an external AppImage and read what the file says about itself: its
/// name, version, icon and update information. Its menu entry and the user's
/// icon theme are not touched; the icon is kept in the manager's own folder.
pub fn adopt_path(op_id: String, path: String) -> Result<AppDto, CoreError> {
    guard(move || {
        let op = OperationGuard::begin(&op_id, TaskKind::Adopt, "Adopting", &path);
        let mut controller = controller()?;
        match controller.adopt_external_with(&path, op.cancel_flag()) {
            Ok(app) => {
                op.finish(Ok(()));
                Ok(AppDto::from_core(&app, false))
            }
            Err(error) => {
                op.finish(Err(error.clone()));
                Err(classify(error))
            }
        }
    })
}

/// The apps this process has already tried to give an icon, so each is looked
/// at once however often the Library asks.
fn icon_attempts() -> &'static Mutex<HashSet<String>> {
    static ATTEMPTED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ATTEMPTED.get_or_init(Default::default)
}

/// Give the apps that have no icon file another look at their AppImage, and
/// return how many records changed.
///
/// Apps adopted before adoption read the file, and apps integrated before icons
/// were found in every layout, have none; opening the file again gives it to
/// them. It is quiet maintenance, not a task on the Tasks page, and each app is
/// tried once per run. An app whose file cannot be read is left as it is.
pub fn heal_library_icons() -> Result<i64, CoreError> {
    guard(|| {
        let mut controller = controller()?;
        Ok(heal_icons_on(&mut controller, icon_attempts()))
    })
}

/// The body of `heal_library_icons`, run on a controller and an attempt log the
/// caller provides, so a test can run it on scratch seams.
fn heal_icons_on(controller: &mut AppController, attempts: &Mutex<HashSet<String>>) -> i64 {
    let never_cancelled = AtomicBool::new(false);
    let mut changed = 0;
    for app in controller.apps_missing_icons() {
        let first_try = attempts
            .lock()
            .map(|mut seen| seen.insert(format!("{}\n{}", app.uuid, app.managed_path)))
            .unwrap_or(false);
        if !first_try {
            continue;
        }
        if controller
            .heal_icon(&app.uuid, &never_cancelled)
            .unwrap_or(false)
        {
            changed += 1;
        }
    }
    changed
}

/// Re-read the app's metadata, icon and update information, and rewrite its
/// menu entry.
pub fn refresh_metadata(op_id: String, uuid: String) -> Result<AppDto, CoreError> {
    guard(move || {
        let op = OperationGuard::begin(
            &op_id,
            TaskKind::RefreshMetadata,
            "Refreshing metadata",
            &uuid,
        );
        let mut controller = controller()?;
        match controller.refresh_metadata(&uuid, op.cancel_flag()) {
            Ok(_name) => {
                let app = find_app(&controller, &uuid)?;
                op.finish(Ok(()));
                let running = controller.is_running(&app);
                Ok(AppDto::from_core(&app, running))
            }
            Err(error) => {
                op.finish(Err(error.clone()));
                Err(classify(error))
            }
        }
    })
}

/// Remove an app: to the Trash by default, or permanently when asked.
pub fn remove_app(op_id: String, uuid: String, permanent: bool) -> Result<OutcomeDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        remove_with(&mut controller, &op_id, uuid, permanent)
    })
}

/// The body of `remove_app`, run on a controller the caller provides. Split out
/// so a test can run it on fake seams instead of the real Trash.
fn remove_with(
    controller: &mut AppController,
    op_id: &str,
    uuid: String,
    permanent: bool,
) -> Result<OutcomeDto, CoreError> {
    let app = find_app(controller, &uuid)?;
    let request = RemovalRequest {
        path_or_uuid: uuid,
        mode: if permanent {
            RemovalMode::Permanent
        } else {
            RemovalMode::Trash
        },
        assume_yes: true,
    };
    let op = OperationGuard::begin(op_id, TaskKind::Remove, "Removing", &app.name);
    // Recorded as the task starts, so the Tasks entry reads "Deleted" for a
    // permanent removal and "Moved ... to the Trash" for the other mode.
    op.set_permanent(request.mode == RemovalMode::Permanent);
    let result = controller.remove_app(&request);
    let outcome = OutcomeDto::from_removal(&result);
    // The Tasks entry names the version that was removed.
    op.versions(&app.version, "");
    op.finish(if result.ok {
        Ok(())
    } else {
        Err(result.error.clone())
    });
    Ok(outcome)
}

pub(crate) fn find_app(controller: &AppController, uuid: &str) -> Result<InstalledApp, CoreError> {
    controller
        .registry()
        .by_uuid(uuid)
        .ok_or_else(|| CoreError::new(ErrorKind::NotFound, "No installed app with that id."))
}

#[cfg(test)]
mod removal_mode_tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use goshaim_core::controller::AppController;
    use goshaim_core::network::ReqwestClient;
    use goshaim_core::process::SystemRunner;
    use goshaim_core::proctable::SysTable;
    use goshaim_core::settings::Dirs;
    use goshaim_core::trash::FakeTrash;
    use goshaim_core::types::InstalledApp;

    use super::remove_with;
    use crate::api::dto::{TaskDto, TaskKindDto};
    use crate::api::system::list_tasks;

    /// A scratch home for one test, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("gosh-d04-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("a scratch home");
            Self(root)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A controller rooted in `root`, with a fake Trash, so the test never
    /// touches the user's real registry or real Trash.
    fn sandboxed_controller(root: &Path) -> AppController {
        AppController::with_seams(
            Box::new(SystemRunner::new()),
            Box::new(ReqwestClient::new()),
            Box::new(SysTable::new()),
            Box::new(FakeTrash::new()),
            Dirs::under(root),
        )
        .expect("a controller rooted in the scratch home")
    }

    /// Register an owned AppImage in a managed folder under `root`, backed by a
    /// real file. The folder is a subdirectory: the core refuses permanent
    /// deletion of anything sitting directly in the home directory.
    fn register_owned_app(
        controller: &mut AppController,
        root: &Path,
        name: &str,
        uuid: &str,
    ) -> PathBuf {
        let folder = root.join("AppImages");
        fs::create_dir_all(&folder).expect("the managed folder");
        let managed = folder.join(format!("{name}.AppImage"));
        fs::write(&managed, b"fixture, not an AppImage").expect("the fixture file");
        let app = InstalledApp {
            uuid: uuid.to_string(),
            name: name.to_string(),
            version: "1.0".to_string(),
            managed_path: managed.to_string_lossy().into_owned(),
            ..InstalledApp::new_owned()
        };
        controller
            .registry_mut()
            .upsert(app)
            .expect("the registry accepts the app");
        managed
    }

    /// The Tasks-page entry for the removal of `name`, as the front end reads it.
    fn removal_listed_for(name: &str) -> TaskDto {
        list_tasks()
            .into_iter()
            .find(|task| matches!(task.kind, TaskKindDto::Remove) && task.target == name)
            .unwrap_or_else(|| panic!("no removal of {name} is listed on the Tasks page"))
    }

    #[test]
    fn a_permanent_removal_records_permanent_and_a_trash_removal_does_not() {
        let scratch = Scratch::new("mode");
        let mut controller = sandboxed_controller(&scratch.0);
        let deleted_file =
            register_owned_app(&mut controller, &scratch.0, "D04 Delete", "d04-delete");
        register_owned_app(&mut controller, &scratch.0, "D04 Trash", "d04-trash");

        let deleted = remove_with(
            &mut controller,
            "d04-op-delete",
            "d04-delete".to_string(),
            true,
        )
        .expect("the permanent removal runs");
        assert!(
            deleted.ok,
            "the permanent removal succeeds: {}",
            deleted.message
        );
        assert!(
            !deleted_file.exists(),
            "a permanent removal deletes the file"
        );

        let trashed = remove_with(
            &mut controller,
            "d04-op-trash",
            "d04-trash".to_string(),
            false,
        )
        .expect("the Trash removal runs");
        assert!(
            trashed.ok,
            "the Trash removal succeeds: {}",
            trashed.message
        );

        assert!(
            removal_listed_for("D04 Delete").permanent,
            "a permanent removal must be recorded as permanent=true"
        );
        assert!(
            !removal_listed_for("D04 Trash").permanent,
            "a Trash removal must be recorded as permanent=false"
        );
    }
}

#[cfg(test)]
mod update_source_tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use goshaim_core::controller::AppController;
    use goshaim_core::network::ReqwestClient;
    use goshaim_core::process::SystemRunner;
    use goshaim_core::proctable::SysTable;
    use goshaim_core::settings::Dirs;
    use goshaim_core::trash::FakeTrash;
    use goshaim_core::types::InstalledApp;

    use super::set_update_source_on;
    use crate::api::dto::KeyValueDto;

    /// A scratch home for one test, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("gosh-r7-source-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("a scratch home");
            Self(root)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn controller_under(root: &Path) -> AppController {
        AppController::with_seams(
            Box::new(SystemRunner::new()),
            Box::new(ReqwestClient::new()),
            Box::new(SysTable::new()),
            Box::new(FakeTrash::new()),
            Dirs::under(root),
        )
        .expect("a controller rooted in the scratch home")
    }

    /// One owned app, registered in the scratch home.
    fn register_app(root: &Path, name: &str, uuid: &str) {
        let app = InstalledApp {
            uuid: uuid.to_string(),
            name: name.to_string(),
            version: "1.0".to_string(),
            managed_path: root
                .join("AppImages")
                .join(format!("{name}.AppImage"))
                .to_string_lossy()
                .into_owned(),
            ..InstalledApp::new_owned()
        };
        controller_under(root)
            .registry_mut()
            .upsert(app)
            .expect("the registry accepts the app");
    }

    /// A GitHub source with no owner is refused, and the refusal gives the command
    /// filled with the filename the user did give.
    #[test]
    fn a_github_source_without_a_repo_is_refused_with_the_command_to_run() {
        let scratch = Scratch::new("github-no-repo");
        register_app(&scratch.0, "Quill Notes", "quill-no-repo-uuid");
        let mut controller = controller_under(&scratch.0);
        let config = vec![KeyValueDto {
            key: "filename".to_string(),
            value: "Quill-*.AppImage".to_string(),
        }];

        let error = set_update_source_on(
            &mut controller,
            "quill-no-repo-uuid".to_string(),
            "github".to_string(),
            config,
        )
        .expect_err("a source with no repo is refused");

        assert!(
            error.message.contains("GitHub needs repo=owner/name"),
            "the existing message is kept: {}",
            error.message
        );
        assert!(
            error
                .message
                .contains("Run: gosh-appimage-manager --set-update-source ")
                && error
                    .message
                    .contains("--manager github repo=<owner>/<name> filename=Quill-*.AppImage"),
            "the refusal gives the command, with the filename the user gave: {}",
            error.message
        );
    }

    /// A GitHub source given only `repo=owner/name` is refused for its missing
    /// filename, and the refusal keeps the full command that supplies it.
    #[test]
    fn a_github_source_without_a_filename_is_refused_with_the_command_to_run() {
        let scratch = Scratch::new("github-refusal");
        register_app(&scratch.0, "Quill Notes", "quill-uuid");
        let mut controller = controller_under(&scratch.0);
        let config = vec![KeyValueDto {
            key: "repo".to_string(),
            value: "example-org/quill-notes".to_string(),
        }];

        let error = set_update_source_on(
            &mut controller,
            "quill-uuid".to_string(),
            "github".to_string(),
            config,
        )
        .expect_err("a repo without a filename is refused");

        assert!(
            error
                .message
                .contains("Run: gosh-appimage-manager --set-update-source "),
            "the refusal keeps the command to run: {}",
            error.message
        );
        assert!(
            error
                .message
                .contains("--manager github repo=example-org/quill-notes filename=<asset name>"),
            "the refusal keeps the full command: {}",
            error.message
        );
    }
}

#[cfg(test)]
mod heal_tests {
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use goshaim_core::controller::AppController;
    use goshaim_core::network::ReqwestClient;
    use goshaim_core::process::SystemRunner;
    use goshaim_core::proctable::SysTable;
    use goshaim_core::settings::Dirs;
    use goshaim_core::trash::FakeTrash;

    use super::heal_icons_on;

    /// A scratch home for one test, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("gosh-heal-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("a scratch home");
            Self(root)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn controller_in(root: &Path) -> AppController {
        AppController::with_seams(
            Box::new(SystemRunner::new()),
            Box::new(ReqwestClient::new()),
            Box::new(SysTable::new()),
            Box::new(FakeTrash::new()),
            Dirs::under(root),
        )
        .expect("a controller rooted in the scratch home")
    }

    /// An adopted row for a file that is not an AppImage: it has no icon, and
    /// reading it can give none.
    fn row_for_an_unreadable_file(controller: &mut AppController, root: &Path) -> String {
        let folder = root.join("AppImages");
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("Broken.AppImage");
        fs::write(&file, b"not an elf").unwrap();
        controller
            .registry_mut()
            .adopt_external("Broken".into(), file.to_string_lossy().into_owned(), false)
            .expect("the row is added")
            .uuid
    }

    #[test]
    fn a_file_that_cannot_be_read_is_not_an_error_and_not_counted() {
        let scratch = Scratch::new("unreadable");
        let mut controller = controller_in(&scratch.0);
        let uuid = row_for_an_unreadable_file(&mut controller, &scratch.0);
        let attempts = Mutex::new(HashSet::new());

        assert_eq!(heal_icons_on(&mut controller, &attempts), 0);

        // The row is exactly as it was.
        let row = controller.registry().by_uuid(&uuid).unwrap();
        assert!(row.icon_path.is_empty());
        assert_eq!(row.name, "Broken");
    }

    #[test]
    fn each_app_is_tried_once_however_often_the_library_asks() {
        let scratch = Scratch::new("once");
        let mut controller = controller_in(&scratch.0);
        row_for_an_unreadable_file(&mut controller, &scratch.0);
        let attempts = Mutex::new(HashSet::new());

        heal_icons_on(&mut controller, &attempts);
        assert_eq!(attempts.lock().unwrap().len(), 1, "the app was tried");

        // A second pass finds the same app missing its icon and skips it.
        let before = attempts.lock().unwrap().clone();
        assert_eq!(controller.apps_missing_icons().len(), 1);
        heal_icons_on(&mut controller, &attempts);
        assert_eq!(*attempts.lock().unwrap(), before);
    }

    #[test]
    fn an_app_whose_file_is_gone_is_not_tried_at_all() {
        let scratch = Scratch::new("gone");
        let mut controller = controller_in(&scratch.0);
        let uuid = row_for_an_unreadable_file(&mut controller, &scratch.0);
        let row = controller.registry().by_uuid(&uuid).unwrap();
        fs::remove_file(&row.managed_path).unwrap();
        let attempts = Mutex::new(HashSet::new());

        assert_eq!(heal_icons_on(&mut controller, &attempts), 0);
        assert!(attempts.lock().unwrap().is_empty());
    }
}
