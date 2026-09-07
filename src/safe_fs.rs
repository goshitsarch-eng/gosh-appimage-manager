// Gosh AppImage Manager — bounded, atomic filesystem primitives.
// Ports SafeFs. Every mutation stages to a sibling temp file and renames,
// so crashes leave either the old or the new file, never a half-write.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::limits;

/// Create a directory (and parents) with mode 0700.
///
/// An existing path is accepted only if it is a real directory we own and no
/// one else can write to. `dir.exists()` alone was not a check: it follows
/// symlinks and says nothing about ownership or mode, so on a shared `/tmp` a
/// local attacker could pre-create the predictable extraction parent
/// (`$TMPDIR/gosh-appimage-manager`) as a symlink to a directory of their
/// choosing, or as mode 0777, and decide where our output landed.
pub fn mkdir_0700(dir: &Path) -> Result<(), String> {
    match fs::symlink_metadata(dir) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(format!(
                    "Refusing to use {}: it is a symlink",
                    dir.display()
                ));
            }
            if !meta.is_dir() {
                return Err(format!(
                    "Refusing to use {}: it is not a directory",
                    dir.display()
                ));
            }
            #[cfg(unix)]
            {
                let uid = unsafe { libc_geteuid() };
                if meta.uid() != uid {
                    return Err(format!(
                        "Refusing to use {}: it is owned by another user",
                        dir.display()
                    ));
                }
                // Group- or world-writable means someone else can swap what is
                // inside it after we have checked.
                if meta.permissions().mode() & 0o022 != 0 {
                    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                        .map_err(|e| format!("Cannot secure directory {}: {e}", dir.display()))?;
                }
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(|e| format!("Cannot create directory {}: {e}", dir.display()))?;
            Ok(())
        }
        Err(e) => Err(format!("Cannot create directory {}: {e}", dir.display())),
    }
}

#[cfg(unix)]
unsafe fn libc_geteuid() -> u32 {
    extern "C" {
        fn geteuid() -> u32;
    }
    geteuid()
}

/// Create a private temp directory (mode 0700) under `parent`.
pub fn private_temp_dir(parent: &Path, prefix: &str) -> Result<PathBuf, String> {
    mkdir_0700(parent)?;
    for attempt in 0..1000 {
        let name = format!("{}{}-{}-{}", prefix, std::process::id(), nanos(), attempt);
        let dir = parent.join(name);
        match fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Cannot create temp dir: {e}")),
        }
    }
    Err("Cannot create temp dir: too many collisions".to_string())
}

fn nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// Sibling temp path next to `final_path` (same filesystem, safe rename).
pub fn sibling_temp(final_path: &Path, prefix: &str) -> PathBuf {
    let parent = final_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = final_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    parent.join(format!(
        "{}{}-{}-{}",
        prefix,
        stem,
        std::process::id(),
        nanos() % 1_000_000
    ))
}

/// Copy with a byte bound and optional cancellation flag.
/// Returns the number of bytes copied.
pub fn copy_bounded(
    src: &Path,
    dst: &Path,
    max_bytes: u64,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<u64, String> {
    let mut reader = File::open(src).map_err(|e| format!("Cannot read {}: {e}", src.display()))?;
    let mut writer = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(dst)
        .map_err(|e| format!("Cannot write {}: {e}", dst.display()))?;
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = fs::remove_file(dst);
            return Err("Cancelled".to_string());
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Cannot read {}: {e}", src.display()))?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        if total > max_bytes {
            let _ = fs::remove_file(dst);
            return Err(format!(
                "File exceeds configured size bound ({max_bytes} bytes)"
            ));
        }
        writer
            .write_all(&buf[..n])
            .map_err(|e| format!("Cannot write {}: {e}", dst.display()))?;
    }
    writer
        .flush()
        .map_err(|e| format!("Cannot write {}: {e}", dst.display()))?;
    Ok(total)
}

/// Atomically write `bytes` to `path` with `mode` (stage + rename).
pub fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create directory {}: {e}", parent.display()))?;
        }
    }
    let staging = sibling_temp(path, ".gosh-stage-");
    let mut writer = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&staging)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    writer
        .write_all(bytes)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    writer
        .flush()
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    drop(writer);
    let _ = fs::set_permissions(&staging, fs::Permissions::from_mode(mode));
    fs::rename(&staging, path).map_err(|e| format!("Cannot replace {}: {e}", path.display()))?;
    Ok(())
}

/// Rename over an existing destination (commit step).
pub fn rename_over(src: &Path, dst: &Path) -> Result<(), String> {
    fs::rename(src, dst).map_err(|e| format!("Cannot replace {}: {e}", dst.display()))
}

/// Rename only when the destination does not exist (race-safe commit).
pub fn rename_no_replace(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        return Err(format!(
            "Destination appeared before commit; refusing to overwrite ({})",
            dst.display()
        ));
    }
    // Best-effort atomicity: link then unlink avoids overwriting on unix.
    match fs::hard_link(src, dst) {
        Ok(()) => {
            fs::remove_file(src).map_err(|e| format!("Cannot replace {}: {e}", dst.display()))
        }
        Err(_) => {
            // Fall back to a checked rename (destination re-checked above).
            if dst.exists() {
                return Err(format!(
                    "Destination appeared before commit; refusing to overwrite ({})",
                    dst.display()
                ));
            }
            fs::rename(src, dst).map_err(|e| format!("Cannot replace {}: {e}", dst.display()))
        }
    }
}

