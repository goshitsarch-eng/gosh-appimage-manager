// Gosh AppImage Manager — update service (ports UpdateService).
// Check-only flows never download. Apply stages the full file, validates it
// as an AppImage, atomically replaces the live file, and keeps rollback
// material until the registry commit succeeds.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::desktop;
use crate::elf;
use crate::inspector::parse_upd_info;
use crate::limits;
use crate::network::NetworkClient;
use crate::proctable::ProcessTable;
use crate::registry::ManagedRegistry;
use crate::removal::canonical_existing;
use crate::safe_fs;
use crate::settings::SettingsStore;
use crate::types::{InstalledApp, IntegrateResult, UpdateFailPoint, UpdateOffer};
use crate::updates_sources::{Config, UpdateCheckResult, UpdateSourceFactory};

/// Resolved (manager, config) pair for one app.
type ResolvedSource = (Box<dyn crate::updates_sources::UpdateSource>, Config);

pub struct UpdateService<'a> {
    settings: &'a SettingsStore,
    network: &'a dyn NetworkClient,
    processes: &'a dyn ProcessTable,
    fail_point: UpdateFailPoint,
}

impl<'a> UpdateService<'a> {
    pub fn new(
        settings: &'a SettingsStore,
        network: &'a dyn NetworkClient,
        processes: &'a dyn ProcessTable,
    ) -> Self {
        Self {
            settings,
            network,
            processes,
            fail_point: UpdateFailPoint::None,
        }
    }

    pub fn set_fail_point(&mut self, point: UpdateFailPoint) {
        self.fail_point = point;
    }

    /// Resolve the effective (manager, config) for an app.
    fn effective_source(&self, app: &InstalledApp) -> Result<ResolvedSource, String> {
        if !app.update_manager.is_empty() {
            let source = UpdateSourceFactory::by_name(&app.update_manager)
                .ok_or_else(|| format!("Unknown update manager: {}", app.update_manager))?;
            let config = if app.update_config.is_empty() && !app.embedded_update.is_empty() {
                let fields = parse_upd_info(app.embedded_update.as_bytes()).fields;
                source.config_from_embedded(&fields)
            } else {
                app.update_config.clone()
            };
            source.validate_config(&config)?;
            return Ok((source, config));
        }
        if app.embedded_update.is_empty() {
            return Err("No update method was found".to_string());
        }
        let source = UpdateSourceFactory::detect_embedded(&app.embedded_update)
            .ok_or_else(|| "No update method was found".to_string())?;
        let fields = parse_upd_info(app.embedded_update.as_bytes()).fields;
        let config = source.config_from_embedded(&fields);
        source.validate_config(&config)?;
        Ok((source, config))
    }

    /// Metadata-only check (never downloads the AppImage itself).
    pub fn check(&self, app: &InstalledApp, _cancel: &AtomicBool) -> UpdateCheckResult {
        let (source, config) = match self.effective_source(app) {
            Ok(pair) => pair,
            Err(error) => {
                return UpdateCheckResult {
                    error,
                    ..Default::default()
                }
            }
        };
        let mut result = source.check(app, &config, self.network);
        if result.manager.is_empty() {
            result.manager = source.name().to_string();
        }
        result
    }

    /// List offers for apps whose remote version differs. Check-only.
    pub fn list_updates(
        &self,
        registry: &ManagedRegistry,
        cancel: &AtomicBool,
    ) -> Vec<UpdateOffer> {
        let mut offers = Vec::new();
        for app in registry.apps() {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if app.update_manager.is_empty() && app.embedded_update.is_empty() {
                continue;
            }
            let checked = self.check(&app, cancel);
            if !checked.ok || !checked.available || checked.url.is_empty() {
                continue;
            }
            if checked.version.is_empty() || checked.version == app.version {
                continue;
            }
            let running = {
                let canon = canonical_existing(&app.managed_path)
                    .unwrap_or_else(|| PathBuf::from(&app.managed_path));
                self.processes.is_running(&canon.to_string_lossy())
            };
            offers.push(UpdateOffer {
                uuid: app.uuid.clone(),
                name: app.name.clone(),
                current_version: app.version.clone(),
                available_version: checked.version.clone(),
                manager: checked.manager.clone(),
                url: checked.url.clone(),
                download_size: checked.size,
                digest: checked.digest.clone(),
                reduced_verification: checked.reduced_verification,
                embedded_source: app.embedded_update.clone(),
                running,
            });
        }
        offers
    }

