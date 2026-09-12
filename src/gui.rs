// Gosh AppImage Manager 3.0.0 — libcosmic (COSMIC Epoch) GUI shell.
// Made by Gosh. GPL-3.0-or-later.
//
// Safety mirrors the CLI exactly: opening a file only inspects it;
// integration, updates, and removal go through the same transactional core
// with explicit confirmation. No telemetry of any kind.
//
// Threading: every operation that touches disk, spawns a process, hashes, or
// speaks to the network runs on a blocking worker and reports back as a
// message. `update` itself never blocks. The controller lives behind a mutex,
// which also gives the brief's "conflicting mutations are serialised" for
// free.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cosmic::app::{Command, Core, Settings};
use cosmic::iced::keyboard::{Key, Modifiers};
use cosmic::iced::{Length, Subscription};
use cosmic::widget::{self, nav_bar};
use cosmic::{executor, Application, ApplicationExt, Element};

use crate::controller::AppController;
use crate::diagnostics;
use crate::library::{DiscoveredApp, Origin};
use crate::limits;
use crate::t;
use crate::types::{
    app_image_type_name, appearance_name, architecture_name, Appearance, ConflictPolicy, CopyMode,
    EnvPair, InspectionResult, InstalledApp, IntegrateRequest, RemovalMode, RemovalRequest,
    TaskItem, TaskKind, TaskState, UpdateOffer,
};
use crate::updates_service::UpdateScan;

/// Minimum window size. The shell switches to its condensed layout well above
/// this, and `narrow()` stacks action rows there, so nothing overflows on the
/// way down to it.
const MIN_SIZE: (f32, f32) = (420.0, 420.0);

pub fn run(initial_files: Vec<String>) -> i32 {
    let settings = Settings::default()
        .size(cosmic::iced_core::Size::new(1024., 768.))
        .size_limits(
            cosmic::iced::Limits::NONE
                .min_width(MIN_SIZE.0)
                .min_height(MIN_SIZE.1),
        );
    match cosmic::app::run::<App>(settings, Flags { initial_files }) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Gosh AppImage Manager GUI failed: {error}");
            1
        }
    }
}

pub struct Flags {
    pub initial_files: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Library,
    Inspect,
    Updates,
    Tasks,
    Settings,
    About,
}

impl Page {
    /// The nav-bar label, translated.
    fn localized_title(self) -> String {
        match self {
            Page::Library => t!("nav.library", "Library"),
            Page::Inspect => t!("nav.inspect", "Inspect"),
            Page::Updates => t!("nav.updates", "Updates"),
            Page::Tasks => t!("nav.tasks", "Tasks"),
            Page::Settings => t!("nav.settings", "Settings"),
            Page::About => t!("nav.about", "About"),
        }
    }
}

/// A modal question. Every destructive choice appears as one of these.
#[derive(Clone)]
enum PendingDialog {
    IntegrateConflict {
        path: String,
        conflict_name: String,
        /// Resolved from the inspection, so "Replace" knows what it replaces
        /// without the user hunting for a UUID.
        replace_uuid: String,
        replace_label: String,
    },
    Remove {
        uuid: String,
        name: String,
        path: String,
        permanent: bool,
    },
    UnsafeExtract,
    UpdateForce {
        uuid: String,
        name: String,
    },
    Adopt {
        path: String,
    },
}

/// What a finished worker produced.
#[derive(Debug, Clone)]
pub enum Outcome {
    Inspected(Box<InspectionResult>),
    Integrated {
        ok: bool,
        message: String,
        conflict: Option<(String, String)>,
        source_path: String,
    },
    Scanned(Box<UpdateScan>),
    Updated {
        ok: bool,
        message: String,
    },
    BatchUpdated {
        applied: usize,
        failed: usize,
        errors: Vec<String>,
    },
    Removed {
        ok: bool,
        message: String,
    },
    Adopted {
        ok: bool,
        message: String,
    },
    LibraryLoaded {
        apps: Vec<InstalledApp>,
        running: Vec<String>,
        discovered: Vec<DiscoveredApp>,
    },
    MetadataRefreshed {
        ok: bool,
        message: String,
    },
    SourceSaved {
        ok: bool,
        message: String,
    },
    Revealed(Result<(), String>),
    Simple(Result<String, String>),
}

#[derive(Clone, Debug)]
pub enum Message {
    // Navigation and chrome
    LibraryRefresh,
    SearchChanged(String),
    SortChanged(SortOrder),
    SelectApp(String),
    CloseDetail,
    // Inspect
    InspectPathChanged(String),
    InspectBrowse,
    InspectDialog(Result<Vec<url::Url>, String>),
    InspectRun,
    IntegrateRun,
    IntegrateKeepBoth(String),
    IntegrateReplace(String, String),
    NextQueuedFile,
    // Library actions
    Launch(String),
    Reveal(String),
    RemoveAsk(String, bool),
    RemoveConfirm,
    RefreshMetadata(String),
    AdoptAsk(String),
    AdoptConfirm,
    // Detail editors
    ArgumentsChanged(String),
    EnvironmentChanged(String),
    SaveArgumentsAndEnvironment,
    // Updates
    UpdatesRefresh,
    UpdateAll,
    UpdateOne(String),
    UpdateForceConfirm,
    Cancel,
    // Tasks
    ClearFinishedTasks,
    // Settings
    AutostartToggled(bool),
    BackgroundToggled(bool),
    MoveSourceToggled(bool),
    ManageOutsideToggled(bool),
    TerminalSuffixToggled(bool),
    DebugLoggingToggled(bool),
    UnsafeFallbackToggled(bool),
    UnsafeFallbackConfirm,
    AppearanceSelected(Appearance),
    ManagedFolderChanged(String),
    ManagedFolderApply,
    UpdateSourceManagerChanged(String),
    UpdateSourceConfigChanged(String),
    UpdateSourceApply(String),
    UpdateSourceUnset(String),
    // Dialogs and results
    DialogDismiss,
    DismissStatus,
    Finished(String, Box<Outcome>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortOrder {
    Name,
    Version,
    UpdatesFirst,
}

impl SortOrder {
    /// The button label, translated.
    ///
    /// This returned a `&'static str` and so could never be translated; the
    /// pseudolocale run showed the three sort buttons still in plain ASCII
    /// while everything around them was accented.
    fn label(self) -> String {
        match self {
            SortOrder::Name => t!("sort.name", "Name"),
            SortOrder::Version => t!("sort.version", "Version"),
            SortOrder::UpdatesFirst => t!("sort.updates", "Updates first"),
        }
    }
}

/// Severity of the message shown in the status bar.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Severity {
    Info,
    Success,
    Error,
}

#[derive(Default)]
struct InspectState {
    /// The last inspection, kept so its icon staging can be released.
    result: Option<InspectionResult>,
    path_input: String,
    summary: Vec<(String, String)>,
    warnings: Vec<String>,
    error: String,
    inspected_ok: bool,
    /// Remaining files from a multi-file open, inspected one at a time.
    queued: Vec<String>,
}

#[derive(Default)]
struct DetailState {
    uuid: String,
    arguments_input: String,
    environment_input: String,
}

pub struct App {
    core: Core,
    nav_model: nav_bar::Model,
    /// Shared with workers; every operation takes the lock, which serialises
    /// conflicting mutations exactly as the brief requires.
    controller: Arc<Mutex<AppController>>,
    appearance: Appearance,
    library: Vec<InstalledApp>,
    running: Vec<String>,
    discovered: Vec<DiscoveredApp>,
    updates: Vec<UpdateOffer>,
    /// Apps whose update check failed, so "up to date" is never claimed for
    /// something that was not actually checked.
    check_failures: Vec<(String, String)>,
    tasks: Vec<TaskItem>,
    search: String,
    sort: SortOrder,
    detail: Option<DetailState>,
    inspect: InspectState,
    dialog: Option<PendingDialog>,
    /// Set for the duration of a worker; the flag is what Cancel flips.
    busy: Option<(String, Arc<AtomicBool>)>,
    autostart_present: bool,
    managed_folder_input: String,
    source_manager_input: String,
    source_config_input: String,
    status: Option<(Severity, String)>,
}

impl App {
    fn with_controller<T>(&self, f: impl FnOnce(&mut AppController) -> T) -> T {
        let mut guard = self
            .controller
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }

    /// Run `work` on a blocking worker and deliver its result as a message.
    ///
    /// This is the whole of the fix for the UI freezing: `update` returns
    /// immediately with a Command, the window keeps repainting, and the result
    /// arrives as `Message::Finished`. Nothing here waits.
    fn spawn(
        &mut self,
        kind: TaskKind,
        title: &str,
        target: &str,
        work: impl FnOnce(Arc<Mutex<AppController>>, Arc<AtomicBool>) -> Outcome + Send + 'static,
    ) -> Command<Message> {
        let cancel = Arc::new(AtomicBool::new(false));
        let task_id = self.with_controller(|c| c.tasks_mut().begin(kind, title, target, false));
        self.busy = Some((task_id.clone(), cancel.clone()));
        self.refresh_tasks();
        let controller = self.controller.clone();
        let id = task_id.clone();
        Command::perform(
            async move {
                let joined = tokio::task::spawn_blocking(move || work(controller, cancel)).await;
                let outcome = joined.unwrap_or_else(|e| {
                    Outcome::Simple(Err(format!("Background task failed: {e}")))
                });
                Message::Finished(id, Box::new(outcome))
            },
            cosmic::app::Message::App,
        )
    }

