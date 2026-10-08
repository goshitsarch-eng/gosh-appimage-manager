// Gosh AppImage Manager — bounded AppImage inspector (ports AppImageInspector).
// Opening a file NEVER integrates or executes it. Metadata comes from
// bounded extractor-tool output only. The unsafe --appimage-extract fallback
// is the last resort, runs only while the stored setting is on, and is never
// used in tests or background flows.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::desktop;
use crate::elf;
use crate::limits;
use crate::process::{ProcessRequest, ProcessRunner};
use crate::safe_fs;
use crate::types::{
    AppImageMetadata, AppImageType, Architecture, EmbeddedUpdateInfo, InspectOptions,
    InspectionResult,
};

/// The options every inspection runs with. The unsafe fallback may run only when the
/// stored setting is on AND the user confirmed this file. The setting is read here,
/// so no caller chooses it. Every path that inspects an AppImage goes through this.
pub fn gated_options(
    settings: &crate::settings::SettingsStore,
    options: &InspectOptions,
) -> InspectOptions {
    let mut gated = options.clone();
    gated.unsafe_setting_on = settings.unsafe_extraction_fallback();
    gated.allow_unsafe_extract = gated.unsafe_setting_on && gated.confirm_unsafe;
    gated
}

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
                    if cancelled(cancel) {
                        return stopped(result);
                    }
                    result.error = e;
                    return result;
                }
            }
        }
        if cancelled(cancel) {
            return stopped(result);
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
        // The archive read is not interruptible, so the cancel is noticed here.
        // The unsafe fallback below must not run for a cancelled inspection.
        if cancelled(cancel) {
            return stopped(result);
        }

        // The unsafe last resort: run the AppImage's own `--appimage-extract`.
        //
        // This executes the file. It runs only when safe extraction produced no
        // name, and only when `allow_unsafe_extract` is set. `gated_options` sets
        // that from the stored setting for every inspection, so no caller chooses
        // it. The setting is off by default, and the result says when the file ran.
        let need_fallback = result.metadata.name.is_empty() && result.extraction_attempted;
        if need_fallback && options.allow_unsafe_extract {
            result.warnings.push(
                "Unsafe extraction fallback used for this file: the AppImage was executed to \
                 read its own metadata"
                    .to_string(),
            );
            let max_bytes = u64::try_from(options.max_bytes).unwrap_or(0);
            result.unsafe_fallback_ran = true;
            match self.extract_via_appimage(fs_path, max_bytes, limits::EXTRACT_TIMEOUT_MS, cancel)
            {
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
        } else if need_fallback && options.unsafe_setting_on {
            // The setting is on, but this file is not confirmed: nothing runs.
            result.fallback_pending = true;
            result.warnings.push(
                "Metadata could not be read safely. Confirm the unsafe extraction fallback for "
                    .to_string()
                    + "this file to let the AppImage's own --appimage-extract read its metadata. "
                    + "Nothing was run.",
            );
        } else if need_fallback {
            result.warnings.push(
                "Metadata could not be read safely. The unsafe extraction fallback is off, so \
                 the AppImage was not run to read it. It can be turned on in Settings."
                    .to_string(),
            );
        }
        if cancelled(cancel) {
            return stopped(result);
        }
        result
    }

    /// Last resort: run the AppImage's own `--appimage-extract`, then read what it wrote.
    ///
    /// This executes the file, so callers reach it only with the stored setting on
    /// and after safe extraction failed. The file runs as a private copy, inside a
    /// mode-0700 directory the manager created, with no standard input, a minimal
    /// environment, and a timeout that kills it and whatever it started. Only regular
    /// files directly inside the tree it wrote are read. A symlink is never followed,
    /// so nothing outside that tree is read.
    fn extract_via_appimage(
        &self,
        path: &Path,
        max_bytes: u64,
        timeout_ms: u64,
        cancel: &AtomicBool,
    ) -> Result<AppImageMetadata, String> {
        let work_parent = std::env::temp_dir().join("gosh-appimage-manager");
        let work = safe_fs::private_temp_dir(&work_parent, "unsafe-")?;
        let outcome = (|| {
            // Run a copy made here, so the file that runs is the one read here.
            // The copy is removed as soon as the run is over.
            let staged = work.join("appimage-copy");
            safe_fs::copy_bounded(path, &staged, max_bytes, cancel)?;
            fs::set_permissions(&staged, fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("Cannot prepare the copy to run: {e}"))?;
            let out = self.runner.run(&ProcessRequest {
                program: safe_fs::argv_safe_path(&staged),
                args: vec!["--appimage-extract".to_string()],
                work_dir: work.to_string_lossy().into_owned(),
                timeout_ms,
                minimal_env: true,
                ..Default::default()
            });
            let _ = fs::remove_file(&staged);
            if out.refused || out.timed_out || out.exit_code != 0 {
                return Err(format!(
                    "self-extraction failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            // Check the whole tree before anything in it is read.
            verify_extracted_tree(&work, max_bytes)?;
            // The AppImage runtime unpacks into ./squashfs-root.
            let root = work.join("squashfs-root");
            if !is_real_dir(&root) {
                return Err("self-extraction produced no squashfs-root".to_string());
            }
            let desktop_file = find_desktop_in(&root)
                .ok_or_else(|| "no desktop entry in the extracted tree".to_string())?;
            let bytes = safe_fs::read_bounded(&desktop_file, limits::MAX_DESKTOP_FILE_BYTES as u64)
                .map_err(|_| "desktop entry exceeds size bound".to_string())?;
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
            // unsquashfs reads from byte 0 unless told where the squashfs starts.
            // A type-1 image is an ISO that begins at byte 0, so 7zz takes none.
            let offset = if tool == "unsquashfs" {
                Some(squashfs_offset(path, info)?)
            } else {
                None
            };
            let entries = self.list_archive(tool, path, offset)?;
            let desktop_member = pick_desktop_member(&entries)
                .ok_or_else(|| "No desktop entry found in AppImage".to_string())?;
            let staged = self.extract_members(
                tool,
                path,
                &work,
                &[desktop_member.clone(), ".DirIcon".to_string()],
                offset,
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
                        let got = self.extract_members(
                            tool,
                            path,
                            &work,
                            std::slice::from_ref(&member),
                            offset,
                        )?;
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
        offset: Option<u64>,
    ) -> Result<Vec<crate::types::ArchiveEntry>, String> {
        let (program, args) = match tool {
            "unsquashfs" => {
                let offset = offset.ok_or("unsquashfs needs the payload offset")?;
                (
                    "unsquashfs".to_string(),
                    vec![
                        "-o".to_string(),
                        offset.to_string(),
                        "-l".to_string(),
                        safe_fs::argv_safe_path(path),
                    ],
                )
            }
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
        offset: Option<u64>,
    ) -> Result<std::collections::BTreeMap<String, Vec<u8>>, String> {
        if members.len() > limits::MAX_EXTRACTED_FILES {
            return Err("Too many files requested".to_string());
        }
        for member in members {
            safe_fs::valid_archive_member(member)
                .map_err(|e| format!("Refusing unsafe archive path: {e}"))?;
        }
        let (program, mut args) = match tool {
            "unsquashfs" => {
                let offset = offset.ok_or("unsquashfs needs the payload offset")?;
                (
                    "unsquashfs".to_string(),
                    vec![
                        "-o".to_string(),
                        offset.to_string(),
                        // Metadata needs no xattrs. Writing them (SELinux labels,
                        // say) fails for a normal user and exits non-zero after the
                        // file is written, which would hide the metadata.
                        "-no-xattrs".to_string(),
                        "-q".to_string(),
                        "-d".to_string(),
                        dest.to_string_lossy().into_owned(),
                        "-f".to_string(),
                        safe_fs::argv_safe_path(path),
                    ],
                )
            }
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
                // A member that would overrun the output budget is refused before
                // it is read into memory. The read is bounded too, in case the
                // file grows between the check and the read.
                let budget = limits::MAX_EXTRACTED_BYTES - total;
                if meta.len() > budget {
                    return Err("Extraction exceeds output bound".to_string());
                }
                let bytes = safe_fs::read_bounded(&entry.path(), budget)?;
                total += bytes.len() as u64;
                if let Ok(rel) = entry.path().strip_prefix(dest) {
                    collected.insert(rel.to_string_lossy().into_owned(), bytes);
                }
            }
        }
        Ok(collected)
    }
}

/// The error a cancelled inspection reports. The UI shows it in place of the
/// file's facts, so it says what happened.
const CANCELLED_MESSAGE: &str = "The inspection was cancelled.";

/// Whether the operation running this inspection has been cancelled.
fn cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::Relaxed)
}

/// What a cancelled inspection returns. It is never a valid AppImage: the file was
/// not read to the end, so what was read so far is dropped, along with any staged
/// icon. Only the file's identity, its existing-registration facts and the error
/// remain.
fn stopped(result: InspectionResult) -> InspectionResult {
    result.discard_staging();
    InspectionResult {
        identity: result.identity,
        already_managed: result.already_managed,
        existing_managed_id: result.existing_managed_id,
        error: CANCELLED_MESSAGE.to_string(),
        ..Default::default()
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

/// Where the squashfs payload starts, checked against the file's length.
///
/// The offset comes from the ELF header, which is untrusted. An offset at or
/// past the end of the file means the header does not describe a payload, so
/// nothing is handed to the extractor.
fn squashfs_offset(path: &Path, info: &elf::ElfInfo) -> Result<u64, String> {
    let size = fs::metadata(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?
        .len();
    u64::try_from(info.payload_offset)
        .ok()
        .filter(|offset| *offset < size)
        .ok_or_else(|| {
            format!(
                "The ELF header places the payload at byte {} but the file is {size} bytes: \
                 the payload is outside the file, so its archive was not read",
                info.payload_offset
            )
        })
}

/// Whether `path` is a regular file itself. A symlink is not: it is never followed.
fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file())
}

/// Whether `path` is a real directory, not a symlink to one.
fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

/// The first top-level `*.desktop` regular file, in name order, of an extracted tree.
fn find_desktop_in(root: &Path) -> Option<std::path::PathBuf> {
    let mut names: Vec<std::path::PathBuf> = fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "desktop"))
        .collect();
    names.sort();
    names.into_iter().find(|p| is_regular_file(p))
}

/// The referenced icon, or `.DirIcon`, as a top-level regular file of an extracted tree.
fn find_icon_in(root: &Path, icon_name: &str) -> Option<(std::path::PathBuf, String)> {
    if !icon_name.is_empty() {
        for ext in ["png", "svg", "xpm"] {
            let candidate = root.join(format!("{icon_name}.{ext}"));
            if is_regular_file(&candidate) {
                return Some((candidate, ext.to_string()));
            }
        }
    }
    let dir_icon = root.join(".DirIcon");
    if is_regular_file(&dir_icon) {
        let bytes = safe_fs::read_bounded(&dir_icon, limits::MAX_ICON_BYTES).ok()?;
        return Some((dir_icon, sniff_icon_extension(&bytes)));
    }
    None
}

/// How deep the tree a self-extraction writes may nest.
const MAX_TREE_DEPTH: usize = 16;

/// How many files the tree a self-extraction writes may hold.
const MAX_SELF_EXTRACTED_FILES: usize = 100_000;

/// Check an extracted tree before anything in it is read. Every entry must stay
/// inside the tree: a symlink's target must be relative and must not climb out
/// of it, and devices, FIFOs and sockets are refused. The tree is also bounded in
/// nesting, files and bytes. Nothing here follows a symlink.
fn verify_extracted_tree(root: &Path, max_bytes: u64) -> Result<(), String> {
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut files = 0usize;
    let mut bytes = 0u64;
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_TREE_DEPTH {
            return Err("the extracted tree is nested too deeply".to_string());
        }
        let entries =
            fs::read_dir(&dir).map_err(|e| format!("cannot read the extracted tree: {e}"))?;
        for entry in entries {
            let path = entry
                .map_err(|e| format!("cannot read the extracted tree: {e}"))?
                .path();
            let meta = fs::symlink_metadata(&path)
                .map_err(|e| format!("cannot read the extracted tree: {e}"))?;
            let kind = meta.file_type();
            if kind.is_symlink() {
                let target = fs::read_link(&path)
                    .map_err(|e| format!("cannot read a symlink in the extracted tree: {e}"))?;
                if !link_stays_inside(&dir, &target, root) {
                    return Err(
                        "the extracted tree has a symlink that points outside it".to_string()
                    );
                }
            } else if kind.is_dir() {
                stack.push((path, depth + 1));
            } else if kind.is_file() {
                files += 1;
                bytes = bytes.saturating_add(meta.len());
                if files > MAX_SELF_EXTRACTED_FILES {
                    return Err("the extracted tree holds too many files".to_string());
                }
                if bytes > max_bytes {
                    return Err("the extracted tree exceeds the size bound".to_string());
                }
            } else {
                return Err("the extracted tree holds a device or special file".to_string());
            }
        }
    }
    Ok(())
}

/// Whether a symlink in `link_dir` with this target stays inside `root`. The
/// target must be relative, and walking its components must never climb above
/// `root`. Every link is checked on its own, so a chain of links cannot leave.
fn link_stays_inside(link_dir: &Path, target: &Path, root: &Path) -> bool {
    use std::path::Component;
    if target.is_absolute() {
        return false;
    }
    let Ok(below_root) = link_dir.strip_prefix(root) else {
        return false;
    };
    // How many directories below `root` the walk currently stands in.
    let mut depth = below_root.components().count() as i64;
    for component in target.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// Remove leftover work and icon-staging directories untouched for over an hour.
///
/// "Untouched" is judged by the newer of mtime and ctime. The archive's own
/// timestamp can be ancient, so mtime alone would sweep a live directory.
///
/// Best effort and non-fatal: anything still in use by a concurrent
/// inspection is younger than the threshold, and a directory we cannot read
/// or remove is simply left alone.
fn sweep_stale_staging(parent: &Path) {
    use std::os::unix::fs::MetadataExt;
    const STALE_AFTER_SECS: i64 = 3600;
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    let now = crate::tasks::now_unix();
    for entry in entries.flatten().take(4096) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("inspect-") || name.starts_with("icon-")) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        // The most recent of the two times. mtime alone is not enough: unsquashfs
        // gives an extracted directory the archive's own timestamp, which may be
        // decades old, while its ctime is when the directory last changed.
        let last_change = meta.mtime().max(meta.ctime());
        if now - last_change > STALE_AFTER_SECS {
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

#[cfg(test)]
mod staging_sweep_tests {
    use super::*;

    /// unsquashfs gives the directory it extracts into the archive's own timestamp.
    /// An archive built with reproducible timestamps carries the epoch, so the live
    /// work directory of a running inspection looks decades old. The sweep that runs
    /// for every other inspection must not take it, or that inspection loses its files.
    #[test]
    fn live_work_is_not_swept_when_the_archive_gave_it_an_old_mtime() {
        let parent = tempfile::tempdir().unwrap();
        let live = parent.path().join("inspect-live-0");
        fs::create_dir(&live).unwrap();
        fs::File::open(&live)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH)
            .unwrap();

        sweep_stale_staging(parent.path());

        assert!(live.is_dir(), "a live work directory was swept");
    }
}

#[cfg(test)]
mod unsafe_fallback_tests {
    use super::*;
    use crate::process::SystemRunner;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// These tests start real processes. One at a time keeps the thread
    /// and process count of this binary small on a busy host.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A test executable that stands in for an AppImage. It is a shell script,
    /// never a real AppImage, and it answers --appimage-extract the way the
    /// runtime does: by writing squashfs-root/ into its working directory.
    fn stand_in(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("Fake.AppImage");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    fn extract(path: &Path, timeout_ms: u64) -> Result<AppImageMetadata, String> {
        let runner = SystemRunner::new();
        let inspector = AppImageInspector::new(&runner);
        inspector.extract_via_appimage(path, 64 * 1024 * 1024, timeout_ms, &AtomicBool::new(false))
    }

    fn discard_icon(meta: &AppImageMetadata) {
        if let Some(stage) = Path::new(&meta.extracted_icon_path).parent() {
            let _ = safe_fs::remove_dir_no_follow(stage);
        }
    }

    /// Whether the process is gone, or has exited and is only waiting to be reaped.
    fn gone_within(pid: i32, within: Duration) -> bool {
        let stat = PathBuf::from(format!("/proc/{pid}/stat"));
        let deadline = Instant::now() + within;
        loop {
            match fs::read_to_string(&stat) {
                Err(_) => return true,
                Ok(text) => {
                    // The state letter follows the parenthesised command name.
                    let state = text
                        .rsplit(')')
                        .next()
                        .unwrap_or("")
                        .trim_start()
                        .chars()
                        .next();
                    if matches!(state, Some('Z') | Some('X')) {
                        return true;
                    }
                }
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn self_extraction_reads_the_entries_it_writes() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let path = stand_in(
            dir.path(),
            r#"[ "$1" = "--appimage-extract" ] || exit 2
mkdir -p squashfs-root
printf '[Desktop Entry]\nName=Unpacked\nIcon=demo\nX-AppImage-Version=9.9\n' > squashfs-root/demo.desktop
printf 'PNGDATA' > squashfs-root/demo.png"#,
        );

        let meta = extract(&path, 10_000).expect("the entries are read");
        assert_eq!(meta.name, "Unpacked");
        assert_eq!(meta.version, "9.9");
        assert_eq!(meta.icon_format, "png");
        assert_eq!(fs::read(&meta.extracted_icon_path).unwrap(), b"PNGDATA");
        discard_icon(&meta);
    }

    #[test]
    fn self_extraction_is_killed_at_the_timeout() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let path = stand_in(dir.path(), "exec sleep 30");

        let started = Instant::now();
        let outcome = extract(&path, 300);
        assert!(outcome.is_err(), "a run past its timeout must fail");
        assert!(outcome.unwrap_err().contains("timed out"));
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the timeout was not enforced"
        );
    }

    /// A timeout stops what the executable started as well, not only the
    /// executable itself.
    #[test]
    fn self_extraction_kills_the_children_it_started_at_the_timeout() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("child.pid");
        let body = format!("sleep 30 &\necho $! > '{}'\nwait", pidfile.display());
        let path = stand_in(dir.path(), &body);

        assert!(extract(&path, 1_000).is_err());
        let pid: i32 = fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(
            gone_within(pid, Duration::from_secs(3)),
            "the child {pid} outlived the timeout"
        );
    }

    /// A symlink in the extracted tree is never followed: a desktop entry that
    /// points outside the private directory is not read.
    #[test]
    fn self_extraction_does_not_read_a_desktop_entry_that_is_a_symlink() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.desktop");
        fs::write(&outside, "[Desktop Entry]\nName=Escaped\n").unwrap();
        let body = format!(
            "mkdir -p squashfs-root\nln -s '{}' squashfs-root/evil.desktop",
            outside.display()
        );
        let path = stand_in(dir.path(), &body);

        let outcome = extract(&path, 10_000);
        assert!(
            outcome.is_err(),
            "a symlink was followed: {:?}",
            outcome.map(|m| m.name)
        );
    }

    /// The executable gets a minimal environment: nothing from the manager's own
    /// environment leaks into it.
    #[test]
    fn self_extraction_runs_in_a_minimal_environment() {
        let _serial = serial();
        std::env::set_var("GOSH_FALLBACK_PROBE", "leaked");
        let dir = tempfile::tempdir().unwrap();
        let path = stand_in(
            dir.path(),
            r#"mkdir -p squashfs-root
{ printf '[Desktop Entry]\nName=Env\nComment='; env | tr '\n' ' '; printf '\n'; } > squashfs-root/env.desktop"#,
        );

        // The assertion messages name no variable values: a failure must not print
        // the environment, which holds session secrets.
        let meta = extract(&path, 10_000).expect("the entries are read");
        assert!(
            !meta.comment.contains("GOSH_FALLBACK_PROBE"),
            "the manager's environment leaked into the executable"
        );
        assert!(
            meta.comment.contains("PATH="),
            "the executable printed no environment"
        );
    }

    /// The executable gets no standard input: a read sees end of file at once.
    #[test]
    fn self_extraction_gets_no_standard_input() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let path = stand_in(
            dir.path(),
            r#"mkdir -p squashfs-root
if read -r line; then state=open; else state=closed; fi
printf '[Desktop Entry]\nName=%s\n' "$state" > squashfs-root/stdin.desktop"#,
        );

        let started = Instant::now();
        let meta = extract(&path, 10_000).expect("the entries are read");
        assert_eq!(meta.name, "closed");
        assert!(started.elapsed() < Duration::from_secs(8));
    }

    /// The whole extracted tree is checked before anything in it is read. A symlink
    /// that leaves the tree is refused, even when nothing reads it.
    #[test]
    fn self_extraction_refuses_a_symlink_that_leaves_the_tree() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let path = stand_in(
            dir.path(),
            r#"mkdir -p squashfs-root
printf '[Desktop Entry]\nName=Fine\n' > squashfs-root/app.desktop
ln -s ../../../../etc/passwd squashfs-root/escape"#,
        );
        let outcome = extract(&path, 10_000);
        assert!(outcome.is_err(), "a symlink left the tree and was accepted");
        assert!(outcome.unwrap_err().contains("outside"));
    }

    /// A symlink that stays inside the tree is fine: it is an ordinary way for an
    /// AppImage to name its icon.
    #[test]
    fn self_extraction_accepts_a_symlink_that_stays_inside_the_tree() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let path = stand_in(
            dir.path(),
            r#"mkdir -p squashfs-root
printf '[Desktop Entry]\nName=Linked\nIcon=demo\n' > squashfs-root/app.desktop
printf 'PNGDATA' > squashfs-root/demo.png
ln -s demo.png squashfs-root/.DirIcon"#,
        );
        let meta = extract(&path, 10_000).expect("an inside symlink is allowed");
        assert_eq!(meta.name, "Linked");
        discard_icon(&meta);
    }

    /// Only regular files, directories and inside symlinks belong in the tree. A
    /// FIFO is refused.
    #[test]
    fn self_extraction_refuses_a_special_file_in_the_tree() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let path = stand_in(
            dir.path(),
            r#"mkdir -p squashfs-root
printf '[Desktop Entry]\nName=Fine\n' > squashfs-root/app.desktop
mkfifo squashfs-root/pipe"#,
        );
        let outcome = extract(&path, 10_000);
        assert!(outcome.is_err(), "a FIFO in the tree was accepted");
    }

    /// Link targets are checked by their components: relative, and never climbing
    /// above the tree root.
    #[test]
    fn link_targets_are_checked_inside_the_tree() {
        let root = Path::new("/w/squashfs-root");
        assert!(link_stays_inside(root, Path::new("demo.png"), root));
        assert!(link_stays_inside(root, Path::new("a/../demo.png"), root));
        assert!(link_stays_inside(
            &root.join("a"),
            Path::new("../demo.png"),
            root
        ));
        assert!(!link_stays_inside(root, Path::new("../escape"), root));
        assert!(!link_stays_inside(
            &root.join("a"),
            Path::new("../../escape"),
            root
        ));
        assert!(!link_stays_inside(root, Path::new("/etc/passwd"), root));
        assert!(!link_stays_inside(
            Path::new("/elsewhere"),
            Path::new("x"),
            root
        ));
    }
}
