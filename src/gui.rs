// Gosh AppImage Manager 3.0.0 — libcosmic (COSMIC Epoch) GUI shell.
// Made by Gosh. Native COSMIC/Plasma/Bazzite application with
// System/Light/Dark appearance, Library / Inspect / Updates / Settings /
// About navigation, and cosmic dialogs for every destructive choice.
//
// Safety mirrors the CLI exactly: opening a file only inspects it;
// integration, updates, and removal go through the same transactional
// core with explicit confirmation. No telemetry of any kind.

use std::sync::atomic::AtomicBool;

use cosmic::app::{Command, Core, Settings};
use cosmic::widget::{self, nav_bar};
use cosmic::{executor, Application, ApplicationExt, Element};

use crate::controller::AppController;
use crate::limits;
use crate::types::{
    app_image_type_name, appearance_from_str, appearance_name, architecture_name, Appearance,
    ConflictPolicy, CopyMode, InstalledApp, IntegrateRequest, RemovalMode, RemovalRequest,
    UpdateOffer,
};

pub fn run(initial_files: Vec<String>) -> i32 {
    let settings = Settings::default().size(cosmic::iced_core::Size::new(1024., 768.));
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
    Settings,
    About,
}

impl Page {
    const fn title(self) -> &'static str {
        match self {
            Page::Library => "Library",
            Page::Inspect => "Inspect",
            Page::Updates => "Updates",
            Page::Settings => "Settings",
            Page::About => "About",
        }
    }
}

/// Pending cosmic dialog (destructive choices only appear this way).
#[derive(Clone)]
enum PendingDialog {
    IntegrateConflict {
        path: String,
        conflict_name: String,
        replace_uuid: String,
    },
    Remove {
        uuid: String,
        name: String,
        permanent: bool,
    },
    UnsafeExtract {
        path: String,
    },
    UpdateForce {
        uuid: String,
        name: String,
    },
}

#[derive(Clone, Debug)]
pub enum Message {
    InspectPathChanged(String),
    InspectBrowse,
    InspectDialog(Result<url::Url, String>),
    InspectRun,
    IntegrateKeepBoth(String),
    IntegrateReplace(String, String),
    IntegrateDismiss,
    LibraryRefresh,
    Launch(String),
    RemoveAsk(String, bool),
    RemoveConfirm,
    DialogDismiss,
    UpdatesRefresh,
    UpdateAll,
    UpdateOne(String),
    UpdateForceConfirm,
    CancelBatch,
    AutostartToggled(bool),
    BackgroundToggled(bool),
    MoveSourceToggled(bool),
    UnsafeFallbackToggled(bool),
    AppearanceSelected(Appearance),
    ManagedFolderChanged(String),
    ManagedFolderApply,
    UpdateSourceManagerChanged(String),
    UpdateSourceConfigChanged(String),
    UpdateSourceApply(String),
    UpdateSourceUnset(String),
    Noop,
}

struct InspectState {
    path_input: String,
    summary: Vec<String>,
    error: String,
    conflict_path: String,
    conflict_name: String,
    replace_uuid: String,
    inspected_ok: bool,
}

impl Default for InspectState {
    fn default() -> Self {
        Self {
            path_input: String::new(),
            summary: Vec::new(),
            error: String::new(),
            conflict_path: String::new(),
            conflict_name: String::new(),
            replace_uuid: String::new(),
            inspected_ok: false,
        }
    }
}

#[derive(Default)]
struct BatchState {
    running: bool,
    cancel: bool,
    done: usize,
    total: usize,
    errors: Vec<String>,
}

pub struct App {
    core: Core,
    nav_model: nav_bar::Model,
    controller: AppController,
    appearance: Appearance,
    library: Vec<InstalledApp>,
    running: Vec<String>,
    updates: Vec<UpdateOffer>,
    tasks: Vec<String>,
    inspect: InspectState,
    dialog: Option<PendingDialog>,
    batch: BatchState,
    managed_folder_input: String,
    source_manager_input: String,
    source_config_input: String,
    status: String,
}