    fn refresh_tasks(&mut self) {
        self.tasks = self.with_controller(|c| c.tasks().history());
        self.tasks.reverse();
    }

    fn load_library(&mut self) -> Command<Message> {
        self.spawn(
            TaskKind::CheckUpdate,
            "Loading library",
            "",
            |controller, _cancel| {
                let guard = controller
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let apps = guard.registry().apps();
                let running = guard.running_uuids(&apps);
                let discovered = guard.discover();
                Outcome::LibraryLoaded {
                    apps,
                    running,
                    discovered,
                }
            },
        )
    }

    fn apply_appearance(&self) -> Command<Message> {
        let theme = match self.appearance {
            Appearance::Light => cosmic::theme::Theme::light(),
            Appearance::Dark => cosmic::theme::Theme::dark(),
            Appearance::System => cosmic::theme::system_preference(),
        };
        cosmic::app::command::set_theme(theme)
    }

    fn set_status(&mut self, severity: Severity, text: impl Into<String>) {
        self.status = Some((severity, text.into()));
    }

    /// Apply a settings change, reporting a failed save rather than leaving
    /// the switch showing a value that was never written to disk.
    fn apply_setting(
        &mut self,
        what: &str,
        change: impl FnOnce(&mut AppController) -> Result<(), String>,
    ) {
        match self.with_controller(change) {
            Ok(()) => {}
            Err(error) => {
                self.set_status(Severity::Error, format!("Could not save {what}: {error}"))
            }
        }
    }

    fn selected(&self) -> Option<&InstalledApp> {
        let detail = self.detail.as_ref()?;
        self.library.iter().find(|a| a.uuid == detail.uuid)
    }

    /// True when the shell has collapsed to its narrow layout, so rows should
    /// stack rather than sit side by side and overflow.
    fn narrow(&self) -> bool {
        self.core.is_condensed()
    }

