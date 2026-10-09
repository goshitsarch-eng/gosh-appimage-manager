// Gosh AppImage Manager — update sources (ports UpdateSources).
// Managers: static, github, gitlab, codeberg, forgejo, ftp.
// Every config is validated before any network use; every URL passes the
// UrlGuard; every body is bounded. Fail closed everywhere.

use std::collections::BTreeMap;

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::limits;
use crate::network::{Local, NetworkClient};
use crate::types::InstalledApp;
use crate::url_guard;

#[derive(Debug, Clone, Default)]
pub struct UpdateCheckResult {
    pub ok: bool,
    pub available: bool,
    pub version: String,
    pub url: String,
    pub size: i64,
    pub digest: String,
    pub digest_algo: String,
    pub etag: String,
    pub last_modified: String,
    pub reduced_verification: bool,
    pub error: String,
    pub manager: String,
}

impl UpdateCheckResult {
    fn fail(manager: &str, error: String) -> Self {
        Self {
            manager: manager.to_string(),
            error,
            ..Default::default()
        }
    }
}

pub type Config = BTreeMap<String, String>;

fn pct(text: &str) -> String {
    utf8_percent_encode(text, NON_ALPHANUMERIC).to_string()
}

/// HEAD probe size as i64 (-1 when unknown or failed). Check-only.
fn head_size(network: &dyn NetworkClient, url: &str, local: Local) -> i64 {
    network
        .head_len(url, local)
        .ok()
        .flatten()
        .map(|s| s.min(i64::MAX as u64) as i64)
        .unwrap_or(-1)
}

fn get_str(config: &Config, key: &str) -> String {
    config.get(key).cloned().unwrap_or_default()
}

fn json_string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn json_i64(value: &serde_json::Value, keys: &[&str]) -> i64 {
    for key in keys {
        if let Some(v) = value.get(key) {
            if let Some(n) = v.as_i64() {
                return n;
            }
            if let Some(n) = v.as_u64() {
                return n.min(i64::MAX as u64) as i64;
            }
        }
    }
    -1
}

fn parse_json_body(body: &[u8]) -> Result<serde_json::Value, String> {
    serde_json::from_slice(body).map_err(|_| "Update metadata is not valid JSON".to_string())
}

pub trait UpdateSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Does this manager understand the embedded hint (e.g. "gh-releases-zsync")?
    fn handles_embedded(&self, hint: &str) -> bool;
    /// Derive a config from embedded update fields when none is stored.
    fn config_from_embedded(&self, fields: &BTreeMap<String, String>) -> Config;
    /// Bring what the user typed into the stored form, before validation. The
    /// Library takes one key=value pair, so a source can arrive in a shorter
    /// form than it is stored in. `embedded` is the app's own update string.
    fn normalize_config(&self, config: Config, _embedded: &str) -> Config {
        config
    }
    fn validate_config(&self, config: &Config) -> Result<(), String>;
    /// As `validate_config`, for a config on a known app. A source that can name
    /// the fix in a terminal command uses the app's path to name it.
    fn validate_for_app(&self, _app_path: &str, config: &Config) -> Result<(), String> {
        self.validate_config(config)
    }
    fn check(
        &self,
        app: &InstalledApp,
        config: &Config,
        network: &dyn NetworkClient,
    ) -> UpdateCheckResult;
}

pub struct StaticSource;
pub struct GithubSource;
pub struct GitlabSource;
pub struct CodebergSource;
pub struct ForgejoSource;
pub struct FtpSource;

// ---- static (zsync control file or direct https file) ----------------------

