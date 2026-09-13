// Gosh AppImage Manager — library discovery/adoption (ports AppImageLibrary).
// External (unmanaged) AppImages are reported for explicit adoption;
// adoption never rewrites or deletes anything by itself.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::desktop;
use crate::limits;
use crate::registry::ManagedRegistry;
use crate::settings::SettingsStore;
use crate::types::InstalledApp;

/// Where a discovered AppImage was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Sitting in the configured managed folder.
    ManagedFolder,
    /// Referenced by a desktop entry outside the managed folder — a manual
    /// integration, or one made by another tool.
    ExternalDesktopEntry,
}

#[derive(Debug, Clone)]
pub struct DiscoveredApp {
    pub path: String,
    pub name: String,
    pub managed: bool,
    pub uuid: String,
    pub origin: Origin,
    /// The desktop entry that referenced it, for external finds.
    pub desktop_path: String,
}

pub struct AppImageLibrary<'a> {
    settings: &'a SettingsStore,
}

impl<'a> AppImageLibrary<'a> {
    pub fn new(settings: &'a SettingsStore) -> Self {
        Self { settings }
    }

    /// Scan for AppImages and mark the ones the registry already knows.
    ///
    /// Always covers the managed folder. When `manage_outside_folder` is on,
    /// it also reads the user's desktop entries and reports AppImages living
    /// elsewhere -- manual integrations and ones made by other tools -- so
    /// they can be offered for explicit adoption. That setting was persisted
    /// and read by nothing before; external discovery did not exist.
    pub fn scan(&self, registry: &ManagedRegistry) -> Vec<DiscoveredApp> {
        let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
        let mut out = Vec::new();

        for path in self.managed_folder_files() {
            if !seen.insert(path.clone()) {
                continue;
            }
            out.push(self.describe(registry, path, Origin::ManagedFolder, String::new()));
        }

        if self.settings.manage_outside_folder() {
            let managed_dir = self.settings.managed_folder().to_path_buf();
            for (path, desktop_path) in self.external_entries() {
                // Anything already inside the managed folder was covered above.
                if path.starts_with(&managed_dir) || !seen.insert(path.clone()) {
                    continue;
                }
                out.push(self.describe(registry, path, Origin::ExternalDesktopEntry, desktop_path));
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn describe(
        &self,
        registry: &ManagedRegistry,
        path: PathBuf,
        origin: Origin,
        desktop_path: String,
    ) -> DiscoveredApp {
        let text = path.to_string_lossy().into_owned();
        match registry.by_path(&text) {
            Some(app) => DiscoveredApp {
                path: text,
                name: app.name.clone(),
                managed: true,
                uuid: app.uuid.clone(),
                origin,
                desktop_path,
            },
            None => DiscoveredApp {
                name: path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "AppImage".to_string()),
                path: text,
                managed: false,
                uuid: String::new(),
                origin,
                desktop_path,
            },
        }
    }

    fn managed_folder_files(&self) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(self.settings.managed_folder()) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_appimage_file(p))
            .collect()
    }

    /// AppImage paths referenced by desktop entries in the user's
    /// applications directory, paired with the entry that named them.
    ///
    /// Entries we own are skipped: those are already in the registry, and
    /// rediscovering them as "external" would invite adopting them twice.
    fn external_entries(&self) -> Vec<(PathBuf, String)> {
        let dir = self.settings.applications_dir();
        let Ok(entries) = fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten().take(limits::MAX_ARCHIVE_LISTING_ENTRIES) {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "desktop") {
                continue;
            }
            if desktop::verify_ownership(&path).owned {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else { continue };
            let Ok(parsed) = desktop::parse_desktop_bytes(&bytes) else {
                continue;
            };
            // TryExec names the binary directly; Exec needs its first token.
            let candidate = {
                let try_exec = parsed.entry("TryExec");
                if try_exec.is_empty() {
                    first_exec_token(parsed.entry("Exec"))
                } else {
                    try_exec.to_string()
                }
            };
            if candidate.is_empty() {
                continue;
            }
            let target = PathBuf::from(&candidate);
            if target.is_absolute() && is_appimage_file(&target) {
                out.push((target, path.to_string_lossy().into_owned()));
            }
        }
        out
    }

    /// Explicit adoption of an external file: a registry row and nothing else.
    ///
    /// Adoption never rewrites or removes the existing desktop entry or icon;
    /// they are not ours, and the brief requires adoption to be inert.
    pub fn adopt(
        &self,
        registry: &mut ManagedRegistry,
        path: &str,
    ) -> Result<InstalledApp, String> {
        let file = Path::new(path);
        if registry.by_path(path).is_some() {
            return Err("Already managed".to_string());
        }
        if !is_appimage_file(file) {
            return Err(format!("Not a readable AppImage file: {path}"));
        }
        let canonical = crate::safe_fs::canonical_bounded(file)
            .map_err(|e| format!("Cannot resolve {path}: {e}"))?;
        if registry.by_path(&canonical.to_string_lossy()).is_some() {
            return Err("Already managed".to_string());
        }
        let name = canonical
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "AppImage".to_string());
        let external = !canonical.starts_with(self.settings.managed_folder());
        registry.adopt_external(name, canonical.to_string_lossy().into_owned(), external)
    }
}

/// A regular file (following at most a bounded symlink chain) named
/// `*.AppImage`. Extension alone is never enough to act on, but it is the
/// right filter for *discovery*; the inspector validates magic before
/// anything is done with the file.
fn is_appimage_file(path: &Path) -> bool {
    if path
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("appimage"))
    {
        return false;
    }
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

/// First whitespace-separated token of an Exec line, unquoted.
fn first_exec_token(exec: &str) -> String {
    let trimmed = exec.trim();
    if let Some(rest) = trimmed.strip_prefix('"') {
        return rest.split('"').next().unwrap_or_default().to_string();
    }
    trimmed
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
}