    fn visible_library(&self) -> Vec<&InstalledApp> {
        let needle = self.search.trim().to_lowercase();
        let mut rows: Vec<&InstalledApp> = self
            .library
            .iter()
            .filter(|app| {
                needle.is_empty()
                    || app.name.to_lowercase().contains(&needle)
                    || app.managed_path.to_lowercase().contains(&needle)
                    || app.version.to_lowercase().contains(&needle)
            })
            .collect();
        match self.sort {
            SortOrder::Name => rows.sort_by_key(|a| a.name.to_lowercase()),
            SortOrder::Version => rows.sort_by(|a, b| a.version.cmp(&b.version)),
            SortOrder::UpdatesFirst => {
                let has_update =
                    |app: &InstalledApp| self.updates.iter().any(|o| o.uuid == app.uuid);
                rows.sort_by(|a, b| {
                    has_update(b)
                        .cmp(&has_update(a))
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
        }
        rows
    }

    fn start_inspect(&mut self, path: String) -> Command<Message> {
        if let Some(previous) = self.inspect.result.take() {
            previous.discard_staging();
        }
        self.inspect.error.clear();
        self.inspect.summary.clear();
        self.inspect.warnings.clear();
        self.inspect.inspected_ok = false;
        self.inspect.path_input = path.clone();
        let target = path.clone();
        self.spawn(
            TaskKind::Inspect,
            "Inspecting",
            &target,
            move |controller, cancel| {
                let guard = controller
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let existing = guard
                    .registry()
                    .by_path(&path)
                    .map(|app| app.uuid)
                    .unwrap_or_default();
                let options = crate::types::InspectOptions {
                    allow_unsafe_extract: guard.settings().unsafe_extraction_fallback(),
                    confirm_unsafe_extract: false,
                    max_bytes: guard.settings().max_appimage_bytes(),
                    ..Default::default()
                };
                let result = guard.inspect_with(
                    &path,
                    &options,
                    &cancel,
                    if existing.is_empty() {
                        None
                    } else {
                        Some(existing.as_str())
                    },
                );
                Outcome::Inspected(Box::new(result))
            },
        )
    }

    fn start_integrate(
        &mut self,
        path: String,
        policy: ConflictPolicy,
        replace_uuid: String,
    ) -> Command<Message> {
        let target = path.clone();
        self.spawn(
            TaskKind::Integrate,
            "Integrating",
            &target,
            move |controller, cancel| {
                let mut guard = controller
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let copy_mode = if guard.settings().move_source() {
                    CopyMode::Move
                } else {
                    CopyMode::Copy
                };
                // Resolve what a Replace would target *before* asking, so the
                // conflict dialog can name it instead of demanding a UUID.
                let conflict_candidate = conflict_candidate(&guard, &path);
                let result = guard.integrate(
                    &IntegrateRequest {
                        source_path: path.clone(),
                        conflict: policy,
                        replace_uuid: replace_uuid.clone(),
                        copy_mode,
                        assume_yes: true,
                    },
                    &cancel,
                );
                if result.ok {
                    Outcome::Integrated {
                        ok: true,
                        message: format!("Integrated {}", result.app.managed_path),
                        conflict: None,
                        source_path: path,
                    }
                } else {
                    let needs_choice = result.error.contains("keep-both")
                        || result.error.contains("Replace requires");
                    Outcome::Integrated {
                        ok: false,
                        message: result.error,
                        conflict: if needs_choice {
                            conflict_candidate
                        } else {
                            None
                        },
                        source_path: path,
                    }
                }
            },
        )
    }

    fn finish_inspect(&mut self, result: InspectionResult) {
        if !result.magic_valid {
            self.inspect.error = if result.error.is_empty() {
                "Not a valid AppImage".to_string()
            } else {
                result.error.clone()
            };
            result.discard_staging();
            return;
        }
        self.inspect.inspected_ok = true;
        let unknown = "(unknown)";
        self.inspect.summary = vec![
            ("Path".into(), result.identity.path.clone()),
            ("Size".into(), human_size(result.identity.size)),
            ("Type".into(), app_image_type_name(result.app_type).into()),
            (
                "Architecture".into(),
                architecture_name(result.architecture).into(),
            ),
            ("SHA-256".into(), hex::encode(&result.identity.sha256)),
            ("Name".into(), non_empty(&result.metadata.name, unknown)),
            (
                "Version".into(),
                non_empty(&result.metadata.version, unknown),
            ),
            (
                "Update source".into(),
                non_empty(&result.update_info.raw, "(none embedded)"),
            ),
            (
                "Already managed".into(),
                if result.already_managed { "yes" } else { "no" }.into(),
            ),
        ];
        self.inspect.warnings = result.warnings.clone();
        self.inspect.result = Some(result);
    }
}

/// Which managed installation a Replace would target for this source.
fn conflict_candidate(controller: &AppController, source: &str) -> Option<(String, String)> {
    let file_name = std::path::Path::new(source).file_name()?.to_string_lossy();
    let base = crate::desktop::sanitize_file_base(&file_name);
    let candidate = controller.settings().managed_folder().join(&base);
    let matches: Vec<InstalledApp> = controller
        .registry()
        .apps()
        .into_iter()
        .filter(|app| {
            app.owned
                && std::path::Path::new(&app.managed_path)
                    .file_stem()
                    .map(|s| crate::desktop::sanitize_file_base(&s.to_string_lossy()))
                    .as_deref()
                    == candidate
                        .file_stem()
                        .map(|s| s.to_string_lossy())
                        .as_deref()
        })
        .collect();
    // Only offer Replace when exactly one installation is implicated; more
    // than one is genuinely ambiguous and the user must pick from the Library.
    if matches.len() == 1 {
        Some((matches[0].uuid.clone(), matches[0].name.clone()))
    } else {
        None
    }
}

fn non_empty(value: &str, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn human_size(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes.max(0) as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = "com.goshapps.AppImageManager";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let mut nav_model = nav_bar::Model::default();
        for page in [
            Page::Library,
            Page::Inspect,
            Page::Updates,
            Page::Tasks,
            Page::Settings,
            Page::About,
        ] {
            nav_model
                .insert()
                .text(page.localized_title())
                .data::<Page>(page)
                .activate();
        }
        let first_page = nav_model.iter().next();
        if let Some(id) = first_page {
            nav_model.activate(id);
        }
        let controller = match AppController::new() {
            Ok(controller) => controller,
            Err(error) => {
                // A read-only home, a corrupt registry or a full disk must not
                // abort before a window exists; report and leave cleanly.
                eprintln!("Gosh AppImage Manager cannot start: {error}");
                std::process::exit(1);
            }
        };
        let appearance = controller.settings().appearance();
        let managed_folder_input = controller
            .settings()
            .managed_folder()
            .to_string_lossy()
            .into_owned();
        let autostart_present = controller.autostart_desktop_path().exists();
        // A settings file that could not be read used to reset everything to
        // defaults in silence, and the next change overwrote it.
        let load_error = controller.settings().load_error().map(str::to_string);
        let mut app = App {
            core,
            nav_model,
            controller: Arc::new(Mutex::new(controller)),
            appearance,
            library: Vec::new(),
            running: Vec::new(),
            discovered: Vec::new(),
            updates: Vec::new(),
            check_failures: Vec::new(),
            tasks: Vec::new(),
            search: String::new(),
            sort: SortOrder::Name,
            detail: None,
            inspect: InspectState::default(),
            dialog: None,
            busy: None,
            autostart_present,
            managed_folder_input,
            source_manager_input: String::new(),
            source_config_input: String::new(),
            status: load_error.map(|error| (Severity::Error, error)),
        };

        // Positional files open straight into Inspect. All of them: taking
        // only the first silently dropped the rest of a multi-file open.
        let mut queued = flags.initial_files;
        let first = if queued.is_empty() {
            None
        } else {
            Some(queued.remove(0))
        };
        app.inspect.queued = queued;

        let mut commands = vec![
            app.update_title(),
            app.apply_appearance(),
            app.load_library(),
        ];
        if let Some(path) = first {
            let path = normalise_open_target(&path);
            commands.push(app.start_inspect(path));
            let inspect_page = app.nav_model.iter().nth(1);
            if let Some(id) = inspect_page {
                app.nav_model.activate(id);
            }
        }
        (app, Command::batch(commands))
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav_model)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Command<Self::Message> {
        self.nav_model.activate(id);
        self.update_title()
    }

    /// Keyboard shortcuts, following the platform's usual assignments.
    fn subscription(&self) -> Subscription<Self::Message> {
        cosmic::iced::keyboard::on_key_press(|key, modifiers| {
            let ctrl = modifiers.contains(Modifiers::CTRL);
            match key.as_ref() {
                Key::Character("o") if ctrl => Some(Message::InspectBrowse),
                Key::Character("r") if ctrl => Some(Message::LibraryRefresh),
                Key::Character("f") if ctrl => Some(Message::UpdatesRefresh),
                Key::Named(cosmic::iced::keyboard::key::Named::F5) => Some(Message::LibraryRefresh),
                Key::Named(cosmic::iced::keyboard::key::Named::Escape) => {
                    Some(Message::DialogDismiss)
                }
                _ => None,
            }
        })
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::LibraryRefresh => self.load_library(),
            Message::SearchChanged(text) => {
                self.search = text;
                Command::none()
            }
            Message::SortChanged(order) => {
                self.sort = order;
                Command::none()
            }
            Message::SelectApp(uuid) => {
                let app = self.library.iter().find(|a| a.uuid == uuid).cloned();
                self.detail = app.map(|app| DetailState {
                    uuid: app.uuid.clone(),
                    arguments_input: app.arguments.join("\n"),
                    environment_input: app
                        .environment
                        .iter()
                        .map(|p| format!("{}={}", p.name, p.value))
                        .collect::<Vec<_>>()
                        .join("\n"),
                });
                Command::none()
            }
            Message::CloseDetail => {
                self.detail = None;
                Command::none()
            }
            Message::InspectPathChanged(path) => {
                self.inspect.path_input = path;
                Command::none()
            }
            Message::InspectBrowse => Command::perform(
                async move {
                    let dialog = cosmic::dialog::file_chooser::open::Dialog::new()
                        .title("Choose one or more AppImages");
                    match dialog.open_files().await {
                        Ok(response) => Message::InspectDialog(Ok(response.urls().to_vec())),
                        Err(error) => Message::InspectDialog(Err(error.to_string())),
                    }
                },
                cosmic::app::Message::App,
            ),
            Message::InspectDialog(result) => match result {
                Ok(urls) => {
                    let mut paths: Vec<String> = urls
                        .iter()
                        .filter_map(|u| u.to_file_path().ok())
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect();
                    if paths.is_empty() {
                        self.inspect.error = "Only local files can be inspected".to_string();
                        return Command::none();
                    }
                    let first = paths.remove(0);
                    self.inspect.queued = paths;
                    self.start_inspect(first)
                }
                Err(error) => {
                    self.inspect.error = error;
                    Command::none()
                }
            },
            Message::InspectRun => {
                let path = self.inspect.path_input.trim().to_string();
                if path.is_empty() {
                    self.inspect.error = "Choose an AppImage file first".to_string();
                    Command::none()
                } else {
                    self.start_inspect(normalise_open_target(&path))
                }
            }
            Message::NextQueuedFile => {
                if self.inspect.queued.is_empty() {
                    Command::none()
                } else {
                    let next = self.inspect.queued.remove(0);
                    self.start_inspect(next)
                }
            }
            Message::IntegrateRun => {
                let path = self.inspect.path_input.clone();
                if path.is_empty() {
                    self.inspect.error = "Choose an AppImage file first".to_string();
                    Command::none()
                } else {
                    self.start_integrate(path, ConflictPolicy::Unspecified, String::new())
                }
            }
            Message::IntegrateKeepBoth(path) => {
                self.dialog = None;
                self.start_integrate(path, ConflictPolicy::KeepBoth, String::new())
            }
            Message::IntegrateReplace(path, uuid) => {
                self.dialog = None;
                self.start_integrate(path, ConflictPolicy::Replace, uuid)
            }
            Message::Launch(uuid) => {
                let result = self.with_controller(|c| {
                    c.registry()
                        .by_uuid(&uuid)
                        .map(|app| (app.name.clone(), c.launch_service().launch(&app)))
                });
                match result {
                    Some((name, Ok(()))) => {
                        self.set_status(Severity::Success, format!("Launched {name}"))
                    }
                    Some((_, Err(error))) => {
                        self.set_status(Severity::Error, format!("Launch failed: {error}"))
                    }
                    None => self.set_status(Severity::Error, "No such application"),
                }
                Command::none()
            }
            Message::Reveal(path) => self.spawn(
                TaskKind::Inspect,
                "Revealing in file manager",
                &path.clone(),
                move |controller, _cancel| {
                    let guard = controller
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    Outcome::Revealed(guard.reveal_in_file_manager(&path))
                },
            ),
            Message::RemoveAsk(uuid, permanent) => {
                if let Some(app) = self.library.iter().find(|a| a.uuid == uuid) {
                    self.dialog = Some(PendingDialog::Remove {
                        uuid,
                        name: app.name.clone(),
                        path: app.managed_path.clone(),
                        permanent,
                    });
                }
                Command::none()
            }
            Message::RemoveConfirm => {
                let Some(PendingDialog::Remove {
                    uuid, permanent, ..
                }) = self.dialog.clone()
                else {
                    return Command::none();
                };
                self.dialog = None;
                self.detail = None;
                self.spawn(
                    TaskKind::Remove,
                    "Removing",
                    &uuid.clone(),
                    move |controller, _cancel| {
                        let mut guard = controller
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        let outcome = guard.remove_app(&RemovalRequest {
                            path_or_uuid: uuid,
                            mode: if permanent {
                                RemovalMode::Permanent
                            } else {
                                RemovalMode::Trash
                            },
                            assume_yes: true,
                        });
                        Outcome::Removed {
                            ok: outcome.ok,
                            message: if outcome.ok {
                                "Removed".to_string()
                            } else {
                                outcome.error
                            },
                        }
                    },
                )
            }
            Message::RefreshMetadata(uuid) => self.spawn(
                TaskKind::RefreshMetadata,
                "Refreshing metadata",
                &uuid.clone(),
                move |controller, cancel| {
                    let mut guard = controller
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    match guard.refresh_metadata(&uuid, &cancel) {
                        Ok(name) => Outcome::MetadataRefreshed {
                            ok: true,
                            message: format!("Refreshed metadata for {name}"),
                        },
                        Err(error) => Outcome::MetadataRefreshed {
                            ok: false,
                            message: error,
                        },
                    }
                },
            ),
            Message::AdoptAsk(path) => {
                self.dialog = Some(PendingDialog::Adopt { path });
                Command::none()
            }
            Message::AdoptConfirm => {
                let Some(PendingDialog::Adopt { path }) = self.dialog.clone() else {
                    return Command::none();
                };
                self.dialog = None;
                self.spawn(
                    TaskKind::Adopt,
                    "Adopting",
                    &path.clone(),
                    move |controller, _cancel| {
                        let mut guard = controller
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        match guard.adopt_external(&path) {
                            Ok(app) => Outcome::Adopted {
                                ok: true,
                                message: format!("Adopted {}", app.name),
                            },
                            Err(error) => Outcome::Adopted {
                                ok: false,
                                message: error,
                            },
                        }
                    },
                )
            }
            Message::ArgumentsChanged(text) => {
                if let Some(detail) = self.detail.as_mut() {
                    detail.arguments_input = text;
                }
                Command::none()
            }
            Message::EnvironmentChanged(text) => {
                if let Some(detail) = self.detail.as_mut() {
                    detail.environment_input = text;
                }
                Command::none()
            }
            Message::SaveArgumentsAndEnvironment => {
                let Some(detail) = self.detail.as_ref() else {
                    return Command::none();
                };
                let uuid = detail.uuid.clone();
                let arguments: Vec<String> = detail
                    .arguments_input
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .take(limits::MAX_ARGUMENTS)
                    .collect();
                let (environment, rejected) = parse_environment(&detail.environment_input);
                if !rejected.is_empty() {
                    self.set_status(
                        Severity::Error,
                        format!(
                            "Not a valid environment variable name: {}",
                            rejected.join(", ")
                        ),
                    );
                    return Command::none();
                }
                self.spawn(
                    TaskKind::RefreshMetadata,
                    "Saving arguments",
                    &uuid.clone(),
                    move |controller, _cancel| {
                        let mut guard = controller
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        match guard.set_arguments_and_environment(&uuid, arguments, environment) {
                            Ok(()) => Outcome::SourceSaved {
                                ok: true,
                                message: "Saved arguments and environment".to_string(),
                            },
                            Err(error) => Outcome::SourceSaved {
                                ok: false,
                                message: error,
                            },
                        }
                    },
                )
            }
            Message::UpdatesRefresh => self.spawn(
                TaskKind::CheckUpdate,
                "Checking for updates",
                "",
                |controller, cancel| {
                    let guard = controller
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    Outcome::Scanned(Box::new(guard.scan_updates(&cancel)))
                },
            ),
            Message::UpdateAll => {
                let uuids: Vec<String> = self.updates.iter().map(|o| o.uuid.clone()).collect();
                if uuids.is_empty() {
                    self.set_status(Severity::Info, "Nothing to update");
                    return Command::none();
                }
                self.spawn(
                    TaskKind::Update,
                    "Updating all",
                    "",
                    move |controller, cancel| {
                        let mut applied = 0usize;
                        let mut failed = 0usize;
                        let mut errors = Vec::new();
                        for uuid in uuids {
                            // Cancellation is checked between items, so Cancel now
                            // takes effect: the loop no longer runs inside a
                            // single blocking UI callback.
                            if cancel.load(Ordering::Relaxed) {
                                errors.push("Cancelled before finishing".to_string());
                                break;
                            }
                            let mut guard = controller
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            let Some(app) = guard.registry().by_uuid(&uuid) else {
                                continue;
                            };
                            let result = guard.apply_update(&app, false, &cancel);
                            if result.ok {
                                applied += 1;
                            } else {
                                failed += 1;
                                errors.push(format!("{}: {}", app.name, result.error));
                            }
                        }
                        Outcome::BatchUpdated {
                            applied,
                            failed,
                            errors,
                        }
                    },
                )
            }
            Message::UpdateOne(uuid) => {
                let running = self
                    .updates
                    .iter()
                    .find(|o| o.uuid == uuid)
                    .map(|o| o.running)
                    .unwrap_or(false);
                if running {
                    let name = self
                        .library
                        .iter()
                        .find(|a| a.uuid == uuid)
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| uuid.clone());
                    self.dialog = Some(PendingDialog::UpdateForce { uuid, name });
                    return Command::none();
                }
                self.start_update(uuid, false)
            }
            Message::UpdateForceConfirm => {
                let Some(PendingDialog::UpdateForce { uuid, .. }) = self.dialog.clone() else {
                    return Command::none();
                };
                self.dialog = None;
                self.start_update(uuid, true)
            }
            Message::Cancel => {
                if let Some((id, flag)) = &self.busy {
                    flag.store(true, Ordering::Relaxed);
                    let id = id.clone();
                    self.with_controller(|c| c.tasks_mut().mark_cancelling(&id));
                    self.refresh_tasks();
                    self.set_status(Severity::Info, "Cancelling…");
                }
                Command::none()
            }
            Message::ClearFinishedTasks => {
                self.with_controller(|c| c.tasks_mut().clear_finished());
                self.refresh_tasks();
                Command::none()
            }
            Message::AutostartToggled(enabled) => {
                let result = self.with_controller(|c| c.sync_autostart(enabled));
                match result {
                    Ok(()) => {
                        self.autostart_present = enabled;
                        self.set_status(
                            Severity::Success,
                            if enabled {
                                "Update checks will run at login (notify only)"
                            } else {
                                "Login update checks off"
                            },
                        );
                    }
                    Err(error) => {
                        self.set_status(Severity::Error, format!("Autostart failed: {error}"))
                    }
                }
                Command::none()
            }
            Message::BackgroundToggled(enabled) => {
                // Turning background checks off must also remove the login
                // entry; otherwise --fetch-updates keeps contacting update
                // endpoints at every login after the user opted out.
                if !enabled {
                    if let Err(error) = self.with_controller(|c| c.sync_autostart(false)) {
                        self.set_status(
                            Severity::Error,
                            format!("Cannot disable background checks: {error}"),
                        );
                        return Command::none();
                    }
                    self.autostart_present = false;
                }
                self.apply_setting("background update checks", |c| {
                    c.settings_mut().set_background_update_checks(enabled)
                });
                self.set_status(
                    Severity::Success,
                    if enabled {
                        "Background update checks on (notify only)"
                    } else {
                        "Background update checks off"
                    },
                );
                Command::none()
            }
            Message::MoveSourceToggled(enabled) => {
                self.apply_setting("the copy/move preference", |c| {
                    c.settings_mut().set_move_source(enabled)
                });
                Command::none()
            }
            Message::ManageOutsideToggled(enabled) => {
                self.apply_setting("outside-folder discovery", |c| {
                    c.settings_mut().set_manage_outside_folder(enabled)
                });
                self.load_library()
            }
            Message::TerminalSuffixToggled(enabled) => {
                self.apply_setting("the terminal-suffix preference", |c| {
                    c.settings_mut().set_terminal_omit_suffix(enabled)
                });
                Command::none()
            }
            Message::DebugLoggingToggled(enabled) => {
                self.apply_setting("the diagnostics preference", |c| {
                    c.settings_mut().set_debug_logging(enabled)
                });
                // The switch is real: enabling immediately emits one line so
                // the preference is observably wired, and every worker outcome
                // below emits when the saved flag is on.
                diagnostics::emit_if(enabled, "settings", "verbose diagnostics on");
                Command::none()
            }
            Message::UnsafeFallbackToggled(enabled) => {
                if enabled {
                    self.dialog = Some(PendingDialog::UnsafeExtract);
                } else {
                    self.apply_setting("the extraction fallback", |c| {
                        c.settings_mut().set_unsafe_extraction_fallback(false)
                    });
                }
                Command::none()
            }
            Message::UnsafeFallbackConfirm => {
                self.dialog = None;
                self.apply_setting("the extraction fallback", |c| {
                    c.settings_mut().set_unsafe_extraction_fallback(true)
                });
                self.set_status(
                    Severity::Info,
                    "Unsafe extraction fallback on; each file still needs confirming",
                );
                Command::none()
            }
            Message::AppearanceSelected(appearance) => {
                self.appearance = appearance;
                self.apply_setting("the appearance preference", |c| {
                    c.settings_mut().set_appearance(appearance)
                });
                self.apply_appearance()
            }
            Message::ManagedFolderChanged(path) => {
                self.managed_folder_input = path;
                Command::none()
            }
            Message::ManagedFolderApply => {
                let path = std::path::PathBuf::from(self.managed_folder_input.trim());
                if path.as_os_str().is_empty() {
                    self.set_status(Severity::Error, "Managed folder cannot be empty");
                    return Command::none();
                }
                if !path.is_absolute() {
                    self.set_status(Severity::Error, "Managed folder must be an absolute path");
                    return Command::none();
                }
                self.apply_setting("the managed folder", |c| {
                    c.settings_mut().set_managed_folder(path)
                });
                if self.status.is_none() {
                    self.set_status(Severity::Success, "Managed folder updated");
                }
                self.load_library()
            }
            Message::UpdateSourceManagerChanged(manager) => {
                self.source_manager_input = manager;
                Command::none()
            }
            Message::UpdateSourceConfigChanged(config) => {
                self.source_config_input = config;
                Command::none()
            }
            Message::UpdateSourceApply(uuid) => {
                let manager = self.source_manager_input.trim().to_string();
                let config = parse_config_lines(&self.source_config_input);
                self.spawn(
                    TaskKind::RefreshMetadata,
                    "Saving update source",
                    &uuid.clone(),
                    move |controller, _cancel| {
                        let mut guard = controller
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        let Some(app) = guard.registry().by_uuid(&uuid) else {
                            return Outcome::SourceSaved {
                                ok: false,
                                message: "No installed app with that id".to_string(),
                            };
                        };
                        let mut error = String::new();
                        let ok = guard.set_update_source(app, &manager, config, &mut error);
                        Outcome::SourceSaved {
                            ok,
                            message: if ok {
                                "Update source saved".to_string()
                            } else {
                                error
                            },
                        }
                    },
                )
            }
            Message::UpdateSourceUnset(uuid) => self.spawn(
                TaskKind::RefreshMetadata,
                "Removing update source",
                &uuid.clone(),
                move |controller, _cancel| {
                    let mut guard = controller
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let Some(app) = guard.registry().by_uuid(&uuid) else {
                        return Outcome::SourceSaved {
                            ok: false,
                            message: "No installed app with that id".to_string(),
                        };
                    };
                    let mut error = String::new();
                    let ok = guard.unset_update_source(app, &mut error);
                    Outcome::SourceSaved {
                        ok,
                        message: if ok {
                            "Update source removed".to_string()
                        } else {
                            error
                        },
                    }
                },
            ),
            Message::DialogDismiss => {
                self.dialog = None;
                Command::none()
            }
            Message::DismissStatus => {
                self.status = None;
                Command::none()
            }
            Message::Finished(id, outcome) => self.handle_finished(id, *outcome),
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let page = self
            .nav_model
            .active_data::<Page>()
            .copied()
            .unwrap_or(Page::Library);
        let body: Element<Message> = match page {
            Page::Library => self.view_library(),
            Page::Inspect => self.view_inspect(),
            Page::Updates => self.view_updates(),
            Page::Tasks => self.view_tasks(),
            Page::Settings => self.view_settings(),
            Page::About => self.view_about(),
        };
        let mut children: Vec<Element<Message>> = vec![body];
        if let Some(bar) = self.view_status_bar() {
            children.push(bar);
        }
        widget::container(widget::column::with_children(children).spacing(8))
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Dialogs are presented on the shell's own dialog surface, so they get a
    /// scrim and focus handling. Composing them into the page column left the
    /// controls behind them live -- a confirmation that did not actually
    /// block the thing it was guarding.
    fn dialog(&self) -> Option<Element<'_, Self::Message>> {
        self.view_dialog()
    }
}

impl App {
    fn start_update(&mut self, uuid: String, force: bool) -> Command<Message> {
        self.spawn(
            TaskKind::Update,
            "Updating",
            &uuid.clone(),
            move |controller, cancel| {
                let mut guard = controller
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let Some(app) = guard.registry().by_uuid(&uuid) else {
                    return Outcome::Updated {
                        ok: false,
                        message: "No installed app with that id".to_string(),
                    };
                };
                let name = app.name.clone();
                let result = guard.apply_update(&app, force, &cancel);
                Outcome::Updated {
                    ok: result.ok,
                    message: if result.ok {
                        format!("Updated {name}")
                    } else {
                        result.error
                    },
                }
            },
        )
    }

    fn handle_finished(&mut self, id: String, outcome: Outcome) -> Command<Message> {
        let cancelled = self
            .busy
            .as_ref()
            .map(|(_, flag)| flag.load(Ordering::Relaxed))
            .unwrap_or(false);
        self.busy = None;
        let verbose = self
            .controller
            .lock()
            .map(|c| c.settings().debug_logging())
            .unwrap_or(false);

        let mut follow_up = Vec::new();
        let task_result: Result<(), String> = match outcome {
            Outcome::Inspected(result) => {
                self.finish_inspect(*result);
                if self.inspect.error.is_empty() {
                    Ok(())
                } else {
                    Err(self.inspect.error.clone())
                }
            }
            Outcome::Integrated {
                ok,
                message,
                conflict,
                source_path,
            } => {
                if ok {
                    self.set_status(Severity::Success, &message);
                    if let Some(previous) = self.inspect.result.take() {
                        previous.discard_staging();
                    }
                    self.inspect = InspectState {
                        queued: std::mem::take(&mut self.inspect.queued),
                        ..Default::default()
                    };
                    follow_up.push(self.load_library());
                    if !self.inspect.queued.is_empty() {
                        follow_up.push(Command::perform(async {}, |()| {
                            cosmic::app::Message::App(Message::NextQueuedFile)
                        }));
                    }
                    Ok(())
                } else if let Some((uuid, label)) = conflict {
                    self.dialog = Some(PendingDialog::IntegrateConflict {
                        path: source_path,
                        conflict_name: self.inspect.path_input.clone(),
                        replace_uuid: uuid,
                        replace_label: label,
                    });
                    Err(message)
                } else {
                    self.set_status(Severity::Error, format!("Integrate failed: {message}"));
                    Err(message)
                }
            }
            Outcome::Scanned(scan) => {
                self.updates = scan.offers.clone();
                self.check_failures = scan
                    .failures
                    .iter()
                    .map(|f| (f.name.clone(), f.error.clone()))
                    .collect();
                if self.check_failures.is_empty() {
                    self.set_status(
                        Severity::Info,
                        format!("{} update(s) available", self.updates.len()),
                    );
                    Ok(())
                } else {
                    self.set_status(
                        Severity::Error,
                        format!(
                            "{} update(s) available; {} app(s) could not be checked",
                            self.updates.len(),
                            self.check_failures.len()
                        ),
                    );
                    Err(format!("{} check(s) failed", self.check_failures.len()))
                }
            }
            Outcome::Updated { ok, message }
            | Outcome::Removed { ok, message }
            | Outcome::Adopted { ok, message }
            | Outcome::MetadataRefreshed { ok, message }
            | Outcome::SourceSaved { ok, message } => {
                self.set_status(
                    if ok {
                        Severity::Success
                    } else {
                        Severity::Error
                    },
                    &message,
                );
                follow_up.push(self.load_library());
                if ok {
                    Ok(())
                } else {
                    Err(message)
                }
            }
            Outcome::BatchUpdated {
                applied,
                failed,
                errors,
            } => {
                self.set_status(
                    if failed == 0 {
                        Severity::Success
                    } else {
                        Severity::Error
                    },
                    format!("Updated {applied}; {failed} failed"),
                );
                self.check_failures = errors
                    .iter()
                    .map(|e| ("update".to_string(), e.clone()))
                    .collect();
                follow_up.push(self.load_library());
                if failed == 0 {
                    Ok(())
                } else {
                    Err(format!("{failed} update(s) failed"))
                }
            }
            Outcome::LibraryLoaded {
                apps,
                running,
                discovered,
            } => {
                self.library = apps;
                self.running = running;
                self.discovered = discovered;
                Ok(())
            }
            Outcome::Revealed(result) => match result {
                Ok(()) => Ok(()),
                Err(error) => {
                    self.set_status(Severity::Error, format!("Cannot reveal: {error}"));
                    Err(error)
                }
            },
            Outcome::Simple(result) => match result {
                Ok(text) => {
                    self.set_status(Severity::Success, &text);
                    Ok(())
                }
                Err(error) => {
                    self.set_status(Severity::Error, &error);
                    Err(error)
                }
            },
        };

        diagnostics::emit_if(
            verbose,
            "gui",
            &format!("task finished ok={}", task_result.is_ok()),
        );
        self.with_controller(|c| c.tasks_mut().finish(&id, task_result, cancelled));
        self.refresh_tasks();
        Command::batch(follow_up)
    }

    fn active_page_title(&self) -> &str {
        self.nav_model
            .text(self.nav_model.active())
            .unwrap_or("Library")
    }

    fn update_title(&mut self) -> Command<Message> {
        // The window title is drawn by the compositor (taskbars, alt-tab), so
        // it carries the full string.
        //
        // The in-window header title does not: measured against a rendered
        // frame it came out at 3.38:1, below the 4.5:1 WCAG AA threshold for
        // body text, and it repeated information already on screen twice --
        // the nav bar highlights the active page at 8.7:1 and every page
        // carries its own heading at 13.9:1. Removing it drops a failing
        // element rather than restyling the toolkit around it.
        self.core_mut().set_header_title(String::new());
        self.set_window_title(format!(
            "Gosh AppImage Manager — {}",
            self.active_page_title()
        ))
    }

    /// The status line, with severity carried by text as well as colour so
    /// state is never communicated by colour alone.
    fn view_status_bar(&self) -> Option<Element<'_, Message>> {
        if let Some((task, _)) = &self.busy {
            let label = self
                .tasks
                .iter()
                .find(|t| &t.id == task)
                .map(|t| t.title.clone())
                .unwrap_or_else(|| "Working".to_string());
            return Some(
                widget::row::with_children(vec![
                    widget::text::body(format!("Working: {label}…")).into(),
                    widget::horizontal_space(Length::Fill).into(),
                    widget::button::standard(t!("action.cancel", "Cancel"))
                        .on_press(Message::Cancel)
                        .into(),
                ])
                .spacing(8)
                .align_items(cosmic::iced::Alignment::Center)
                .into(),
            );
        }
        let (severity, text) = self.status.as_ref()?;
        let prefix = match severity {
            Severity::Info => "Note",
            Severity::Success => "Done",
            Severity::Error => "Error",
        };
        Some(
            widget::row::with_children(vec![
                widget::text::body(format!("{prefix}: {text}")).into(),
                widget::horizontal_space(Length::Fill).into(),
                widget::button::text(t!("action.dismiss", "Dismiss"))
                    .on_press(Message::DismissStatus)
                    .into(),
            ])
            .spacing(8)
            .align_items(cosmic::iced::Alignment::Center)
            .into(),
        )
    }

    fn view_library(&self) -> Element<'_, Message> {
        if let Some(detail) = self.view_detail() {
            return detail;
        }
        let mut rows: Vec<Element<Message>> = Vec::new();
        rows.push(
            widget::row::with_children(vec![
                widget::text::title3(t!("nav.library", "Library")).into(),
                widget::horizontal_space(Length::Fill).into(),
                widget::button::standard(t!("action.refresh", "Refresh"))
                    .on_press(Message::LibraryRefresh)
                    .into(),
            ])
            .spacing(8)
            .into(),
        );
        // Search and sort share a row when there is room. In the condensed
        // layout the sort buttons win the fixed space and squeeze the search
        // field down to a stub barely wider than its clear button, so they
        // move to their own line instead.
        let search: Element<Message> = widget::search_input(
            t!("library.search", "Search by name, version or path"),
            &self.search,
        )
        .on_input(Message::SearchChanged)
        .on_clear(Message::SearchChanged(String::new()))
        .into();
        let sorts = vec![
            sort_button(SortOrder::Name, self.sort),
            sort_button(SortOrder::Version, self.sort),
            sort_button(SortOrder::UpdatesFirst, self.sort),
        ];
        rows.push(if self.narrow() {
            widget::column::with_children(vec![
                search,
                widget::row::with_children(sorts).spacing(8).into(),
            ])
            .spacing(8)
            .into()
        } else {
            let mut children = vec![search];
            children.extend(sorts);
            widget::row::with_children(children).spacing(8).into()
        });

        if self.library.is_empty() {
            // Empty state, distinguished from "still loading".
            rows.push(if self.busy.is_some() {
                widget::text::body(t!("library.loading", "Loading your library…")).into()
            } else {
                widget::column::with_children(vec![
                    widget::text::heading(t!("library.empty.title", "No AppImages yet")).into(),
                    widget::text::body(
                        "Open one from the Inspect page. Opening a file never integrates or \
                         executes it.",
                    )
                    .into(),
                ])
                .spacing(4)
                .into()
            });
        }

        let visible = self.visible_library();
        if !self.library.is_empty() && visible.is_empty() {
            rows.push(
                widget::text::body(format!("No AppImage matches “{}”.", self.search.trim())).into(),
            );
        }
        for app in visible {
            rows.push(self.view_library_row(app));
        }

        // Anything discovered but not registered, offered for explicit
        // adoption. This used to be filtered to external desktop entries
        // only, which silently skipped the commonest case of all: an AppImage
        // dropped straight into the managed folder. Nothing offered to adopt
        // it and it never appeared anywhere in the UI.
        let adoptable: Vec<&DiscoveredApp> =
            self.discovered.iter().filter(|d| !d.managed).collect();
        if !adoptable.is_empty() {
            rows.push(
                widget::text::title4(t!("library.adoptable.title", "Not managed yet")).into(),
            );
            rows.push(
                widget::text::caption(t!(
                    "library.adoptable.caption",
                    "Adopting registers an AppImage so it can be updated and removed here. \
                     Nothing on disk is changed."
                ))
                .into(),
            );
            for found in adoptable {
                let origin = match found.origin {
                    Origin::ManagedFolder => "in the managed folder".to_string(),
                    Origin::ExternalDesktopEntry => {
                        format!("outside the managed folder · {}", found.desktop_path)
                    }
                };
                rows.push(
                    widget::container(
                        widget::row::with_children(vec![
                            widget::column::with_children(vec![
                                widget::text::heading(found.name.clone()).into(),
                                widget::text::caption(found.path.clone()).into(),
                                widget::text::caption(origin).into(),
                            ])
                            .spacing(2)
                            .into(),
                            widget::horizontal_space(Length::Fill).into(),
                            widget::button::suggested(t!("action.adopt", "Adopt"))
                                .on_press(Message::AdoptAsk(found.path.clone()))
                                .into(),
                        ])
                        .spacing(8)
                        .align_items(cosmic::iced::Alignment::Center),
                    )
                    .padding(8)
                    .into(),
                );
            }
        }

        widget::scrollable(widget::column::with_children(rows).spacing(8)).into()
    }