impl UpdateSource for StaticSource {
    fn name(&self) -> &'static str {
        "static"
    }
    fn label(&self) -> &'static str {
        "Static file"
    }
    fn handles_embedded(&self, hint: &str) -> bool {
        // Only the plain forms. This used to accept any hint that contained
        // "zsync", which also claimed "gh-releases-zsync" -- the form nearly
        // every AppImage uses -- and failed its check with "needs a url".
        hint == "static" || hint == "zsync" || hint.contains("bintray")
    }
    fn config_from_embedded(&self, fields: &BTreeMap<String, String>) -> Config {
        let mut config = Config::new();
        if let Some(url) = fields.get("url") {
            config.insert("url".to_string(), url.clone());
        }
        config
    }
    fn validate_config(&self, config: &Config) -> Result<(), String> {
        let url = get_str(config, "url");
        if url.is_empty() {
            return Err("Static source needs a url".to_string());
        }
        // A source the user created may reach their own network when they say so
        // (`allow_local_network=true`). This used to refuse every local address
        // whatever the config said, so the opt-in the check honours could never
        // be saved.
        url_guard::validate(&url, false, Local::from_config(config).allowed()).map(|_| ())
    }
    fn check(
        &self,
        app: &InstalledApp,
        config: &Config,
        network: &dyn NetworkClient,
    ) -> UpdateCheckResult {
        let url = get_str(config, "url");
        if url.is_empty() {
            return UpdateCheckResult::fail(self.name(), "Static source needs a url".to_string());
        }
        // Reaching a private-network endpoint is allowed only when the user
        // set it on this source; embedded metadata can never set the key.
        let local = Local::from_config(config);
        if url.ends_with(".zsync") {
            let body = match network.get(&url, &[], local) {
                Ok(result) => {
                    if result.body.len() > limits::MAX_ZSYNC_BYTES {
                        return UpdateCheckResult::fail(
                            self.name(),
                            format!(
                                "Zsync metadata exceeds size bound ({} bytes)",
                                limits::MAX_ZSYNC_BYTES
                            ),
                        );
                    }
                    result.body
                }
                Err(e) => return UpdateCheckResult::fail(self.name(), e),
            };
            let control = parse_zsync_control(&body);
            // A control file names its target relative to itself (`URL:
            // App-x86_64.AppImage`), which is how zsync writes it. It was taken
            // as written, so the check failed on an address with no host.
            let download = match control.get("download_url") {
                Some(named) => url::Url::parse(&url)
                    .ok()
                    .and_then(|base| base.join(named).ok())
                    .map(String::from)
                    .unwrap_or_else(|| named.clone()),
                None => url.trim_end_matches(".zsync").to_string(),
            };
            if let Err(e) = url_guard::validate(&download, false, local.allowed()) {
                return UpdateCheckResult::fail(self.name(), e);
            }
            let version = control
                .get("version")
                .cloned()
                .or_else(|| get_str(config, "version").into())
                .unwrap_or_default();
            if version.is_empty() {
                return UpdateCheckResult {
                    manager: self.name().to_string(),
                    ok: true,
                    available: false,
                    error: "No version information in update metadata".to_string(),
                    ..Default::default()
                };
            }
            let size = head_size(network, &download, local);
            return UpdateCheckResult {
                ok: true,
                available: version != app.version,
                version,
                url: download,
                size,
                manager: self.name().to_string(),
                ..Default::default()
            };
        }
        // Direct file URL: only a HEAD probe (check-only, never downloads).
        match network.head_len(&url, local) {
            Ok(size) => {
                let version = get_str(config, "version");
                if version.is_empty() {
                    UpdateCheckResult {
                        manager: self.name().to_string(),
                        ok: true,
                        available: false,
                        error: "No version information in update metadata".to_string(),
                        ..Default::default()
                    }
                } else {
                    let size_i64 = size.map(|s| s.min(i64::MAX as u64) as i64).unwrap_or(-1);
                    UpdateCheckResult {
                        ok: true,
                        available: version != app.version,
                        version,
                        url,
                        size: size_i64,
                        manager: self.name().to_string(),
                        ..Default::default()
                    }
                }
            }
            Err(e) => UpdateCheckResult::fail(self.name(), e),
        }
    }
}