/// Make `backup` a rollback copy of `live`, cheaply where possible.
///
/// A hard link is the same bytes under a second name: the original stays
/// reachable through `backup` even after `live` is renamed over, which is
/// exactly what rollback needs, and it costs no space and no I/O regardless
/// of how large the AppImage is. Copying a multi-gigabyte file to make a
/// backup that is usually discarded seconds later is pure waste.
///
/// Falls back to a real copy when linking is refused -- a filesystem without
/// hard links, or a destination on a different device.
pub fn backup_copy(live: &Path, backup: &Path) -> Result<(), String> {
    if fs::hard_link(live, backup).is_ok() {
        return Ok(());
    }
    fs::copy(live, backup)
        .map(|_| ())
        .map_err(|e| format!("Cannot create backup of {}: {e}", live.display()))
}

/// Remove all `.{prefix}*` leftovers in `dir` (rollback), deepest first.
pub fn rollback_temps(dir: &Path, prefix: &str) -> Vec<String> {
    let mut removed = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return removed;
    };
    let mut names: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with(prefix))
                .unwrap_or(false)
        })
        .collect();
    names.sort();
    names.reverse();
    for path in names {
        if path.is_dir() {
            let _ = remove_dir_no_follow(&path);
        } else {
            let _ = fs::remove_file(&path);
        }
        removed.push(path.to_string_lossy().into_owned());
    }
    removed
}

/// Remove a directory tree without following symlinks.
pub fn remove_dir_no_follow(dir: &Path) -> Result<(), String> {
    let mut stack = vec![dir.to_path_buf()];
    let mut dirs = Vec::new();
    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current)
            .map_err(|e| format!("Cannot clean {}: {e}", current.display()))?;
        dirs.push(current.clone());
        for entry in entries {
            let entry = entry.map_err(|e| format!("Cannot clean dir: {e}"))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|e| format!("Cannot clean dir: {e}"))?;
            if kind.is_dir() && !kind.is_symlink() {
                stack.push(path);
            } else {
                let _ = fs::remove_file(&path);
            }
        }
    }
    for dir in dirs.iter().rev() {
        let _ = fs::remove_dir(dir);
    }
    Ok(())
}

/// Streaming SHA-256 with periodic cancellation checks.
pub fn sha256_file(path: &Path, cancel: &std::sync::atomic::AtomicBool) -> Result<Vec<u8>, String> {
    use sha2::Digest;
    let mut file = File::open(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut since_check: u64 = 0;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        since_check += n as u64;
        if since_check >= limits::HASH_CANCEL_CHECK_BYTES {
            since_check = 0;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("Cancelled".to_string());
            }
        }
    }
    Ok(hasher.finalize().to_vec())
}

/// Render `path` safe to pass as a positional argument to an external tool.
///
/// A relative path beginning with `-` (`./-x.AppImage` opened as `-x.AppImage`)
/// would be read as a switch by every extractor we shell out to. Prefixing
/// `./` keeps the path meaning exactly the same file while making it
/// unambiguously positional. Absolute paths already cannot be confused.
pub fn argv_safe_path(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    if text.starts_with('-') {
        format!("./{text}")
    } else {
        text
    }
}

/// Resolve symlinks up to a hop limit; fails closed on loops/escape.
pub fn canonical_bounded(path: &Path) -> Result<PathBuf, String> {
    let mut current = path.to_path_buf();
    for _ in 0..limits::SYMLINK_HOP_LIMIT {
        let kind = fs::symlink_metadata(&current)
            .map_err(|e| format!("Cannot stat {}: {e}", current.display()))?;
        if !kind.file_type().is_symlink() {
            return Ok(current);
        }
        let target = fs::read_link(&current)
            .map_err(|e| format!("Cannot resolve symlink {}: {e}", current.display()))?;
        current = if target.is_absolute() {
            target
        } else {
            match current.parent() {
                Some(parent) => parent.join(&target),
                None => target,
            }
        };
    }
    Err(format!("Too many symlink levels: {}", path.display()))
}

/// Reject archive member paths: absolute, `..`, overlong, device-ish, or
/// anything that an extractor would read as a command-line switch.
pub fn valid_archive_member(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("Empty archive path".to_string());
    }
    if path.len() > limits::MAX_PATH_LENGTH * 4 {
        return Err("Archive path too long".to_string());
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(format!("Archive path is absolute: {path}"));
    }
    // Member names are appended to the extractor's argv as positional
    // arguments. `unsquashfs` has no `--` end-of-options terminator, so a
    // member called `-o/somewhere` or `-x` would be parsed as a switch —
    // with 7-Zip, `-o` redirects extraction out of the private temp dir
    // entirely. Names are attacker-controlled (they come from the archive's
    // own listing), so refuse the shape rather than trust the tool.
    if path.starts_with('-') {
        return Err(format!("Archive path looks like an option: {path}"));
    }
    let mut depth: i32 = 0;
    for part in path.split('/').flat_map(|s| s.split('\\')) {
        if part == ".." {
            return Err(format!("Archive path escapes: {path}"));
        }
        if part.is_empty() || part == "." {
            continue;
        }
        depth += 1;
        if depth as usize > limits::MAX_ARCHIVE_DEPTH * 16 {
            return Err("Archive nesting too deep".to_string());
        }
    }
    Ok(())
}