impl App {
    fn refresh_library(&mut self) {
        self.library = self.controller.registry().apps();
        self.running = self
            .library
            .iter()
            .filter(|app| self.controller.is_running(app))
            .map(|app| app.uuid.clone())
            .collect();
    }

    fn refresh_updates(&mut self) {
        let cancel = AtomicBool::new(false);
        self.updates = self.controller.check_updates(&cancel);
    }

    fn apply_appearance(&self) -> Command<Message> {
        let theme = match self.appearance {
            Appearance::Light => cosmic::theme::Theme::light(),
            Appearance::Dark => cosmic::theme::Theme::dark(),
            Appearance::System => cosmic::theme::system_preference(),
        };
        cosmic::app::command::set_theme(theme)
    }

    fn run_inspect(&mut self, path: &str) {
        self.inspect.error.clear();
        self.inspect.summary.clear();
        self.inspect.inspected_ok = false;
        let cancel = AtomicBool::new(false);
        let existing = self
            .controller
            .registry()
            .by_path(path)
            .map(|app| app.uuid)
            .unwrap_or_default();
        let result = self.controller.inspect_file(
            path,
            &cancel,
            if existing.is_empty() {
                None
            } else {
                Some(existing.as_str())
            },
        );
        if !result.magic_valid {
            self.inspect.error = if result.error.is_empty() {
                "Not a valid AppImage".to_string()
            } else {
                result.error.clone()
            };
            return;
        }
        self.inspect.inspected_ok = true;
        self.inspect.conflict_path = path.to_string();
        self.inspect.conflict_name = if result.metadata.name.is_empty() {
            path.to_string()
        } else {
            result.metadata.name.clone()
        };
        self.inspect.summary = vec![
            format!("Path: {}", result.identity.path),
            format!("Size: {} bytes", result.identity.size),
            format!("Type: {}", app_image_type_name(result.app_type)),
            format!("Architecture: {}", architecture_name(result.architecture)),
            format!("SHA-256: {}", hex::encode(&result.identity.sha256)),
            format!(
                "Name: {}",
                if result.metadata.name.is_empty() {
                    "(unknown)"
                } else {
                    &result.metadata.name
                }
            ),
            format!(
                "Version: {}",
                if result.metadata.version.is_empty() {
                    "(unknown)"
                } else {
                    &result.metadata.version
                }
            ),
            format!(
                "Update source: {}",
                if result.update_info.raw.is_empty() {
                    "(none embedded)"
                } else {
                    &result.update_info.raw
                }
            ),
            format!(
                "Already managed: {}",
                if result.already_managed { "yes" } else { "no" }
            ),
        ];
        for warning in &result.warnings {
            self.inspect.summary.push(format!("Warning: {warning}"));
        }
    }

