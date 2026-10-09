//! Plain data transfer objects that cross the boundary. Each one is built from
//! a core type; none holds a core object. The front end only reads these.

use goshaim_core::library::{DiscoveredApp, Origin};
use goshaim_core::types::{
    app_image_type_name, architecture_name, InspectionResult, InstalledApp, IntegrateResult,
    RemovalResult, TaskItem, TaskKind, TaskState, UpdateOffer,
};

use crate::api::common::is_timeout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValueDto {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvVarDto {
    pub name: String,
    pub value: String,
}

/// One AppImage the manager tracks, as shown on the Library and Details pages.
#[derive(Debug, Clone)]
pub struct AppDto {
    pub uuid: String,
    pub name: String,
    pub version: String,
    pub comment: String,
    pub managed_path: String,
    pub desktop_id: String,
    pub desktop_path: String,
    pub icon_path: String,
    pub sha256: String,
    pub app_type: String,
    pub architecture: String,
    pub size_bytes: i64,
    pub arguments: Vec<String>,
    pub environment: Vec<EnvVarDto>,
    pub update_manager: String,
    pub update_config: Vec<KeyValueDto>,
    pub embedded_update: String,
    pub last_update_check: String,
    pub available_version: String,
    pub available_url: String,
    pub available_size: i64,
    pub update_available: bool,
    pub digest: String,
    pub reduced_verification: bool,
    pub running: bool,
    pub external_folder: bool,
    pub owned: bool,
    pub adopted: bool,
    pub website: String,
    pub terminal: bool,
    pub categories: Vec<String>,
    pub mime_types: Vec<String>,
    pub startup_wm_class: String,
    pub action_names: Vec<String>,
    /// Unix seconds of the first integration or adoption. Zero when the core
    /// never recorded it (an entry from before the column existed).
    pub integrated_at: i64,
    /// The folder the AppImage was integrated from. Empty when the core never
    /// recorded it, as for an adoption or an entry from before the column.
    pub integrated_folder: String,
}

impl AppDto {
    pub(crate) fn from_core(app: &InstalledApp, running: bool) -> Self {
        Self {
            uuid: app.uuid.clone(),
            name: app.name.clone(),
            version: app.version.clone(),
            comment: app.comment.clone(),
            managed_path: app.managed_path.clone(),
            desktop_id: app.desktop_id.clone(),
            desktop_path: app.desktop_path.clone(),
            icon_path: app.icon_path.clone(),
            sha256: hex_string(&app.sha256),
            app_type: app_image_type_name(app.app_type).to_string(),
            architecture: architecture_name(app.architecture).to_string(),
            size_bytes: app.size,
            arguments: app.arguments.clone(),
            environment: app
                .environment
                .iter()
                .map(|pair| EnvVarDto {
                    name: pair.name.clone(),
                    value: pair.value.clone(),
                })
                .collect(),
            update_manager: app.update_manager.clone(),
            update_config: app
                .update_config
                .iter()
                .map(|(key, value)| KeyValueDto {
                    key: key.clone(),
                    value: value.clone(),
                })
                .collect(),
            embedded_update: app.embedded_update.clone(),
            last_update_check: app.last_update_check.clone(),
            available_version: app.available_version.clone(),
            available_url: app.available_url.clone(),
            available_size: app.available_size,
            update_available: app.update_available,
            digest: app.digest.clone(),
            reduced_verification: app.reduced_verification,
            running,
            external_folder: app.external_folder,
            owned: app.owned,
            adopted: app.adopted,
            website: app.website.clone(),
            terminal: app.terminal,
            categories: app.categories.clone(),
            mime_types: app.mime_types.clone(),
            startup_wm_class: app.startup_wm_class.clone(),
            action_names: app
                .actions
                .iter()
                .map(|action| action.name.clone())
                .collect(),
            integrated_at: app.integrated_at,
            integrated_folder: app.integrated_folder.clone(),
        }
    }
}

/// Facts about one file, as the Inspect page shows them. Nothing is installed
/// or executed to produce it.
#[derive(Debug, Clone)]
pub struct InspectDto {
    pub path: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub app_type: String,
    pub architecture: String,
    pub magic_valid: bool,
    pub architecture_supported: bool,
    pub truncated: bool,
    pub name: String,
    pub version: String,
    pub comment: String,
    pub icon_name: String,
    pub icon_format: String,
    pub icon_bytes: Option<Vec<u8>>,
    pub categories: Vec<String>,
    pub mime_types: Vec<String>,
    pub terminal: bool,
    pub website: String,
    pub startup_wm_class: String,
    pub action_names: Vec<String>,
    pub embedded_update: String,
    pub embedded_manager_hint: String,
    pub warnings: Vec<String>,
    pub error: String,
    pub already_managed: bool,
    pub existing_uuid: String,
    pub conflict_status: String,
    pub conflicting_uuid: String,
    pub conflicting_name: String,
    pub needs_conflict_decision: bool,
    pub can_replace: bool,
    pub planned_target: String,
    pub extractor_used: String,
    pub used_unsafe_fallback: bool,
    /// The fallback is needed for this file and was not confirmed. Nothing was run.
    pub fallback_pending: bool,
}

impl InspectDto {
    pub(crate) fn from_core(result: &InspectionResult, icon_bytes: Option<Vec<u8>>) -> Self {
        Self {
            path: result.identity.path.clone(),
            size_bytes: result.identity.size,
            sha256: hex_string(&result.identity.sha256),
            app_type: app_image_type_name(result.app_type).to_string(),
            architecture: architecture_name(result.architecture).to_string(),
            magic_valid: result.magic_valid,
            architecture_supported: result.architecture_supported,
            truncated: result.truncated,
            name: result.metadata.name.clone(),
            version: result.metadata.version.clone(),
            comment: result.metadata.comment.clone(),
            icon_name: result.metadata.icon_name.clone(),
            icon_format: result.metadata.icon_format.clone(),
            icon_bytes,
            categories: result.metadata.categories.clone(),
            mime_types: result.metadata.mime_types.clone(),
            terminal: result.metadata.terminal,
            website: result.metadata.website.clone(),
            startup_wm_class: result.metadata.startup_wm_class.clone(),
            action_names: result
                .metadata
                .actions
                .iter()
                .map(|action| action.name.clone())
                .collect(),
            embedded_update: result.update_info.raw.clone(),
            embedded_manager_hint: result.update_info.manager_hint.clone(),
            warnings: result.warnings.clone(),
            error: result.error.clone(),
            already_managed: result.already_managed,
            existing_uuid: result.existing_managed_id.clone(),
            conflict_status: result.conflict_status.clone(),
            conflicting_uuid: result.conflicting_uuid.clone(),
            conflicting_name: result.conflicting_name.clone(),
            needs_conflict_decision: result.needs_conflict_decision,
            can_replace: result.can_replace,
            planned_target: result.planned_target.clone(),
            extractor_used: result.extractor_used.clone(),
            used_unsafe_fallback: result.extraction_used_unsafe_fallback,
            fallback_pending: result.fallback_pending,
        }
    }
}

/// The result of an install, update or removal. Failures that the user can act
/// on (a name conflict, a running app) are reported here, not as errors.
#[derive(Debug, Clone)]
pub struct OutcomeDto {
    pub ok: bool,
    pub partial: bool,
    pub message: String,
    pub conflict: bool,
    pub running: bool,
    pub app: Option<AppDto>,
    pub rolled_back: Vec<String>,
    pub source_removed: bool,
    /// The installation a Replace would target. Set only on a name conflict,
    /// and empty when no single installation is implicated.
    pub conflict_uuid: String,
    pub conflict_name: String,
    /// The file needs the unsafe fallback, not yet confirmed. Nothing was installed.
    pub fallback_pending: bool,
}

impl OutcomeDto {
    pub(crate) fn from_integrate(result: &IntegrateResult) -> Self {
        let mut outcome = Self::failure_fields(result.ok, result.partial, &result.error);
        outcome.app = if result.ok {
            Some(AppDto::from_core(&result.app, false))
        } else {
            None
        };
        outcome.rolled_back = result.rolled_back.clone();
        outcome.source_removed = result.source_removed;
        outcome.fallback_pending = result.fallback_pending;
        outcome
    }

    pub(crate) fn from_removal(result: &RemovalResult) -> Self {
        Self::failure_fields(result.ok, result.partial, &result.error)
    }

    fn failure_fields(ok: bool, partial: bool, error: &str) -> Self {
        let lower = error.to_lowercase();
        Self {
            ok,
            partial,
            message: error.to_string(),
            conflict: !ok && (lower.contains("keep-both") || lower.contains("replace")),
            running: !ok && lower.contains("running"),
            app: None,
            rolled_back: Vec::new(),
            source_removed: false,
            conflict_uuid: String::new(),
            conflict_name: String::new(),
            fallback_pending: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredDto {
    pub path: String,
    pub name: String,
    pub managed: bool,
    pub uuid: String,
    pub external_desktop_entry: bool,
    pub desktop_path: String,
}

impl DiscoveredDto {
    pub(crate) fn from_core(app: &DiscoveredApp) -> Self {
        Self {
            path: app.path.clone(),
            name: app.name.clone(),
            managed: app.managed,
            uuid: app.uuid.clone(),
            external_desktop_entry: matches!(app.origin, Origin::ExternalDesktopEntry),
            desktop_path: app.desktop_path.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateOfferDto {
    pub uuid: String,
    pub name: String,
    pub current_version: String,
    pub available_version: String,
    pub manager: String,
    pub url: String,
    pub download_size: i64,
    pub digest: String,
    pub reduced_verification: bool,
    pub digest_algo: String,
    pub embedded_source: String,
    pub running: bool,
}

impl UpdateOfferDto {
    pub(crate) fn from_core(offer: &UpdateOffer) -> Self {
        Self {
            uuid: offer.uuid.clone(),
            name: offer.name.clone(),
            current_version: offer.current_version.clone(),
            available_version: offer.available_version.clone(),
            manager: offer.manager.clone(),
            url: offer.url.clone(),
            download_size: offer.download_size,
            digest: offer.digest.clone(),
            reduced_verification: offer.reduced_verification,
            digest_algo: offer.digest_algo.clone(),
            embedded_source: offer.embedded_source.clone(),
            running: offer.running,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateFailureDto {
    pub uuid: String,
    pub name: String,
    pub manager: String,
    pub error: String,
    /// The source did not answer in time. The status is unknown, not "up to
    /// date", and the Updates page says so.
    pub timed_out: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateScanDto {
    pub offers: Vec<UpdateOfferDto>,
    pub failures: Vec<UpdateFailureDto>,
    pub skipped: i64,
    pub checked: i64,
    pub cancelled: bool,
}

/// The result of "Check for update" on one app. It reports what the source
/// offers. It never downloads or applies anything.
#[derive(Debug, Clone)]
pub struct UpdateCheckDto {
    pub uuid: String,
    pub current_version: String,
    /// The newer version the source offers, or empty when there is none.
    pub available_version: String,
    pub download_size: i64,
    pub reduced_verification: bool,
    /// Why the check did not complete. Empty when it did.
    pub error: String,
    pub timed_out: bool,
}

impl UpdateCheckDto {
    pub(crate) fn from_core(
        app: &goshaim_core::types::InstalledApp,
        checked: &goshaim_core::updates_sources::UpdateCheckResult,
    ) -> Self {
        let offered = goshaim_core::updates_service::offers_update(app, checked);
        let error = if checked.ok {
            String::new()
        } else if checked.error.is_empty() {
            "Update check failed".to_string()
        } else {
            checked.error.clone()
        };
        Self {
            uuid: app.uuid.clone(),
            current_version: app.version.clone(),
            available_version: if offered {
                checked.version.clone()
            } else {
                String::new()
            },
            download_size: if offered { checked.size } else { 0 },
            reduced_verification: offered && checked.reduced_verification,
            timed_out: !checked.ok && is_timeout(&error),
            error,
        }
    }
}

/// The outcome of "update all": applied, failed and skipped apps by name.
#[derive(Debug, Clone)]
pub struct BatchDto {
    pub applied: Vec<String>,
    pub failed: Vec<UpdateFailureDto>,
    pub skipped_running: Vec<String>,
    pub check_failures: Vec<UpdateFailureDto>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKindDto {
    Inspect,
    Integrate,
    Update,
    Remove,
    RefreshMetadata,
    CheckUpdate,
    Adopt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStateDto {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct TaskDto {
    pub id: String,
    pub kind: TaskKindDto,
    pub state: TaskStateDto,
    pub title: String,
    pub target: String,
    pub progress: i32,
    pub status_text: String,
    pub error: String,
    pub retryable: bool,
    /// Unix seconds when the task began, and when it ended (0 while running).
    pub started_at: i64,
    pub finished_at: i64,
    /// The versions the task moves between. An integration sets only
    /// `to_version`; a removal sets only `from_version`. Empty when unknown.
    pub from_version: String,
    pub to_version: String,
    /// The update stage (1 Download, 2 Verify, 3 Swap in), or 0 when none.
    pub phase_index: i32,
    pub phase: String,
    /// Bytes moved in the current stage, and the total (0 when unknown).
    pub bytes_done: i64,
    pub bytes_total: i64,
    /// True for a removal that deleted the AppImage permanently. False for a
    /// Trash removal and for every other task.
    pub permanent: bool,
}

impl TaskDto {
    pub(crate) fn from_core(item: &TaskItem) -> Self {
        Self {
            id: item.id.clone(),
            kind: match item.kind {
                TaskKind::Inspect => TaskKindDto::Inspect,
                TaskKind::Integrate => TaskKindDto::Integrate,
                TaskKind::Update => TaskKindDto::Update,
                TaskKind::Remove => TaskKindDto::Remove,
                TaskKind::RefreshMetadata => TaskKindDto::RefreshMetadata,
                TaskKind::CheckUpdate => TaskKindDto::CheckUpdate,
                TaskKind::Adopt => TaskKindDto::Adopt,
            },
            state: match item.state {
                TaskState::Queued => TaskStateDto::Queued,
                TaskState::Running => TaskStateDto::Running,
                TaskState::Cancelling => TaskStateDto::Cancelling,
                TaskState::Succeeded => TaskStateDto::Succeeded,
                TaskState::Failed => TaskStateDto::Failed,
                TaskState::Cancelled => TaskStateDto::Cancelled,
            },
            title: item.title.clone(),
            target: item.target.clone(),
            progress: item.progress,
            status_text: item.status_text.clone(),
            error: item.error.clone(),
            retryable: item.retryable,
            started_at: item.started_at,
            finished_at: item.finished_at,
            from_version: item.from_version.clone(),
            to_version: item.to_version.clone(),
            phase_index: item.phase_index,
            phase: item.phase.clone(),
            bytes_done: item.bytes_done as i64,
            bytes_total: item.bytes_total as i64,
            permanent: item.permanent,
        }
    }
}

/// Lowercase hex, the form the Details and Inspect pages display.
pub(crate) fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod pending_tests {
    use super::*;
    use goshaim_core::types::{InspectionResult, IntegrateResult};

    #[test]
    fn the_pending_flag_reaches_the_inspection_dto() {
        let result = InspectionResult {
            fallback_pending: true,
            ..Default::default()
        };
        assert!(InspectDto::from_core(&result, None).fallback_pending);
    }

    #[test]
    fn the_pending_flag_reaches_the_integration_outcome() {
        let result = IntegrateResult {
            fallback_pending: true,
            ..Default::default()
        };
        assert!(OutcomeDto::from_integrate(&result).fallback_pending);
    }
}
