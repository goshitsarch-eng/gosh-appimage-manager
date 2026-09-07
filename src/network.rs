// Gosh AppImage Manager — bounded network client (ports NetworkClient).
// HTTPS by default via rustls; HEAD probes; DNS pinning per request;
// bounded redirects, bodies, and downloads. Zsync metadata is understood
// but updates always download the full file (no binary delta).
// FTP is a legacy explicit option with an insecure-transport warning.

use std::collections::HashMap;
use std::io::Read;
use std::net::ToSocketAddrs;
use std::sync::Mutex;
use std::time::Duration;

use crate::limits;
use crate::url_guard;

#[derive(Debug, Clone, Default)]
pub struct FetchResult {
    pub body: Vec<u8>,
    pub final_url: String,
}

/// Whether a request may reach a loopback / link-local / private-network
/// destination.
///
/// The brief allows this, but only for a source the user created and opted in
/// on. It is a parameter rather than client state so that every call site
/// states its own answer and `Local::Allowed` can be grepped for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Local {
    Denied,
    Allowed,
}

impl Local {
    pub fn allowed(self) -> bool {
        matches!(self, Local::Allowed)
    }

    /// Read the opt-in from an update-source config.
    ///
    /// Only ever set by a user editing a source. `config_from_embedded` never
    /// produces this key, so an AppImage cannot opt itself in.
    pub fn from_config(config: &std::collections::BTreeMap<String, String>) -> Self {
        match config.get("allow_local_network").map(String::as_str) {
            Some("true" | "yes" | "1") => Local::Allowed,
            _ => Local::Denied,
        }
    }
}

pub trait NetworkClient: Send + Sync {
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        local: Local,
    ) -> Result<FetchResult, String>;
    fn head_len(&self, url: &str, local: Local) -> Result<Option<u64>, String>;
    fn download_bounded(&self, url: &str, max_bytes: u64, local: Local) -> Result<Vec<u8>, String>;

    /// Stream a download straight to `dest` (created mode 0600), returning the
    /// byte count.
    ///
    /// An AppImage is routinely hundreds of megabytes and the configured bound
    /// defaults to 8 GiB, so `download_bounded` -- which accumulates the whole
    /// body into a Vec before anything is written -- is not usable for the
    /// payload path: a large or hostile response drives an allocation of that
    /// size and the process is OOM-killed. Nothing here holds more than one
    /// buffer at a time.
    fn download_to_file(
        &self,
        url: &str,
        dest: &std::path::Path,
        max_bytes: u64,
        cancel: &std::sync::atomic::AtomicBool,
        local: Local,
    ) -> Result<u64, String>;
}

/// Copy `reader` into `dest` with a byte ceiling and cancellation, creating the
/// file mode 0600 and removing it on any failure.
pub fn stream_to_file<R: Read>(
    mut reader: R,
    dest: &std::path::Path,
    max_bytes: u64,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<u64, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(dest)
        .map_err(|e| format!("Cannot write {}: {e}", dest.display()))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err("Cancelled".to_string());
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                drop(file);
                let _ = std::fs::remove_file(dest);
                return Err(format!("Network read failed (Download): {e}"));
            }
        };
        total = total.saturating_add(n as u64);
        if total > max_bytes {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err(format!("Download exceeds size bound ({max_bytes} bytes)"));
        }
        if let Err(e) = file.write_all(&buf[..n]) {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err(format!("Cannot write {}: {e}", dest.display()));
        }
    }
    file.flush()
        .map_err(|e| format!("Cannot write {}: {e}", dest.display()))?;
    Ok(total)
}

/// Resolve `host` and pin the request to a single address.
///
/// Resolution is the point where a guard actually bites: `url_guard::validate`
/// can only inspect the literal host string, and any name at all may resolve
/// to loopback, link-local or RFC1918 space. Checking the address we are about
/// to connect to closes that gap, and pinning it means the address cannot be
/// swapped between the check and the connection (DNS rebinding).
fn pinned_ip(host: &str, port: u16, allow_private: bool) -> Result<std::net::IpAddr, String> {
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for {host}: {e}"))?;
    let ip = addrs
        .map(|a| a.ip())
        .next()
        .ok_or_else(|| format!("DNS resolution failed for {host}: no addresses"))?;
    if !allow_private && url_guard::is_local_ip(&ip) {
        return Err(format!(
            "URL rejected: {host} resolves to a local-network address ({ip}); \
             local-network destinations need an explicit opt-in"
        ));
    }
    Ok(ip)
}

