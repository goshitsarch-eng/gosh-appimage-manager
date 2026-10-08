//! The bridge API the Flutter front end calls.
//!
//! Every function here is a coarse operation, and every type is a DTO that is
//! copied across the boundary. No Rust object is handed to Dart. Domain
//! behaviour stays in `goshaim_core`; this module only translates.

use std::any::Any;
use std::fmt;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::frb_generated::StreamSink;
use flutter_rust_bridge::frb;

use goshaim_core::inspector::AppImageInspector;
use goshaim_core::process::SystemRunner;
use goshaim_core::types::{AppImageType, Architecture, InspectOptions};

/// The failure kinds the front end distinguishes (design doc, section 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    UserInput,
    Validation,
    NotFound,
    Permission,
    CorruptData,
    Internal,
}

/// A typed core failure. `message` is safe to show; `details` is for the log only.
#[derive(Debug, Clone)]
pub struct CoreError {
    pub kind: ErrorKind,
    pub message: String,
    pub details: String,
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CoreError {}

/// The result of inspecting one file. Read-only: nothing is installed or executed.
#[derive(Debug, Clone)]
pub struct InspectSummary {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub app_type: String,
    pub architecture: String,
    pub magic_valid: bool,
    pub architecture_supported: bool,
    pub name: String,
    pub warnings: Vec<String>,
}

/// The bridge's own version. Synchronous, so it doubles as a cheap round-trip check.
#[frb(sync)]
pub fn bridge_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Inspect an AppImage through the core's inspection pipeline. Never executes it.
pub fn inspect_path(path: String) -> Result<InspectSummary, CoreError> {
    if path.trim().is_empty() {
        return Err(CoreError {
            kind: ErrorKind::UserInput,
            message: "Choose an AppImage to inspect.".to_string(),
            details: String::new(),
        });
    }
    if !std::path::Path::new(&path).exists() {
        return Err(CoreError {
            kind: ErrorKind::NotFound,
            message: "That file does not exist.".to_string(),
            details: path,
        });
    }

    let runner = SystemRunner;
    let cancel = AtomicBool::new(false);
    let result =
        AppImageInspector::new(&runner).inspect(&path, &InspectOptions::default(), &cancel, None);
    if !result.error.is_empty() {
        return Err(CoreError {
            kind: ErrorKind::Validation,
            message: result.error.clone(),
            details: path,
        });
    }

    Ok(InspectSummary {
        path: result.identity.path.clone(),
        size_bytes: u64::try_from(result.identity.size).unwrap_or(0),
        sha256: result
            .identity
            .sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        app_type: app_type_name(&result.app_type).to_string(),
        architecture: architecture_name(&result.architecture).to_string(),
        magic_valid: result.magic_valid,
        architecture_supported: result.architecture_supported,
        name: result.metadata.name.clone(),
        warnings: result.warnings.clone(),
    })
}

/// Emit `0..total` on a stream, one value every 20 ms. Shows that a long Rust
/// operation can report progress without blocking the Dart isolate.
pub fn count_ticks(total: u32, sink: StreamSink<u32>) {
    for tick in 0..total {
        if sink.add(tick).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Panics on purpose and proves the containment rule: a Rust panic reaches Dart
/// as a `CoreError` of kind `Internal` and does not end the process.
/// Spike-only; remove before the bridge is used by real pages.
pub fn panic_for_contract_test() -> Result<u32, CoreError> {
    std::panic::catch_unwind(|| {
        panic!("deliberate contract-test panic");
    })
    .map(|_| 0)
    .map_err(|payload| CoreError {
        kind: ErrorKind::Internal,
        message: "Something went wrong inside the core. Details were written to the log."
            .to_string(),
        details: panic_message(payload),
    })
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        return (*text).to_string();
    }
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    "non-string panic payload".to_string()
}

fn app_type_name(kind: &AppImageType) -> &'static str {
    match kind {
        AppImageType::Unknown => "unknown",
        AppImageType::Type1 => "type1",
        AppImageType::Type2 => "type2",
        AppImageType::Dwarfs => "dwarfs",
    }
}

fn architecture_name(arch: &Architecture) -> &'static str {
    match arch {
        Architecture::Unknown => "unknown",
        Architecture::X86_64 => "x86_64",
        Architecture::AArch64 => "aarch64",
        Architecture::I386 => "i386",
        Architecture::Arm => "arm",
    }
}