    fn view_library_row<'a>(&'a self, app: &'a InstalledApp) -> Element<'a, Message> {
        let badges = badges_for(app, &self.running, &self.updates);
        let mut lines: Vec<Element<Message>> = vec![widget::row::with_children(vec![
            widget::text::heading(app.name.clone()).into(),
            widget::text::caption(badges).into(),
        ])
        .spacing(8)
        .into()];
        lines.push(
            widget::text::caption(format!(
                "{}  ·  {}",
                if app.version.is_empty() {
                    "no version".to_string()
                } else {
                    app.version.clone()
                },
                app.managed_path
            ))
            .into(),
        );
        let actions: Vec<Element<Message>> = vec![
            widget::button::suggested(t!("action.launch", "Launch"))
                .on_press(Message::Launch(app.uuid.clone()))
                .into(),
            widget::button::standard(t!("action.details", "Details"))
                .on_press(Message::SelectApp(app.uuid.clone()))
                .into(),
            widget::button::standard(t!("action.trash", "Trash"))
                .on_press(Message::RemoveAsk(app.uuid.clone(), false))
                .into(),
        ];
        // Below the breakpoint the actions stack rather than overflowing.
        lines.push(if self.narrow() {
            widget::column::with_children(actions).spacing(4).into()
        } else {
            widget::row::with_children(actions).spacing(8).into()
        });
        widget::container(widget::column::with_children(lines).spacing(4))
            .padding(8)
            .into()
    }