fn parse_zsync_control(body: &[u8]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let text = String::from_utf8_lossy(body);
    let mut urls = Vec::new();
    for line in text.lines().take(256) {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("Filename:") {
            map.entry("filename".to_string())
                .or_insert_with(|| value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("Version:") {
            map.insert("version".to_string(), value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("URL:") {
            urls.push(value.trim().to_string());
        }
    }
    if let Some(first) = urls.into_iter().next() {
        map.insert("download_url".to_string(), first);
    }
    map
}

// ---- github releases --------------------------------------------------------

/// What is wrong with a GitHub source config, as far as the user can act on it.
#[derive(Debug, PartialEq, Eq)]
enum GithubProblem {
    /// No owner/name: the repo is missing, or bare.
    NoRepo,
    /// The named key holds a value that is not a safe repository component.
    BadComponent(&'static str),
    /// No asset name to match, which only the user can supply.
    NoFilename,
}

fn github_problem(config: &Config) -> Option<GithubProblem> {
    if get_str(config, "username").is_empty() {
        return Some(GithubProblem::NoRepo);
    }
    for key in ["username", "repo"] {
        if !url_guard::is_safe_repo_component(&get_str(config, key)) {
            return Some(GithubProblem::BadComponent(key));
        }
    }
    if get_str(config, "filename").is_empty() {
        return Some(GithubProblem::NoFilename);
    }
    None
}

impl GithubProblem {
    /// The message for the user. A refusal the user can fix from the terminal
    /// carries the exact command that supplies what is missing, built from what
    /// the app and the user gave. The app's path is a placeholder when it is not
    /// known. The asset name is a placeholder unless the user gave one.
    fn message(&self, config: &Config, app_path: Option<&str>) -> String {
        match self {
            GithubProblem::NoRepo => format!(
                "GitHub needs repo=owner/name, the owner and the repository joined by a slash, for example repo=<owner>/<name>. {}",
                run_command(config, app_path)
            ),
            GithubProblem::BadComponent(key) => format!("GitHub {key} is not valid"),
            GithubProblem::NoFilename => {
                let base = "GitHub needs filename=<release asset name, * and ? allowed>, for \
                            example filename=<app>-x86_64.AppImage";
                match app_path {
                    None => base.to_string(),
                    Some(_) => format!("{base}. {}", run_command(config, app_path)),
                }
            }
        }
    }
}

/// The command that supplies the missing GitHub settings. A value the user gave
/// is shown as given, and a missing one is a placeholder. The app's path is quoted
/// so the command can be pasted.
fn run_command(config: &Config, app_path: Option<&str>) -> String {
    let path = match app_path {
        Some(path) => crate::safe_fs::copyable_path(path),
        None => "<path>".to_string(),
    };
    let owner = value_or(config, "username", "<owner>");
    let name = value_or(config, "repo", "<name>");
    let filename = value_or(config, "filename", "<asset name>");
    format!(
        "Run: gosh-appimage-manager --set-update-source {path} --manager github repo={owner}/{name} filename={filename}"
    )
}

/// The value under `key`, or `placeholder` when the user gave none.
fn value_or(config: &Config, key: &str, placeholder: &str) -> String {
    let value = get_str(config, key);
    if value.is_empty() {
        placeholder.to_string()
    } else {
        value
    }
}

impl UpdateSource for GithubSource {
    fn name(&self) -> &'static str {
        "github"
    }
    fn label(&self) -> &'static str {
        "GitHub releases"
    }
    fn handles_embedded(&self, hint: &str) -> bool {
        // `gh-releases-zsync` and `gh-releases-direct` are the AppImage
        // specification's two GitHub forms.
        hint.contains("github") || hint.starts_with("gh-releases")
    }
    fn config_from_embedded(&self, fields: &BTreeMap<String, String>) -> Config {
        let mut config = Config::new();
        for key in ["username", "repo", "release", "filename"] {
            if let Some(value) = fields.get(key) {
                config.insert(key.to_string(), value.clone());
            }
        }
        config
    }
    fn normalize_config(&self, mut config: Config, embedded: &str) -> Config {
        // `repo=owner/name` is the one pair the Library takes, so the owner can
        // arrive inside repo. Split it into the two keys the source stores.
        if !config.contains_key("username") {
            let repo = get_str(&config, "repo");
            if let Some((owner, name)) = repo.split_once('/') {
                if !name.contains('/') {
                    let (owner, name) = (owner.to_string(), name.to_string());
                    config.insert("username".to_string(), owner);
                    config.insert("repo".to_string(), name);
                }
            }
        }
        // A missing filename may come from the AppImage's own update information,
        // but only when that information is for the same repository.
        if get_str(&config, "filename").is_empty() {
            let fields = crate::inspector::parse_upd_info(embedded.as_bytes()).fields;
            let same_repo = fields.get("username") == config.get("username")
                && fields.get("repo") == config.get("repo");
            if let (true, Some(pattern)) = (same_repo, fields.get("filename")) {
                if !pattern.is_empty() {
                    config.insert("filename".to_string(), pattern.clone());
                }
            }
        }
        config
    }
    fn validate_config(&self, config: &Config) -> Result<(), String> {
        match github_problem(config) {
            Some(problem) => Err(problem.message(config, None)),
            None => Ok(()),
        }
    }
    fn validate_for_app(&self, app_path: &str, config: &Config) -> Result<(), String> {
        match github_problem(config) {
            Some(problem) => Err(problem.message(config, Some(app_path))),
            None => Ok(()),
        }
    }
    fn check(
        &self,
        app: &InstalledApp,
        config: &Config,
        network: &dyn NetworkClient,
    ) -> UpdateCheckResult {
        if let Err(e) = self.validate_for_app(&app.managed_path, config) {
            return UpdateCheckResult::fail(self.name(), e);
        }
        let user = get_str(config, "username");
        let repo = get_str(config, "repo");
        let filename = release_asset_pattern(&get_str(config, "filename")).to_string();
        let release = get_str(config, "release");
        // `latest` is the newest published release. Any other value names a
        // tag, which is how a project keeps one moving "continuous" release.
        let named = !release.is_empty() && !release.eq_ignore_ascii_case("latest");
        let api = if named {
            format!(
                "https://api.github.com/repos/{}/{}/releases/tags/{}",
                pct(&user),
                pct(&repo),
                pct(&release)
            )
        } else {
            format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                pct(&user),
                pct(&repo)
            )
        };
        let headers = [(
            "Accept".to_string(),
            "application/vnd.github+json".to_string(),
        )];
        let body = match network.get(&api, &headers, Local::from_config(config)) {
            Ok(result) => result.body,
            Err(e) => {
                let what = if named {
                    format!("release tagged {release}")
                } else {
                    "published release".to_string()
                };
                return UpdateCheckResult::fail(
                    self.name(),
                    explain_github_error(&e, &user, &repo, &what),
                );
            }
        };
        let json = match parse_json_body(&body) {
            Ok(json) => json,
            Err(e) => return UpdateCheckResult::fail(self.name(), e),
        };
        let version = json_string(&json, "tag_name");
        let assets = json
            .get("assets")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut candidates: Vec<(&serde_json::Value, String, String)> = Vec::new();
        let mut matched_but_refused = false;
        for asset in &assets {
            let name = json_string(asset, "name");
            if !asset_matches(&name, &filename) {
                continue;
            }
            let download = json_string(asset, "browser_download_url");
            if download.is_empty() {
                continue;
            }
            if url_guard::validate(&download, false, false).is_err()
                || !github_asset_host_allowed(&download)
            {
                matched_but_refused = true;
                continue;
            }
            candidates.push((asset, name, download));
        }
        if candidates.is_empty() {
            return UpdateCheckResult::fail(
                self.name(),
                if matched_but_refused {
                    "GitHub asset host is not allowed".to_string()
                } else {
                    format!(
                        "No GitHub asset named {filename} in {}. The release lists {} file(s).",
                        if version.is_empty() {
                            "the release"
                        } else {
                            &version
                        },
                        assets.len()
                    )
                },
            );
        }
        // A pattern that does not name an architecture matches the release's
        // builds for every one. Drop the ones for another architecture, so the
        // download is not wasted on a file this install cannot run.
        let names: Vec<&str> = candidates
            .iter()
            .map(|(_, name, _)| name.as_str())
            .collect();
        let Some(choice) = pick_for_architecture(&names, app.architecture) else {
            return UpdateCheckResult::fail(
                self.name(),
                format!(
                    "The release has no {} build matching {filename}.",
                    crate::types::architecture_name(app.architecture)
                ),
            );
        };
        let (asset, _, download) = &candidates[choice];
        let mut result = UpdateCheckResult {
            ok: true,
            available: true,
            version: version.clone(),
            url: download.clone(),
            size: json_i64(asset, &["size"]),
            digest: json_string(asset, "digest"),
            manager: self.name().to_string(),
            ..Default::default()
        };
        if result.version.is_empty() {
            result.available = false;
            result.error = "No version information in update metadata".to_string();
        }
        result
    }
}

/// An asset pattern as the release lists it. An AppImage's embedded update
/// string names the build's `.zsync` control file (`App-*x86_64.AppImage.zsync`),
/// but the file to download is the AppImage beside it.
fn release_asset_pattern(pattern: &str) -> &str {
    let trimmed = pattern.trim();
    match trimmed.len().checked_sub(".zsync".len()) {
        Some(cut)
            if trimmed.is_char_boundary(cut) && trimmed[cut..].eq_ignore_ascii_case(".zsync") =>
        {
            &trimmed[..cut]
        }
        _ => trimmed,
    }
}

/// The CPU architecture an asset's name says it is for, when it says one.
fn architecture_of_name(name: &str) -> Option<crate::types::Architecture> {
    use crate::types::Architecture;
    let lower = name.to_ascii_lowercase();
    let has = |tokens: &[&str]| tokens.iter().any(|token| lower.contains(token));
    if has(&["x86_64", "x86-64", "amd64", "x64"]) {
        Some(Architecture::X86_64)
    } else if has(&["aarch64", "arm64"]) {
        Some(Architecture::AArch64)
    } else if has(&["armhf", "armv7", "armv6", "armel"]) {
        Some(Architecture::Arm)
    } else if has(&["i386", "i686", "ia32"]) {
        Some(Architecture::I386)
    } else {
        None
    }
}

/// The index of the best asset among `names` for an install of `architecture`:
/// one built for it first, then one that does not say, never one built for
/// another. `None` when every candidate is for another architecture. With the
/// architecture unknown, the first candidate stands.
fn pick_for_architecture(
    names: &[&str],
    architecture: crate::types::Architecture,
) -> Option<usize> {
    if names.is_empty() {
        return None;
    }
    if matches!(architecture, crate::types::Architecture::Unknown) {
        return Some(0);
    }
    let mut unlabeled = None;
    for (index, name) in names.iter().enumerate() {
        match architecture_of_name(name) {
            Some(found) if found == architecture => return Some(index),
            None if unlabeled.is_none() => unlabeled = Some(index),
            _ => {}
        }
    }
    unlabeled
}

/// Say what a failed GitHub request means for the person reading it.
fn explain_github_error(error: &str, user: &str, repo: &str, what: &str) -> String {
    if error.contains("HTTP 404") {
        return format!(
            "GitHub has no {what} for {user}/{repo} (HTTP 404). Check the repository name, \
             that it is public, and that it publishes releases."
        );
    }
    if error.contains("rate limit") {
        return format!(
            "GitHub's request limit was reached while checking {user}/{repo}. \
             Try again in a little while."
        );
    }
    if error.contains("HTTP 403") {
        return format!(
            "GitHub refused the request for {user}/{repo} (HTTP 403). The hourly limit for \
             requests without an account may be used up, or a network policy may block \
             api.github.com. Try again in a little while."
        );
    }
    error.to_string()
}

fn asset_matches(asset_name: &str, wanted: &str) -> bool {
    if wanted.contains('*') || wanted.contains('?') {
        // Both operands come from untrusted sources; cap them so even the
        // linear matcher cannot be handed a pathological amount of work.
        if wanted.len() > limits::MAX_GLOB_PATTERN_LENGTH
            || asset_name.len() > limits::MAX_GLOB_TEXT_LENGTH
        {
            return false;
        }
        return glob_match(wanted, asset_name);
    }
    asset_name == wanted
}

/// Linear-time `*`/`?` matcher.
///
/// The previous recursive form branched on every `*` (`for i in 0..=t.len()`
/// then recursing), which is exponential: a pattern of `*a` repeated blows up
/// ~8x per two characters. Both inputs are hostile — the pattern is the
/// `filename` config, which `config_from_embedded` lifts straight out of an
/// AppImage's `.upd_info` section, and the text is an asset name from a remote
/// release document — so the cost has to be bounded by construction.
///
/// This is the standard backtrack-once greedy walk: remember the most recent
/// `*` and the text position it matched to, and on a mismatch resume from
/// there having consumed one more character. Worst case O(pattern x text),
/// with no recursion and so no stack growth either.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let (mut pi, mut ti) = (0usize, 0usize);
    // Position of the last `*` in the pattern, and where in the text it
    // currently starts matching.
    let mut star: Option<usize> = None;
    let mut star_text = 0usize;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            star_text = ti;
            pi += 1;
        } else if let Some(s) = star {
            // Give the last `*` one more character and retry from just after it.
            pi = s + 1;
            star_text += 1;
            ti = star_text;
        } else {
            return false;
        }
    }
    // Trailing `*`s may match the empty remainder.
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

