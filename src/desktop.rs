// Gosh AppImage Manager — desktop entry parsing + owned integration files.
// Ports DesktopParser + DesktopIntegration. Exec lines are built from
// program + argument tokens only; shell strings are never constructed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::limits;
use crate::types::{EnvPair, InstalledApp};

/// Escape one Exec token (mirrors DesktopParser::escapeExecArg).
pub fn escape_exec_arg(token: &str) -> String {
    if token.len() == 2 && token.starts_with('%') {
        return token.to_string();
    }
    let need_quote = token
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\\' | '$' | '`' | '\''));
    if !need_quote {
        return token.to_string();
    }
    let mut out = String::from("\"");
    for ch in token.chars() {
        if matches!(ch, '"' | '\\' | '$' | '`') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

/// Escape a Desktop Entry string value (Name/Comment/Icon).
pub fn escape_entry_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars().take(limits::MAX_NAME_LENGTH) {
        match ch {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out
}

/// Build the Exec line: env pairs + program + bounded arguments. No shell.
pub fn build_exec_line(program: &str, environment: &[EnvPair], arguments: &[String]) -> String {
    let mut parts = Vec::new();
    for pair in environment.iter().take(limits::MAX_ENV_PAIRS) {
        if !valid_env_name(&pair.name) || pair.value.contains('\0') {
            continue;
        }
        parts.push(format!("{}={}", pair.name, escape_exec_arg(&pair.value)));
    }
    parts.push(escape_exec_arg(program));
    for argument in arguments.iter().take(limits::MAX_ARGUMENTS) {
        if argument.contains('\0') || argument.len() > limits::MAX_ARGUMENT_LENGTH {
            continue;
        }
        parts.push(escape_exec_arg(argument));
    }
    parts.join(" ")
}

pub fn valid_env_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 128 {
        return false;
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Keep only a basename-safe icon name (no paths, no traversal).
pub fn sanitize_icon_name(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    let base = base.rsplit('\\').next().unwrap_or(base);
    base.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
        .take(128)
        .collect()
}

/// Filename-safe base derived from a display name.
pub fn sanitize_file_base(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(96)
        .collect();
    if out.trim_matches('_').is_empty() {
        out = "AppImage".to_string();
    }
    out
}

pub fn desktop_file_name(uuid: &str) -> String {
    format!("{}{}.desktop", limits::DESKTOP_PREFIX, uuid)
}

/// Minimal bounded .desktop parser: sections, key=value, locale stripping.
#[derive(Debug, Default)]
pub struct DesktopFile {
    pub groups: BTreeMap<String, BTreeMap<String, String>>,
}

pub fn parse_desktop_bytes(bytes: &[u8]) -> Result<DesktopFile, String> {
    if bytes.len() > limits::MAX_DESKTOP_FILE_BYTES {
        return Err("Desktop file exceeds size bound".to_string());
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| "Desktop file is not valid UTF-8".to_string())?;
    let mut file = DesktopFile::default();
    let mut group = String::from("Desktop Entry");
    let mut count = 0;
    for line in text.lines().take(4096) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            group = line[1..line.len() - 1].to_string();
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        let (mut key, value) = (line[..eq].trim(), line[eq + 1..].trim());
        if let Some(bracket) = key.find('[') {
            key = &key[..bracket]; // ignore localized variants
        }
        if key.is_empty() || key.len() > 128 || value.len() > 8192 {
            continue;
        }
        count += 1;
        if count > 512 {
            break;
        }
        file.groups
            .entry(group.clone())
            .or_default()
            .insert(key.to_string(), value.to_string());
    }
    Ok(file)
}

impl DesktopFile {
    pub fn entry(&self, key: &str) -> &str {
        self.groups
            .get("Desktop Entry")
            .and_then(|g| g.get(key))
            .map(|s| s.as_str())
            .unwrap_or_default()
    }
}

/// Unescape `\n \t \r \\` sequences in entry values.
pub fn unescape_entry_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Build the full owned .desktop body for an installed app.
pub fn build_desktop_file(app: &InstalledApp, managed_path: &str, terminal_suffix: bool) -> String {
    let mut name = app.name.clone();
    if name.is_empty() {
        name = "AppImage".to_string();
    }
    let exec = build_exec_line(managed_path, &app.environment, &app.arguments);
    let icon = if app.icon_path.is_empty() {
        "application-x-executable".to_string()
    } else {
        app.icon_path.clone()
    };
    let mut body = String::new();
    body.push_str("[Desktop Entry]\n");
    body.push_str("Type=Application\n");
    body.push_str(&format!("Name={}\n", escape_entry_value(&name)));
    if !app.comment.is_empty() {
        body.push_str(&format!("Comment={}\n", escape_entry_value(&app.comment)));
    }
    body.push_str(&format!("Exec={exec}\n"));
    body.push_str(&format!("TryExec={}\n", escape_exec_arg(managed_path)));
    body.push_str(&format!("Icon={}\n", escape_entry_value(&icon)));
    body.push_str(&format!(
        "Terminal={}\n",
        if app.terminal { "true" } else { "false" }
    ));
    let _ = terminal_suffix;
    body.push_str("Categories=Utility;\n");
    body.push_str("StartupNotify=true\n");
    if !app.version.is_empty() {
        body.push_str(&format!(
            "X-AppImage-Version={}\n",
            escape_entry_value(&app.version)
        ));
    }
    body.push_str(&format!("{}=true\n", limits::OWNERSHIP_KEY));
    body.push_str(&format!("{}={}\n", limits::OWNERSHIP_UUID_KEY, app.uuid));
    body.push_str(&format!(
        "{}={}\n",
        limits::OWNERSHIP_PATH_KEY,
        escape_entry_value(managed_path)
    ));
    if !app.actions.is_empty() {
        let ids: Vec<String> = app
            .actions
            .iter()
            .map(|a| a.id.clone())
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 64
                    && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
            .take(16)
            .collect();
        if !ids.is_empty() {
            body.push_str(&format!("Actions={};\n", ids.join(";")));
            for action in &app.actions {
                if !ids.contains(&action.id) {
                    continue;
                }
                body.push_str(&format!("\n[Desktop Action {}]\n", action.id));
                body.push_str(&format!("Name={}\n", escape_entry_value(&action.name)));
                body.push_str(&format!(
                    "Exec={}\n",
                    build_exec_line(managed_path, &[], &action.arguments)
                ));
            }
        }
    }
    body
}

/// Provenance of a desktop file: owned by us only when markers match.
#[derive(Debug, Default)]
pub struct DesktopOwnership {
    pub owned: bool,
    pub uuid: String,
    pub managed_path: String,
}

pub fn verify_ownership(desktop_path: &Path) -> DesktopOwnership {
    let mut ownership = DesktopOwnership::default();
    let Ok(bytes) = fs::read(desktop_path) else {
        return ownership;
    };
    if bytes.len() > limits::MAX_DESKTOP_FILE_BYTES {
        return ownership;
    }
    let Ok(file) = parse_desktop_bytes(&bytes) else {
        return ownership;
    };
    if file.entry(limits::OWNERSHIP_KEY) != "true" {
        return ownership;
    }
    ownership.uuid = file.entry(limits::OWNERSHIP_UUID_KEY).to_string();
    ownership.managed_path = unescape_entry_value(file.entry(limits::OWNERSHIP_PATH_KEY));
    ownership.owned = !ownership.uuid.is_empty() && !ownership.managed_path.is_empty();
    ownership
}

/// Install (stage + rename) a desktop file and icon; returns final paths.
pub fn install_files(
    applications_dir: &Path,
    icons_dir: &Path,
    desktop_name: &str,
    desktop_body: &str,
    icon_source: Option<&Path>,
    icon_ext: &str,
    uuid: &str,
) -> Result<(PathBuf, PathBuf), String> {
    if desktop_body.len() > limits::MAX_DESKTOP_FILE_BYTES {
        return Err("Desktop file exceeds size bound".to_string());
    }
    fs::create_dir_all(applications_dir)
        .map_err(|e| format!("Cannot create applications dir: {e}"))?;
    let desktop_path = applications_dir.join(desktop_name);
    let staging = crate::safe_fs::sibling_temp(&desktop_path, ".gosh-desk-");
    fs::write(&staging, desktop_body).map_err(|e| format!("Cannot write desktop file: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&staging, fs::Permissions::from_mode(0o644));
    }
    fs::rename(&staging, &desktop_path).map_err(|e| format!("Cannot install desktop file: {e}"))?;

    let mut icon_path = PathBuf::new();
    if let Some(source) = icon_source {
        let ext = match icon_ext {
            "svg" => "svg",
            _ => "png",
        };
        let dir = icons_dir.join("256x256/apps");
        fs::create_dir_all(&dir).map_err(|e| format!("Cannot create icons dir: {e}"))?;
        let final_icon = dir.join(format!("gosh-appimage-{uuid}.{ext}"));
        let staged = crate::safe_fs::sibling_temp(&final_icon, ".gosh-icon-");
        crate::safe_fs::copy_bounded(
            source,
            &staged,
            limits::MAX_ICON_BYTES,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .map_err(|e| format!("Cannot stage icon: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&staged, fs::Permissions::from_mode(0o644));
        }
        fs::rename(&staged, &final_icon).map_err(|e| format!("Cannot install icon: {e}"))?;
        icon_path = final_icon;
    }
    Ok((desktop_path, icon_path))
}
