// Gosh AppImage Manager — bounded AppImage inspector (ports AppImageInspector).
// Opening a file NEVER integrates or executes it. Metadata comes from
// bounded extractor-tool output only. The unsafe --appimage-extract fallback
// requires an explicit per-file opt-in and is never used in tests or
// background flows.

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::desktop;
use crate::elf;
use crate::limits;
use crate::process::{ProcessRequest, ProcessRunner};
use crate::safe_fs;
use crate::types::{
    AppImageMetadata, AppImageType, Architecture, EmbeddedUpdateInfo, InspectOptions,
    InspectionResult,
};

pub struct AppImageInspector<'a> {
    runner: &'a dyn ProcessRunner,
}

impl<'a> AppImageInspector<'a> {
    pub fn new(runner: &'a dyn ProcessRunner) -> Self {
        Self { runner }
    }

    pub fn inspect(
        &self,
        path: &str,
        options: &InspectOptions,
        cancel: &AtomicBool,
        existing_managed_id: Option<&str>,
    ) -> InspectionResult {
        let mut result = InspectionResult::default();
        result.identity.path = path.to_string();
        if let Some(id) = existing_managed_id {
            result.existing_managed_id = id.to_string();
            result.already_managed = true;
        }

        let fs_path = Path::new(path);
        // 1. Must be a regular file; never follow unsafe symlinks blindly.
        let meta = match fs::symlink_metadata(fs_path) {
            Ok(m) => m,
            Err(_) => {
                result.error = format!("Cannot open file: {path}");
                return result;
            }
        };
        if meta.file_type().is_symlink() {
            match safe_fs::canonical_bounded(fs_path) {
                Ok(canon) => {
                    let kind = fs::metadata(&canon).map(|m| m.file_type());
                    match kind {
                        Ok(k) if k.is_file() => {}
                        _ => {
                            result.error = "Not a regular file".to_string();
                            return result;
                        }
                    }
                }
                Err(e) => {
                    result.error = e;
                    return result;
                }
            }
        } else if !meta.file_type().is_file() {
            result.error = "Not a regular file".to_string();
            return result;
        }
        let size = fs::metadata(fs_path).map(|m| m.len()).unwrap_or(0) as i64;
        result.identity.size = size;
        if size <= 0 {
            result.error = "Empty file".to_string();
            return result;
        }
        if size > options.max_bytes {
            result.error = format!(
                "File exceeds configured size bound ({} bytes)",
                options.max_bytes
            );
            return result;
        }

        // 2. ELF + AppImage magic (MIME/extension alone is never enough).
        let info = elf::parse_file(fs_path, limits::ELF_HEADER_READ_BYTES);
        result.truncated = info.truncated;
        result.payload_offset = info.payload_offset;
        result.app_type = info.app_image_type;
        result.architecture = info.architecture;
        if !info.upd_info.is_empty() {
            result.update_info = parse_upd_info(&info.upd_info);
        }
        if info.error.is_empty()
            && !matches!(info.app_image_type, AppImageType::Unknown)
            && !matches!(info.architecture, Architecture::Unknown)
        {
            result.magic_valid = true;
        } else {
            result.magic_valid = false;
            result.error = if info.error.is_empty() {
                "Missing AppImage magic at ELF offset 8".to_string()
            } else {
                info.error.clone()
            };
            return result;
        }

        // 3. Architecture support is a warning, not a silent pass.
        result.architecture_supported = elf::architecture_supported(info.architecture);
        if !result.architecture_supported {
            result.warnings.push(format!(
                "Unsupported architecture: {}",
                crate::types::architecture_name(info.architecture)
            ));
        }

        // 4. Streaming hash (bounded, cancellable).
        if options.compute_hash {
            match safe_fs::sha256_file(fs_path, cancel) {
                Ok(sum) => result.identity.sha256 = sum,
                Err(e) => {
                    result.error = e;
                    return result;
                }
            }
        }

        // 5. Safe metadata extraction (never executes the AppImage).
        if options.extract_metadata {
            result.extraction_attempted = true;
            match self.extract_metadata(fs_path, &info, &result.update_info) {
                Ok((metadata, extractor)) => {
                    if !metadata.extracted_icon_path.is_empty() {
                        result.icon_staging_dir = Path::new(&metadata.extracted_icon_path)
                            .parent()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default();
                    }
                    result.metadata = metadata;
                    result.extractor_used = extractor;
                }
                Err(warning) => result.warnings.push(warning),
            }
        }

        // The unsafe fallback: run the AppImage's own `--appimage-extract`.
        //
        // This executes untrusted code, so it is reached only when metadata
        // extraction failed *and* the setting is on *and* this exact file was
        // confirmed. It is never used by tests or background flows, because
        // neither sets confirm_unsafe_extract.
        //
        // Previously this branch only pushed a warning claiming code was being
        // executed, while nothing ran: the setting, the confirmation dialog and
        // the "proof of no execution" probe all pointed at a feature that did
        // not exist, and the warning text was untrue.
        let need_fallback = result.metadata.name.is_empty() && result.extraction_attempted;
        if need_fallback && options.allow_unsafe_extract && options.confirm_unsafe_extract {
            result.warnings.push(
                "Unsafe extraction fallback used for this file: the AppImage was executed to \
                 read its own metadata"
                    .to_string(),
            );
            match self.extract_via_appimage(fs_path) {
                Ok(metadata) => {
                    result.extraction_used_unsafe_fallback = true;
                    result.extractor_used = "--appimage-extract".to_string();
                    if !metadata.extracted_icon_path.is_empty() {
                        result.icon_staging_dir = Path::new(&metadata.extracted_icon_path)
                            .parent()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default();
                    }
                    result.metadata = metadata;
                }
                Err(error) => result
                    .warnings
                    .push(format!("Unsafe extraction fallback failed: {error}")),
            }
        } else if need_fallback && options.allow_unsafe_extract {
            result.warnings.push(
                "Metadata could not be read safely. The unsafe fallback is enabled but needs \
                 per-file confirmation."
                    .to_string(),
            );
        }
        result
    }