fn github_asset_host_allowed(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    host == "github.com" || host.ends_with(".githubusercontent.com")
}

/// Is this asset URL served by the forge we asked, or a subdomain of it?
///
/// A release document is attacker-influenced -- anyone who can publish a
/// release, or a compromised instance, chooses the download URL. GitHub assets
/// were already pinned to github.com and its CDN, but GitLab, Codeberg and
/// Forgejo accepted any HTTPS host their JSON named, so a release could point
/// the download anywhere.
///
/// Self-hosted instances do legitimately serve assets from a separate domain,
/// so this is not absolute: `allow_any_asset_host=true` on a source the user
/// created opts out. Embedded metadata cannot set it.
fn forge_asset_host_allowed(url: &str, forge_host: &str, config: &Config) -> bool {
    if get_str(config, "allow_any_asset_host") == "true" {
        return true;
    }
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let forge = forge_host.trim().to_ascii_lowercase();
    if forge.is_empty() {
        return false;
    }
    host == forge || host.ends_with(&format!(".{forge}"))
}

// ---- gitlab ------------------------------------------------------------------

impl UpdateSource for GitlabSource {
    fn name(&self) -> &'static str {
        "gitlab"
    }
    fn label(&self) -> &'static str {
        "GitLab releases"
    }
    fn handles_embedded(&self, hint: &str) -> bool {
        hint.contains("gitlab")
    }
    fn config_from_embedded(&self, fields: &BTreeMap<String, String>) -> Config {
        let mut config = Config::new();
        for key in ["host", "project", "filename", "package", "package_name"] {
            if let Some(value) = fields.get(key) {
                config.insert(key.to_string(), value.clone());
            }
        }
        config
    }
    fn validate_config(&self, config: &Config) -> Result<(), String> {
        let project = get_str(config, "project");
        if project.is_empty() || project.contains("..") || project.contains('\0') {
            return Err("GitLab project is not valid".to_string());
        }
        let host = get_str(config, "host");
        let host = if host.is_empty() {
            "gitlab.com".to_string()
        } else {
            host.to_lowercase()
        };
        if host.contains('/') || host.contains(':') || host.is_empty() {
            return Err("GitLab host is not valid".to_string());
        }
        if host != "gitlab.com"
            && is_private_hostname(&host)
            && !Local::from_config(config).allowed()
        {
            return Err("Private GitLab hosts require explicit opt-in".to_string());
        }
        Ok(())
    }
    fn check(
        &self,
        _app: &InstalledApp,
        config: &Config,
        network: &dyn NetworkClient,
    ) -> UpdateCheckResult {
        if let Err(e) = self.validate_config(config) {
            return UpdateCheckResult::fail(self.name(), e);
        }
        let host = {
            let raw = get_str(config, "host").to_lowercase();
            if raw.is_empty() {
                "gitlab.com".to_string()
            } else {
                raw
            }
        };
        let project = get_str(config, "project");
        let filename = get_str(config, "filename");
        let api = format!(
            "https://{}/api/v4/projects/{}/releases",
            host,
            pct(&project)
        );
        let local = Local::from_config(config);
        let body = match network.get(&api, &[], local) {
            Ok(result) => result.body,
            Err(e) => return UpdateCheckResult::fail(self.name(), e),
        };
        let json = match parse_json_body(&body) {
            Ok(json) => json,
            Err(e) => return UpdateCheckResult::fail(self.name(), e),
        };
        let releases = json.as_array().cloned().unwrap_or_default();
        let Some(first) = releases.first() else {
            return UpdateCheckResult::fail(self.name(), "No GitLab releases".to_string());
        };
        let version = json_string(first, "tag_name");
        let links = first
            .get("assets")
            .and_then(|a| a.get("links"))
            .and_then(|l| l.as_array())
            .cloned()
            .unwrap_or_default();
        for link in &links {
            let name = json_string(link, "name");
            if !filename.is_empty() && name != filename {
                continue;
            }
            let download = json_string(link, "url");
            if download.is_empty()
                || url_guard::validate(&download, false, local.allowed()).is_err()
            {
                continue;
            }
            if !forge_asset_host_allowed(&download, &host, config) {
                continue;
            }
            return UpdateCheckResult {
                ok: true,
                available: true,
                version: version.clone(),
                url: download,
                size: json_i64(link, &["size"]),
                digest: json_string(link, "checksum"),
                manager: self.name().to_string(),
                ..Default::default()
            };
        }
        // Direct asset URL fallback: first .AppImage link.
        for link in &links {
            let download = json_string(link, "direct_asset_url");
            if download.ends_with(".AppImage")
                && url_guard::validate(&download, false, local.allowed()).is_ok()
                && forge_asset_host_allowed(&download, &host, config)
            {
                return UpdateCheckResult {
                    ok: true,
                    available: true,
                    version: version.clone(),
                    url: download,
                    size: -1,
                    manager: self.name().to_string(),
                    ..Default::default()
                };
            }
        }
        // Generic package fallback when configured.
        if let Some(result) = self.check_package(&host, &project, config, network) {
            return result;
        }
        UpdateCheckResult::fail(self.name(), "No matching GitLab asset".to_string())
    }
}