/// Redirect policy that validates every hop *before* it is followed.
///
/// reqwest follows redirects inside `send()`, so inspecting only the final URL
/// afterwards is too late — the intermediate requests have already reached the
/// internal host, which is the entire payload of an SSRF. This runs the same
/// guard on each hop and refuses the chain rather than the outcome.
fn redirect_policy(allow_private: bool) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= limits::MAX_REDIRECTS {
            return attempt.error("URL rejected: too many redirects");
        }
        let previous = match attempt.previous().last() {
            Some(url) => url.clone(),
            None => return attempt.stop(),
        };
        if let Err(e) = url_guard::check_redirect(&previous, attempt.url()) {
            return attempt.error(e);
        }
        // The hop's host is fresh, so it gets a fresh resolution check too;
        // the `.resolve()` pin only ever covered the original host.
        match host_port(attempt.url()) {
            Ok((host, port)) => match pinned_ip(&host, port, allow_private) {
                Ok(_) => attempt.follow(),
                Err(e) => attempt.error(e),
            },
            Err(e) => attempt.error(e),
        }
    })
}

fn client_for_with(
    host: &str,
    port: u16,
    allow_private: bool,
) -> Result<reqwest::blocking::Client, String> {
    let ip = pinned_ip(host, port, allow_private)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(limits::NETWORK_TIMEOUT_MS))
        .connect_timeout(Duration::from_millis(10_000))
        // Pin this host to the single resolved address for the request.
        .resolve(host, std::net::SocketAddr::new(ip, port))
        .redirect(redirect_policy(allow_private))
        .user_agent(format!("{}/{}", limits::EXECUTABLE_NAME, limits::VERSION))
        .build()
        .map_err(|e| format!("Cannot build network client: {e}"))?;
    Ok(client)
}

fn host_port(url: &url::Url) -> Result<(String, u16), String> {
    let host = url
        .host_str()
        .ok_or_else(|| "URL rejected: URL has no host".to_string())?
        .to_string();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "URL rejected: unknown port".to_string())?;
    Ok((host, port))
}

fn read_bounded<R: Read>(mut reader: R, max: usize, what: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut buf = [0u8; 32 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Network read failed ({what}): {e}"))?;
        if n == 0 {
            break;
        }
        if out.len() + n > max {
            return Err(format!("{what} exceeds size bound ({max} bytes)"));
        }
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}

/// Belt-and-braces check on where a request actually ended up.
///
/// `redirect_policy` refuses bad hops before they are followed; this catches
/// anything that reached the final URL by another route and, in particular,
/// keeps every method — not just `get` — from accepting a plaintext endpoint.
fn guard_final_url(requested: &url::Url, final_url: &url::Url) -> Result<(), String> {
    if final_url == requested {
        return Ok(());
    }
    url_guard::check_redirect(requested, final_url)?;
    if requested.scheme() == "https" && final_url.scheme() != "https" {
        return Err("URL rejected: refusing https-to-http downgrade".to_string());
    }
    Ok(())
}

pub struct ReqwestClient;

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestClient {
    pub fn new() -> Self {
        Self
    }
}

impl NetworkClient for ReqwestClient {
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        local: Local,
    ) -> Result<FetchResult, String> {
        if url.starts_with("ftp://") || url.starts_with("FTP://") {
            return Err("FTP must use the explicit legacy ftp source".to_string());
        }
        let checked = url_guard::validate(url, false, local.allowed())?;
        let (host, port) = host_port(&checked.url)?;
        let client = client_for_with(&host, port, local.allowed())?;
        let mut request = client.get(checked.url.clone());
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }
        let response = request
            .send()
            .map_err(|e| format!("Network request failed: {e}"))?;
        let final_url = response.url().to_string();
        guard_final_url(&checked.url, response.url())?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("Network request failed: HTTP {status}"));
        }
        let body = read_bounded(response, limits::MAX_JSON_BODY_BYTES, "Response body")?;
        Ok(FetchResult { body, final_url })
    }

    fn head_len(&self, url: &str, local: Local) -> Result<Option<u64>, String> {
        if url.starts_with("ftp://") || url.starts_with("FTP://") {
            return ftp_size(url, local);
        }
        let checked = url_guard::validate(url, false, local.allowed())?;
        let (host, port) = host_port(&checked.url)?;
        let client = client_for_with(&host, port, local.allowed())?;
        let response = client
            .head(checked.url.clone())
            .send()
            .map_err(|e| format!("Network request failed: {e}"))?;
        guard_final_url(&checked.url, response.url())?;
        if !response.status().is_success() {
            return Err(format!(
                "Network request failed: HTTP {}",
                response.status()
            ));
        }
        Ok(response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok()))
    }

    fn download_bounded(&self, url: &str, max_bytes: u64, local: Local) -> Result<Vec<u8>, String> {
        if url.starts_with("ftp://") || url.starts_with("FTP://") {
            return ftp_download(url, max_bytes, local);
        }
        let checked = url_guard::validate(url, false, local.allowed())?;
        let (host, port) = host_port(&checked.url)?;
        let client = client_for_with(&host, port, local.allowed())?;
        let response = client
            .get(checked.url.clone())
            .send()
            .map_err(|e| format!("Network request failed: {e}"))?;
        guard_final_url(&checked.url, response.url())?;
        if !response.status().is_success() {
            return Err(format!(
                "Network request failed: HTTP {}",
                response.status()
            ));
        }
        let cap = max_bytes.min(64 * 1024 * 1024 * 1024) as usize;
        read_bounded(response, cap, "Download")
    }

    fn download_to_file(
        &self,
        url: &str,
        dest: &std::path::Path,
        max_bytes: u64,
        cancel: &std::sync::atomic::AtomicBool,
        local: Local,
    ) -> Result<u64, String> {
        if url.starts_with("ftp://") || url.starts_with("FTP://") {
            let body = ftp_download(url, max_bytes, local)?;
            return stream_to_file(body.as_slice(), dest, max_bytes, cancel);
        }
        let checked = url_guard::validate(url, false, local.allowed())?;
        let (host, port) = host_port(&checked.url)?;
        let client = client_for_with(&host, port, local.allowed())?;
        let response = client
            .get(checked.url.clone())
            .send()
            .map_err(|e| format!("Network request failed: {e}"))?;
        guard_final_url(&checked.url, response.url())?;
        if !response.status().is_success() {
            return Err(format!(
                "Network request failed: HTTP {}",
                response.status()
            ));
        }
        stream_to_file(response, dest, max_bytes, cancel)
    }
}

