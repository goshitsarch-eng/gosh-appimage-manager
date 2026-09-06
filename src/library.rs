// Gosh AppImage Manager — library discovery/adoption (ports AppImageLibrary).
// External (unmanaged) AppImages are reported for explicit adoption;
// adoption never rewrites or deletes anything by itself.

use std::fs;

use crate::registry::ManagedRegistry;
use crate::settings::SettingsStore;
use crate::types::InstalledApp;

#[derive(Debug, Clone)]
pub struct DiscoveredApp {
    pub path: String,
    pub name: String,
    pub managed: bool,
    pub uuid: String,
}

pub struct AppImageLibrary<'a> {
    settings: &'a SettingsStore,
}

impl<'a> AppImageLibrary<'a> {
    pub fn new(settings: &'a SettingsStore) -> Self {
        Self { settings }
    }

    /// Scan the managed folder for AppImage files and mark registry members.
    pub fn scan(&self, registry: &ManagedRegistry) -> Vec<DiscoveredApp> {
        let mut out = Vec::new();
        let dir = self.settings.managed_folder();
        let Ok(entries) = fs::read_dir(dir) else {
            return out;
        };
        let mut paths: Vec<String> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .map(|e| e.eq_ignore_ascii_case("appimage"))
                        .unwrap_or(false)
            })
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        paths.sort();
        for path in paths {
            match registry.by_path(&path) {
                Some(app) => out.push(DiscoveredApp {
                    path,
                    name: app.name.clone(),
                    managed: true,
                    uuid: app.uuid.clone(),
                }),
                None => out.push(DiscoveredApp {
                    name: Path::new(&path)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "AppImage".to_string()),
                    path,
                    managed: false,
                    uuid: String::new(),
                }),
            }
        }
        out
    }

    /// Explicit adoption of an external file: registry row only.
    pub fn adopt(
        &self,
        registry: &mut ManagedRegistry,
        path: &str,
    ) -> Result<InstalledApp, String> {
        if registry.by_path(path).is_some() {
            return Err("Already managed".to_string());
        }
        let name = Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "AppImage".to_string());
        registry.adopt_external(name, path.to_string())
    }
}

use std::path::Path;
