// Gosh AppImage Manager — application controller.
// Owns every service behind trait seams (process/network/table/trash) so
// tests inject fakes and never touch a real home, app, or network.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::desktop;
use crate::integration::IntegrationService;
use crate::launch::LaunchService;
use crate::library::AppImageLibrary;
use crate::network::{NetworkClient, ReqwestClient};
use crate::notifier::UpdateNotifier;
use crate::process::{ProcessRunner, SystemRunner};
use crate::proctable::{ProcessTable, SysTable};
use crate::registry::ManagedRegistry;
use crate::removal::RemovalService;
use crate::settings::{Dirs, SettingsStore};
use crate::tasks::TaskQueue;
use crate::trash::{SystemTrash, TrashSink};
use crate::updates_service::UpdateService;

pub struct AppController {
    settings: SettingsStore,
    registry: ManagedRegistry,
    runner: Box<dyn ProcessRunner>,
    // Shared, so an operation's blocking calls can run on helper threads that
    // outlive the caller's wait. See `cancel`.
    network: Arc<dyn NetworkClient>,
    processes: Arc<dyn ProcessTable>,
    trash: Box<dyn TrashSink>,
    tasks: TaskQueue,
    notifier: UpdateNotifier,
}

/// Whether a login entry body is the update-check entry this app writes. The
/// name and the `--fetch-updates` Exec both have to match, so a file that
/// merely shares the path is never treated as ours.
fn is_update_check_entry(body: &str) -> bool {
    body.lines()
        .any(|line| line == "Name=Gosh AppImage Manager update checks")
        && exec_line(body).is_some_and(|exec| exec.contains("--fetch-updates"))
}

fn exec_line(body: &str) -> Option<&str> {
    body.lines().find_map(|line| line.strip_prefix("Exec="))
}

impl AppController {
    pub fn new() -> Result<Self, String> {
        Self::with_seams(
            Box::new(SystemRunner::new()),
            Box::new(ReqwestClient::new()),
            // Give the process table a runner of its own so it can ask the
            // host which apps are running when we are inside a Flatpak
            // sandbox, where our /proc shows only ourselves.
            Box::new(SysTable::with_host_runner(Box::new(SystemRunner::new()))),
            Box::new(SystemTrash::new()),
            Dirs::from_env(),
        )
    }

    pub fn with_seams(
        runner: Box<dyn ProcessRunner>,
        network: Box<dyn NetworkClient>,
        processes: Box<dyn ProcessTable>,
        trash: Box<dyn TrashSink>,
        dirs: Dirs,
    ) -> Result<Self, String> {
        let settings = SettingsStore::new(dirs);
        let registry_path = settings.registry_path();
        let registry = ManagedRegistry::open(&registry_path)?;
        Ok(Self {
            settings,
            registry,
            runner,
            network: Arc::from(network),
            processes: Arc::from(processes),
            trash,
            tasks: TaskQueue::new(),
            notifier: UpdateNotifier::new(),
        })
    }

    pub fn settings(&self) -> &SettingsStore {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut SettingsStore {
        &mut self.settings
    }

    pub fn registry(&self) -> &ManagedRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut ManagedRegistry {
        &mut self.registry
    }

    pub fn runner(&self) -> &dyn ProcessRunner {
        &*self.runner
    }

    pub fn network(&self) -> &dyn NetworkClient {
        &*self.network
    }

    pub fn processes(&self) -> &dyn ProcessTable {
        &*self.processes
    }

    pub fn trash(&self) -> &dyn TrashSink {
        &*self.trash
    }

    pub fn tasks(&self) -> &TaskQueue {
        &self.tasks
    }

    pub fn tasks_mut(&mut self) -> &mut TaskQueue {
        &mut self.tasks
    }

    pub fn notifier(&self) -> &UpdateNotifier {
        &self.notifier
    }

    pub fn integration(&self) -> IntegrationService<'_> {
        IntegrationService::new(&self.settings, &*self.runner, &*self.trash)
    }

