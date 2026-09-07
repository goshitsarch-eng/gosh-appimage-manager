// Gosh AppImage Manager — transactional integration (ports IntegrationService).
// Validate -> stage -> verify -> commit -> registry -> source handling.
// Any failure rolls back newly created artifacts without touching
// pre-existing files.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::desktop;
use crate::inspector::AppImageInspector;
use crate::registry::ManagedRegistry;
use crate::safe_fs;
use crate::settings::SettingsStore;
use crate::trash::TrashSink;
use crate::types::{
    ConflictPolicy, CopyMode, InspectOptions, InstalledApp, IntegrateFailPoint, IntegrateRequest,
    IntegrateResult,
};

pub struct IntegrationService<'a> {
    settings: &'a SettingsStore,
    inspector: AppImageInspector<'a>,
    runner: &'a dyn crate::process::ProcessRunner,
    trash: &'a dyn TrashSink,
    fail_point: IntegrateFailPoint,
}

impl<'a> IntegrationService<'a> {
    pub fn new(
        settings: &'a SettingsStore,
        runner: &'a dyn crate::process::ProcessRunner,
        trash: &'a dyn TrashSink,
    ) -> Self {
        Self {
            settings,
            inspector: AppImageInspector::new(runner),
            runner,
            trash,
            fail_point: IntegrateFailPoint::None,
        }
    }

    pub fn set_fail_point(&mut self, point: IntegrateFailPoint) {
        self.fail_point = point;
    }

    fn fail(&self, point: IntegrateFailPoint) -> Result<(), String> {
        if self.fail_point == point {
            return Err(format!("Injected failure at {point:?}"));
        }
        Ok(())
    }

    pub fn inspect_only(
        &self,
        path: &str,
        cancel: &AtomicBool,
        existing: Option<&str>,
    ) -> crate::types::InspectionResult {
        let options = InspectOptions {
            allow_unsafe_extract: false,
            confirm_unsafe_extract: false,
            max_bytes: self.settings.max_appimage_bytes(),
            ..Default::default()
        };
        self.inspector.inspect(path, &options, cancel, existing)
    }

    pub fn integrate(
        &self,
        registry: &mut ManagedRegistry,
        req: &IntegrateRequest,
        cancel: &AtomicBool,
    ) -> IntegrateResult {
        let mut result = IntegrateResult::default();
        match self.integrate_inner(registry, req, cancel) {
            Ok((app, source_removed)) => {
                result.ok = true;
                result.app = app;
                result.source_removed = source_removed;
            }
            Err(Failure {
                error,
                rolled_back,
                partial,
                source_removed,
            }) => {
                result.error = error;
                result.rolled_back = rolled_back;
                result.partial = partial;
                result.source_removed = source_removed;
            }
        }
        result
    }