    /// Per-item detail: everything the brief asks a detail page to expose.
    fn view_detail(&self) -> Option<Element<'_, Message>> {
        let detail = self.detail.as_ref()?;
        let app = self.selected()?;
        let mut rows: Vec<Element<Message>> = vec![widget::row::with_children(vec![
            widget::button::standard(t!("detail.back", "← Library"))
                .on_press(Message::CloseDetail)
                .into(),
            widget::text::title3(&app.name).into(),
        ])
        .spacing(8)
        .align_items(cosmic::iced::Alignment::Center)
        .into()];

        let facts = [
            ("Path", app.managed_path.clone()),
            ("Desktop ID", app.desktop_id.clone()),
            ("SHA-256", hex::encode(&app.sha256)),
            ("Type", app_image_type_name(app.app_type).to_string()),
            (
                "Architecture",
                architecture_name(app.architecture).to_string(),
            ),
            ("Version", app.version.clone()),
            ("Size", human_size(app.size)),
            (
                "Update manager",
                if app.update_manager.is_empty() {
                    "none configured".to_string()
                } else {
                    app.update_manager.clone()
                },
            ),
            (
                "Embedded source",
                if app.embedded_update.is_empty() {
                    "none".to_string()
                } else {
                    app.embedded_update.clone()
                },
            ),
            (
                "Provenance",
                if app.adopted {
                    "adopted".to_string()
                } else {
                    "integrated here".to_string()
                },
            ),
        ];
        let mut fact_rows: Vec<Element<Message>> = Vec::new();
        for (label, value) in facts {
            fact_rows.push(label_value_row(label, value, self.narrow()));
        }
        rows.push(
            widget::settings::section()
                .title(t!("action.details", "Details"))
                .add(widget::column::with_children(fact_rows).spacing(2))
                .into(),
        );