impl GitlabSource {
    fn check_package(
        &self,
        host: &str,
        project: &str,
        config: &Config,
        network: &dyn NetworkClient,
    ) -> Option<UpdateCheckResult> {
        let package = get_str(config, "package");
        let package = if package.is_empty() {
            get_str(config, "package_name")
        } else {
            package
        };
        if package.is_empty() {
            return None;
        }
        let list_url = format!(
            "https://{}/api/v4/projects/{}/packages?package_name={}",
            host,
            pct(project),
            pct(&package)
        );
        let local = Local::from_config(config);
        let body = network.get(&list_url, &[], local).ok()?.body;
        let packages = parse_json_body(&body).ok()?;
        let id = packages.as_array()?.first()?.get("id")?.as_i64()?;
        let files_url = format!(
            "https://{}/api/v4/projects/{}/packages/{}/package_files",
            host,
            pct(project),
            id
        );
        let files_body = network.get(&files_url, &[], local).ok()?.body;
        let files = parse_json_body(&files_body).ok()?;
        let wanted = get_str(config, "filename");
        for file in files.as_array()? {
            let name = json_string(file, "file_name");
            if !wanted.is_empty() && name != wanted && !name.ends_with(".AppImage") {
                continue;
            }
            let fid = file.get("id")?.as_i64()?;
            let download = format!(
                "https://{}/api/v4/projects/{}/packages/package_files/{}/download",
                host,
                pct(project),
                fid
            );
            // Only SHA-256 counts as a digest. `file_md5` used to be passed
            // through here and then compared as if it were a SHA-256, which
            // could never match; an unusable digest is worse than none,
            // because it turns every update into a verification failure.
            let digest = json_string(file, "file_sha256");
            return Some(UpdateCheckResult {
                ok: true,
                available: true,
                version: String::new(),
                url: download,
                size: json_i64(file, &["size"]),
                digest,
                manager: self.name().to_string(),
                ..Default::default()
            });
        }
        None
    }
}