    /// Download, validate, atomically replace, and re-register.
    pub fn apply(
        &self,
        registry: &mut ManagedRegistry,
        app: &InstalledApp,
        force: bool,
        cancel: &AtomicBool,
    ) -> IntegrateResult {
        let mut result = IntegrateResult::default();
        if !app.owned {
            result.error = "Not an owned managed AppImage".to_string();
            return result;
        }
        let checked = self.check(app, cancel);
        if !checked.ok {
            result.error = checked.error;
            return result;
        }
        if !checked.available || checked.url.is_empty() {
            result.error = "No update available".to_string();
            return result;
        }
        let canon = canonical_existing(&app.managed_path)
            .unwrap_or_else(|| PathBuf::from(&app.managed_path));
        let running = self.processes.is_running(&canon.to_string_lossy());
        if running && !force {
            result.error = "Application is running; use --force to override".to_string();
            return result;
        }
        let managed_dir = self.settings.managed_folder().to_path_buf();
        if safe_fs::mkdir_0700(&managed_dir).is_err() {
            result.error = "Cannot prepare managed folder".to_string();
            return result;
        }
        let live = PathBuf::from(&app.managed_path);
        let staging = safe_fs::sibling_temp(&live, ".gosh-upd-");
        let max_bytes = self.settings.max_appimage_bytes() as u64;
        let body = match self.network.download_bounded(&checked.url, max_bytes) {
            Ok(body) => body,
            Err(error) => {
                let _ = fs::remove_file(&staging);
                result.error = error;
                return result;
            }
        };
        if let Err(error) = safe_fs::atomic_write(&staging, &body, 0o755) {
            let _ = fs::remove_file(&staging);
            result.error = error;
            return result;
        }
        if self.fail_point == UpdateFailPoint::AfterDownload {
            let _ = fs::remove_file(&staging);
            result.error = "Injected failure at AfterDownload".to_string();
            return result;
        }
        // Validate the staged file as an AppImage (unsafe fallback never).
        let staged_info = elf::parse_file(&staging, limits::ELF_HEADER_READ_BYTES);
        if !staged_info.error.is_empty()
            || matches!(
                staged_info.app_image_type,
                crate::types::AppImageType::Unknown
            )
            || matches!(
                staged_info.architecture,
                crate::types::Architecture::Unknown
            )
        {
            let _ = fs::remove_file(&staging);
            result.error = "Downloaded file is not a valid AppImage".to_string();
            return result;
        }
        if !checked.digest.is_empty() {
            let sum = safe_fs::sha256_file(&staging, cancel).unwrap_or_default();
            if hex::encode(&sum) != checked.digest.to_lowercase() {
                let _ = fs::remove_file(&staging);
                result.error = "Staged update failed digest verification".to_string();
                return result;
            }
        }
        // Rollback copy of the live file.
        let backup = safe_fs::sibling_temp(&live, ".gosh-upd-bak-");
        if live.exists() && fs::copy(&live, &backup).is_err() {
            let _ = fs::remove_file(&staging);
            result.error = "Cannot create replacement backup".to_string();
            return result;
        }
        if self.fail_point == UpdateFailPoint::BackupCreate {
            let _ = fs::remove_file(&staging);
            let _ = fs::remove_file(&backup);
            result.error = "Injected failure at BackupCreate".to_string();
            return result;
        }
        if safe_fs::rename_over(&staging, &live).is_err() {
            let _ = fs::rename(&backup, &live);
            let _ = fs::remove_file(&staging);
            result.error = "Cannot replace AppImage".to_string();
            return result;
        }
        if self.fail_point == UpdateFailPoint::AfterReplace {
            let _ = fs::rename(&backup, &live);
            result.error = "Injected failure at AfterReplace".to_string();
            return result;
        }
        // Refresh desktop entry (same path, new version marker) + registry.
        let snapshot = registry.snapshot();
        let mut updated = app.clone();
        updated.version = checked.version.clone();
        updated.available_version.clear();
        updated.available_url.clear();
        updated.available_size = 0;
        updated.update_available = false;
        updated.digest = checked.digest.clone();
        updated.size = fs::metadata(&live)
            .map(|m| m.len() as i64)
            .unwrap_or(app.size);
        updated.sha256 = safe_fs::sha256_file(&live, cancel).unwrap_or_default();
        if self.fail_point == UpdateFailPoint::DesktopInstall {
            let _ = fs::rename(&backup, &live);
            let _ = registry.restore(snapshot);
            result.error = "Injected failure at DesktopInstall".to_string();
            return result;
        }
        if !updated.desktop_path.is_empty() {
            let body = desktop::build_desktop_file(
                &updated,
                &updated.managed_path,
                self.settings.terminal_omit_suffix(),
            );
            if safe_fs::atomic_write(Path::new(&updated.desktop_path), body.as_bytes(), 0o644)
                .is_err()
            {
                let _ = fs::rename(&backup, &live);
                let _ = registry.restore(snapshot);
                result.error = "Cannot refresh desktop entry".to_string();
                return result;
            }
        }
        if self.fail_point == UpdateFailPoint::RegistrySave {
            let _ = fs::rename(&backup, &live);
            let _ = registry.restore(snapshot);
            result.error = "Injected failure at RegistrySave".to_string();
            return result;
        }
        if registry.upsert(updated.clone()).is_err() {
            let _ = fs::rename(&backup, &live);
            let _ = registry.restore(snapshot);
            result.error = "Cannot save registry".to_string();
            return result;
        }
        // Success: rollback material is dropped only now.
        let _ = fs::remove_file(&backup);
        result.ok = true;
        result.app = updated;
        result
    }