    fn integrate_inner(
        &self,
        registry: &mut ManagedRegistry,
        req: &IntegrateRequest,
        cancel: &AtomicBool,
    ) -> Result<(InstalledApp, bool), Failure> {
        let mut rolled_back: Vec<String> = Vec::new();
        // 1. Validate + inspect before any write.
        let existing = registry
            .by_path(&req.source_path)
            .map(|a| a.uuid)
            .unwrap_or_default();
        let inspected = self.inspect_only(
            &req.source_path,
            cancel,
            if existing.is_empty() {
                None
            } else {
                Some(existing.as_str())
            },
        );
        if !inspected.magic_valid {
            return Err(Failure::new(if inspected.error.is_empty() {
                "Not a valid AppImage".to_string()
            } else {
                inspected.error.clone()
            }));
        }
        if !inspected.architecture_supported {
            return Err(Failure::new(format!(
                "Unsupported architecture: {}",
                crate::types::architecture_name(inspected.architecture)
            )));
        }
        let managed_dir = self.settings.managed_folder().to_path_buf();
        safe_fs::mkdir_0700(&managed_dir).map_err(Failure::new)?;

        // 2. Conflict gate.
        let replacing: Option<InstalledApp> = match req.conflict {
            ConflictPolicy::Replace => {
                let owned = registry.by_uuid(&req.replace_uuid);
                match owned {
                    Some(app) if app.owned => Some(app),
                    _ => {
                        return Err(Failure::new(
                            "Replace requires a specific owned managed installation".to_string(),
                        ))
                    }
                }
            }
            _ => None,
        };
        let source_name = Path::new(&req.source_path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "AppImage".to_string());
        let destination: PathBuf = match &replacing {
            Some(app) => PathBuf::from(&app.managed_path),
            None => {
                let candidate = managed_dir.join(desktop::sanitize_file_base(&source_name));
                let candidate = ensure_appimage_suffix(&candidate);
                if candidate.exists()
                    || registry
                        .by_path(candidate.to_str().unwrap_or_default())
                        .is_some()
                {
                    match req.conflict {
                        ConflictPolicy::KeepBoth => choose_keep_both(&managed_dir, &candidate),
                        ConflictPolicy::Unspecified => {
                            return Err(Failure::new(
                                "Name conflict requires keep-both or replace".to_string(),
                            ))
                        }
                        ConflictPolicy::Replace => candidate,
                    }
                } else if !inspected.existing_managed_id.is_empty()
                    && req.conflict == ConflictPolicy::Unspecified
                {
                    return Err(Failure::new(
                        "Name conflict requires keep-both or replace".to_string(),
                    ));
                } else {
                    candidate
                }
            }
        };

        // 3. Stage the AppImage next to its destination + verify.
        let staged = safe_fs::sibling_temp(&destination, ".gosh-stage-");
        let max_bytes = self.settings.max_appimage_bytes() as u64;
        safe_fs::copy_bounded(Path::new(&req.source_path), &staged, max_bytes, cancel).map_err(
            |e| {
                let _ = fs::remove_file(&staged);
                Failure::new(e)
            },
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&staged, fs::Permissions::from_mode(0o755));
        }
        // Verify size + hash of the staged copy.
        let staged_size = fs::metadata(&staged).map(|m| m.len() as i64).unwrap_or(-1);
        if staged_size != inspected.identity.size {
            let _ = fs::remove_file(&staged);
            return Err(Failure::new("Staged copy failed verification".to_string()));
        }
        let staged_hash = safe_fs::sha256_file(&staged, cancel).map_err(|e| {
            let _ = fs::remove_file(&staged);
            Failure::new(e)
        })?;
        if !inspected.identity.sha256.is_empty() && staged_hash != inspected.identity.sha256 {
            let _ = fs::remove_file(&staged);
            return Err(Failure::new("Staged copy failed verification".to_string()));
        }
        self.fail(IntegrateFailPoint::AfterStage).map_err(|e| {
            let _ = fs::remove_file(&staged);
            Failure::new(e)
        })?;