    /// Last resort: ask the AppImage to unpack itself.
    ///
    /// This runs the untrusted binary. Callers gate it behind an opt-in
    /// setting plus a per-file confirmation; nothing here decides that.
    /// The process is bounded by the extractor timeout and runs with its
    /// working directory inside a private mode-0700 temp directory, so
    /// whatever it writes lands there.
    fn extract_via_appimage(&self, path: &Path) -> Result<AppImageMetadata, String> {
        let work_parent = std::env::temp_dir().join("gosh-appimage-manager");
        let work = safe_fs::private_temp_dir(&work_parent, "unsafe-")?;
        let outcome = (|| {
            let out = self.runner.run(&ProcessRequest {
                program: safe_fs::argv_safe_path(path),
                args: vec!["--appimage-extract".to_string()],
                work_dir: work.to_string_lossy().into_owned(),
                timeout_ms: limits::EXTRACT_TIMEOUT_MS,
                ..Default::default()
            });
            if out.refused || out.timed_out || out.exit_code != 0 {
                return Err(format!(
                    "self-extraction failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            // The AppImage runtime unpacks into ./squashfs-root.
            let root = work.join("squashfs-root");
            if !root.is_dir() {
                return Err("self-extraction produced no squashfs-root".to_string());
            }
            let desktop_file = find_desktop_in(&root)
                .ok_or_else(|| "no desktop entry in the extracted tree".to_string())?;
            let bytes = fs::read(&desktop_file).map_err(|e| e.to_string())?;
            if bytes.len() > limits::MAX_DESKTOP_FILE_BYTES {
                return Err("desktop entry exceeds size bound".to_string());
            }
            let file = desktop::parse_desktop_bytes(&bytes)
                .map_err(|e| format!("cannot parse desktop entry: {e}"))?;
            let mut metadata = metadata_from_desktop(&file);
            // Stage the icon the same way the safe path does.
            if let Some((source, ext)) = find_icon_in(&root, &metadata.icon_name) {
                let stage = safe_fs::private_temp_dir(&work_parent, "icon-")?;
                let dest = stage.join(format!("icon.{ext}"));
                match safe_fs::copy_bounded(
                    &source,
                    &dest,
                    limits::MAX_ICON_BYTES,
                    &AtomicBool::new(false),
                ) {
                    Ok(_) => {
                        metadata.extracted_icon_path = dest.to_string_lossy().into_owned();
                        metadata.icon_format = ext;
                    }
                    Err(_) => {
                        let _ = safe_fs::remove_dir_no_follow(&stage);
                    }
                }
            }
            Ok(metadata)
        })();
        let _ = safe_fs::remove_dir_no_follow(&work);
        outcome
    }

    /// Best-effort metadata extraction via pinned helper tools.
    fn extract_metadata(
        &self,
        path: &Path,
        info: &elf::ElfInfo,
        _update: &EmbeddedUpdateInfo,
    ) -> Result<(AppImageMetadata, String), String> {
        let work_parent = std::env::temp_dir().join("gosh-appimage-manager");
        // A caller that inspects without integrating has nothing to clean up
        // its icon staging, so sweep anything left from an earlier run before
        // creating more. Bounds the directory without requiring every caller
        // to remember.
        sweep_stale_staging(&work_parent);
        let work = safe_fs::private_temp_dir(&work_parent, "inspect-")?;
        let cleanup = || {
            let _ = safe_fs::remove_dir_no_follow(&work);
        };
        let outcome: Result<(AppImageMetadata, String), String> = (|| {
            let tool = match info.app_image_type {
                AppImageType::Type1 => "7zz",
                AppImageType::Type2 => {
                    if info.squashfs_magic || !info.dwarfs_magic {
                        "unsquashfs"
                    } else {
                        "dwarfsextract"
                    }
                }
                AppImageType::Dwarfs => "dwarfsextract",
                AppImageType::Unknown => return Err("Unknown AppImage type".to_string()),
            };
            let entries = self.list_archive(tool, path)?;
            let desktop_member = pick_desktop_member(&entries)
                .ok_or_else(|| "No desktop entry found in AppImage".to_string())?;
            let staged = self.extract_members(
                tool,
                path,
                &work,
                &[desktop_member.clone(), ".DirIcon".to_string()],
            )?;
            let desktop_bytes = staged.get(&desktop_member).cloned().unwrap_or_default();
            let file = desktop::parse_desktop_bytes(&desktop_bytes)
                .map_err(|e| format!("Cannot parse desktop entry: {e}"))?;
            let mut metadata = metadata_from_desktop(&file);

            // Icon: prefer the referenced icon, fall back to .DirIcon. The
            // bytes have to outlive `work`, which is removed on the way out,
            // so copy the chosen icon into a staging directory the caller
            // owns. Recording the archive member name here (as this used to)
            // left a path into a deleted directory, which is why no
            // integrated AppImage ever got an icon.
            let mut chosen: Option<(String, String)> = None;
            if !metadata.icon_name.is_empty() {
                for ext in ["png", "svg", "xpm"] {
                    let candidate = format!("{}.{}", metadata.icon_name, ext);
                    if let Some(member) = find_member(&entries, &candidate) {
                        let got =
                            self.extract_members(tool, path, &work, std::slice::from_ref(&member))?;
                        if let Some(bytes) = got.get(&member) {
                            if (bytes.len() as u64) <= limits::MAX_ICON_BYTES {
                                chosen = Some((member, ext.to_string()));
                                break;
                            }
                        }
                    }
                }
            }
            if chosen.is_none() {
                if let Some(bytes) = staged.get(".DirIcon") {
                    if (bytes.len() as u64) <= limits::MAX_ICON_BYTES {
                        // .DirIcon carries no extension; sniff the content so
                        // the installed file is named correctly.
                        chosen = Some((".DirIcon".to_string(), sniff_icon_extension(bytes)));
                    }
                }
            }
            if let Some((member, ext)) = chosen {
                let source = work.join(&member);
                if source.is_file() {
                    let stage = safe_fs::private_temp_dir(&work_parent, "icon-")?;
                    let dest = stage.join(format!("icon.{ext}"));
                    match safe_fs::copy_bounded(
                        &source,
                        &dest,
                        limits::MAX_ICON_BYTES,
                        &AtomicBool::new(false),
                    ) {
                        Ok(_) => {
                            metadata.extracted_icon_path = dest.to_string_lossy().into_owned();
                            metadata.icon_format = ext;
                        }
                        Err(_) => {
                            let _ = safe_fs::remove_dir_no_follow(&stage);
                        }
                    }
                }
            }
            Ok((metadata, tool.to_string()))
        })();
        cleanup();
        outcome
    }

    fn list_archive(
        &self,
        tool: &str,
        path: &Path,
    ) -> Result<Vec<crate::types::ArchiveEntry>, String> {
        let (program, args) = match tool {
            "unsquashfs" => (
                "unsquashfs".to_string(),
                vec!["-l".to_string(), safe_fs::argv_safe_path(path)],
            ),
            "7zz" => (
                "7zz".to_string(),
                vec![
                    "l".to_string(),
                    "-ba".to_string(),
                    safe_fs::argv_safe_path(path),
                ],
            ),
            _ => {
                return Err(format!(
                    "No safe lister for extractor {tool}; install the pinned helper tools"
                ))
            }
        };
        let out = self.runner.run(&ProcessRequest {
            program,
            args,
            timeout_ms: limits::EXTRACT_TIMEOUT_MS,
            ..Default::default()
        });
        if out.exit_code != 0 {
            return Err(format!(
                "Archive listing failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        parse_listing(tool, &out.stdout)
    }

    fn extract_members(
        &self,
        tool: &str,
        path: &Path,
        dest: &Path,
        members: &[String],
    ) -> Result<std::collections::BTreeMap<String, Vec<u8>>, String> {
        if members.len() > limits::MAX_EXTRACTED_FILES {
            return Err("Too many files requested".to_string());
        }
        for member in members {
            safe_fs::valid_archive_member(member)
                .map_err(|e| format!("Refusing unsafe archive path: {e}"))?;
        }
        let (program, mut args) = match tool {
            "unsquashfs" => (
                "unsquashfs".to_string(),
                vec![
                    "-q".to_string(),
                    "-d".to_string(),
                    dest.to_string_lossy().into_owned(),
                    "-f".to_string(),
                    safe_fs::argv_safe_path(path),
                ],
            ),
            "7zz" => (
                "7zz".to_string(),
                vec![
                    "x".to_string(),
                    format!("-o{}", dest.to_string_lossy()),
                    safe_fs::argv_safe_path(path),
                ],
            ),
            _ => return Err(format!("No safe extractor {tool}")),
        };
        args.extend(members.iter().cloned());
        let out = self.runner.run(&ProcessRequest {
            program,
            args,
            timeout_ms: limits::EXTRACT_TIMEOUT_MS,
            ..Default::default()
        });
        if out.exit_code != 0 {
            return Err(format!(
                "Archive extraction failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        // Collect extracted bytes with a total bound.
        let mut collected = std::collections::BTreeMap::new();
        let mut total: u64 = 0;
        let mut files = 0;
        let mut stack = vec![dest.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let file_type = entry.file_type().map_err(|e| e.to_string()).map(|_| ());
                let _ = file_type;
                let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
                if meta.file_type().is_symlink() {
                    continue; // never follow escaping symlinks
                }
                if meta.is_dir() {
                    stack.push(entry.path());
                    continue;
                }
                files += 1;
                if files > limits::MAX_EXTRACTED_FILES {
                    return Err("Too many extracted files".to_string());
                }
                let bytes = fs::read(entry.path()).map_err(|e| e.to_string())?;
                total += bytes.len() as u64;
                if total > limits::MAX_EXTRACTED_BYTES {
                    return Err("Extraction exceeds output bound".to_string());
                }
                if let Ok(rel) = entry.path().strip_prefix(dest) {
                    collected.insert(rel.to_string_lossy().into_owned(), bytes);
                }
            }
        }
        Ok(collected)
    }
}

/// Build metadata from a parsed desktop entry (shared by both extract paths).
fn metadata_from_desktop(file: &desktop::DesktopFile) -> AppImageMetadata {
    let mut metadata = AppImageMetadata {
        name: desktop::unescape_entry_value(file.entry("Name"))
            .chars()
            .take(limits::MAX_NAME_LENGTH)
            .collect(),
        version: desktop::unescape_entry_value(file.entry("X-AppImage-Version")),
        comment: desktop::unescape_entry_value(file.entry("Comment")),
        icon_name: desktop::sanitize_icon_name(file.entry("Icon")),
        exec_raw: file.entry("Exec").to_string(),
        ..Default::default()
    };
    metadata.try_exec = file.entry("TryExec").to_string();
    metadata.terminal = file.entry("Terminal") == "true";
    metadata.website = file.entry("Url").to_string();
    metadata.startup_wm_class = file.entry("StartupWMClass").to_string();
    metadata.categories = split_desktop_list(file.entry("Categories"));
    metadata.mime_types = split_desktop_list(file.entry("MimeType"));
    metadata.exec_arguments = exec_arguments(file.entry("Exec"));
    metadata.actions = parse_desktop_actions(file);
    metadata
}

/// Top-level `*.desktop` in a self-extracted tree.
fn find_desktop_in(root: &Path) -> Option<std::path::PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    entries
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_file() && p.extension().is_some_and(|e| e == "desktop"))
}

/// The referenced icon, or `.DirIcon`, in a self-extracted tree.
fn find_icon_in(root: &Path, icon_name: &str) -> Option<(std::path::PathBuf, String)> {
    if !icon_name.is_empty() {
        for ext in ["png", "svg", "xpm"] {
            let candidate = root.join(format!("{icon_name}.{ext}"));
            if candidate.is_file() {
                return Some((candidate, ext.to_string()));
            }
        }
    }
    let dir_icon = root.join(".DirIcon");
    if dir_icon.is_file() {
        let bytes = fs::read(&dir_icon).ok()?;
        return Some((dir_icon, sniff_icon_extension(&bytes)));
    }
    None
}

/// Remove leftover work and icon-staging directories older than an hour.
///
/// Best effort and non-fatal: anything still in use by a concurrent
/// inspection is younger than the threshold, and a directory we cannot read
/// or remove is simply left alone.
fn sweep_stale_staging(parent: &Path) {
    const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(3600);
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten().take(4096) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("inspect-") || name.starts_with("icon-")) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        let stale = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .map(|age| age > STALE_AFTER)
            .unwrap_or(false);
        if stale {
            let _ = safe_fs::remove_dir_no_follow(&entry.path());
        }
    }
}

/// Split a `;`-delimited Desktop Entry list, bounded and sanitised.
fn split_desktop_list(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(|item| item.trim())
        .filter(|item| {
            !item.is_empty()
                && item.len() <= 128
                && item
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '.' | '/'))
        })
        .map(|item| item.to_string())
        .take(32)
        .collect()
}

/// Take the argument tokens from an `Exec` line, dropping the program itself
/// and any field codes. The result is an argument *array*, never a shell
/// fragment; it becomes the app's default arguments.
fn exec_arguments(exec: &str) -> Vec<String> {
    let mut tokens = split_exec_tokens(exec);
    if tokens.is_empty() {
        return Vec::new();
    }
    tokens.remove(0);
    tokens
        .into_iter()
        .filter(|t| !(t.len() == 2 && t.starts_with('%')))
        .filter(|t| t.len() <= limits::MAX_ARGUMENT_LENGTH && !t.contains('\0'))
        .take(limits::MAX_ARGUMENTS)
        .collect()
}

/// Split an Exec value into tokens, honouring the spec's quoting.
fn split_exec_tokens(exec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escaped = false;
    let mut started = false;
    for ch in exec.chars().take(limits::MAX_ARGUMENT_LENGTH * 4) {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => {
                in_quotes = !in_quotes;
                started = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if started || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
        if out.len() > limits::MAX_ARGUMENTS {
            break;
        }
    }
    if started || !current.is_empty() {
        out.push(current);
    }
    out
}

/// Read `[Desktop Action <id>]` groups, keeping only argument tokens.
///
/// Actions from an untrusted AppImage may not carry their own program: the
/// Exec is rewritten against the managed path when the entry is written, so
/// only the arguments survive.
fn parse_desktop_actions(file: &desktop::DesktopFile) -> Vec<crate::types::DesktopAction> {
    let ids = split_desktop_list(file.entry("Actions"));
    let mut actions = Vec::new();
    for id in ids.into_iter().take(16) {
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || id.len() > 64 {
            continue;
        }
        let Some(group) = file.groups.get(&format!("Desktop Action {id}")) else {
            continue;
        };
        let name =
            desktop::unescape_entry_value(group.get("Name").map(String::as_str).unwrap_or(""))
                .chars()
                .take(limits::MAX_NAME_LENGTH)
                .collect::<String>();
        let arguments = exec_arguments(group.get("Exec").map(String::as_str).unwrap_or(""));
        actions.push(crate::types::DesktopAction {
            id,
            name,
            arguments,
        });
    }
    actions
}

/// Identify an icon's format from its leading bytes; `.DirIcon` has no
/// extension and the file name decides how the desktop reads it.
fn sniff_icon_extension(bytes: &[u8]) -> String {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return "png".to_string();
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    if head.contains("<svg") || head.contains("<?xml") {
        return "svg".to_string();
    }
    if bytes.starts_with(b"/* XPM */") {
        return "xpm".to_string();
    }
    "png".to_string()
}

fn parse_listing(tool: &str, stdout: &[u8]) -> Result<Vec<crate::types::ArchiveEntry>, String> {
    let text = String::from_utf8_lossy(stdout);
    let mut entries = Vec::new();
    let mut lines = 0;
    for line in text.lines() {
        lines += 1;
        if lines > limits::MAX_ARCHIVE_LISTING_ENTRIES {
            return Err("Archive listing exceeded output bound".to_string());
        }
        let member = match tool {
            "7zz" => line
                .split_whitespace()
                .last()
                .unwrap_or_default()
                .to_string(),
            _ => {
                // unsquashfs -l prints "squashfs-root/..." lines; strip prefix.
                let trimmed = line.trim();
                trimmed
                    .strip_prefix("squashfs-root/")
                    .unwrap_or(trimmed)
                    .to_string()
            }
        };
        if member.is_empty() {
            continue;
        }
        if safe_fs::valid_archive_member(&member).is_err() {
            continue; // skip unsafe members, never extract them
        }
        entries.push(crate::types::ArchiveEntry {
            path: member,
            ..Default::default()
        });
        if entries.len() > limits::MAX_ARCHIVE_ENTRIES * 16 {
            break;
        }
    }
    Ok(entries)
}

fn pick_desktop_member(entries: &[crate::types::ArchiveEntry]) -> Option<String> {
    // Prefer top-level *.desktop files.
    for entry in entries {
        if entry.path.ends_with(".desktop") && !entry.path.contains('/') {
            return Some(entry.path.clone());
        }
    }
    entries
        .iter()
        .find(|e| e.path.ends_with(".desktop"))
        .map(|e| e.path.clone())
}

fn find_member(entries: &[crate::types::ArchiveEntry], name: &str) -> Option<String> {
    entries
        .iter()
        .find(|e| {
            e.path == name
                || e.path.ends_with(&format!("/{name}"))
                || e.path.rsplit('/').next() == Some(name)
        })
        .map(|e| e.path.clone())
}

/// Parse an embedded `.upd_info` string into manager hint + fields.
pub fn parse_upd_info(raw: &[u8]) -> EmbeddedUpdateInfo {
    let mut info = EmbeddedUpdateInfo::default();
    let text = String::from_utf8_lossy(raw);
    let text = text.trim_matches('\0').trim().to_string();
    if text.is_empty() {
        return info;
    }
    info.raw = text.clone();
    let parts: Vec<&str> = text.split('|').collect();
    let hint = parts.first().copied().unwrap_or_default().to_lowercase();
    info.manager_hint = if hint.contains("github") {
        "github".to_string()
    } else if hint.contains("gitlab") {
        "gitlab".to_string()
    } else if hint.contains("codeberg") {
        "codeberg".to_string()
    } else if hint.contains("forgejo") {
        "forgejo".to_string()
    } else if hint.contains("ftp") {
        "ftp".to_string()
    } else if hint.contains("static")
        || hint.contains("file")
        || hint == "zsync"
        || hint.contains("bintray")
    {
        "static".to_string()
    } else {
        hint.clone()
    };
    match info.manager_hint.as_str() {
        "github" | "gh-releases-zsync" => {
            // gh-releases-zsync|user|repo|release|filename
            let keys = ["username", "repo", "release", "filename"];
            for (i, key) in keys.iter().enumerate() {
                if let Some(value) = parts.get(i + 1) {
                    info.fields.insert(key.to_string(), value.to_string());
                }
            }
        }
        "static" => {
            if hint == "zsync" {
                if let Some(url) = parts.get(1) {
                    info.fields.insert("url".to_string(), url.to_string());
                }
            } else {
                info.fields.insert("url".to_string(), text.clone());
            }
        }
        "ftp" => {
            if let Some(url) = parts.get(1) {
                info.fields.insert("url".to_string(), url.to_string());
            }
        }
        _ => {
            for (i, part) in parts.iter().skip(1).enumerate() {
                info.fields.insert(format!("field{i}"), part.to_string());
            }
        }
    }
    info
}

/// Synthetic Type-2 ELF fixture builder for tests (never a real AppImage).
pub fn make_test_elf(arch: Architecture, app_type: AppImageType) -> Vec<u8> {
    let mut data = vec![0u8; 128];
    data[0] = 0x7f;
    data[1] = b'E';
    data[2] = b'L';
    data[3] = b'F';
    data[4] = 2; // 64-bit
    data[5] = 1; // little-endian
    data[6] = 1; // version
    data[7] = 0;
    data[8] = b'A';
    data[9] = b'I';
    data[10] = match app_type {
        AppImageType::Type1 => 1,
        AppImageType::Type2 | AppImageType::Dwarfs => 2,
        AppImageType::Unknown => 0,
    };
    let machine: u16 = match arch {
        Architecture::X86_64 => 0x3E,
        Architecture::AArch64 => 0xB7,
        Architecture::I386 => 0x03,
        Architecture::Arm => 0x28,
        Architecture::Unknown => 0,
    };
    data[18..20].copy_from_slice(&machine.to_le_bytes());
    data
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    #[test]
    fn fixture_parses() {
        let data = make_test_elf(Architecture::X86_64, AppImageType::Type2);
        let info = elf::parse(&data);
        assert!(info.error.is_empty(), "unexpected error: {}", info.error);
        assert_eq!(info.app_image_type, AppImageType::Type2);
        assert_eq!(info.architecture, Architecture::X86_64);
    }
}