/// Is this host one we refuse to treat as a public forge?
///
/// Delegates the address classification to url_guard so there is one
/// definition. This used to carry its own copy, which missed IPv6
/// unique-local and link-local entirely and had a hand-rolled dotted-name
/// heuristic that clippy could simplify because it said nothing useful.
fn is_private_hostname(host: &str) -> bool {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<std::net::IpAddr>() {
        return url_guard::is_local_ip(&ip);
    }
    let name = bare.strip_suffix('.').unwrap_or(bare);
    if name == "localhost"
        || name.ends_with(".localhost")
        || name.ends_with(".local")
        || name.ends_with(".internal")
    {
        return true;
    }
    // A single-label name is not a public DNS name; treat it as internal
    // until the user opts in.
    !name.contains('.')
}

// ---- codeberg + forgejo (Forgejo API shape) ------------------------------------

fn forgejo_check(
    manager: &str,
    default_host: &str,
    _app: &InstalledApp,
    config: &Config,
    network: &dyn NetworkClient,
) -> UpdateCheckResult {
    let host = {
        let raw = get_str(config, "host").to_lowercase();
        if raw.is_empty() {
            default_host.to_string()
        } else {
            raw
        }
    };
    if host.contains('/') || host.contains(':') || host.is_empty() {
        return UpdateCheckResult::fail(manager, "Forgejo host is not valid".to_string());
    }
    for key in ["owner", "repo"] {
        if !url_guard::is_safe_repo_component(&get_str(config, key)) {
            return UpdateCheckResult::fail(manager, format!("Forgejo {key} is not valid"));
        }
    }
    let owner = get_str(config, "owner");
    let repo = get_str(config, "repo");
    let filename = release_asset_pattern(&get_str(config, "filename")).to_string();
    let api = format!(
        "https://{}/api/v1/repos/{}/{}/releases?limit=1",
        host,
        pct(&owner),
        pct(&repo)
    );
    let local = Local::from_config(config);
    let body = match network.get(&api, &[], local) {
        Ok(result) => result.body,
        Err(e) => return UpdateCheckResult::fail(manager, e),
    };
    let json = match parse_json_body(&body) {
        Ok(json) => json,
        Err(e) => return UpdateCheckResult::fail(manager, e),
    };
    let releases = json.as_array().cloned().unwrap_or_default();
    let Some(first) = releases.first() else {
        return UpdateCheckResult::fail(manager, "No releases".to_string());
    };
    let version = json_string(first, "tag_name");
    let assets = first
        .get("assets")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    for asset in &assets {
        let name = json_string(asset, "name");
        if !filename.is_empty() && !asset_matches(&name, &filename) {
            continue;
        }
        let download = json_string(asset, "browser_download_url");
        if download.is_empty() || url_guard::validate(&download, false, local.allowed()).is_err() {
            continue;
        }
        if !forge_asset_host_allowed(&download, &host, config) {
            continue;
        }
        return UpdateCheckResult {
            ok: true,
            available: true,
            version: version.clone(),
            url: download,
            size: json_i64(asset, &["size_bytes", "size"]),
            manager: manager.to_string(),
            ..Default::default()
        };
    }
    UpdateCheckResult::fail(manager, "No matching release asset".to_string())
}