        rows.push(
            widget::settings::section()
                .title(t!("detail.section.actions", "Actions"))
                .add(
                    widget::column::with_children(vec![
                        widget::button::suggested(t!("action.launch", "Launch"))
                            .on_press(Message::Launch(app.uuid.clone()))
                            .into(),
                        widget::button::standard(t!("action.reveal", "Reveal in file manager"))
                            .on_press(Message::Reveal(app.managed_path.clone()))
                            .into(),
                        widget::button::standard(t!("action.checkupdate", "Check and update now"))
                            .on_press(Message::UpdateOne(app.uuid.clone()))
                            .into(),
                        widget::button::standard(t!("action.refreshmeta", "Refresh metadata"))
                            .on_press(Message::RefreshMetadata(app.uuid.clone()))
                            .into(),
                        widget::button::standard(t!("action.trash.long", "Move to Trash"))
                            .on_press(Message::RemoveAsk(app.uuid.clone(), false))
                            .into(),
                        widget::button::destructive(t!("action.delete", "Delete permanently"))
                            .on_press(Message::RemoveAsk(app.uuid.clone(), true))
                            .into(),
                    ])
                    .spacing(6),
                )
                .into(),
        );

        rows.push(
            widget::settings::section()
                .title(t!("detail.section.arguments", "Command arguments"))
                .add(
                    widget::column::with_children(vec![
                        widget::text::caption(t!(
                            "detail.arguments.caption",
                            "One argument per line. These are passed as separate arguments, \
                             never as a shell command."
                        ))
                        .into(),
                        widget::text_input("--example-flag", &detail.arguments_input)
                            .on_input(Message::ArgumentsChanged)
                            .into(),
                    ])
                    .spacing(4),
                )
                .add(
                    widget::column::with_children(vec![
                        widget::text::caption(t!(
                            "detail.environment.caption",
                            "Environment variables, one NAME=value per line."
                        ))
                        .into(),
                        widget::text_input("NAME=value", &detail.environment_input)
                            .on_input(Message::EnvironmentChanged)
                            .into(),
                        widget::button::suggested(t!("action.save", "Save"))
                            .on_press(Message::SaveArgumentsAndEnvironment)
                            .into(),
                    ])
                    .spacing(4),
                )
                .into(),
        );

        rows.push(
            widget::settings::section()
                .title(t!("detail.section.source", "Update source"))
                .add(
                    widget::column::with_children(vec![
                        widget::text_input(
                            "Manager (static, github, gitlab, codeberg, forgejo, ftp)",
                            &self.source_manager_input,
                        )
                        .on_input(Message::UpdateSourceManagerChanged)
                        .into(),
                        widget::text_input("key=value", &self.source_config_input)
                            .on_input(Message::UpdateSourceConfigChanged)
                            .into(),
                        widget::row::with_children(vec![
                            widget::button::suggested(t!("action.apply", "Apply"))
                                .on_press(Message::UpdateSourceApply(app.uuid.clone()))
                                .into(),
                            widget::button::standard(t!("action.reset", "Reset"))
                                .on_press(Message::UpdateSourceUnset(app.uuid.clone()))
                                .into(),
                        ])
                        .spacing(8)
                        .into(),
                    ])
                    .spacing(4),
                )
                .into(),
        );

        Some(widget::scrollable(widget::column::with_children(rows).spacing(12)).into())
    }

    fn view_inspect(&self) -> Element<'_, Message> {
        let mut col: Vec<Element<Message>> = vec![
            widget::text::title3(t!("inspect.title", "Inspect an AppImage")).into(),
            widget::text::caption(t!(
                "inspect.caption",
                "Opening a file only inspects it. Nothing is integrated or executed."
            ))
            .into(),
        ];
        col.push(
            widget::row::with_children(vec![
                widget::text_input(
                    t!("inspect.path.placeholder", "Path to .AppImage"),
                    &self.inspect.path_input,
                )
                .on_input(Message::InspectPathChanged)
                .on_submit(Message::InspectRun)
                .into(),
                widget::button::standard(t!("action.browse", "Browse…"))
                    .on_press(Message::InspectBrowse)
                    .into(),
                widget::button::suggested(t!("nav.inspect", "Inspect"))
                    .on_press(Message::InspectRun)
                    .into(),
            ])
            .spacing(8)
            .into(),
        );

        if !self.inspect.queued.is_empty() {
            col.push(
                widget::text::caption(format!(
                    "{} more file(s) queued; each is inspected and confirmed on its own.",
                    self.inspect.queued.len()
                ))
                .into(),
            );
        }