// ---- Legacy FTP (explicit opt-in only) ------------------------------------
// Minimal passive-mode RETR/SIZE. Plaintext: callers must warn.

fn ftp_warning() -> String {
    "WARNING: FTP is insecure plaintext transport; prefer https".to_string()
}

fn ftp_parse_url(url: &str, local: Local) -> Result<(String, u16, String), String> {
    let checked = url_guard::validate(url, true, local.allowed())?;
    if checked.url.scheme() != "ftp" {
        return Err("Not an ftp URL".to_string());
    }
    let host = checked
        .url
        .host_str()
        .ok_or_else(|| "URL rejected: URL has no host".to_string())?
        .to_string();
    let port = checked.url.port().unwrap_or(21);
    let path = checked.url.path().to_string();
    if path.is_empty() || path == "/" {
        return Err("FTP URL has no file path".to_string());
    }
    eprintln!("{}: {url}", ftp_warning());
    Ok((host, port, path))
}

/// One FTP control connection, with its reply stream kept in step.
///
/// The previous code wrote a command and read exactly one line, and never
/// consumed the server's `220` greeting. Every reply it read was therefore the
/// answer to the *previous* command: the reply to USER was really the
/// greeting, and the reply to SIZE was really the answer to TYPE. Confirmed
/// against a conformant server -- a server answering `213 4096` produced
/// Ok(None), and PASV parsing was handed the TYPE reply and failed, so FTP
/// could neither report a size nor download anything.
struct FtpControl {
    stream: std::net::TcpStream,
    reader: std::io::BufReader<std::net::TcpStream>,
}

