// Gosh AppImage Manager — XDG-aware directory layout + settings store.
// Replaces KConfig with a plain JSON file; all paths honour test overrides
// (GOSHAIM_HOME / GOSHAIM_XDG_*_HOME) so tests never touch a real home.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::limits;
use crate::types::{appearance_from_str, appearance_name, Appearance};

/// Resolved base directories. Every path in the app derives from here.
#[derive(Debug, Clone)]
pub struct Dirs {
    pub home: PathBuf,
    pub data_home: PathBuf,
    pub config_home: PathBuf,
    pub cache_home: PathBuf,
}

impl Dirs {
    pub fn from_env() -> Self {
        let home = env_path("GOSHAIM_HOME")
            .or_else(|| env_path("HOME"))
            .unwrap_or_else(|| PathBuf::from("/root"));
        let data_home =
            env_path("GOSHAIM_XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
        let config_home =
            env_path("GOSHAIM_XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
        let cache_home = env_path("GOSHAIM_XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache"));
        Self {
            home,
            data_home,
            config_home,
            cache_home,
        }
    }

    /// Isolated dirs rooted at `home` (used by tests).
    pub fn under(home: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
            data_home: home.join(".local/share"),
            config_home: home.join(".config"),
            cache_home: home.join(".cache"),
        }
    }

    pub fn applications_dir(&self) -> PathBuf {
        self.data_home.join("applications")
    }

    pub fn icons_dir(&self) -> PathBuf {
        self.data_home.join("icons/hicolor")
    }

    pub fn app_data_dir(&self) -> PathBuf {
        self.data_home.join("gosh-appimage-manager")
    }

    pub fn autostart_dir(&self) -> PathBuf {
        self.config_home.join("autostart")
    }

