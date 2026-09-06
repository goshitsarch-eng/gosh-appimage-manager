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
    pub extracted_icon_path: String,
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
}

#[derive(Debug, Clone)]
pub struct InspectOptions {
    pub extract_metadata: bool,
    pub compute_hash: bool,
    pub allow_unsafe_extract: bool,
    pub confirm_unsafe_extract: bool,
    pub max_bytes: i64,
}

impl Default for InspectOptions {
    fn default() -> Self {
        Self {
            extract_metadata: true,
            compute_hash: true,
            allow_unsafe_extract: false,
            confirm_unsafe_extract: false,
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
}

impl Default for IntegrateRequest {
    fn default() -> Self {
        Self {
            source_path: String::new(),
            conflict: ConflictPolicy::Unspecified,
            replace_uuid: String::new(),
            copy_mode: CopyMode::Copy,
            assume_yes: false,
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
    pub reduced_verification: bool,
    pub embedded_source: String,
    pub running: bool,
}

#[derive(Debug, Clone)]
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