    pub fn set_source(
        &self,
        registry: &mut ManagedRegistry,
        mut app: InstalledApp,
        manager: &str,
        config: Config,
        error: &mut String,
    ) -> bool {
        let source = match UpdateSourceFactory::by_name(manager) {
            Some(source) => source,
            None => {
                *error = format!("Unknown update manager: {manager}");
                return false;
            }
        };
        let config = if config.is_empty() && !app.embedded_update.is_empty() {
            let fields = parse_upd_info(app.embedded_update.as_bytes()).fields;
            source.config_from_embedded(&fields)
        } else {
            config
        };
        if let Err(e) = source.validate_config(&config) {
            *error = e;
            return false;
        }
        let snapshot = registry.snapshot();
        app.update_manager = source.name().to_string();
        app.update_config = config;
        if registry.upsert(app).is_err() {
            let _ = registry.restore(snapshot);
            *error = "Cannot save registry".to_string();
            return false;
        }
        true
    }

    pub fn unset_source(
        &self,
        registry: &mut ManagedRegistry,
        mut app: InstalledApp,
        error: &mut String,
    ) -> bool {
        let snapshot = registry.snapshot();
        app.update_manager.clear();
        app.update_config.clear();
        app.available_version.clear();
        app.available_url.clear();
        app.available_size = 0;
        app.update_available = false;
        if registry.upsert(app).is_err() {
            let _ = registry.restore(snapshot);
            *error = "Cannot save registry".to_string();
            return false;
        }
        true
    }
}
