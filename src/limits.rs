// Gosh AppImage Manager — bounded constants (ports src/core/Limits.h).
// All sizes, counts, and timeouts that touch untrusted data live here.

pub const DEFAULT_MAX_APPIMAGE_BYTES: i64 = 8 * 1024 * 1024 * 1024;
pub const MIN_MAX_APPIMAGE_BYTES: i64 = 1024 * 1024;
pub const ABSOLUTE_MAX_APPIMAGE_BYTES: i64 = 32 * 1024 * 1024 * 1024;
pub const MAX_DESKTOP_FILE_BYTES: usize = 64 * 1024;
pub const MAX_EXTRACTED_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_PROCESS_OUTPUT_BYTES: usize = 1024 * 1024;
pub const MAX_JSON_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ZSYNC_BYTES: usize = 256 * 1024;
pub const MAX_ARCHIVE_ENTRIES: usize = 64;
pub const MAX_ARCHIVE_LISTING_ENTRIES: usize = 100_000;
pub const MAX_EXTRACTED_FILES: usize = 12;
pub const MAX_TASK_HISTORY: usize = 128;
pub const PROGRESS_EMIT_INTERVAL_MS: u64 = 100;
pub const MAX_PATH_LENGTH: usize = 255;
pub const MAX_ARCHIVE_DEPTH: usize = 3;
pub const MAX_REDIRECTS: usize = 3;
pub const DEFAULT_PROCESS_TIMEOUT_MS: u64 = 30_000;
pub const EXTRACT_TIMEOUT_MS: u64 = 30_000;
pub const NETWORK_TIMEOUT_MS: u64 = 30_000;
pub const HASH_CANCEL_CHECK_BYTES: u64 = 1024 * 1024;
pub const MAX_ENV_PAIRS: usize = 32;
pub const MAX_ARGUMENTS: usize = 64;
pub const MAX_ARGUMENT_LENGTH: usize = 4096;
pub const MAX_NAME_LENGTH: usize = 256;
pub const SYMLINK_HOP_LIMIT: u32 = 8;
pub const REGISTRY_SCHEMA_VERSION: i64 = 1;
pub const JSON_SCHEMA_VERSION: i64 = 1;
pub const ELF_HEADER_READ_BYTES: u64 = 1024 * 1024;
pub const MAX_UPD_INFO_BYTES: usize = 4096;
/// Asset-name glob bounds. The pattern reaches us from an AppImage's
/// `.upd_info` and the text from a remote release document, so both are
/// untrusted and both are capped before matching.
pub const MAX_GLOB_PATTERN_LENGTH: usize = 256;
pub const MAX_GLOB_TEXT_LENGTH: usize = 512;

pub const APP_ID: &str = "com.goshapps.AppImageManager";
pub const EXECUTABLE_NAME: &str = "gosh-appimage-manager";
pub const OWNERSHIP_KEY: &str = "X-Gosh-AppImage-Manager";
pub const OWNERSHIP_UUID_KEY: &str = "X-Gosh-AppImage-Id";
pub const OWNERSHIP_PATH_KEY: &str = "X-Gosh-Managed-Path";
pub const DESKTOP_PREFIX: &str = "gosh-appimage-";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn clamp_max_appimage_bytes(value: i64) -> i64 {
    value.clamp(MIN_MAX_APPIMAGE_BYTES, ABSOLUTE_MAX_APPIMAGE_BYTES)
}

/// Parse the Settings "max size (MB)" field. Strict (no silent clamp) so a
/// typo surfaces as an error instead of an unexpected bound.
pub fn parse_max_appimage_mb(input: &str) -> Result<i64, String> {
    const MB: i64 = 1024 * 1024;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Maximum size cannot be empty".to_string());
    }
    let mb: i64 = trimmed
        .parse()
        .map_err(|_| format!("\"{trimmed}\" is not a whole number of megabytes"))?;
    let min_mb = MIN_MAX_APPIMAGE_BYTES / MB;
    let max_mb = ABSOLUTE_MAX_APPIMAGE_BYTES / MB;
    if !(min_mb..=max_mb).contains(&mb) {
        return Err(format!(
            "Enter {min_mb}–{max_mb} MB (default {})",
            DEFAULT_MAX_APPIMAGE_BYTES / MB
        ));
    }
    mb.checked_mul(MB)
        .ok_or_else(|| "Maximum size is too large".to_string())
}

/// Whole megabytes for the Settings field (8 GB default renders as "8192").
pub fn max_appimage_mb(bytes: i64) -> i64 {
    bytes / (1024 * 1024)
}