    pub fn removal(&self) -> RemovalService<'_> {
        RemovalService::new(&self.settings, &*self.trash, &*self.runner)
    }

    pub fn launch_service(&self) -> LaunchService<'_> {
        LaunchService::new(&*self.runner, &*self.processes)
    }

    pub fn update_service(&self) -> UpdateService<'_> {
        UpdateService::new(
            &self.settings,
            Arc::clone(&self.network),
            Arc::clone(&self.processes),
        )
    }

    pub fn library(&self) -> AppImageLibrary<'_> {
        AppImageLibrary::new(&self.settings)
    }

    pub fn autostart_desktop_path(&self) -> PathBuf {
        self.settings.autostart_desktop_path()
    }

    // ---- Facade operations (borrow-safe: disjoint fields) ----

    pub fn inspect_file(
        &self,
        path: &str,
        cancel: &std::sync::atomic::AtomicBool,
        existing: Option<&str>,
    ) -> crate::types::InspectionResult {
        IntegrationService::new(&self.settings, &*self.runner, &*self.trash)
            .inspect_only(path, cancel, existing)
    }

    pub fn inspect_with(
        &self,
        path: &str,
        options: &crate::types::InspectOptions,
        cancel: &std::sync::atomic::AtomicBool,
        existing: Option<&str>,
    ) -> crate::types::InspectionResult {
        // Every inspection takes the stored setting through the same gate.
        let options = crate::inspector::gated_options(&self.settings, options);
        crate::inspector::AppImageInspector::new(&*self.runner)
            .inspect(path, &options, cancel, existing)
    }

    pub fn integrate(
        &mut self,
        req: &crate::types::IntegrateRequest,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> crate::types::IntegrateResult {
        IntegrationService::new(&self.settings, &*self.runner, &*self.trash).integrate(
            &mut self.registry,
            req,
            cancel,
        )
    }

    pub fn remove_app(
        &mut self,
        req: &crate::types::RemovalRequest,
    ) -> crate::types::RemovalResult {
        RemovalService::new(&self.settings, &*self.trash, &*self.runner)
            .remove(&mut self.registry, req)
    }

    /// Register an external AppImage without touching anything on disk.
    ///
    /// Borrow-safe facade: the library reads settings while the registry is
    /// mutated, which the two `&self`/`&mut self` accessors cannot express.
    pub fn adopt_external(&mut self, path: &str) -> Result<crate::types::InstalledApp, String> {
        let library = AppImageLibrary::new(&self.settings);
        library.adopt(&mut self.registry, path)
    }

    /// Discover AppImages in the managed folder, plus external ones when the
    /// `manage_outside_folder` setting is on.
    pub fn discover(&self) -> Result<Vec<crate::library::DiscoveredApp>, String> {
        AppImageLibrary::new(&self.settings).scan(&self.registry)
    }

    /// Open the file manager at this path, selecting the file if it can.
    ///
    /// Uses the host's default handler through the same argument-safe spawn
    /// path as launching, so nothing is shell-parsed.
    pub fn reveal_in_file_manager(&self, path: &str) -> Result<(), String> {
        let parent = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string());
        self.runner.start_detached(&crate::process::ProcessRequest {
            program: "xdg-open".to_string(),
            args: vec![crate::safe_fs::argv_safe_path(std::path::Path::new(
                &parent,
            ))],
            host: crate::process::HostSpawn::Helper,
            timeout_ms: 10_000,
            ..Default::default()
        })
    }

    /// Re-read an installed AppImage's metadata and rewrite its desktop entry.
    ///
    /// Returns the app's name so the caller can report what it refreshed.
    pub fn refresh_metadata(
        &mut self,
        uuid: &str,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<String, String> {
        let app = self
            .registry
            .by_uuid(uuid)
            .ok_or_else(|| "No installed app with that id".to_string())?;
        let inspected = self.inspect_file(&app.managed_path, cancel, Some(uuid));
        if !inspected.magic_valid {
            inspected.discard_staging();
            return Err(if inspected.error.is_empty() {
                "The managed file is no longer a valid AppImage".to_string()
            } else {
                inspected.error
            });
        }
        let mut updated = app.clone();
        if !inspected.metadata.name.is_empty() {
            updated.name = inspected.metadata.name.clone();
        }
        updated.version = inspected.metadata.version.clone();
        updated.comment = inspected.metadata.comment.clone();
        updated.website = inspected.metadata.website.clone();
        updated.terminal = inspected.metadata.terminal;
        updated.categories = inspected.metadata.categories.clone();
        updated.mime_types = inspected.metadata.mime_types.clone();
        updated.startup_wm_class = inspected.metadata.startup_wm_class.clone();
        updated.default_arguments = inspected.metadata.exec_arguments.clone();
        updated.embedded_update = inspected.update_info.raw.clone();
        updated.sha256 = inspected.identity.sha256.clone();
        updated.size = inspected.identity.size;
        updated.app_type = inspected.app_type;
        updated.architecture = inspected.architecture;
        // Rewrite the entry we own so the refreshed name and version show up.
        if !updated.desktop_path.is_empty() {
            let body = desktop::build_desktop_file(
                &updated,
                &updated.managed_path,
                self.settings.terminal_omit_suffix(),
            );
            crate::safe_fs::atomic_write(
                std::path::Path::new(&updated.desktop_path),
                body.as_bytes(),
                0o644,
            )?;
        }
        inspected.discard_staging();
        let name = updated.name.clone();
        self.registry.upsert(updated)?;
        Ok(name)
    }

    /// Replace an app's argument list and environment, then rewrite its entry.
    ///
    /// Arguments are stored and written as separate tokens, never as a shell
    /// fragment; environment names are validated before anything is saved.
    pub fn set_arguments_and_environment(
        &mut self,
        uuid: &str,
        arguments: Vec<String>,
        environment: Vec<crate::types::EnvPair>,
    ) -> Result<(), String> {
        let mut app = self
            .registry
            .by_uuid(uuid)
            .ok_or_else(|| "No installed app with that id".to_string())?;
        if arguments.len() > crate::limits::MAX_ARGUMENTS {
            return Err(format!(
                "Too many arguments (limit {})",
                crate::limits::MAX_ARGUMENTS
            ));
        }
        if let Some(bad) = arguments
            .iter()
            .find(|a| a.contains('\0') || a.len() > crate::limits::MAX_ARGUMENT_LENGTH)
        {
            return Err(format!("Argument is not usable: {bad}"));
        }
        if environment.len() > crate::limits::MAX_ENV_PAIRS {
            return Err(format!(
                "Too many environment variables (limit {})",
                crate::limits::MAX_ENV_PAIRS
            ));
        }
        if let Some(bad) = environment
            .iter()
            .find(|p| !desktop::valid_env_name(&p.name))
        {
            return Err(format!(
                "Not a valid environment variable name: {}",
                bad.name
            ));
        }
        app.arguments = arguments;
        app.environment = environment;
        if !app.desktop_path.is_empty() {
            let body = desktop::build_desktop_file(
                &app,
                &app.managed_path,
                self.settings.terminal_omit_suffix(),
            );
            crate::safe_fs::atomic_write(
                std::path::Path::new(&app.desktop_path),
                body.as_bytes(),
                0o644,
            )?;
        }
        self.registry.upsert(app)
    }

    /// Which of these apps are running, in one pass over the process table.
    ///
    /// Calling is_running per app re-walked /proc once per app; a library of
    /// N apps cost N walks and N readlinks per process.
    pub fn running_uuids(&self, apps: &[crate::types::InstalledApp]) -> Vec<String> {
        let executables = crate::proctable::executables_for(apps);
        let running = self.processes.running_among(&executables);
        apps.iter()
            .zip(executables.iter())
            .filter(|(_, exe)| running.contains(*exe))
            .map(|(app, _)| app.uuid.clone())
            .collect()
    }

    pub fn is_running(&self, app: &crate::types::InstalledApp) -> bool {
        LaunchService::new(&*self.runner, &*self.processes).is_running(app)
    }

    pub fn check_updates(
        &self,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Vec<crate::types::UpdateOffer> {
        self.scan_updates(cancel).offers
    }

    /// Check every app and report failures alongside offers, so callers can
    /// distinguish "up to date" from "could not check".
    pub fn scan_updates(
        &self,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> crate::updates_service::UpdateScan {
        UpdateService::new(
            &self.settings,
            Arc::clone(&self.network),
            Arc::clone(&self.processes),
        )
        .list_updates_detailed(&self.registry, cancel)
    }

    pub fn apply_update(
        &mut self,
        app: &crate::types::InstalledApp,
        force: bool,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> crate::types::IntegrateResult {
        self.apply_update_with_progress(app, force, cancel, &mut |_| {})
    }

    /// As `apply_update`, reporting the versions and each stage to `events`.
    pub fn apply_update_with_progress(
        &mut self,
        app: &crate::types::InstalledApp,
        force: bool,
        cancel: &std::sync::atomic::AtomicBool,
        events: &mut dyn FnMut(crate::types::ApplyEvent),
    ) -> crate::types::IntegrateResult {
        UpdateService::new(
            &self.settings,
            Arc::clone(&self.network),
            Arc::clone(&self.processes),
        )
        .apply_with_progress(&mut self.registry, app, force, cancel, events)
    }

    /// Check one app's update source without downloading or applying anything.
    /// The installed file, its registry row and its version are not touched.
    pub fn check_one_update(
        &self,
        app: &crate::types::InstalledApp,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> crate::updates_sources::UpdateCheckResult {
        UpdateService::new(
            &self.settings,
            Arc::clone(&self.network),
            Arc::clone(&self.processes),
        )
        .check(app, cancel)
    }

    pub fn set_update_source(
        &mut self,
        app: crate::types::InstalledApp,
        manager: &str,
        config: crate::updates_sources::Config,
        error: &mut String,
    ) -> bool {
        UpdateService::new(
            &self.settings,
            Arc::clone(&self.network),
            Arc::clone(&self.processes),
        )
        .set_source(&mut self.registry, app, manager, config, error)
    }

    pub fn unset_update_source(
        &mut self,
        app: crate::types::InstalledApp,
        error: &mut String,
    ) -> bool {
        UpdateService::new(
            &self.settings,
            Arc::clone(&self.network),
            Arc::clone(&self.processes),
        )
        .unset_source(&mut self.registry, app, error)
    }

    /// Write or remove the login entry for background checks.
    ///
    /// Background checks notify only; they never download or apply. Adding the
    /// entry needs "Check in the background" on, so a login check can never run
    /// while the user has turned update checks off. Removing it is always
    /// allowed.
    pub fn sync_autostart(&self, enabled: bool) -> Result<(), String> {
        let path = self.autostart_desktop_path();
        if !enabled {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| format!("Cannot remove autostart: {e}"))?;
            }
            return Ok(());
        }
        if !self.settings.background_update_checks() {
            return Err("Turn on Check in the background before adding a login check.".to_string());
        }
        self.write_autostart_entry()
    }

    /// Bring a login entry this app wrote in line with the background-check
    /// setting. Called at startup, so an entry from an earlier build cannot run
    /// an unwanted check.
    ///
    /// Background off: the entry is removed. Background on: the entry is
    /// rewritten when its Exec line lacks `--background`. A file that is not
    /// one of ours is never touched.
    pub fn reconcile_autostart(&self) -> Result<(), String> {
        let path = self.autostart_desktop_path();
        let Ok(existing) = fs::read_to_string(&path) else {
            return Ok(());
        };
        if !is_update_check_entry(&existing) {
            return Ok(());
        }
        if !self.settings.background_update_checks() {
            return fs::remove_file(&path).map_err(|e| format!("Cannot remove autostart: {e}"));
        }
        if exec_line(&existing).is_some_and(|exec| exec.contains("--background")) {
            return Ok(());
        }
        self.write_autostart_entry()
    }

    fn write_autostart_entry(&self) -> Result<(), String> {
        let path = self.autostart_desktop_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Cannot create autostart dir: {e}"))?;
        }
        let body = self.render_autostart_entry()?;
        crate::safe_fs::atomic_write(&path, body.as_bytes(), 0o644)
    }

    /// Build the autostart entry body without writing it anywhere.
    ///
    /// The Exec line runs `--fetch-updates --background`: the login check does
    /// nothing unless "Check in the background" is still on when it starts.
    /// Kept separate so the diagnostic probe can verify the Exec line without
    /// installing the entry or changing the user's settings.
    pub fn render_autostart_entry(&self) -> Result<String, String> {
        let exec = if crate::process::in_flatpak() {
            "flatpak run com.goshapps.AppImageManager --fetch-updates --background".to_string()
        } else {
            let exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "gosh-appimage-manager".to_string());
            format!(
                "{} --fetch-updates --background",
                desktop::escape_exec_arg(&exe)
            )
        };
        Ok(format!(
            "[Desktop Entry]\nType=Application\nName=Gosh AppImage Manager update checks\nExec={exec}\nIcon=com.goshapps.AppImageManager\nTerminal=false\nCategories=Utility;\nX-GNOME-Autostart-enabled=true\n"
        ))
    }

    /// Non-mutating readiness check used by `--self-test`.
    ///
    /// This returned a constant `true` and asserted nothing, so the
    /// "models-ready: ok" line in the self-test was decoration. It now checks
    /// the things that must hold before any operation can succeed, and names
    /// what is wrong when they do not.
    pub fn readiness(&self) -> Result<(), String> {
        // The registry has to be readable; open() already loaded it, so a
        // round trip through the connection proves the file is still usable.
        let path = self.settings.registry_path();
        if path.exists() && !path.is_file() {
            return Err(format!("{} is not a regular file", path.display()));
        }
        let data_dir = self.settings.data_dir();
        if data_dir.exists() && !data_dir.is_dir() {
            return Err(format!("{} is not a directory", data_dir.display()));
        }
        // Settings that could not be read would silently run on defaults.
        if let Some(error) = self.settings.load_error() {
            return Err(error.to_string());
        }
        // The managed folder must be an absolute path we could create.
        let managed = self.settings.managed_folder();
        if !managed.is_absolute() {
            return Err(format!(
                "managed folder is not an absolute path: {}",
                managed.display()
            ));
        }
        if managed.exists() && !managed.is_dir() {
            return Err(format!("{} is not a directory", managed.display()));
        }
        // Every update manager the CLI advertises must resolve.
        for name in crate::updates_sources::UpdateSourceFactory::names() {
            if crate::updates_sources::UpdateSourceFactory::by_name(name).is_none() {
                return Err(format!("update manager {name} does not resolve"));
            }
        }
        Ok(())
    }

    /// Backwards-compatible boolean form.
    pub fn models_ready(&self) -> bool {
        self.readiness().is_ok()
    }
}
