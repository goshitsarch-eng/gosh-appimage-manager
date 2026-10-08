//! Settings: read and change the saved preferences, and the login-time check.

use std::path::PathBuf;

use goshaim_core::controller::AppController;
use goshaim_core::types::Appearance;

use crate::api::common::{classify, controller, guard, CoreError};

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

pub fn load_settings() -> Result<SettingsDto, CoreError> {
    guard(|| {
        let controller = controller()?;
        Ok(settings_dto(&controller))
    })
}

/// Save the given preferences. Each change is written to disk as it is made.
pub fn save_settings(patch: SettingsPatchDto) -> Result<SettingsDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        {
            let settings = controller.settings_mut();
            if let Some(folder) = patch.managed_folder {
                settings
                    .set_managed_folder(PathBuf::from(folder))
                    .map_err(classify)?;
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
        Ok(settings_dto(&controller))
    })
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
