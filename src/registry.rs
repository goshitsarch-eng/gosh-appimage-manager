// Gosh AppImage Manager — installed-app registry (ports ManagedRegistry).
// SQLite store (mode 0600) with snapshot/restore for transactional callers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use crate::limits;
use crate::types::{AppImageType, Architecture, DesktopAction, EnvPair, InstalledApp};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS apps(
    uuid TEXT PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    version TEXT NOT NULL DEFAULT '',
    comment TEXT NOT NULL DEFAULT '',
    managed_path TEXT NOT NULL DEFAULT '',
    desktop_id TEXT NOT NULL DEFAULT '',
    desktop_path TEXT NOT NULL DEFAULT '',
    icon_path TEXT NOT NULL DEFAULT '',
    sha256 TEXT NOT NULL DEFAULT '',
    app_type INTEGER NOT NULL DEFAULT 0,
    architecture INTEGER NOT NULL DEFAULT 0,
    size INTEGER NOT NULL DEFAULT 0,
    arguments TEXT NOT NULL DEFAULT '[]',
    default_arguments TEXT NOT NULL DEFAULT '[]',
    environment TEXT NOT NULL DEFAULT '[]',
    update_manager TEXT NOT NULL DEFAULT '',
    update_config TEXT NOT NULL DEFAULT '{}',
    embedded_update TEXT NOT NULL DEFAULT '',
    last_update_check TEXT NOT NULL DEFAULT '',
    available_version TEXT NOT NULL DEFAULT '',
    available_url TEXT NOT NULL DEFAULT '',
    available_size INTEGER NOT NULL DEFAULT 0,
    update_available INTEGER NOT NULL DEFAULT 0,
    digest TEXT NOT NULL DEFAULT '',
    reduced_verification INTEGER NOT NULL DEFAULT 0,
    external_folder INTEGER NOT NULL DEFAULT 0,
    owned INTEGER NOT NULL DEFAULT 1,
    adopted INTEGER NOT NULL DEFAULT 0,
    website TEXT NOT NULL DEFAULT '',
    terminal INTEGER NOT NULL DEFAULT 0,
    actions TEXT NOT NULL DEFAULT '[]'
);";

fn app_type_to_int(t: AppImageType) -> i64 {
    match t {
        AppImageType::Unknown => 0,
        AppImageType::Type1 => 1,
        AppImageType::Type2 => 2,
        AppImageType::Dwarfs => 3,
    }
}

fn int_to_app_type(v: i64) -> AppImageType {
    match v {
        1 => AppImageType::Type1,
        2 => AppImageType::Type2,
        3 => AppImageType::Dwarfs,
        _ => AppImageType::Unknown,
    }
}

fn arch_to_int(a: Architecture) -> i64 {
    match a {
        Architecture::Unknown => 0,
        Architecture::X86_64 => 1,
        Architecture::AArch64 => 2,
        Architecture::I386 => 3,
        Architecture::Arm => 4,
    }
}

fn int_to_arch(v: i64) -> Architecture {
    match v {
        1 => Architecture::X86_64,
        2 => Architecture::AArch64,
        3 => Architecture::I386,
        4 => Architecture::Arm,
        _ => Architecture::Unknown,
    }
}

fn parse_env_pairs(json: &str) -> Vec<EnvPair> {
    serde_json::from_str::<Vec<EnvPair>>(json).unwrap_or_default()
}

fn parse_actions(json: &str) -> Vec<DesktopAction> {
    serde_json::from_str::<Vec<DesktopAction>>(json).unwrap_or_default()
}

fn parse_map(json: &str) -> BTreeMap<String, String> {
    serde_json::from_str::<BTreeMap<String, String>>(json).unwrap_or_default()
}