impl UpdateSource for CodebergSource {
    fn name(&self) -> &'static str {
        "codeberg"
    }
    fn label(&self) -> &'static str {
        "Codeberg releases"
    }
    fn handles_embedded(&self, hint: &str) -> bool {
        hint.contains("codeberg")
    }
    fn config_from_embedded(&self, fields: &BTreeMap<String, String>) -> Config {
        let mut config = Config::new();
        for key in ["owner", "repo", "filename", "release"] {
            if let Some(value) = fields.get(key) {
                config.insert(key.to_string(), value.clone());
            }
        }
        config
    }
    fn validate_config(&self, config: &Config) -> Result<(), String> {
        for key in ["owner", "repo"] {
            if !url_guard::is_safe_repo_component(&get_str(config, key)) {
                return Err(format!("Codeberg {key} is not valid"));
            }
        }
        Ok(())
    }
    fn check(
        &self,
        app: &InstalledApp,
        config: &Config,
        network: &dyn NetworkClient,
    ) -> UpdateCheckResult {
        if let Err(e) = self.validate_config(config) {
            return UpdateCheckResult::fail(self.name(), e);
        }
        forgejo_check(self.name(), "codeberg.org", app, config, network)
    }
}

impl UpdateSource for ForgejoSource {
    fn name(&self) -> &'static str {
        "forgejo"
    }
    fn label(&self) -> &'static str {
        "Forgejo releases"
    }
    fn handles_embedded(&self, hint: &str) -> bool {
        hint.contains("forgejo")
    }
    fn config_from_embedded(&self, fields: &BTreeMap<String, String>) -> Config {
        let mut config = Config::new();
        for key in ["host", "owner", "repo", "filename", "release"] {
            if let Some(value) = fields.get(key) {
                config.insert(key.to_string(), value.clone());
            }
        }
        config
    }
    fn validate_config(&self, config: &Config) -> Result<(), String> {
        let host = get_str(config, "host");
        if host.is_empty() || host.contains('/') || host.contains(':') {
            return Err("Forgejo host is not valid".to_string());
        }
        for key in ["owner", "repo"] {
            if !url_guard::is_safe_repo_component(&get_str(config, key)) {
                return Err(format!("Forgejo {key} is not valid"));
            }
        }
        Ok(())
    }
    fn check(
        &self,
        app: &InstalledApp,
        config: &Config,
        network: &dyn NetworkClient,
    ) -> UpdateCheckResult {
        if let Err(e) = self.validate_config(config) {
            return UpdateCheckResult::fail(self.name(), e);
        }
        forgejo_check(self.name(), "forgejo", app, config, network)
    }
}

