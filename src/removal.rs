// Gosh AppImage Manager — removal service (ports RemovalService).
// Trash first; owned artifacts only after Trash succeeds. Trash failure
// never becomes delete. Permanent deletion needs an owned, canonical,
// non-symlink, non-protected target.

use std::fs;
use std::path::{Path, PathBuf};

use crate::desktop;
use crate::process::{ProcessRequest, ProcessRunner};
use crate::registry::ManagedRegistry;
use crate::safe_fs;
use crate::settings::SettingsStore;
use crate::trash::TrashSink;
use crate::types::{InstalledApp, RemovalMode, RemovalRequest, RemovalResult};

pub struct RemovalService<'a> {
    settings: &'a SettingsStore,
    trash: &'a dyn TrashSink,
    runner: &'a dyn ProcessRunner,
}

impl<'a> RemovalService<'a> {
    pub fn new(
        settings: &'a SettingsStore,
        trash: &'a dyn TrashSink,
        runner: &'a dyn ProcessRunner,
    ) -> Self {
        Self {
            settings,
            trash,
            runner,
        }
    }

    /// Trash one file: native trash, then `gio trash` on the host.
    pub fn trash_file(&self, path: &str) -> Result<(), String> {
        if self.trash.trash(Path::new(path)).is_ok() {
            return Ok(());
        }
        let out = self.runner.run(&ProcessRequest {
            program: "gio".to_string(),
            args: vec!["trash".to_string(), path.to_string()],
            host: true,
            timeout_ms: 10_000,
            ..Default::default()
        });
        if out.exit_code == 0 && !out.refused && !out.timed_out {
            return Ok(());
        }
        Err("Trash failed; leaving files intact".to_string())
    }

    pub fn resolve(
        &self,
        registry: &ManagedRegistry,
        path_or_uuid: &str,
    ) -> Result<InstalledApp, String> {
        if let Some(app) = registry.by_uuid(path_or_uuid) {
            if app.owned {
                return Ok(app);
            }
        }
        if let Some(app) = registry.by_path(path_or_uuid) {
            if app.owned {
                return Ok(app);
            }
        }
        Err("Not an owned managed AppImage".to_string())
    }

    pub fn remove(&self, registry: &mut ManagedRegistry, req: &RemovalRequest) -> RemovalResult {
        let mut result = RemovalResult::default();
        let app = match self.resolve(registry, &req.path_or_uuid) {
            Ok(app) => app,
            Err(error) => {
                result.error = error;
                return result;
            }
        };
        // Ownership markers must verify before anything is touched.
        if !app.desktop_path.is_empty() && Path::new(&app.desktop_path).exists() {
            let ownership = desktop::verify_ownership(Path::new(&app.desktop_path));
            if !ownership.owned || ownership.uuid != app.uuid {
                result.error = "Desktop file is missing ownership markers".to_string();
                return result;
            }
        }
        if !app.icon_path.is_empty()
            && Path::new(&app.icon_path).exists()
            && !app.icon_path.contains(&app.uuid)
        {
            result.error = "Icon path is not owned by this installation".to_string();
            return result;
        }
        let canonical = fs::canonicalize(&app.managed_path).ok();
        let missing = canonical.is_none();
        if !missing {
            let canon = canonical.clone().unwrap();
            if req.mode == RemovalMode::Permanent {
                if is_forbidden_permanent_target(&canon, &self.settings.dirs().home) {
                    result.error = "Refusing to permanently delete a protected path".to_string();
                    return result;
                }
                if fs::symlink_metadata(&app.managed_path)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    result.error =
                        "Refusing to follow a symlink for permanent deletion".to_string();
                    return result;
                }
                if fs::remove_file(&app.managed_path).is_err() {
                    result.error = format!("Cannot delete {}", app.managed_path);
                    return result;
                }
            } else if let Err(error) = self.trash_file(&app.managed_path) {
                result.error = error;
                return result;
            }
        }

        // Owned artifacts only (verified above); registry always updated.
        let mut artifacts_ok = true;
        let mut artifact_error = String::new();
        let has_desktop = !app.desktop_path.is_empty() && Path::new(&app.desktop_path).exists();
        let has_icon = !app.icon_path.is_empty()
            && app.icon_path.contains(&app.uuid)
            && Path::new(&app.icon_path).exists();
        if has_desktop || has_icon {
            if let Err(e) = remove_owned_artifacts(&app) {
                artifacts_ok = false;
                artifact_error = e;
            }
        } else if !app.icon_path.is_empty()
            && app.icon_path.contains(&app.uuid)
            && Path::new(&app.icon_path).exists()
        {
            let _ = fs::remove_file(&app.icon_path);
        }
        let saved = registry.remove_uuid(&app.uuid).is_ok();
        if !artifacts_ok || !saved {
            result.partial = true;
            result.error = if artifacts_ok {
                String::new()
            } else {
                artifact_error
            };
            if result.error.is_empty() {
                result.error =
                    "AppImage removed but owned artifacts or registry could not be fully cleaned"
                        .to_string();
            }
            return result;
        }
        result.ok = true;
        if missing {
            result.error = "already gone".to_string();
        }
        result
    }
}

/// Remove desktop + icon artifacts proven owned (uuid marker / uuid in name).
pub fn remove_owned_artifacts(app: &InstalledApp) -> Result<(), String> {
    if !app.desktop_path.is_empty() {
        let ownership = desktop::verify_ownership(Path::new(&app.desktop_path));
        if !ownership.owned || ownership.uuid != app.uuid {
            return Err("Desktop file is missing ownership markers".to_string());
        }
        if Path::new(&app.desktop_path).exists() && fs::remove_file(&app.desktop_path).is_err() {
            return Err(format!("Cannot remove {}", app.desktop_path));
        }
    }
    if !app.icon_path.is_empty() {
        if !app.icon_path.contains(&app.uuid) {
            return Err("Icon path is not owned by this installation".to_string());
        }
        if Path::new(&app.icon_path).exists() && fs::remove_file(&app.icon_path).is_err() {
            return Err(format!("Cannot remove {}", app.icon_path));
        }
    }
    Ok(())
}

/// Protected targets for permanent deletion: filesystem roots, home
/// directories, and anything at their top level. Fails closed.
pub fn is_forbidden_permanent_target(canonical: &Path, home: &Path) -> bool {
    if canonical == Path::new("/") {
        return true;
    }
    for root in [Path::new("/home"), Path::new("/root"), home] {
        if canonical == root {
            return true;
        }
        if let Some(parent) = canonical.parent() {
            if parent == Path::new("/") || parent == Path::new("/home") || parent == home {
                return true;
            }
        }
    }
    // Never delete the managed file's own containing tree roots.
    if canonical.components().count() <= 2 {
        return true;
    }
    false
}

/// Canonicalize only when the file exists (mirrors canonicalExisting).
pub fn canonical_existing(path: &str) -> Option<PathBuf> {
    let p = Path::new(path);
    if !p.exists() {
        return None;
    }
    safe_fs::canonical_bounded(p).ok()
}