fn parse_strings(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

fn legacy_str(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn legacy_app_from_json(item: &serde_json::Value) -> InstalledApp {
    let mut app = InstalledApp::new_owned();
    app.uuid = legacy_str(item, "uuid");
    app.name = legacy_str(item, "name");
    app.version = legacy_str(item, "version");
    app.comment = legacy_str(item, "comment");
    app.managed_path = legacy_str(item, "managed_path");
    app.desktop_id = legacy_str(item, "desktop_id");
    app.desktop_path = legacy_str(item, "desktop_path");
    app.icon_path = legacy_str(item, "icon_path");
    app.sha256 = hex::decode(legacy_str(item, "sha256")).unwrap_or_default();
    app.app_type = match legacy_str(item, "type").as_str() {
        "type-1" => AppImageType::Type1,
        "type-2" => AppImageType::Type2,
        "dwarfs" => AppImageType::Dwarfs,
        _ => AppImageType::Unknown,
    };
    app.architecture = match legacy_str(item, "architecture").as_str() {
        "x86_64" => Architecture::X86_64,
        "aarch64" => Architecture::AArch64,
        "i386" => Architecture::I386,
        "arm" => Architecture::Arm,
        _ => Architecture::Unknown,
    };
    app.size = item.get("size").and_then(|v| v.as_i64()).unwrap_or(0);
    for key in ["arguments", "default_arguments"] {
        let values: Vec<String> = item
            .get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if key == "arguments" {
            app.arguments = values;
        } else {
            app.default_arguments = values;
        }
    }
    if let Some(env) = item.get("environment").and_then(|v| v.as_object()) {
        for (name, value) in env {
            if let Some(text) = value.as_str() {
                app.environment.push(EnvPair {
                    name: name.clone(),
                    value: text.to_string(),
                });
            }
        }
    }
    app.update_manager = legacy_str(item, "update_manager");
    if let Some(config) = item.get("update_config").and_then(|v| v.as_object()) {
        for (key, value) in config {
            if let Some(text) = value.as_str() {
                app.update_config.insert(key.clone(), text.to_string());
            }
        }
    }
    app.embedded_update = legacy_str(item, "embedded_update");
    app.last_update_check = legacy_str(item, "last_update_check");
    app.available_version = legacy_str(item, "available_version");
    app.available_url = legacy_str(item, "available_url");
    app.available_size = item
        .get("available_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    app.update_available = item
        .get("update_available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    app.digest = legacy_str(item, "digest");
    app.reduced_verification = item
        .get("reduced_verification")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    app.external_folder = item
        .get("external_folder")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    app.owned = item.get("owned").and_then(|v| v.as_bool()).unwrap_or(true);
    app.adopted = item
        .get("adopted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    app.website = legacy_str(item, "website");
    app.terminal = item
        .get("terminal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    app
}

pub struct ManagedRegistry {
    path: PathBuf,
    apps: Vec<InstalledApp>,
}

impl ManagedRegistry {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create data dir: {e}"))?;
            }
        }
        let fresh = !path.exists();
        let mut registry = Self {
            path: path.to_path_buf(),
            apps: Vec::new(),
        };
        if fresh {
            // One-time upgrade from the v2 (Qt) registry.json beside us.
            // The legacy file is left untouched; a corrupt legacy file
            // never blocks startup (explicit imports still report errors).
            let legacy = path.with_file_name("registry.json");
            if legacy.exists() {
                let _ = registry.import_legacy_json(&legacy);
            }
        }
        registry.load()?;
        Ok(registry)
    }

    /// Import a v2 `{"schema_version": 1, "apps": [...]}` registry.
    /// Bounded, validated, and fail-closed like every other untrusted input.
    pub fn import_legacy_json(&mut self, legacy: &Path) -> Result<usize, String> {
        let body = std::fs::read(legacy).map_err(|e| format!("Cannot read registry: {e}"))?;
        if body.len() > crate::limits::MAX_JSON_BODY_BYTES + 1 {
            return Err("Registry exceeds size bound".to_string());
        }
        let root: serde_json::Value =
            serde_json::from_slice(&body).map_err(|_| "Invalid registry JSON".to_string())?;
        if !root.is_object() {
            return Err("Invalid registry JSON".to_string());
        }
        if root
            .get("schema_version")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            > limits::REGISTRY_SCHEMA_VERSION
        {
            return Err("Unsupported registry schema".to_string());
        }
        let mut imported = 0;
        if let Some(apps) = root.get("apps").and_then(|v| v.as_array()) {
            for item in apps.iter().take(100_000) {
                let app = legacy_app_from_json(item);
                if app.uuid.is_empty() || app.managed_path.is_empty() {
                    continue;
                }
                if self.apps.iter().any(|a| a.uuid == app.uuid) {
                    continue;
                }
                self.apps.push(app);
                imported += 1;
            }
        }
        if imported > 0 {
            self.save()?;
        }
        Ok(imported)
    }

    pub fn new_uuid() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn connect(&self) -> Result<Connection, String> {
        let conn =
            Connection::open(&self.path).map_err(|e| format!("Cannot open registry: {e}"))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| format!("Cannot migrate registry: {e}"))?;
        conn.execute(
            "INSERT OR IGNORE INTO meta(key, value) VALUES ('schema_version', ?)",
            params![limits::REGISTRY_SCHEMA_VERSION.to_string()],
        )
        .map_err(|e| format!("Cannot migrate registry: {e}"))?;
        Ok(conn)
    }

    fn load(&mut self) -> Result<(), String> {
        if !self.path.exists() {
            self.apps.clear();
            return Ok(());
        }
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                "SELECT uuid, name, version, comment, managed_path, desktop_id, desktop_path,
                        icon_path, sha256, app_type, architecture, size, arguments,
                        default_arguments, environment, update_manager, update_config,
                        embedded_update, last_update_check, available_version, available_url,
                        available_size, update_available, digest, reduced_verification,
                        external_folder, owned, adopted, website, terminal, actions
                 FROM apps ORDER BY rowid",
            )
            .map_err(|e| format!("Cannot read registry: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(InstalledApp {
                    uuid: row.get(0)?,
                    name: row.get(1)?,
                    version: row.get(2)?,
                    comment: row.get(3)?,
                    managed_path: row.get(4)?,
                    desktop_id: row.get(5)?,
                    desktop_path: row.get(6)?,
                    icon_path: row.get(7)?,
                    sha256: hex::decode(row.get::<_, String>(8)?).unwrap_or_default(),
                    app_type: int_to_app_type(row.get(9)?),
                    architecture: int_to_arch(row.get(10)?),
                    size: row.get(11)?,
                    arguments: parse_strings(&row.get::<_, String>(12)?),
                    default_arguments: parse_strings(&row.get::<_, String>(13)?),
                    environment: parse_env_pairs(&row.get::<_, String>(14)?),
                    update_manager: row.get(15)?,
                    update_config: parse_map(&row.get::<_, String>(16)?),
                    embedded_update: row.get(17)?,
                    last_update_check: row.get(18)?,
                    available_version: row.get(19)?,
                    available_url: row.get(20)?,
                    available_size: row.get(21)?,
                    update_available: row.get::<_, i64>(22)? != 0,
                    digest: row.get(23)?,
                    reduced_verification: row.get::<_, i64>(24)? != 0,
                    external_folder: row.get::<_, i64>(25)? != 0,
                    owned: row.get::<_, i64>(26)? != 0,
                    adopted: row.get::<_, i64>(27)? != 0,
                    website: row.get(28)?,
                    terminal: row.get::<_, i64>(29)? != 0,
                    actions: parse_actions(&row.get::<_, String>(30)?),
                    ..Default::default()
                })
            })
            .map_err(|e| format!("Cannot read registry: {e}"))?;
        self.apps.clear();
        for row in rows {
            self.apps
                .push(row.map_err(|e| format!("Cannot read registry: {e}"))?);
        }
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        let conn = self.connect()?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("Cannot save registry: {e}"))?;
        tx.execute("DELETE FROM apps", [])
            .map_err(|e| format!("Cannot save registry: {e}"))?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO apps(uuid, name, version, comment, managed_path, desktop_id,
                     desktop_path, icon_path, sha256, app_type, architecture, size, arguments,
                     default_arguments, environment, update_manager, update_config,
                     embedded_update, last_update_check, available_version, available_url,
                     available_size, update_available, digest, reduced_verification,
                     external_folder, owned, adopted, website, terminal, actions)
                 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                )
                .map_err(|e| format!("Cannot save registry: {e}"))?;
            for app in &self.apps {
                stmt.execute(params![
                    app.uuid,
                    app.name,
                    app.version,
                    app.comment,
                    app.managed_path,
                    app.desktop_id,
                    app.desktop_path,
                    app.icon_path,
                    hex::encode(&app.sha256),
                    app_type_to_int(app.app_type),
                    arch_to_int(app.architecture),
                    app.size,
                    serde_json::to_string(&app.arguments).unwrap_or_else(|_| "[]".into()),
                    serde_json::to_string(&app.default_arguments).unwrap_or_else(|_| "[]".into()),
                    serde_json::to_string(&app.environment).unwrap_or_else(|_| "[]".into()),
                    app.update_manager,
                    serde_json::to_string(&app.update_config).unwrap_or_else(|_| "{}".into()),
                    app.embedded_update,
                    app.last_update_check,
                    app.available_version,
                    app.available_url,
                    app.available_size,
                    i64::from(app.update_available),
                    app.digest,
                    i64::from(app.reduced_verification),
                    i64::from(app.external_folder),
                    i64::from(app.owned),
                    i64::from(app.adopted),
                    app.website,
                    i64::from(app.terminal),
                    serde_json::to_string(&app.actions).unwrap_or_else(|_| "[]".into()),
                ])
                .map_err(|e| format!("Cannot save registry: {e}"))?;
            }
        }
        tx.commit()
            .map_err(|e| format!("Cannot save registry: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn apps(&self) -> Vec<InstalledApp> {
        self.apps.clone()
    }

    pub fn snapshot(&self) -> Vec<InstalledApp> {
        self.apps.clone()
    }

    pub fn restore(&mut self, snapshot: Vec<InstalledApp>) -> Result<(), String> {
        self.apps = snapshot;
        self.save()
    }

    pub fn by_uuid(&self, uuid: &str) -> Option<InstalledApp> {
        self.apps.iter().find(|a| a.uuid == uuid).cloned()
    }

    /// Lookup by canonical managed path (symlink-hop bounded).
    pub fn by_path(&self, path: &str) -> Option<InstalledApp> {
        let needle = Path::new(path);
        let needle_canon =
            crate::safe_fs::canonical_bounded(needle).unwrap_or_else(|_| needle.to_path_buf());
        self.apps
            .iter()
            .find(|a| {
                let p = Path::new(&a.managed_path);
                let canon =
                    crate::safe_fs::canonical_bounded(p).unwrap_or_else(|_| p.to_path_buf());
                canon == needle_canon
            })
            .cloned()
    }

    pub fn upsert(&mut self, app: InstalledApp) -> Result<(), String> {
        if app.uuid.is_empty() {
            return Err("Refusing to register an app without a UUID".to_string());
        }
        match self.apps.iter_mut().find(|a| a.uuid == app.uuid) {
            Some(slot) => *slot = app,
            None => self.apps.push(app),
        }
        self.save()
    }

    pub fn remove_uuid(&mut self, uuid: &str) -> Result<(), String> {
        self.apps.retain(|a| a.uuid != uuid);
        self.save()
    }

    /// Adopt an external desktop entry's target without rewriting anything.
    pub fn adopt_external(
        &mut self,
        name: String,
        managed_path: String,
    ) -> Result<InstalledApp, String> {
        let mut app = InstalledApp::new_owned();
        app.uuid = Self::new_uuid();
        app.name = if name.is_empty() {
            "AppImage".to_string()
        } else {
            name
        };
        app.managed_path = managed_path;
        app.owned = true;
        app.adopted = true;
        app.external_folder = true;
        self.upsert(app.clone())?;
        Ok(app)
    }
}