        if self.busy.is_some() && self.inspect.summary.is_empty() {
            col.push(
                widget::text::body(t!(
                    "inspect.working",
                    "Inspecting… this reads and hashes the file."
                ))
                .into(),
            );
        }

        if !self.inspect.error.is_empty() {
            col.push(
                widget::text::heading(t!("inspect.error.title", "Cannot inspect this file")).into(),
            );
            col.push(widget::text::body(&self.inspect.error).into());
        }

        for (label, value) in &self.inspect.summary {
            col.push(label_value_row(label, value, self.narrow()));
        }
        for warning in &self.inspect.warnings {
            col.push(widget::text::body(format!("Warning: {warning}")).into());
        }

        if self.inspect.inspected_ok {
            col.push(
                widget::button::suggested(t!("action.integrate", "Integrate"))
                    .on_press(Message::IntegrateRun)
                    .into(),
            );
        }
        widget::scrollable(widget::column::with_children(col).spacing(8)).into()
    }

    fn view_updates(&self) -> Element<'_, Message> {
        let mut col: Vec<Element<Message>> = Vec::new();
        let controls: Vec<Element<Message>> = vec![
            widget::button::standard(t!("updates.check", "Check now"))
                .on_press(Message::UpdatesRefresh)
                .into(),
            widget::button::suggested(t!("updates.all", "Update all"))
                .on_press(Message::UpdateAll)
                .into(),
        ];
        col.push(
            widget::row::with_children(vec![
                widget::text::title3(t!("nav.updates", "Updates")).into(),
                widget::horizontal_space(Length::Fill).into(),
                if self.narrow() {
                    widget::column::with_children(controls).spacing(4).into()
                } else {
                    widget::row::with_children(controls).spacing(8).into()
                },
            ])
            .spacing(8)
            .into(),
        );

        if self.busy.is_some() {
            col.push(widget::text::body(t!("updates.checking", "Checking update sources…")).into());
        }

        // Failures first: "everything is up to date" must never be shown for
        // apps that could not be checked at all.
        if !self.check_failures.is_empty() {
            col.push(
                widget::text::heading(t!(
                    "updates.failures.title",
                    "Some apps could not be checked"
                ))
                .into(),
            );
            for (name, error) in &self.check_failures {
                col.push(widget::text::body(format!("{name}: {error}")).into());
            }
        }

        if self.updates.is_empty() && self.check_failures.is_empty() && self.busy.is_none() {
            col.push(if self.library.is_empty() {
                widget::text::body(t!(
                    "updates.none.integrated",
                    "No AppImages are integrated yet."
                ))
                .into()
            } else {
                widget::text::body(t!("updates.uptodate", "Everything is up to date.")).into()
            });
        }

        for offer in &self.updates {
            let mut labels = vec![format!(
                "{} → {} ({})",
                if offer.current_version.is_empty() {
                    "unknown".to_string()
                } else {
                    offer.current_version.clone()
                },
                offer.available_version,
                offer.manager
            )];
            if offer.running {
                labels.push("running — updating replaces the file under a live app".into());
            }
            if offer.reduced_verification {
                labels.push("reduced verification: no checksum published".into());
            }
            if offer.download_size > 0 {
                labels.push(human_size(offer.download_size));
            }
            col.push(
                widget::container(
                    widget::row::with_children(vec![
                        widget::column::with_children({
                            let mut lines: Vec<Element<Message>> =
                                vec![widget::text::heading(offer.name.clone()).into()];
                            for label in labels {
                                lines.push(widget::text::caption(label).into());
                            }
                            lines
                        })
                        .spacing(2)
                        .into(),
                        widget::horizontal_space(Length::Fill).into(),
                        widget::button::suggested(t!("action.update", "Update"))
                            .on_press(Message::UpdateOne(offer.uuid.clone()))
                            .into(),
                    ])
                    .spacing(8)
                    .align_items(cosmic::iced::Alignment::Center),
                )
                .padding(8)
                .into(),
            );
        }
        widget::scrollable(widget::column::with_children(col).spacing(8)).into()
    }

    fn view_tasks(&self) -> Element<'_, Message> {
        let mut col: Vec<Element<Message>> = vec![widget::row::with_children(vec![
            widget::text::title3(t!("nav.tasks", "Tasks")).into(),
            widget::horizontal_space(Length::Fill).into(),
            widget::button::standard(t!("tasks.clear", "Clear finished"))
                .on_press(Message::ClearFinishedTasks)
                .into(),
        ])
        .spacing(8)
        .into()];

        if self.tasks.is_empty() {
            col.push(widget::text::body(t!("tasks.empty", "Nothing has run yet.")).into());
        }
        for task in &self.tasks {
            let state = match task.state {
                TaskState::Queued => "queued",
                TaskState::Running => "running",
                TaskState::Cancelling => "cancelling",
                TaskState::Succeeded => "succeeded",
                TaskState::Failed => "failed",
                TaskState::Cancelled => "cancelled",
            };
            let mut lines: Vec<Element<Message>> =
                vec![widget::text::heading(format!("{} — {state}", task.title)).into()];
            if !task.target.is_empty() {
                lines.push(widget::text::caption(&task.target).into());
            }
            if matches!(task.state, TaskState::Running | TaskState::Cancelling) {
                #[allow(clippy::cast_precision_loss)]
                let progress = task.progress as f32;
                lines.push(widget::progress_bar(0.0..=100.0, progress).into());
            }
            if !task.error.is_empty() {
                lines.push(widget::text::body(format!("Error: {}", task.error)).into());
            }
            col.push(
                widget::container(widget::column::with_children(lines).spacing(4))
                    .padding(8)
                    .into(),
            );
        }
        widget::scrollable(widget::column::with_children(col).spacing(8)).into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let settings_snapshot = self.with_controller(|c| SettingsSnapshot::read(c));
        let appearance_row = widget::row::with_children(vec![
            widget::button::standard(t!("appearance.system", "System"))
                .on_press(Message::AppearanceSelected(Appearance::System))
                .into(),
            widget::button::standard(t!("appearance.light", "Light"))
                .on_press(Message::AppearanceSelected(Appearance::Light))
                .into(),
            widget::button::standard(t!("appearance.dark", "Dark"))
                .on_press(Message::AppearanceSelected(Appearance::Dark))
                .into(),
            widget::text::body(format!(
                "{} {}",
                t!("settings.appearance.current", "Current:"),
                t!(
                    match self.appearance {
                        Appearance::System => "appearance.system",
                        Appearance::Light => "appearance.light",
                        Appearance::Dark => "appearance.dark",
                    },
                    appearance_name(self.appearance)
                )
            ))
            .into(),
        ])
        .spacing(8);

        let col = widget::column::with_children(vec![
            widget::text::title3(t!("nav.settings", "Settings")).into(),
            widget::settings::section()
                .title(t!("settings.appearance", "Appearance"))
                .add(appearance_row)
                .into(),
            widget::settings::section()
                .title(t!("settings.folder", "Integration folder"))
                .add(
                    widget::row::with_children(vec![
                        widget::text_input(
                            t!("settings.folder.placeholder", "Managed folder"),
                            &self.managed_folder_input,
                        )
                        .on_input(Message::ManagedFolderChanged)
                        .on_submit(Message::ManagedFolderApply)
                        .into(),
                        widget::button::standard(t!("action.apply", "Apply"))
                            .on_press(Message::ManagedFolderApply)
                            .into(),
                    ])
                    .spacing(8),
                )
                .into(),
            widget::settings::section()
                .title(t!("settings.behaviour", "Behaviour"))
                // Each toggler carries its own label, so assistive technology
                // announces what the switch is for.
                .add(widget::settings::item(
                    t!(
                        "settings.movesource",
                        "Move the original into the library instead of copying"
                    ),
                    widget::toggler(
                        None,
                        settings_snapshot.move_source,
                        Message::MoveSourceToggled,
                    ),
                ))
                .add(widget::settings::item(
                    t!(
                        "settings.outside",
                        "Discover AppImages outside the managed folder"
                    ),
                    widget::toggler(
                        None,
                        settings_snapshot.manage_outside_folder,
                        Message::ManageOutsideToggled,
                    ),
                ))
                .add(widget::settings::item(
                    t!(
                        "settings.terminalsuffix",
                        "Terminal apps: drop the .AppImage suffix from the name"
                    ),
                    widget::toggler(
                        None,
                        settings_snapshot.terminal_omit_suffix,
                        Message::TerminalSuffixToggled,
                    ),
                ))
                .add(widget::settings::item(
                    t!("settings.debug", "Verbose diagnostics"),
                    widget::toggler(
                        None,
                        settings_snapshot.debug_logging,
                        Message::DebugLoggingToggled,
                    ),
                ))
                .into(),
            widget::settings::section()
                .title(t!("settings.updates", "Update checks"))
                .add(widget::settings::item(
                    t!(
                        "settings.background",
                        "Check for updates in the background (notify only)"
                    ),
                    widget::toggler(
                        None,
                        settings_snapshot.background_update_checks,
                        Message::BackgroundToggled,
                    ),
                ))
                .add(widget::settings::item(
                    t!("settings.autostart", "Run those checks at login"),
                    widget::toggler(None, self.autostart_present, Message::AutostartToggled),
                ))
                .add(widget::text::caption(t!(
                    "settings.background.caption",
                    "Background checks only look at the update endpoints you configured, and \
                         never download or apply anything."
                )))
                .into(),
            widget::settings::section()
                .title(t!("settings.unsafe", "Unsafe extraction fallback"))
                .add(widget::settings::item(
                    t!(
                        "settings.unsafe.item",
                        "Run the AppImage to read its metadata when safe extraction fails"
                    ),
                    widget::toggler(
                        None,
                        settings_snapshot.unsafe_extraction_fallback,
                        Message::UnsafeFallbackToggled,
                    ),
                ))
                .add(widget::text::caption(t!(
                    "settings.unsafe.caption",
                    "Off by default. This executes untrusted code, and each file still has to be \
                     confirmed individually."
                )))
                .into(),
        ])
        .spacing(12);
        widget::scrollable(col).into()
    }

    fn view_about(&self) -> Element<'_, Message> {
        widget::scrollable(
            widget::column::with_children(vec![
                // The product name is not translated.
                widget::text::title3("Gosh AppImage Manager").into(),
                widget::text::body(format!(
                    "{} {}",
                    t!("about.version", "Version"),
                    limits::VERSION
                ))
                .into(),
                widget::text::body(t!("about.byline", "Made by Gosh")).into(),
                widget::text::body(t!(
                "about.description",
                "Native COSMIC Epoch application for safely inspecting, integrating, launching, \
                 organizing, updating, and removing AppImages."
            ))
                .into(),
                widget::text::body(t!(
                    "about.safety",
                    "Opening an AppImage never integrates or executes it. Updates are staged, \
                 validated, and applied atomically with rollback."
                ))
                .into(),
                widget::text::body(t!(
                    "about.attribution",
                    "Behavioural reference: Gear Lever by Lorenzo Paderi. This is an independent \
                 original implementation and is not endorsed by its authors."
                ))
                .into(),
                widget::text::body(t!(
                "about.licence",
                "Licensed under the GNU General Public License, version 3 or later. This program \
                 comes with absolutely no warranty."
            ))
                .into(),
                widget::text::body(t!("about.telemetry", "No telemetry of any kind.")).into(),
            ])
            .spacing(6),
        )
        .into()
    }

    fn view_dialog(&self) -> Option<Element<'_, Message>> {
        let pending = self.dialog.clone()?;
        Some(match pending {
            PendingDialog::IntegrateConflict {
                path,
                conflict_name,
                replace_uuid,
                replace_label,
            } => {
                let mut dialog = widget::dialog("An AppImage with that name is already managed")
                    .body(format!(
                        "{conflict_name} would collide with {replace_label}. Keep both copies, \
                         or replace the managed installation?"
                    ))
                    .primary_action(
                        widget::button::suggested(t!("dialog.keepboth", "Keep both"))
                            .on_press(Message::IntegrateKeepBoth(path.clone())),
                    )
                    .tertiary_action(
                        widget::button::standard(t!("action.cancel", "Cancel"))
                            .on_press(Message::DialogDismiss),
                    );
                // Replace is only offered when exactly one installation is
                // implicated; the user no longer has to supply a UUID.
                if !replace_uuid.is_empty() {
                    dialog = dialog.secondary_action(
                        widget::button::destructive(format!("Replace {replace_label}"))
                            .on_press(Message::IntegrateReplace(path, replace_uuid)),
                    );
                }
                dialog.into()
            }
            PendingDialog::Remove {
                name,
                path,
                permanent,
                ..
            } => {
                let (title, body) = if permanent {
                    (
                        format!("Permanently delete {name}?"),
                        format!(
                            "{path} will be deleted immediately. This cannot be undone, and the \
                             Gosh desktop entry and icon are removed with it."
                        ),
                    )
                } else {
                    (
                        format!("Move {name} to Trash?"),
                        format!(
                            "{path} goes to Trash. The desktop entry and icon are removed only \
                             after Trash succeeds."
                        ),
                    )
                };
                widget::dialog(title)
                    .body(body)
                    .primary_action(
                        widget::button::destructive(if permanent {
                            "Delete permanently"
                        } else {
                            "Move to Trash"
                        })
                        .on_press(Message::RemoveConfirm),
                    )
                    .secondary_action(
                        widget::button::standard(t!("action.cancel", "Cancel"))
                            .on_press(Message::DialogDismiss),
                    )
                    .into()
            }
            PendingDialog::UnsafeExtract => widget::dialog(
                "Allow running AppImages to read metadata?",
            )
            .body(
                "When safe extraction fails, this runs the AppImage's own --appimage-extract, \
                     which executes untrusted code. Each file still has to be confirmed \
                     separately, and background checks never use it.",
            )
            .primary_action(
                widget::button::destructive(t!("action.enable", "Enable"))
                    .on_press(Message::UnsafeFallbackConfirm),
            )
            .secondary_action(
                widget::button::standard(t!("action.keepoff", "Keep off"))
                    .on_press(Message::DialogDismiss),
            )
            .into(),
            PendingDialog::UpdateForce { name, .. } => widget::dialog(format!("{name} is running"))
                .body(
                    "Updating replaces the file underneath a running application, which may \
                         make it behave unpredictably until it is restarted.",
                )
                .primary_action(
                    widget::button::destructive(t!("dialog.forceupdate", "Update anyway"))
                        .on_press(Message::UpdateForceConfirm),
                )
                .secondary_action(
                    widget::button::standard(t!("action.cancel", "Cancel"))
                        .on_press(Message::DialogDismiss),
                )
                .into(),
            PendingDialog::Adopt { path } => widget::dialog("Adopt this AppImage?")
                .body(format!(
                    "{path} will be registered so it can be updated and removed here. Nothing on \
                     disk is changed, and its existing desktop entry is left alone."
                ))
                .primary_action(
                    widget::button::suggested(t!("action.adopt", "Adopt"))
                        .on_press(Message::AdoptConfirm),
                )
                .secondary_action(
                    widget::button::standard(t!("action.cancel", "Cancel"))
                        .on_press(Message::DialogDismiss),
                )
                .into(),
        })
    }
}