        // Identity: reuse UUID + preserve customisation on replace.
        let mut app = InstalledApp::new_owned();
        if let Some(old) = &replacing {
            app.uuid = old.uuid.clone();
            app.arguments = old.arguments.clone();
            app.default_arguments = old.default_arguments.clone();
            app.environment = old.environment.clone();
            app.update_manager = old.update_manager.clone();
            app.update_config = old.update_config.clone();
            app.actions = old.actions.clone();
        } else {
            app.uuid = ManagedRegistry::new_uuid();
        }
        app.name = if inspected.metadata.name.is_empty() {
            destination
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "AppImage".to_string())
        } else {
            inspected.metadata.name.clone()
        };
        app.version = inspected.metadata.version.clone();
        app.comment = inspected.metadata.comment.clone();
        app.managed_path = destination.to_string_lossy().into_owned();
        app.desktop_id = desktop::desktop_file_name(&app.uuid);
        app.sha256 = staged_hash;
        app.app_type = inspected.app_type;
        app.architecture = inspected.architecture;
        app.size = staged_size;
        app.terminal = inspected.metadata.terminal;
        app.website = inspected.metadata.website.clone();
        app.categories = inspected.metadata.categories.clone();
        app.mime_types = inspected.metadata.mime_types.clone();
        app.startup_wm_class = inspected.metadata.startup_wm_class.clone();
        if app.default_arguments.is_empty() {
            app.default_arguments = inspected.metadata.exec_arguments.clone();
        }
        if app.arguments.is_empty() {
            app.arguments = app.default_arguments.clone();
        }
        if app.actions.is_empty() {
            app.actions = inspected.metadata.actions.clone();
        }
        app.embedded_update = inspected.update_info.raw.clone();
        if app.update_manager.is_empty() && !inspected.update_info.manager_hint.is_empty() {
            app.update_manager = inspected.update_info.manager_hint.clone();
        }

        // 4. Install the icon, then build the desktop entry that references
        //    it. The entry used to be built first, so `Icon=` was always
        //    written from an empty icon_path -- every entry read
        //    `Icon=application-x-executable` even once an icon existed.
        let desktop_path = self.settings.applications_dir().join(&app.desktop_id);
        // Whether these artifacts existed before this transaction decides
        // whether rollback deletes them or restores them.
        let desktop_existed = desktop_path.exists();
        let staged_icon_temp: Option<(PathBuf, String)> = self.stage_icon(&inspected, &app.uuid);
        let planned_icon = staged_icon_temp
            .as_ref()
            .map(|(_, ext)| self.icon_destination(&app.uuid, ext));
        let icon_existed = planned_icon.as_ref().is_some_and(|p| p.exists());
        if let Some(icon) = &planned_icon {
            app.icon_path = icon.to_string_lossy().into_owned();
        }
        let desktop_body = desktop::build_desktop_file(
            &app,
            &app.managed_path,
            self.settings.terminal_omit_suffix(),
        );
        self.fail(IntegrateFailPoint::DesktopWrite).map_err(|e| {
            let _ = fs::remove_file(&staged);
            Failure::new(e)
        })?;
        let (final_desktop, final_icon) = self
            .install_desktop_and_icon(&desktop_path, &desktop_body, staged_icon_temp, &app.uuid)
            .map_err(|e| {
                let _ = fs::remove_file(&staged);
                Failure::new(e)
            })?;
        app.desktop_path = final_desktop.to_string_lossy().into_owned();
        app.icon_path = final_icon.to_string_lossy().into_owned();
        self.fail(IntegrateFailPoint::DesktopInstall).map_err(|e| {
            let _ = fs::remove_file(&staged);
            let _ = fs::remove_file(&final_desktop);
            if !final_icon.as_os_str().is_empty() {
                let _ = fs::remove_file(&final_icon);
            }
            Failure::new(e)
        })?;

        // Back up live artifacts when replacing (fail closed).
        let mut backup_appimage: Option<PathBuf> = None;
        let mut backup_desktop: Option<PathBuf> = None;
        let mut backup_icon: Option<PathBuf> = None;
        if replacing.is_some() {
            let live = Path::new(&destination);
            if live.exists() {
                let backup = safe_fs::sibling_temp(&destination, ".gosh-bak-app-");
                safe_fs::backup_copy(live, &backup).map_err(|_| {
                    let _ = fs::remove_file(&staged);
                    Failure::new("Cannot create replacement backup".to_string())
                })?;
                backup_appimage = Some(backup);
            }
            let live_desk = final_desktop.clone();
            if live_desk.exists() {
                let backup = safe_fs::sibling_temp(&live_desk, ".gosh-desk-bak-");
                if fs::copy(&live_desk, &backup).is_err() {
                    let _ = fs::remove_file(&staged);
                    self.fail(IntegrateFailPoint::BackupCreate)
                        .map_err(Failure::new)?;
                    if backup_appimage.is_none() {
                        // fall through to generic backup error below
                    }
                    return Err(Failure::new("Cannot create replacement backup".to_string()));
                }
                backup_desktop = Some(backup);
            }
            if !app.icon_path.is_empty() && Path::new(&app.icon_path).exists() {
                let icon_p = PathBuf::from(&app.icon_path);
                let backup = safe_fs::sibling_temp(&icon_p, ".gosh-icon-bak-");
                if fs::copy(&icon_p, &backup).is_err() {
                    let _ = fs::remove_file(&staged);
                    return Err(Failure::new("Cannot create replacement backup".to_string()));
                }
                backup_icon = Some(backup);
            }
            self.fail(IntegrateFailPoint::BackupCreate).map_err(|e| {
                let _ = fs::remove_file(&staged);
                for backup in [&backup_appimage, &backup_desktop, &backup_icon]
                    .into_iter()
                    .flatten()
                {
                    let _ = fs::remove_file(backup);
                }
                Failure::new(e)
            })?;
        }

        // 5. Snapshot registry; race-checked commit.
        let snapshot = registry.snapshot();
        let replacing_owned = replacing.as_ref().map(|a| {
            safe_fs::canonical_bounded(Path::new(&a.managed_path))
                .unwrap_or_else(|_| PathBuf::from(&a.managed_path))
        });
        let dest_canon =
            safe_fs::canonical_bounded(&destination).unwrap_or_else(|_| destination.clone());
        let commit = if replacing_owned.as_ref() == Some(&dest_canon) {
            safe_fs::rename_over(&staged, &destination)
        } else {
            safe_fs::rename_no_replace(&staged, &destination)
        };
        if self.fail_point == IntegrateFailPoint::BeforeCommit {
            let _ = fs::remove_file(&staged);
            restore_live(&backup_appimage, &destination);
            return Err(Failure::new(
                "Destination appeared before commit; refusing to overwrite".to_string(),
            ));
        }
        let commit_ok = commit.is_ok();
        let commit_result = commit.and_then(|_| {
            // Commit desktop over the top, then persist the registry.
            self.fail(IntegrateFailPoint::RegistrySave)?;
            registry.upsert(app.clone())?;
            Ok(())
        });
        if let Err(error) = commit_result {
            // Roll back every artifact this transaction created, and restore
            // every one it replaced. Anything that existed beforehand is put
            // back; anything we introduced is removed.
            let committed = commit_ok && destination.exists();

            // 1. The AppImage. When replacing, the backup goes back over the
            //    top. When installing fresh there is no backup, so the file we
            //    just committed has to be removed -- leaving it behind was the
            //    bug: it survived with no registry row, invisible to the app
            //    and impossible to remove through it.
            if backup_appimage.is_some() {
                restore_live(&backup_appimage, &destination);
                rolled_back.push(destination.to_string_lossy().into_owned());
            } else if committed && fs::remove_file(&destination).is_ok() {
                rolled_back.push(destination.to_string_lossy().into_owned());
            }

            // 2. The desktop entry, which install_files wrote before the
            //    commit was attempted.
            if desktop_existed {
                restore_live(&backup_desktop, &final_desktop);
            } else if fs::remove_file(&final_desktop).is_ok() {
                rolled_back.push(final_desktop.to_string_lossy().into_owned());
            }

            // 3. The icon, same rule.
            if !final_icon.as_os_str().is_empty() {
                if icon_existed {
                    restore_live(&backup_icon, &final_icon);
                } else if fs::remove_file(&final_icon).is_ok() {
                    rolled_back.push(final_icon.to_string_lossy().into_owned());
                }
            }

            let _ = registry.restore(snapshot);
            rolled_back.extend(safe_fs::rollback_temps(&managed_dir, ".gosh-"));
            let _ = fs::remove_file(&staged);
            Self::clear_icon_staging(&inspected);
            return Err(Failure {
                error,
                rolled_back,
                partial: false,
                source_removed: false,
            });
        }
        // Success: drop backups + rollback material.
        for backup in [&backup_appimage, &backup_desktop, &backup_icon]
            .into_iter()
            .flatten()
        {
            let _ = fs::remove_file(backup);
        }

        // Desktop database refresh is best-effort (never fails integration).
        let _ = self.refresh_desktop_db();

        // 9. Move-mode tail: trash the source only after final verification.
        let mut source_removed = false;
        let mut partial = false;
        let mut tail_error = String::new();
        if req.copy_mode == CopyMode::Move {
            self.fail(IntegrateFailPoint::SourceDelete)
                .unwrap_or_else(|e| {
                    tail_error = e;
                    partial = true;
                });
            if tail_error.is_empty() {
                let same = safe_fs::canonical_bounded(Path::new(&req.source_path))
                    .map(|c| c == dest_canon)
                    .unwrap_or(false);
                if !same {
                    match self.trash.trash(Path::new(&req.source_path)) {
                        Ok(()) => source_removed = true,
                        Err(_) => {
                            partial = true;
                            tail_error = "Trash failed; leaving source intact".to_string();
                        }
                    }
                }
            }
        }
        if !tail_error.is_empty() {
            return Err(Failure {
                error: tail_error,
                rolled_back,
                partial,
                source_removed,
            });
        }
        Self::clear_icon_staging(&inspected);
        Ok((app, source_removed))
    }

    /// The icon the inspector staged, if any.
    ///
    /// Inspection copies the chosen icon out of its work directory into a
    /// private staging directory precisely so it can be installed here. This
    /// used to return None unconditionally, so no integrated AppImage ever got
    /// an icon -- every entry fell back to `Icon=application-x-executable`.
    fn stage_icon(
        &self,
        inspected: &crate::types::InspectionResult,
        _uuid: &str,
    ) -> Option<(PathBuf, String)> {
        let staged = &inspected.metadata.extracted_icon_path;
        if staged.is_empty() {
            return None;
        }
        let path = PathBuf::from(staged);
        if !path.is_file() {
            return None;
        }
        let ext = if inspected.metadata.icon_format.is_empty() {
            "png".to_string()
        } else {
            inspected.metadata.icon_format.clone()
        };
        Some((path, ext))
    }

    /// Remove the inspector's icon staging directory once we are done with it.
    fn clear_icon_staging(inspected: &crate::types::InspectionResult) {
        inspected.discard_staging();
    }

    /// Where an icon with this extension will be installed. Known before the
    /// copy happens, so the desktop entry can name it.
    fn icon_destination(&self, uuid: &str, ext: &str) -> PathBuf {
        let ext = match ext {
            "svg" => "svg",
            _ => "png",
        };
        self.settings
            .icons_dir()
            .join("256x256/apps")
            .join(format!("gosh-appimage-{uuid}.{ext}"))
    }

    fn install_desktop_and_icon(
        &self,
        desktop_path: &Path,
        desktop_body: &str,
        staged_icon: Option<(PathBuf, String)>,
        uuid: &str,
    ) -> Result<(PathBuf, PathBuf), String> {
        let icon_source = staged_icon.as_ref().map(|(p, _)| p.as_path());
        let ext = staged_icon
            .as_ref()
            .map(|(_, e)| e.as_str())
            .unwrap_or("png");
        let desktop_name = desktop_path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| desktop::desktop_file_name(uuid));
        desktop::install_files(
            &self.settings.applications_dir(),
            &self.settings.icons_dir(),
            &desktop_name,
            desktop_body,
            icon_source,
            ext,
            uuid,
        )
    }

    /// Ask the desktop to notice the entry we just installed.
    ///
    /// Best effort by design: a missing `update-desktop-database` is normal on
    /// many systems and must never fail an otherwise successful integration.
    /// This used to build the request and throw it away (`let _ = req;`), so
    /// a newly integrated app could stay absent from the launcher until the
    /// next login.
    fn refresh_desktop_db(&self) -> Result<(), String> {
        let result = self.runner.run(&crate::process::ProcessRequest {
            program: "update-desktop-database".to_string(),
            args: vec![safe_fs::argv_safe_path(&self.settings.applications_dir())],
            host: crate::process::HostSpawn::Helper,
            timeout_ms: 10_000,
            ..Default::default()
        });
        if result.refused {
            // The tool is not installed; nothing to do and nothing to report.
            return Ok(());
        }
        Ok(())
    }
}

fn restore_live(backup: &Option<PathBuf>, live: &Path) {
    if let Some(path) = backup {
        let _ = fs::rename(path, live);
    }
}

fn ensure_appimage_suffix(path: &Path) -> PathBuf {
    if path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("appimage"))
        .unwrap_or(false)
    {
        path.to_path_buf()
    } else {
        let mut name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "AppImage".to_string());
        name.push_str(".AppImage");
        path.with_file_name(name)
    }
}

fn choose_keep_both(dir: &Path, candidate: &Path) -> PathBuf {
    let stem = candidate
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "AppImage".to_string());
    let ext = candidate
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "AppImage".to_string());
    for i in 1..=1000 {
        let next = dir.join(format!("{stem}-{i}.{ext}"));
        if !next.exists() {
            return next;
        }
    }
    dir.join(format!(
        "{stem}-{}.{ext}",
        &ManagedRegistry::new_uuid()[..8]
    ))
}

#[derive(Debug)]
struct Failure {
    error: String,
    rolled_back: Vec<String>,
    partial: bool,
    source_removed: bool,
}

impl Failure {
    fn new(error: String) -> Self {
        Self {
            error,
            rolled_back: Vec::new(),
            partial: false,
            source_removed: false,
        }
    }
}
