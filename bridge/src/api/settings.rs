//! Settings: read and change the saved preferences, and the login-time check.

use std::path::PathBuf;

use goshaim_core::controller::AppController;
use goshaim_core::types::Appearance;

use crate::api::common::{classify, controller, guard, CoreError, ErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceChoice {
    System,
    Light,
    Dark,
}

/// Every saved preference, plus whether the login-time check is installed.
#[derive(Debug, Clone)]
pub struct SettingsDto {
    pub managed_folder: String,
    pub move_source: bool,
    pub manage_outside_folder: bool,
    pub terminal_omit_suffix: bool,
    pub background_update_checks: bool,
    pub unsafe_extraction_fallback: bool,
    pub debug_logging: bool,
    pub appearance: AppearanceChoice,
    pub max_appimage_bytes: i64,
    pub load_error: Option<String>,
    pub autostart_enabled: bool,
}

/// A partial update. Only the fields that are set are changed.
#[derive(Debug, Clone, Default)]
pub struct SettingsPatchDto {
    pub managed_folder: Option<String>,
    pub move_source: Option<bool>,
    pub manage_outside_folder: Option<bool>,
    pub terminal_omit_suffix: Option<bool>,
    pub background_update_checks: Option<bool>,
    pub unsafe_extraction_fallback: Option<bool>,
    pub debug_logging: Option<bool>,
    pub appearance: Option<AppearanceChoice>,
    pub max_appimage_bytes: Option<i64>,
}

/// Read the preferences. This runs at startup, and it first reconciles the
/// login entry with "Check in the background", so an entry left by an earlier
/// build cannot run an ungated check.
pub fn load_settings() -> Result<SettingsDto, CoreError> {
    guard(|| {
        let controller = controller()?;
        if let Err(error) = controller.reconcile_autostart() {
            eprintln!("Cannot reconcile the login check: {error}");
        }
        Ok(settings_dto(&controller))
    })
}

/// Save the given preferences. Each change is written to disk as it is made.
pub fn save_settings(patch: SettingsPatchDto) -> Result<SettingsDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        save_settings_on(&mut controller, patch)
    })
}

/// The body of `save_settings`, run on a controller the caller provides. Split
/// out so a test can run it on a scratch home instead of the user's settings.
fn save_settings_on(
    controller: &mut AppController,
    patch: SettingsPatchDto,
) -> Result<SettingsDto, CoreError> {
    // Everything that can be refused is checked before anything is changed.
    let managed_folder = match patch.managed_folder {
        Some(folder) => {
            let folder = PathBuf::from(folder);
            // A folder that is not usable is a validation error, shown as such.
            goshaim_core::settings::validate_managed_folder(&folder)
                .map_err(|message| CoreError::new(ErrorKind::Validation, message))?;
            Some(folder)
        }
        None => None,
    };
    if patch.background_update_checks == Some(false) {
        // Turning checks off removes the login entry first, so a failed
        // removal leaves the setting on, never a login check with the
        // setting off.
        controller.sync_autostart(false).map_err(classify)?;
    }
    {
        let settings = controller.settings_mut();
        if let Some(folder) = managed_folder {
            settings.set_managed_folder(folder).map_err(classify)?;
        }
        if let Some(value) = patch.move_source {
            settings.set_move_source(value).map_err(classify)?;
        }
        if let Some(value) = patch.manage_outside_folder {
            settings
                .set_manage_outside_folder(value)
                .map_err(classify)?;
        }
        if let Some(value) = patch.terminal_omit_suffix {
            settings.set_terminal_omit_suffix(value).map_err(classify)?;
        }
        if let Some(value) = patch.background_update_checks {
            settings
                .set_background_update_checks(value)
                .map_err(classify)?;
        }
        if let Some(value) = patch.unsafe_extraction_fallback {
            settings
                .set_unsafe_extraction_fallback(value)
                .map_err(classify)?;
        }
        if let Some(value) = patch.debug_logging {
            settings.set_debug_logging(value).map_err(classify)?;
        }
        if let Some(value) = patch.appearance {
            settings
                .set_appearance(to_core_appearance(value))
                .map_err(classify)?;
        }
        if let Some(value) = patch.max_appimage_bytes {
            settings.set_max_appimage_bytes(value).map_err(classify)?;
        }
    }
    Ok(settings_dto(controller))
}