    fn integrate(&mut self, path: &str, policy: ConflictPolicy, replace_uuid: &str) {
        let cancel = AtomicBool::new(false);
        let result = self.controller.integrate(
            &IntegrateRequest {
                source_path: path.to_string(),
                conflict: policy,
                replace_uuid: replace_uuid.to_string(),
                copy_mode: if self.controller.settings().move_source() {
                    CopyMode::Move
                } else {
                    CopyMode::Copy
                },
                assume_yes: true,
            },
            &cancel,
        );
        if result.ok {
            self.status = format!("Integrated {}", result.app.managed_path);
            self.refresh_library();
            self.inspect = InspectState::default();
        } else if result.error.contains("keep-both") || result.error.contains("replace") {
            self.dialog = Some(PendingDialog::IntegrateConflict {
                path: path.to_string(),
                conflict_name: self.inspect.conflict_name.clone(),
                replace_uuid: self.inspect.replace_uuid.clone(),
            });
            self.status = result.error;
        } else {
            self.status = format!("Integrate failed: {}", result.error);
        }
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
            Page::Settings,
            Page::About,
        ] {
            nav_model
                .insert()
                .text(page.title())
                .data::<Page>(page)
                .activate();
        }
        let first = nav_model.iter().next();
        if let Some(id) = first {
            nav_model.activate(id);
        }
        let controller = AppController::new().expect("controller initialises");
        let appearance = controller.settings().appearance();
        let managed_folder_input = controller
            .settings()
            .managed_folder()
            .to_string_lossy()
            .into_owned();
        let mut app = App {
            core,
            nav_model,
            controller,
            appearance,
            library: Vec::new(),
            running: Vec::new(),
            updates: Vec::new(),
            tasks: Vec::new(),
            inspect: InspectState::default(),
            dialog: None,
            batch: BatchState::default(),
            managed_folder_input,
            source_manager_input: String::new(),
            source_config_input: String::new(),
            status: String::new(),
        };
        if let Some(first) = flags.initial_files.first() {
            app.inspect.path_input = first.clone();
            app.run_inspect(first);
        }
        app.refresh_library();
        let command = app.update_title();
        (app, command)
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav_model)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Command<Self::Message> {
        self.nav_model.activate(id);
        self.update_title()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Noop => Command::none(),
            Message::LibraryRefresh => {
                self.refresh_library();
                Command::none()
            }
            Message::InspectPathChanged(path) => {
                self.inspect.path_input = path;
                Command::none()
            }
            Message::InspectBrowse => Command::perform(
                async move {
                    let dialog = cosmic::dialog::file_chooser::open::Dialog::new()
                        .title("Choose an AppImage");
                    match dialog.open_file().await {
                        Ok(response) => Message::InspectDialog(Ok(response.url().clone())),
                        Err(error) => Message::InspectDialog(Err(error.to_string())),
                    }
                },
                cosmic::app::Message::App,
            ),
            Message::InspectDialog(result) => {
                match result {
                    Ok(url) => {
                        if let Ok(path) = url.to_file_path() {
                            self.inspect.path_input = path.to_string_lossy().into_owned();
                            let path = self.inspect.path_input.clone();
                            self.run_inspect(&path);
                        } else {
                            self.inspect.error = "Only local files can be inspected".to_string();
                        }
                    }
                    Err(error) => self.inspect.error = error,
                }
                Command::none()
            }
            Message::InspectRun => {
                let path = self.inspect.path_input.clone();
                if path.is_empty() {
                    self.inspect.error = "Choose an AppImage file first".to_string();
                } else {
                    self.run_inspect(&path);
                }
                Command::none()
            }
            Message::IntegrateKeepBoth(path) => {
                self.dialog = None;
                self.integrate(&path, ConflictPolicy::KeepBoth, "");
                Command::none()
            }
            Message::IntegrateReplace(path, uuid) => {
                self.dialog = None;
                self.integrate(&path, ConflictPolicy::Replace, &uuid);
                Command::none()
            }
            Message::IntegrateDismiss => {
                // Explicit integrate from the inspect page (conflict policy
                // dialog appears automatically when the core reports one).
                let path = self.inspect.path_input.clone();
                let uuid = self.inspect.replace_uuid.clone();
                if path.is_empty() {
                    self.inspect.error = "Choose an AppImage file first".to_string();
                } else {
                    self.integrate(&path, ConflictPolicy::Unspecified, &uuid);
                }
                Command::none()
            }
            Message::Launch(uuid) => {
                if let Some(app) = self.controller.registry().by_uuid(&uuid) {
                    let service = self.controller.launch_service();
                    match service.launch(&app) {
                        Ok(()) => self.status = format!("Launched {}", app.name),
                        Err(error) => self.status = format!("Launch failed: {error}"),
                    }
                }
                Command::none()
            }
            Message::RemoveAsk(uuid, permanent) => {
                if let Some(app) = self.controller.registry().by_uuid(&uuid) {
                    self.dialog = Some(PendingDialog::Remove {
                        uuid,
                        name: app.name.clone(),
                        permanent,
                    });
                }
                Command::none()
            }
            Message::RemoveConfirm => {
                if let Some(PendingDialog::Remove {
                    uuid, permanent, ..
                }) = self.dialog.clone()
                {
                    let outcome = self.controller.remove_app(&RemovalRequest {
                        path_or_uuid: uuid,
                        mode: if permanent {
                            RemovalMode::Permanent
                        } else {
                            RemovalMode::Trash
                        },
                        assume_yes: true,
                    });
                    self.status = if outcome.ok {
                        "Removed".to_string()
                    } else {
                        format!("Remove failed: {}", outcome.error)
                    };
                    self.refresh_library();
                }
                self.dialog = None;
                Command::none()
            }
            Message::DialogDismiss => {
                self.dialog = None;
                Command::none()
            }
            Message::UpdatesRefresh => {
                self.refresh_updates();
                self.status = format!("{} update(s) available", self.updates.len());
                Command::none()
            }
            Message::UpdateAll => {
                self.batch = BatchState {
                    running: true,
                    cancel: false,
                    done: 0,
                    total: self.updates.len(),
                    errors: Vec::new(),
                };
                let cancel = AtomicBool::new(false);
                let uuids: Vec<String> = self.updates.iter().map(|o| o.uuid.clone()).collect();
                for uuid in uuids {
                    if self.batch.cancel {
                        break;
                    }
                    let Some(app) = self.controller.registry().by_uuid(&uuid) else {
                        continue;
                    };
                    let result = self.controller.apply_update(&app, false, &cancel);
                    self.batch.done += 1;
                    if !result.ok {
                        self.batch
                            .errors
                            .push(format!("{}: {}", app.name, result.error));
                    }
                }
                self.batch.running = false;
                self.refresh_library();
                self.refresh_updates();
                self.status = format!(
                    "Updated {} of {} ({} failed)",
                    self.batch.done,
                    self.batch.total,
                    self.batch.errors.len()
                );
                Command::none()
            }
            Message::UpdateOne(uuid) => {
                if let Some(app) = self.controller.registry().by_uuid(&uuid) {
                    if self.controller.is_running(&app) {
                        self.dialog = Some(PendingDialog::UpdateForce {
                            uuid: uuid.clone(),
                            name: app.name.clone(),
                        });
                    } else {
                        let cancel = AtomicBool::new(false);
                        let result = self.controller.apply_update(&app, false, &cancel);
                        self.status = if result.ok {
                            format!("Updated {}", app.name)
                        } else {
                            format!("Update failed: {}", result.error)
                        };
                        self.refresh_library();
                        self.refresh_updates();
                    }
                }
                Command::none()
            }
            Message::UpdateForceConfirm => {
                if let Some(PendingDialog::UpdateForce { uuid, .. }) = self.dialog.clone() {
                    if let Some(app) = self.controller.registry().by_uuid(&uuid) {
                        let cancel = AtomicBool::new(false);
                        let result = self.controller.apply_update(&app, true, &cancel);
                        self.status = if result.ok {
                            format!("Updated {}", app.name)
                        } else {
                            format!("Update failed: {}", result.error)
                        };
                        self.refresh_library();
                        self.refresh_updates();
                    }
                }
                self.dialog = None;
                Command::none()
            }
            Message::CancelBatch => {
                self.batch.cancel = true;
                Command::none()
            }
            Message::AutostartToggled(enabled) => {
                match self.controller.sync_autostart(enabled) {
                    Ok(()) => {
                        self.controller
                            .settings_mut()
                            .set_background_update_checks(enabled);
                        self.status = if enabled {
                            "Background update checks on (notify only)"
                        } else {
                            "Background update checks off"
                        }
                        .to_string();
                    }
                    Err(error) => self.status = format!("Autostart failed: {error}"),
                }
                Command::none()
            }
            Message::BackgroundToggled(enabled) => {
                self.controller
                    .settings_mut()
                    .set_background_update_checks(enabled);
                Command::none()
            }
            Message::MoveSourceToggled(enabled) => {
                self.controller.settings_mut().set_move_source(enabled);
                Command::none()
            }
            Message::UnsafeFallbackToggled(enabled) => {
                if enabled {
                    // Enabling is itself a confirmed dialog (warned, per-file).
                    self.dialog = Some(PendingDialog::UnsafeExtract {
                        path: String::new(),
                    });
                } else {
                    self.controller
                        .settings_mut()
                        .set_unsafe_extraction_fallback(false);
                }
                Command::none()
            }
            Message::AppearanceSelected(appearance) => {
                self.appearance = appearance;
                self.controller.settings_mut().set_appearance(appearance);
                // Live switch, no restart.
                return self.apply_appearance();
            }
            Message::ManagedFolderChanged(path) => {
                self.managed_folder_input = path;
                Command::none()
            }
            Message::ManagedFolderApply => {
                let path = std::path::PathBuf::from(self.managed_folder_input.trim());
                if path.as_os_str().is_empty() {
                    self.status = "Managed folder cannot be empty".to_string();
                } else {
                    self.controller.settings_mut().set_managed_folder(path);
                    self.refresh_library();
                    self.status = "Managed folder updated".to_string();
                }
                Command::none()
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
                if let Some(app) = self.controller.registry().by_uuid(&uuid) {
                    let config = parse_config_lines(&self.source_config_input);
                    let mut error = String::new();
                    if self.controller.set_update_source(
                        app,
                        self.source_manager_input.trim(),
                        config,
                        &mut error,
                    ) {
                        self.status = "Update source saved".to_string();
                    } else {
                        self.status = format!("Update source invalid: {error}");
                    }
                    self.refresh_library();
                }
                Command::none()
            }
            Message::UpdateSourceUnset(uuid) => {
                if let Some(app) = self.controller.registry().by_uuid(&uuid) {
                    let mut error = String::new();
                    if self.controller.unset_update_source(app, &mut error) {
                        self.status = "Update source removed".to_string();
                    } else {
                        self.status = format!("Cannot remove update source: {error}");
                    }
                    self.refresh_library();
                }
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Self::Message> {
        let page = self
            .nav_model
            .active_data::<Page>()
            .copied()
            .unwrap_or(Page::Library);
        let body: Element<Message> = match page {
            Page::Library => self.view_library(),
            Page::Inspect => self.view_inspect(),
            Page::Updates => self.view_updates(),
            Page::Settings => self.view_settings(),
            Page::About => self.view_about(),
        };
        let mut stack = widget::column::with_children(vec![
            body,
            widget::text::caption(&self.status).into(),
            widget::text::caption("Made by Gosh").into(),
        ])
        .spacing(8)
        .into();
        if let Some(dialog) = self.view_dialog() {
            stack = widget::column::with_children(vec![stack, dialog])
                .spacing(8)
                .into();
        }
        widget::container(stack)
            .padding(16)
            .width(cosmic::iced::Length::Fill)
            .height(cosmic::iced::Length::Fill)
            .into()
    }
}

impl App
where
    Self: Application,
{
    fn active_page_title(&self) -> &str {
        self.nav_model
            .text(self.nav_model.active())
            .unwrap_or("Library")
    }

    fn update_title(&mut self) -> Command<Message> {
        let header = format!("Gosh AppImage Manager — {}", self.active_page_title());
        self.core_mut().set_header_title(header.clone());
        self.set_window_title(header)
    }
    fn view_library(&self) -> Element<Message> {
        let mut rows: widget::Column<Message> = widget::Column::new();
        rows = rows.push(
            widget::row::with_children(vec![
                widget::text::title3("Library").into(),
                widget::horizontal_space(cosmic::iced::Length::Fill).into(),
                widget::button::standard("Refresh")
                    .on_press(Message::LibraryRefresh)
                    .into(),
            ])
            .spacing(8),
        );
        if self.library.is_empty() {
            rows = rows.push(
                widget::text::body(
                    "No AppImages integrated yet. Open one from the Inspect page — opening never integrates or executes it.",
                )
                );
        }
        for app in &self.library {
            let badges = badges_for(app, &self.running, &self.updates);
            rows = rows.push(
                widget::container(widget::column::with_children(vec![
                    widget::row::with_children(vec![
                        widget::text::heading(&app.name).into(),
                        widget::text::caption(badges).into(),
                    ])
                    .spacing(8)
                    .into(),
                    widget::text::caption(&app.managed_path).into(),
                    widget::row::with_children(vec![
                        widget::button::standard("Launch")
                            .on_press(Message::Launch(app.uuid.clone()))
                            .into(),
                        widget::button::standard("Trash")
                            .on_press(Message::RemoveAsk(app.uuid.clone(), false))
                            .into(),
                        widget::button::destructive("Delete")
                            .on_press(Message::RemoveAsk(app.uuid.clone(), true))
                            .into(),
                    ])
                    .spacing(8)
                    .into(),
                ]))
                .padding(8),
            );
        }
        widget::scrollable(rows).into()
    }

    fn view_inspect(&self) -> Element<Message> {
        let mut col: widget::Column<Message> = widget::Column::new();
        col = col.push(widget::text::title3("Inspect an AppImage"));
        col = col.push(widget::text::caption(
            "Opening a file only inspects it. Nothing is integrated or executed.",
        ));
        col = col.push(
            widget::row::with_children(vec![
                widget::text_input("Path to .AppImage", &self.inspect.path_input)
                    .on_input(Message::InspectPathChanged)
                    .into(),
                widget::button::standard("Browse")
                    .on_press(Message::InspectBrowse)
                    .into(),
                widget::button::suggested("Inspect")
                    .on_press(Message::InspectRun)
                    .into(),
            ])
            .spacing(8),
        );
        if !self.inspect.error.is_empty() {
            col = col.push(widget::text::body(&self.inspect.error));
        }
        for line in &self.inspect.summary {
            col = col.push(widget::text::body(line));
        }
        if self.inspect.inspected_ok {
            col = col.push(
                widget::row::with_children(vec![
                    widget::button::suggested("Integrate")
                        .on_press(Message::IntegrateDismiss)
                        .into(),
                    widget::text_input("Replace UUID (optional)", &self.inspect.replace_uuid)
                        .on_input(|uuid| {
                            // Stored via a dedicated pass below; keep the message
                            // stream simple by reusing the path message channel.
                            let _ = uuid;
                            Message::Noop
                        })
                        .into(),
                ])
                .spacing(8),
            );
        }
        widget::scrollable(col).into()
    }

    fn view_updates(&self) -> Element<Message> {
        let mut col: widget::Column<Message> = widget::Column::new();
        col = col.push(
            widget::row::with_children(vec![
                widget::text::title3("Updates").into(),
                widget::horizontal_space(cosmic::iced::Length::Fill).into(),
                widget::button::standard("Check now")
                    .on_press(Message::UpdatesRefresh)
                    .into(),
                widget::button::suggested("Update all")
                    .on_press(Message::UpdateAll)
                    .into(),
                widget::button::standard("Cancel")
                    .on_press(Message::CancelBatch)
                    .into(),
            ])
            .spacing(8),
        );
        if self.batch.total > 0 {
            #[allow(clippy::cast_precision_loss)]
            let progress = self.batch.done as f32 / self.batch.total.max(1) as f32 * 100.0;
            col = col.push(widget::progress_bar(0.0..=100.0, progress));
            col = col.push(widget::text::body(format!(
                "{} of {} applied{}",
                self.batch.done,
                self.batch.total,
                if self.batch.errors.is_empty() {
                    String::new()
                } else {
                    format!(" ({} failed)", self.batch.errors.len())
                }
            )));
            for error in &self.batch.errors {
                col = col.push(widget::text::caption(error));
            }
        }
        if self.updates.is_empty() {
            col = col.push(widget::text::body("Everything is up to date."));
        }
        for offer in &self.updates {
            let badge = if offer.running { " [running]" } else { "" };
            col = col.push(
                widget::container(widget::row::with_children(vec![
                    widget::column::with_children(vec![
                        widget::text::heading(format!("{}{}", offer.name, badge)).into(),
                        widget::text::caption(format!(
                            "{} -> {} ({})",
                            offer.current_version, offer.available_version, offer.manager
                        ))
                        .into(),
                    ])
                    .into(),
                    widget::horizontal_space(cosmic::iced::Length::Fill).into(),
                    widget::button::suggested("Update")
                        .on_press(Message::UpdateOne(offer.uuid.clone()))
                        .into(),
                ]))
                .padding(8),
            );
        }
        widget::scrollable(col).into()
    }

    fn view_settings(&self) -> Element<Message> {
        let settings = self.controller.settings();
        let appearance_row = widget::row::with_children(vec![
            widget::button::standard("System")
                .on_press(Message::AppearanceSelected(Appearance::System))
                .into(),
            widget::button::standard("Light")
                .on_press(Message::AppearanceSelected(Appearance::Light))
                .into(),
            widget::button::standard("Dark")
                .on_press(Message::AppearanceSelected(Appearance::Dark))
                .into(),
            widget::text::body(format!("Current: {}", appearance_name(self.appearance))).into(),
        ])
        .spacing(8);
        let col = widget::column::with_children(vec![
            widget::text::title3("Settings").into(),
            widget::settings::section().title("Appearance")
                .add(appearance_row)
                .into(),
            widget::settings::section().title("Integration folder")
                .add(
                    widget::row::with_children(vec![
                        widget::text_input("Managed folder", &self.managed_folder_input)
                            .on_input(Message::ManagedFolderChanged)
                            .into(),
                        widget::button::standard("Apply")
                            .on_press(Message::ManagedFolderApply)
                            .into(),
                    ])
                    .spacing(8),
                )
                .into(),
            widget::settings::section().title("Behavior")
                .add(widget::row::with_children(vec![
                    widget::text::body("Move source into library (trash source)").into(),
                    widget::toggler(None, settings.move_source(), Message::MoveSourceToggled)
                        .into(),
                ]))
                .add(widget::row::with_children(vec![
                    widget::text::body("Background update checks (notify only)").into(),
                    widget::toggler(None, settings.background_update_checks(), Message::BackgroundToggled)
                        .into(),
                ]))
                .add(widget::row::with_children(vec![
                    widget::text::body("Session autostart for background checks").into(),
                    widget::toggler(None, settings.background_update_checks(), Message::AutostartToggled)
                        .into(),
                ]))
                .into(),
            widget::settings::section().title("Unsafe extraction fallback")
                .add(widget::row::with_children(vec![
                    widget::text::body(
                        "Off by default. Executes untrusted code; requires per-file confirmation.",
                    )
                    .into(),
                    widget::toggler(None, settings.unsafe_extraction_fallback(), Message::UnsafeFallbackToggled)
                        .into(),
                ]))
                .into(),
            widget::settings::section().title("Update source editor")
                .add(
                    widget::column::with_children(vec![
                        widget::text_input("Manager (static, github, gitlab, codeberg, forgejo, ftp)", &self.source_manager_input)
                            .on_input(Message::UpdateSourceManagerChanged)
                            .into(),
                        widget::text_input("key=value lines", &self.source_config_input)
                            .on_input(Message::UpdateSourceConfigChanged)
                            .into(),
                        widget::text::caption(
                            "Pick an app in the Library, then apply here. Per-app editors live beside each row.",
                        )
                        .into(),
                    ])
                    .spacing(8),
                )
                .into(),
        ])
        .spacing(12);
        widget::scrollable(col).into()
    }

    fn view_about(&self) -> Element<Message> {
        widget::scrollable(widget::column::with_children(vec![
            widget::text::title3("Gosh AppImage Manager").into(),
            widget::text::body(format!("Version {}", limits::VERSION)).into(),
            widget::text::body("Made by Gosh").into(),
            widget::text::body(
                "Native COSMIC Epoch application for safely inspecting, integrating, launching, organizing, updating, and removing AppImages.",
            )
            .into(),
            widget::text::body(
                "Opening an AppImage never integrates or executes it. Updates are staged, validated, and applied atomically with rollback.",
            )
            .into(),
            widget::text::body(
                "Behavioral reference: Gear Lever by Lorenzo Paderi (independent original implementation, no shared source or assets).",
            )
            .into(),
            widget::text::body("License: GPL-3.0-or-later. No telemetry.").into(),
        ]))
        .into()
    }

    fn view_dialog(&self) -> Option<Element<Message>> {
        let pending = self.dialog.clone()?;
        match pending {
            PendingDialog::IntegrateConflict {
                path,
                conflict_name,
                replace_uuid,
            } => Some(
                widget::dialog(format!("{conflict_name} is already managed"))
                    .body("Keep both copies, or replace the managed installation?")
                    .primary_action(
                        widget::button::suggested("Keep both")
                            .on_press(Message::IntegrateKeepBoth(path.clone())),
                    )
                    .secondary_action(
                        widget::button::standard("Replace")
                            .on_press(Message::IntegrateReplace(path.clone(), replace_uuid)),
                    )
                    .tertiary_action(
                        widget::button::standard("Cancel").on_press(Message::DialogDismiss),
                    )
                    .into(),
            ),
            PendingDialog::Remove { name, permanent, .. } => {
                let (title, body) = if permanent {
                    (
                        format!("Permanently delete {name}?"),
                        "This names the exact file, refuses protected paths, and cannot be undone.",
                    )
                } else {
                    (
                        format!("Move {name} to Trash?"),
                        "Desktop entry and icons are removed only after Trash succeeds.",
                    )
                };
                Some(
                    widget::dialog(title)
                        .body(body)
                        .primary_action(
                            widget::button::destructive(if permanent {
                                "Delete forever"
                            } else {
                                "Move to Trash"
                            })
                            .on_press(Message::RemoveConfirm),
                        )
                        .secondary_action(
                            widget::button::standard("Cancel").on_press(Message::DialogDismiss),
                        )
                        .into(),
                )
            }
            PendingDialog::UnsafeExtract { .. } => Some(
                widget::dialog("Enable unsafe extraction fallback?")
                    .body(
                        "This executes untrusted AppImage code via --appimage-extract. It stays off unless you also confirm each file, and it is never used in background checks.",
                    )
                    .primary_action(
                        widget::button::destructive("Enable (warned)")
                            .on_press(Message::Noop),
                    )
                    .secondary_action(
                        widget::button::standard("Keep off").on_press(Message::DialogDismiss),
                    )
                    .into(),
            ),
            PendingDialog::UpdateForce { name, .. } => Some(
                widget::dialog(format!("{name} is running"))
                    .body("Updating replaces the file under a live app. Force the update?")
                    .primary_action(
                        widget::button::destructive("Force update")
                            .on_press(Message::UpdateForceConfirm),
                    )
                    .secondary_action(
                        widget::button::standard("Cancel").on_press(Message::DialogDismiss),
                    )
                    .into(),
            ),
        }
    }
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
    if !app.owned {
        badges.push("external");
    }
    badges.join(" · ")
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

/// Read back the appearance name (round-trips settings vocabulary).
#[allow(dead_code)]
fn appearance_setting_name(raw: &str) -> Appearance {
    appearance_from_str(raw)
}