/// Settings read once per frame instead of per widget, so the render path
/// takes the controller lock once and performs no filesystem calls.
struct SettingsSnapshot {
    move_source: bool,
    manage_outside_folder: bool,
    terminal_omit_suffix: bool,
    debug_logging: bool,
    background_update_checks: bool,
    unsafe_extraction_fallback: bool,
}

impl SettingsSnapshot {
    fn read(controller: &AppController) -> Self {
        let s = controller.settings();
        Self {
            move_source: s.move_source(),
            manage_outside_folder: s.manage_outside_folder(),
            terminal_omit_suffix: s.terminal_omit_suffix(),
            debug_logging: s.debug_logging(),
            background_update_checks: s.background_update_checks(),
            unsafe_extraction_fallback: s.unsafe_extraction_fallback(),
        }
    }
}

/// A label/value pair.
///
/// The label column used to be `Length::Fixed(140.0)`. A fixed pixel width
/// does not grow with the user's text size, so at large scaling the label
/// clips while the value beside it moves on without it. Proportional widths
/// hold the same split whatever the text size, and in the condensed layout
/// the pair stacks rather than fighting over a narrow row.
fn label_value_row<'a>(
    label: impl Into<String>,
    value: impl Into<String>,
    narrow: bool,
) -> Element<'a, Message> {
    let label = widget::text::caption(label.into());
    let value = widget::text::body(value.into());
    if narrow {
        widget::column::with_children(vec![label.into(), value.into()])
            .spacing(2)
            .into()
    } else {
        widget::row::with_children(vec![
            label.width(Length::FillPortion(2)).into(),
            value.width(Length::FillPortion(5)).into(),
        ])
        .spacing(8)
        .into()
    }
}

/// One sort choice, styled to show which is active.
fn sort_button<'a>(order: SortOrder, active: SortOrder) -> Element<'a, Message> {
    let button = if order == active {
        widget::button::suggested(order.label())
    } else {
        widget::button::standard(order.label())
    };
    button.on_press(Message::SortChanged(order)).into()
}

fn badges_for(app: &InstalledApp, running: &[String], updates: &[UpdateOffer]) -> String {
    let mut badges = Vec::new();
    if running.contains(&app.uuid) {
        badges.push("running");
    }
    if updates.iter().any(|o| o.uuid == app.uuid) {
        badges.push("update available");
    }
    if app.external_folder {
        badges.push("external folder");
    }
    if app.adopted {
        badges.push("adopted");
    }
    badges.join(" · ")
}

/// A file manager passing `%U` hands us a URL, not a path.
fn normalise_open_target(raw: &str) -> String {
    if let Ok(url) = url::Url::parse(raw) {
        if url.scheme() == "file" {
            if let Ok(path) = url.to_file_path() {
                return path.to_string_lossy().into_owned();
            }
        }
    }
    raw.to_string()
}

/// Parse `NAME=value` lines, reporting names that are not valid rather than
/// dropping them silently.
fn parse_environment(text: &str) -> (Vec<EnvPair>, Vec<String>) {
    let mut pairs = Vec::new();
    let mut rejected = Vec::new();
    for line in text.lines().take(limits::MAX_ENV_PAIRS * 2) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once('=') {
            Some((name, value)) => {
                let name = name.trim();
                if crate::desktop::valid_env_name(name) {
                    pairs.push(EnvPair {
                        name: name.to_string(),
                        value: value.trim().to_string(),
                    });
                } else {
                    rejected.push(name.to_string());
                }
            }
            None => rejected.push(line.to_string()),
        }
    }
    (pairs, rejected)
}

fn parse_config_lines(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}