/// Install or remove the login-time check. It only notifies; it never applies.
pub fn set_autostart(enabled: bool) -> Result<(), CoreError> {
    guard(move || {
        let controller = controller()?;
        controller.sync_autostart(enabled).map_err(classify)
    })
}

fn settings_dto(controller: &AppController) -> SettingsDto {
    let settings = controller.settings();
    SettingsDto {
        managed_folder: settings.managed_folder().to_string_lossy().into_owned(),
        move_source: settings.move_source(),
        manage_outside_folder: settings.manage_outside_folder(),
        terminal_omit_suffix: settings.terminal_omit_suffix(),
        background_update_checks: settings.background_update_checks(),
        unsafe_extraction_fallback: settings.unsafe_extraction_fallback(),
        debug_logging: settings.debug_logging(),
        appearance: match settings.appearance() {
            Appearance::System => AppearanceChoice::System,
            Appearance::Light => AppearanceChoice::Light,
            Appearance::Dark => AppearanceChoice::Dark,
        },
        max_appimage_bytes: settings.max_appimage_bytes(),
        load_error: settings.load_error().map(str::to_string),
        autostart_enabled: controller.autostart_desktop_path().exists(),
    }
}

fn to_core_appearance(choice: AppearanceChoice) -> Appearance {
    match choice {
        AppearanceChoice::System => Appearance::System,
        AppearanceChoice::Light => Appearance::Light,
        AppearanceChoice::Dark => Appearance::Dark,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use goshaim_core::controller::AppController;
    use goshaim_core::network::ReqwestClient;
    use goshaim_core::process::SystemRunner;
    use goshaim_core::proctable::SysTable;
    use goshaim_core::settings::Dirs;
    use goshaim_core::trash::FakeTrash;

    use super::{save_settings_on, settings_dto, SettingsPatchDto};
    use crate::api::common::ErrorKind;

    /// A scratch home for one test, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("gosh-r6-settings-{label}-{}", std::process::id()));
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

    /// A controller rooted in `root`, on fake seams, so the test never touches
    /// the real settings or the real Trash.
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

    fn folder_patch(folder: &Path) -> SettingsPatchDto {
        SettingsPatchDto {
            managed_folder: Some(folder.to_string_lossy().into_owned()),
            ..Default::default()
        }
    }

    /// Audit R6-05 (core side): the managed folder saved through the settings
    /// call is the one a new controller, that is a restarted app, reads back.
    #[test]
    fn a_managed_folder_saved_through_the_bridge_is_read_back_after_a_restart() {
        let scratch = Scratch::new("folder-round-trip");
        let folder = scratch.0.join("My AppImages");
        fs::create_dir_all(&folder).unwrap();
        let mut controller = controller_under(&scratch.0);

        let saved = save_settings_on(&mut controller, folder_patch(&folder))
            .expect("an existing folder is saved");
        assert_eq!(saved.managed_folder, folder.to_string_lossy());

        drop(controller);
        let restarted = controller_under(&scratch.0);
        assert_eq!(
            settings_dto(&restarted).managed_folder,
            folder.to_string_lossy(),
            "the restarted app reads the saved folder"
        );
    }

    /// Audit R6-02: a file offered as the managed folder is a validation error
    /// that names the reason, and the folder in use is left as it was.
    #[test]
    fn a_file_offered_as_the_managed_folder_is_a_validation_error_and_is_not_saved() {
        let scratch = Scratch::new("folder-refused");
        let default_folder = scratch.0.join("AppImages");
        let a_file = scratch.0.join("not-a-folder");
        fs::write(&a_file, b"x").unwrap();
        let mut controller = controller_under(&scratch.0);

        let error = save_settings_on(&mut controller, folder_patch(&a_file))
            .expect_err("a file is refused");
        assert!(
            matches!(error.kind, ErrorKind::Validation),
            "a refused folder is a validation error, not {:?}",
            error.kind
        );
        assert!(
            error.message.contains("must be an existing directory"),
            "the message names the rule: {}",
            error.message
        );

        let restarted = controller_under(&scratch.0);
        assert_eq!(
            settings_dto(&restarted).managed_folder,
            default_folder.to_string_lossy(),
            "the folder in use is unchanged"
        );
    }
}