// ---- ftp (legacy explicit) ------------------------------------------------------

impl UpdateSource for FtpSource {
    fn name(&self) -> &'static str {
        "ftp"
    }
    fn label(&self) -> &'static str {
        "FTP file (legacy)"
    }
    fn handles_embedded(&self, hint: &str) -> bool {
        hint == "ftp" || hint.contains("ftp")
    }
    fn config_from_embedded(&self, fields: &BTreeMap<String, String>) -> Config {
        let mut config = Config::new();
        if let Some(url) = fields.get("url") {
            config.insert("url".to_string(), url.clone());
        }
        config
    }
    fn validate_config(&self, config: &Config) -> Result<(), String> {
        let url = get_str(config, "url");
        if url.is_empty() {
            return Err("FTP source needs a url".to_string());
        }
        if !(url.starts_with("ftp://") || url.starts_with("FTP://")) {
            return Err("FTP source needs an ftp:// URL".to_string());
        }
        // Credentials fail closed, no network.
        url_guard::validate(&url, true, Local::from_config(config).allowed()).map(|_| ())
    }
    fn check(
        &self,
        app: &InstalledApp,
        config: &Config,
        network: &dyn NetworkClient,
    ) -> UpdateCheckResult {
        if let Err(e) = self.validate_config(config) {
            return UpdateCheckResult::fail(self.name(), e);
        }
        let url = get_str(config, "url");
        let version = get_str(config, "version");
        if version.is_empty() {
            return UpdateCheckResult {
                manager: self.name().to_string(),
                ok: true,
                available: false,
                error: "No version information in update metadata".to_string(),
                ..Default::default()
            };
        }
        match network.head_len(&url, Local::from_config(config)) {
            Ok(size) => UpdateCheckResult {
                ok: true,
                available: version != app.version,
                version,
                url,
                size: size.map(|s| s.min(i64::MAX as u64) as i64).unwrap_or(-1),
                manager: self.name().to_string(),
                ..Default::default()
            },
            Err(e) => UpdateCheckResult::fail(self.name(), e),
        }
    }
}

// ---- factory ---------------------------------------------------------------------

pub struct UpdateSourceFactory;

impl UpdateSourceFactory {
    pub fn names() -> Vec<&'static str> {
        vec!["static", "github", "gitlab", "codeberg", "forgejo", "ftp"]
    }

    pub fn by_name(name: &str) -> Option<Box<dyn UpdateSource>> {
        match name {
            "static" => Some(Box::new(StaticSource)),
            "github" => Some(Box::new(GithubSource)),
            "gitlab" => Some(Box::new(GitlabSource)),
            "codeberg" => Some(Box::new(CodebergSource)),
            "forgejo" => Some(Box::new(ForgejoSource)),
            "ftp" => Some(Box::new(FtpSource)),
            _ => None,
        }
    }

    /// Guess the manager from an embedded update string.
    pub fn detect_embedded(raw: &str) -> Option<Box<dyn UpdateSource>> {
        let hint = raw
            .split('|')
            .next()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        // The forge sources are asked before the plain file sources: a hint such
        // as "gh-releases-zsync" names GitHub, whatever else it also mentions.
        // `names()` keeps its own order, which the CLI and self-test list.
        for name in ["github", "gitlab", "codeberg", "forgejo", "ftp", "static"] {
            if let Some(source) = Self::by_name(name) {
                if source.handles_embedded(&hint) {
                    return Some(source);
                }
            }
        }
        // Substring fallback mirrors the C++ heuristic, for the forges only. The
        // plain-file forms are named by their hint above; a string that merely
        // mentions "zsync" (`pling-v1-zsync`) is not one of them.
        let lower = raw.to_lowercase();
        for (key, name) in [
            ("github", "github"),
            ("gitlab", "gitlab"),
            ("codeberg", "codeberg"),
            ("forgejo", "forgejo"),
            ("ftp", "ftp"),
        ] {
            if lower.contains(key) {
                return Self::by_name(name);
            }
        }
        None
    }
}
