// Gosh AppImage Manager — value types (ports src/core/Types.h).
// Made by Gosh. GPL-3.0-or-later.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppImageType {
    #[default]
    Unknown,
    Type1,
    Type2,
    Dwarfs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Architecture {
    #[default]
    Unknown,
    X86_64,
    AArch64,
    I386,
    Arm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConflictPolicy {
    #[default]
    Unspecified,
    KeepBoth,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemovalMode {
    Trash,
    Permanent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    Inspect,
    Integrate,
    Update,
    Remove,
    RefreshMetadata,
    CheckUpdate,
    Adopt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CopyMode {
    #[default]
    Copy,
    Move,
}

/// Mirrors C++ ExitCode integers exactly (CLI contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Ok = 0,
    Failure = 1,
    Usage = 2,
    NotFound = 3,
    NotIntegrated = 4,
    NeedsConfirmation = 5,
    Validation = 6,
    Running = 7,
    Network = 8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvPair {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopAction {
    pub id: String,
    pub name: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrateFailPoint {
    None,
    AfterStage,
    DesktopWrite,
    DesktopInstall,
    RegistrySave,
    SourceDelete,
    BackupCreate,
    BeforeCommit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateFailPoint {
    None,
    AfterDownload,
    AfterReplace,
    DesktopInstall,
    RegistrySave,
    BackupCreate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArchiveEntryKind {
    #[default]
    File,
    Directory,
    Symlink,
    Device,
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct ArchiveEntry {
    pub path: String,
    pub kind: ArchiveEntryKind,
    pub link_target: String,
    pub size: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileIdentity {
    pub path: String,
    pub size: i64,
    #[serde(with = "hex_bytes", default)]
    pub sha256: Vec<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppImageMetadata {
    pub name: String,
    pub version: String,
    pub comment: String,
    pub icon_name: String,
    /// Filesystem path to a staged copy of the icon, in a private temp
    /// directory the caller owns and removes. Empty when no icon was found.
    pub extracted_icon_path: String,
    /// Extension for the staged icon ("png", "svg", "xpm").
    pub icon_format: String,
    pub terminal: bool,
    pub categories: Vec<String>,
    pub mime_types: Vec<String>,
    pub exec_raw: String,
    pub exec_arguments: Vec<String>,
    pub try_exec: String,
    pub website: String,
    pub startup_wm_class: String,
    pub actions: Vec<DesktopAction>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbeddedUpdateInfo {
    pub raw: String,
    pub manager_hint: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct InspectionResult {
    pub identity: FileIdentity,
    pub app_type: AppImageType,
    pub architecture: Architecture,
    pub magic_valid: bool,
    pub architecture_supported: bool,
    pub truncated: bool,
    pub metadata: AppImageMetadata,
    pub update_info: EmbeddedUpdateInfo,
    pub warnings: Vec<String>,
    pub error: String,
    pub already_managed: bool,
    pub existing_managed_id: String,
    pub extraction_attempted: bool,
    pub extraction_used_unsafe_fallback: bool,
    pub payload_offset: i64,
    pub extractor_used: String,
    pub planned_target: String,
    pub copy_outcome: String,
    pub conflict_status: String,
    pub conflicting_uuid: String,
    pub conflicting_path: String,
    pub conflicting_name: String,
    pub needs_conflict_decision: bool,
    pub can_replace: bool,
    pub chosen_policy: ConflictPolicy,
    pub chosen_replace_uuid: String,
    /// Private directory holding the staged icon, when one was extracted.
    /// The caller owns it; `discard_staging` removes it.
    pub icon_staging_dir: String,
    /// The fallback is needed but not confirmed for this file. Nothing was run.
    pub fallback_pending: bool,
    /// The fallback executed the AppImage, whether or not metadata came out of it.
    pub unsafe_fallback_ran: bool,
}

impl InspectionResult {
    /// Remove the icon staging directory this inspection created.
    ///
    /// Callers that install the icon should do this once they have copied it.
    /// Callers that merely inspected should do it when they discard the
    /// result. Inspection also sweeps stale staging directories, so forgetting
    /// leaks at most one directory until the next inspection.
    pub fn discard_staging(&self) {
        if self.icon_staging_dir.is_empty() {
            return;
        }
        let _ = crate::safe_fs::remove_dir_no_follow(std::path::Path::new(&self.icon_staging_dir));
    }
}

#[derive(Debug, Clone)]
pub struct InspectOptions {
    pub extract_metadata: bool,
    pub compute_hash: bool,
    /// Whether the unsafe fallback may run. `gated_options` sets it from the stored
    /// setting for every inspection; a caller's value is overwritten, never used.
    pub allow_unsafe_extract: bool,
    /// The user confirmed the unsafe fallback for this file. Callers set it.
    pub confirm_unsafe: bool,
    /// The stored setting, as the gate read it. The gate sets it.
    pub unsafe_setting_on: bool,
    pub max_bytes: i64,
}

impl Default for InspectOptions {
    fn default() -> Self {
        Self {
            extract_metadata: true,
            compute_hash: true,
            allow_unsafe_extract: false,
            confirm_unsafe: false,
            unsafe_setting_on: false,
            max_bytes: crate::limits::DEFAULT_MAX_APPIMAGE_BYTES,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstalledApp {
    pub uuid: String,
    pub name: String,
    pub version: String,
    pub comment: String,
    pub managed_path: String,
    pub desktop_id: String,
    pub desktop_path: String,
    pub icon_path: String,
    #[serde(with = "hex_bytes", default)]
    pub sha256: Vec<u8>,
    pub app_type: AppImageType,
    pub architecture: Architecture,
    pub size: i64,
    pub arguments: Vec<String>,
    pub default_arguments: Vec<String>,
    pub environment: Vec<EnvPair>,
    pub update_manager: String,
    pub update_config: BTreeMap<String, String>,
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
    pub actions: Vec<DesktopAction>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub mime_types: Vec<String>,
    #[serde(default)]
    pub startup_wm_class: String,
    /// Unix seconds when the app was first integrated or adopted. Zero for a
    /// row written before the column existed; it is never invented.
    #[serde(default)]
    pub integrated_at: i64,
    /// The folder the AppImage was in when it was first integrated: the parent
    /// of the source path. Empty for a row written before the column existed,
    /// and for an adoption, which records no source folder.
    #[serde(default)]
    pub integrated_folder: String,
}

impl InstalledApp {
    pub fn new_owned() -> Self {
        Self {
            owned: true,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntegrateRequest {
    pub source_path: String,
    pub conflict: ConflictPolicy,
    pub replace_uuid: String,
    pub copy_mode: CopyMode,
    pub assume_yes: bool,
    /// The user confirmed the unsafe fallback for this file.
    pub confirm_unsafe: bool,
}

impl Default for IntegrateRequest {
    fn default() -> Self {
        Self {
            source_path: String::new(),
            conflict: ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: CopyMode::Copy,
            assume_yes: false,
            confirm_unsafe: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IntegrateResult {
    pub ok: bool,
    pub partial: bool,
    pub error: String,
    pub app: InstalledApp,
    pub rolled_back: Vec<String>,
    pub source_removed: bool,
    /// Non-fatal problems met while reading the AppImage (for example its
    /// metadata could not be extracted). The app is integrated regardless.
    pub warnings: Vec<String>,
    /// The file needs the unsafe fallback, which the user has not confirmed for
    /// it. Nothing was run and nothing was installed.
    pub fallback_pending: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RemovalResult {
    pub ok: bool,
    pub partial: bool,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct RemovalRequest {
    pub path_or_uuid: String,
    pub mode: RemovalMode,
    pub assume_yes: bool,
}

impl Default for RemovalRequest {
    fn default() -> Self {
        Self {
            path_or_uuid: String::new(),
            mode: RemovalMode::Trash,
            assume_yes: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct UpdateOffer {
    pub uuid: String,
    pub name: String,
    pub current_version: String,
    pub available_version: String,
    pub manager: String,
    pub url: String,
    pub download_size: i64,
    pub digest: String,
    /// True when nothing beyond AppImage/architecture validation will check
    /// the downloaded payload -- no usable digest was advertised.
    pub reduced_verification: bool,
    pub digest_algo: String,
    pub embedded_source: String,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub kind: TaskKind,
    pub state: TaskState,
    pub title: String,
    pub target: String,
    pub progress: i32,
    pub status_text: String,
    pub error: String,
    pub retryable: bool,
    /// Unix seconds when the task began and when it ended (0 while running).
    pub started_at: i64,
    pub finished_at: i64,
    /// The versions an update moves between (or the version an integration or
    /// removal concerns, in `to_version` / `from_version`). Empty when unknown.
    pub from_version: String,
    pub to_version: String,
    /// The stage an update is in (1 Download, 2 Verify, 3 Swap in), or 0.
    pub phase_index: i32,
    pub phase: String,
    /// Bytes moved so far in the current stage, and the total (0 when unknown).
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// True for a removal that deletes the AppImage for good rather than moving
    /// it to the Trash. False for a Trash removal and for every other task. A
    /// record written before this field existed reads as false.
    #[serde(default)]
    pub permanent: bool,
}

/// The stages of applying an update, in the order they run. The numbers are
/// what the Tasks page shows ("1 Download", "2 Verify", "3 Swap in").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePhase {
    Download,
    Verify,
    SwapIn,
}

impl UpdatePhase {
    pub fn index(self) -> i32 {
        match self {
            UpdatePhase::Download => 1,
            UpdatePhase::Verify => 2,
            UpdatePhase::SwapIn => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            UpdatePhase::Download => "Download",
            UpdatePhase::Verify => "Verify",
            UpdatePhase::SwapIn => "Swap in",
        }
    }
}

/// What an update reports while it runs. `Planned` comes once the source has
/// been checked; `Phase` comes as each stage starts and as download bytes
/// arrive (`total` is 0 when the size is not known).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyEvent {
    Planned {
        from_version: String,
        to_version: String,
        download_size: u64,
    },
    Phase {
        phase: UpdatePhase,
        done: u64,
        total: u64,
    },
}

pub fn app_image_type_name(t: AppImageType) -> &'static str {
    match t {
        AppImageType::Type1 => "type-1",
        AppImageType::Type2 => "type-2",
        AppImageType::Dwarfs => "dwarfs",
        AppImageType::Unknown => "unknown",
    }
}

pub fn architecture_name(a: Architecture) -> &'static str {
    match a {
        Architecture::X86_64 => "x86_64",
        Architecture::AArch64 => "aarch64",
        Architecture::I386 => "i386",
        Architecture::Arm => "arm",
        Architecture::Unknown => "unknown",
    }
}

pub fn appearance_name(a: Appearance) -> &'static str {
    match a {
        Appearance::Light => "light",
        Appearance::Dark => "dark",
        Appearance::System => "system",
    }
}

pub fn appearance_from_str(value: &str) -> Appearance {
    match value {
        "light" => Appearance::Light,
        "dark" => Appearance::Dark,
        _ => Appearance::System,
    }
}

pub fn task_kind_name(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::Integrate => "integrate",
        TaskKind::Update => "update",
        TaskKind::Remove => "remove",
        TaskKind::RefreshMetadata => "refresh",
        TaskKind::CheckUpdate => "check-update",
        TaskKind::Adopt => "adopt",
        TaskKind::Inspect => "inspect",
    }
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        hex::decode(text).map_err(serde::de::Error::custom)
    }
}