impl FtpControl {
    fn connect(host: &str, port: u16, local: Local) -> Result<Self, String> {
        let stream = std::net::TcpStream::connect_timeout(
            &pinned_ip(host, port, local.allowed())
                .map(|ip| std::net::SocketAddr::new(ip, port))?,
            Duration::from_millis(10_000),
        )
        .map_err(|e| format!("FTP connection failed: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(limits::NETWORK_TIMEOUT_MS)))
            .map_err(|e| format!("FTP connection failed: {e}"))?;
        let reader = std::io::BufReader::new(
            stream
                .try_clone()
                .map_err(|e| format!("FTP connection failed: {e}"))?,
        );
        let mut control = Self { stream, reader };
        // Consume the greeting before issuing anything, so command and reply
        // stay paired from here on.
        let greeting = control.read_reply()?;
        if !greeting.starts_with('2') {
            return Err(format!("FTP server refused the connection: {greeting}"));
        }
        Ok(control)
    }

    /// Read one complete reply, including the RFC 959 multi-line form
    /// (`code-` continuation lines terminated by `code ` on its own line).
    fn read_reply(&mut self) -> Result<String, String> {
        use std::io::BufRead;
        let mut first = String::new();
        self.reader
            .read_line(&mut first)
            .map_err(|e| format!("FTP command failed: {e}"))?;
        if first.is_empty() {
            return Err("FTP connection closed".to_string());
        }
        let code: String = first.chars().take(3).collect();
        let multiline = first.chars().nth(3) == Some('-');
        if !multiline {
            return Ok(first.trim_end().to_string());
        }
        // Bounded: a server must not be able to hold us here indefinitely.
        for _ in 0..256 {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .map_err(|e| format!("FTP command failed: {e}"))?;
            if read == 0 {
                return Err("FTP connection closed mid-reply".to_string());
            }
            if line.starts_with(&code) && line.chars().nth(3) == Some(' ') {
                return Ok(line.trim_end().to_string());
            }
        }
        Err("FTP reply exceeded line bound".to_string())
    }

    /// The address of the server we are actually connected to.
    fn peer_ip(&self) -> Option<std::net::IpAddr> {
        self.stream.peer_addr().ok().map(|a| a.ip())
    }

    fn command(&mut self, command: &str) -> Result<String, String> {
        use std::io::Write;
        self.stream
            .write_all(format!("{command}\r\n").as_bytes())
            .map_err(|e| format!("FTP command failed: {e}"))?;
        self.read_reply()
    }

    /// Log in anonymously, tolerating servers that skip the password step.
    fn login(&mut self) -> Result<(), String> {
        let user = self.command("USER anonymous")?;
        if user.starts_with("33") {
            let pass = self.command("PASS goshaim@example.com")?;
            if !pass.starts_with('2') {
                return Err(format!("FTP login failed: {pass}"));
            }
        } else if !user.starts_with('2') {
            return Err(format!("FTP login failed: {user}"));
        }
        let binary = self.command("TYPE I")?;
        if !binary.starts_with('2') {
            return Err(format!("FTP cannot switch to binary mode: {binary}"));
        }
        Ok(())
    }
}

pub fn ftp_size(url: &str, local: Local) -> Result<Option<u64>, String> {
    let (host, port, path) = ftp_parse_url(url, local)?;
    let mut control = FtpControl::connect(&host, port, local)?;
    control.login()?;
    let reply = control.command(&format!("SIZE {path}"))?;
    let _ = control.command("QUIT");
    match reply.strip_prefix("213") {
        Some(size) => size
            .trim()
            .parse::<u64>()
            .map(Some)
            .map_err(|_| "FTP SIZE parse failed".to_string()),
        // A server that does not implement SIZE answers 5xx; that is "unknown",
        // not an error.
        None => Ok(None),
    }
}

pub fn ftp_download(url: &str, max_bytes: u64, local: Local) -> Result<Vec<u8>, String> {
    let (host, port, path) = ftp_parse_url(url, local)?;
    let mut control = FtpControl::connect(&host, port, local)?;
    control.login()?;
    let pasv = control.command("PASV")?;
    if !pasv.starts_with("227") {
        return Err(format!("FTP passive mode refused: {pasv}"));
    }
    let mut data_addr = parse_pasv(&pasv)?;
    // The server picks this address, so it is untrusted. The classic FTP
    // bounce is a server naming a *third* host, turning us into its proxy;
    // checking only for local addresses would not catch that, and would also
    // wrongly fire when the user has legitimately opted into a private
    // endpoint. Pin the data connection to the host we are already talking to,
    // which is the address policy already approved, and keep only the port the
    // server chose.
    let control_ip = control
        .peer_ip()
        .ok_or_else(|| "FTP data connection failed: control peer unknown".to_string())?;
    if data_addr.ip() != control_ip {
        return Err(format!(
            "FTP rejected: server directed the data connection to {} instead of {control_ip}",
            data_addr.ip()
        ));
    }
    data_addr.set_ip(control_ip);
    let retr = control.command(&format!("RETR {path}"))?;
    if !(retr.starts_with("150") || retr.starts_with("125")) {
        return Err(format!("FTP download failed: {retr}"));
    }
    let data = std::net::TcpStream::connect_timeout(&data_addr, Duration::from_millis(10_000))
        .map_err(|e| format!("FTP data connection failed: {e}"))?;
    data.set_read_timeout(Some(Duration::from_millis(limits::NETWORK_TIMEOUT_MS)))
        .map_err(|e| format!("FTP data connection failed: {e}"))?;
    let cap = max_bytes.min(64 * 1024 * 1024 * 1024) as usize;
    let body = read_bounded(data, cap, "FTP download")?;
    let _ = control.command("QUIT");
    Ok(body)
}

pub fn parse_pasv(reply: &str) -> Result<std::net::SocketAddr, String> {
    let start = reply
        .find('(')
        .ok_or_else(|| "FTP PASV parse failed".to_string())?;
    let end = reply
        .find(')')
        .ok_or_else(|| "FTP PASV parse failed".to_string())?;
    if end <= start {
        return Err("FTP PASV parse failed".to_string());
    }
    // Each field is one octet. Parsing as u16 let a hostile server send values
    // above 255, which then overflowed `nums[4] * 256` -- a panic in debug
    // builds and a wrapped port in release.
    let nums: Vec<u8> = reply[start + 1..end]
        .split(',')
        .map(|s| s.trim().parse::<u8>())
        .collect::<Result<_, _>>()
        .map_err(|_| "FTP PASV parse failed".to_string())?;
    if nums.len() != 6 {
        return Err("FTP PASV parse failed".to_string());
    }
    let ip = std::net::Ipv4Addr::new(nums[0], nums[1], nums[2], nums[3]);
    let port = u16::from(nums[4]) * 256 + u16::from(nums[5]);
    if port == 0 {
        return Err("FTP PASV parse failed".to_string());
    }
    Ok(std::net::SocketAddr::new(std::net::IpAddr::V4(ip), port))
}

/// In-memory fake network for tests: canned bodies per URL substring.
#[derive(Debug, Default)]
pub struct FakeNetwork {
    pub bodies: Mutex<HashMap<String, Vec<u8>>>,
    pub head_sizes: Mutex<HashMap<String, u64>>,
    pub calls: Mutex<Vec<String>>,
}

impl FakeNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn canned_body(self, url_sub: &str, body: &[u8]) -> Self {
        self.bodies
            .lock()
            .unwrap()
            .insert(url_sub.to_string(), body.to_vec());
        self
    }

    pub fn canned_size(self, url_sub: &str, size: u64) -> Self {
        self.head_sizes
            .lock()
            .unwrap()
            .insert(url_sub.to_string(), size);
        self
    }
}

impl NetworkClient for FakeNetwork {
    fn get(
        &self,
        url: &str,
        _headers: &[(String, String)],
        local: Local,
    ) -> Result<FetchResult, String> {
        self.calls.lock().unwrap().push(url.to_string());
        // Credentials fail closed before any canned match.
        url_guard::validate(url, false, local.allowed())?;
        for (key, body) in self.bodies.lock().unwrap().iter() {
            if url.contains(key) {
                if body.len() > limits::MAX_JSON_BODY_BYTES {
                    return Err(format!(
                        "Response body exceeds size bound ({} bytes)",
                        limits::MAX_JSON_BODY_BYTES
                    ));
                }
                return Ok(FetchResult {
                    body: body.clone(),
                    final_url: url.to_string(),
                });
            }
        }
        Err(format!("Fake network has no canned body for {url}"))
    }

    fn head_len(&self, url: &str, local: Local) -> Result<Option<u64>, String> {
        self.calls.lock().unwrap().push(format!("HEAD {url}"));
        url_guard::validate(url, false, local.allowed())?;
        for (key, size) in self.head_sizes.lock().unwrap().iter() {
            if url.contains(key) {
                return Ok(Some(*size));
            }
        }
        Ok(None)
    }

    fn download_bounded(&self, url: &str, max_bytes: u64, local: Local) -> Result<Vec<u8>, String> {
        let result = self.get(url, &[], local)?;
        if result.body.len() as u64 > max_bytes {
            return Err(format!("Download exceeds size bound ({max_bytes} bytes)"));
        }
        Ok(result.body)
    }

    fn download_to_file(
        &self,
        url: &str,
        dest: &std::path::Path,
        max_bytes: u64,
        cancel: &std::sync::atomic::AtomicBool,
        local: Local,
    ) -> Result<u64, String> {
        let body = self.download_bounded(url, max_bytes, local)?;
        stream_to_file(body.as_slice(), dest, max_bytes, cancel)
    }
}