    pub fn default_managed_folder(&self) -> PathBuf {
        self.home.join("AppImages")
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Persistent user settings (mirrors SettingsStore keys exactly).
#[derive(Debug, Clone)]
pub struct SettingsStore {
    dirs: Dirs,
    config_path: PathBuf,
    managed_folder: PathBuf,
    move_source: bool,
    manage_outside_folder: bool,
    terminal_omit_suffix: bool,
    background_update_checks: bool,
    unsafe_extraction_fallback: bool,
    appearance: Appearance,
    debug_logging: bool,
    max_appimage_bytes: i64,
    /// Set when an existing settings file could not be read or parsed.
    load_error: Option<String>,
    /// True once an existing file has been read successfully.
    loaded: bool,
}

impl SettingsStore {
    pub fn new(dirs: Dirs) -> Self {
        let config_path = dirs.config_home.join("gosh-appimage-manager/settings.json");
        let mut store = Self {
            dirs,
            config_path,
            managed_folder: PathBuf::new(),
            move_source: false,
            manage_outside_folder: false,
            terminal_omit_suffix: false,
            background_update_checks: false,
            unsafe_extraction_fallback: false,
            appearance: Appearance::System,
            debug_logging: false,
            max_appimage_bytes: limits::DEFAULT_MAX_APPIMAGE_BYTES,
            load_error: None,
            loaded: false,
        };
        store.managed_folder = store.dirs.default_managed_folder();
        store.load();
        store
    }

    pub fn with_config_path(mut self, path: PathBuf) -> Self {
        self.config_path = path;
        self.load();
        self
    }

    pub fn dirs(&self) -> &Dirs {
        &self.dirs
    }

    pub fn managed_folder(&self) -> &Path {
        &self.managed_folder
    }

    pub fn set_managed_folder(&mut self, folder: PathBuf) -> Result<(), String> {
        self.managed_folder = folder;
        self.save()
    }

    pub fn move_source(&self) -> bool {
        self.move_source
    }

    pub fn set_move_source(&mut self, value: bool) -> Result<(), String> {
        self.move_source = value;
        self.save()
    }

    pub fn manage_outside_folder(&self) -> bool {
        self.manage_outside_folder
    }

    pub fn set_manage_outside_folder(&mut self, value: bool) -> Result<(), String> {
        self.manage_outside_folder = value;
        self.save()
    }

    pub fn terminal_omit_suffix(&self) -> bool {
        self.terminal_omit_suffix
    }

    pub fn set_terminal_omit_suffix(&mut self, value: bool) -> Result<(), String> {
        self.terminal_omit_suffix = value;
        self.save()
    }

    pub fn background_update_checks(&self) -> bool {
        self.background_update_checks
    }

    pub fn set_background_update_checks(&mut self, value: bool) -> Result<(), String> {
        self.background_update_checks = value;
        self.save()
    }

    /// Unsafe `--appimage-extract` fallback. Off by default, warned, and never
    /// honoured by tests or background flows.
    pub fn unsafe_extraction_fallback(&self) -> bool {
        self.unsafe_extraction_fallback
    }

    pub fn set_unsafe_extraction_fallback(&mut self, value: bool) -> Result<(), String> {
        self.unsafe_extraction_fallback = value;
        self.save()
    }

    pub fn appearance(&self) -> Appearance {
        self.appearance
    }

    pub fn set_appearance(&mut self, value: Appearance) -> Result<(), String> {
        self.appearance = value;
        self.save()
    }

    pub fn debug_logging(&self) -> bool {
        self.debug_logging
    }

    pub fn set_debug_logging(&mut self, value: bool) -> Result<(), String> {
        self.debug_logging = value;
        self.save()
    }

    pub fn max_appimage_bytes(&self) -> i64 {
        self.max_appimage_bytes
    }

    pub fn set_max_appimage_bytes(&mut self, value: i64) -> Result<(), String> {
        let clamped = limits::clamp_max_appimage_bytes(value);
        if clamped == self.max_appimage_bytes {
            return Ok(());
        }
        self.max_appimage_bytes = clamped;
        self.save()
    }

    pub fn applications_dir(&self) -> PathBuf {
        self.dirs.applications_dir()
    }

    pub fn icons_dir(&self) -> PathBuf {
        self.dirs.icons_dir()
    }

    pub fn data_dir(&self) -> PathBuf {
        self.dirs.app_data_dir()
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.dirs.cache_home.join("gosh-appimage-manager")
    }

    pub fn registry_path(&self) -> PathBuf {
        self.data_dir().join("registry.sqlite")
    }

    pub fn autostart_dir(&self) -> PathBuf {
        self.dirs.autostart_dir()
    }

    pub fn autostart_desktop_path(&self) -> PathBuf {
        self.autostart_dir()
            .join("com.goshapps.AppImageManager-updates.desktop")
    }

    fn load(&mut self) {
        let body = match fs::read(&self.config_path) {
            Ok(body) => body,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                // Unreadable is not the same as absent. Say so, and leave
                // `loaded` false so a save cannot quietly overwrite a file we
                // never managed to read.
                self.load_error = Some(format!(
                    "Cannot read {}: {e}. Using defaults; your settings were not changed.",
                    self.config_path.display()
                ));
                return;
            }
        };
        let map: BTreeMap<String, serde_json::Value> = match serde_json::from_slice(&body) {
            Ok(map) => map,
            Err(e) => {
                // A corrupt file used to reset every setting to its default in
                // silence, and the next change overwrote the original -- so a
                // stray byte destroyed the user's configuration with no notice.
                self.load_error = Some(format!(
                    "{} is not valid JSON ({e}). Using defaults; the existing file is left \
                     untouched until you change a setting.",
                    self.config_path.display()
                ));
                return;
            }
        };
        self.loaded = true;
        let str_entry = |key: &str| {
            map.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let bool_entry = |key: &str| map.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
        let managed = str_entry("ManagedFolder");
        if !managed.is_empty() {
            self.managed_folder = PathBuf::from(managed);
        }
        self.move_source = bool_entry("MoveSource");
        self.manage_outside_folder = bool_entry("ManageOutsideFolder");
        self.terminal_omit_suffix = bool_entry("TerminalOmitSuffix");
        self.background_update_checks = bool_entry("BackgroundUpdateChecks");
        self.unsafe_extraction_fallback = bool_entry("UnsafeExtractionFallback");
        self.appearance = appearance_from_str(&str_entry("Appearance"));
        self.debug_logging = bool_entry("DebugLogging");
        if let Some(max) = map.get("MaxAppImageBytes").and_then(|v| v.as_i64()) {
            self.max_appimage_bytes = limits::clamp_max_appimage_bytes(max);
        }
    }

    /// Persist the current settings.
    ///
    /// Returns the failure rather than discarding it: a setting that silently
    /// fails to save looks exactly like one that saved, and reappears at its
    /// old value on the next launch with no explanation.
    fn save(&self) -> Result<(), String> {
        let mut map = BTreeMap::new();
        map.insert(
            "ManagedFolder".to_string(),
            serde_json::Value::String(self.managed_folder.to_string_lossy().into_owned()),
        );
        for (key, value) in [
            ("MoveSource", self.move_source),
            ("ManageOutsideFolder", self.manage_outside_folder),
            ("TerminalOmitSuffix", self.terminal_omit_suffix),
            ("BackgroundUpdateChecks", self.background_update_checks),
            ("UnsafeExtractionFallback", self.unsafe_extraction_fallback),
            ("DebugLogging", self.debug_logging),
        ] {
            map.insert(key.to_string(), serde_json::Value::Bool(value));
        }
        map.insert(
            "Appearance".to_string(),
            serde_json::Value::String(appearance_name(self.appearance).to_string()),
        );
        map.insert(
            "MaxAppImageBytes".to_string(),
            serde_json::Value::Number(self.max_appimage_bytes.into()),
        );
        let body = serde_json::to_vec_pretty(&map)
            .map_err(|e| format!("Cannot serialise settings: {e}"))?;
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
        }
        crate::safe_fs::atomic_write(&self.config_path, &body, 0o600)
    }

    /// A problem reading the settings file at startup, if there was one.
    pub fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    /// Whether an existing settings file was read successfully. False when the
    /// file was absent, unreadable, or corrupt.
    pub fn loaded(&self) -> bool {
        self.loaded
    }
}
