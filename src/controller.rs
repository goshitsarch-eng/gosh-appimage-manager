// Gosh AppImage Manager — application controller.
// Owns every service behind trait seams (process/network/table/trash) so
// tests inject fakes and never touch a real home, app, or network.

use std::fs;
use std::path::PathBuf;

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
    network: Box<dyn NetworkClient>,
    processes: Box<dyn ProcessTable>,
    trash: Box<dyn TrashSink>,
    tasks: TaskQueue,
    notifier: UpdateNotifier,
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
            network,
            processes,
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
        UpdateService::new(&self.settings, &*self.network, &*self.processes)
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
        crate::inspector::AppImageInspector::new(&*self.runner)
            .inspect(path, options, cancel, existing)
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
    pub fn discover(&self) -> Vec<crate::library::DiscoveredApp> {
        AppImageLibrary::new(&self.settings).scan(&self.registry)
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
        UpdateService::new(&self.settings, &*self.network, &*self.processes)
            .list_updates_detailed(&self.registry, cancel)
    }

    pub fn apply_update(
        &mut self,
        app: &crate::types::InstalledApp,
        force: bool,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> crate::types::IntegrateResult {
        UpdateService::new(&self.settings, &*self.network, &*self.processes).apply(
            &mut self.registry,
            app,
            force,
            cancel,
        )
    }

    pub fn set_update_source(
        &mut self,
        app: crate::types::InstalledApp,
        manager: &str,
        config: crate::updates_sources::Config,
        error: &mut String,
    ) -> bool {
        UpdateService::new(&self.settings, &*self.network, &*self.processes).set_source(
            &mut self.registry,
            app,
            manager,
            config,
            error,
        )
    }

    pub fn unset_update_source(
        &mut self,
        app: crate::types::InstalledApp,
        error: &mut String,
    ) -> bool {
        UpdateService::new(&self.settings, &*self.network, &*self.processes).unset_source(
            &mut self.registry,
            app,
            error,
        )
    }

    /// Write or remove the session autostart entry for background checks.
    /// Background checks notify only; they never download or apply.
    pub fn sync_autostart(&self, enabled: bool) -> Result<(), String> {
        let path = self.autostart_desktop_path();
        if !enabled {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| format!("Cannot remove autostart: {e}"))?;
            }
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Cannot create autostart dir: {e}"))?;
        }
        let body = self.render_autostart_entry()?;
        crate::safe_fs::atomic_write(&path, body.as_bytes(), 0o644)
    }

    /// Build the autostart entry body without writing it anywhere.
    ///
    /// Kept separate so the diagnostic probe can verify the Exec line without
    /// installing the entry or changing the user's settings.
    pub fn render_autostart_entry(&self) -> Result<String, String> {
        let exec = if crate::process::in_flatpak() {
            "flatpak run com.goshapps.AppImageManager --fetch-updates".to_string()
        } else {
            let exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "gosh-appimage-manager".to_string());
            format!("{} --fetch-updates", desktop::escape_exec_arg(&exe))
        };
        Ok(format!(
            "[Desktop Entry]\nType=Application\nName=Gosh AppImage Manager update checks\nExec={exec}\nIcon=com.goshapps.AppImageManager\nTerminal=false\nCategories=Utility;\nX-GNOME-Autostart-enabled=true\n"
        ))
    }

    /// Non-mutating readiness probe used by --self-test.
    pub fn models_ready(&self) -> bool {
        true
    }
}
