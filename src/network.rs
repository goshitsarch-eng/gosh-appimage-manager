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

pub trait NetworkClient: Send + Sync {
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<FetchResult, String>;
    fn head_len(&self, url: &str) -> Result<Option<u64>, String>;
    fn download_bounded(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>, String>;
}

fn pinned_ip(host: &str, port: u16) -> Result<std::net::IpAddr, String> {
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for {host}: {e}"))?;
    addrs
        .map(|a| a.ip())
        .next()
        .ok_or_else(|| format!("DNS resolution failed for {host}: no addresses"))
}

fn client_for(host: &str, port: u16) -> Result<reqwest::blocking::Client, String> {
    let ip = pinned_ip(host, port)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(limits::NETWORK_TIMEOUT_MS))
        .connect_timeout(Duration::from_millis(10_000))
        // Pin this host to the single resolved address for the request.
        .resolve(host, std::net::SocketAddr::new(ip, port))
        .redirect(reqwest::redirect::Policy::limited(limits::MAX_REDIRECTS))
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
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<FetchResult, String> {
        if url.starts_with("ftp://") || url.starts_with("FTP://") {
            return Err("FTP must use the explicit legacy ftp source".to_string());
        }
        let checked = url_guard::validate(url, false, false)?;
        let (host, port) = host_port(&checked.url)?;
        let client = client_for(&host, port)?;
        let mut request = client.get(checked.url.clone());
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }
        let response = request
            .send()
            .map_err(|e| format!("Network request failed: {e}"))?;
        let final_url = response.url().to_string();
        // Downgrade + credential guard across the redirect chain.
        let final_parsed =
            url::Url::parse(&final_url).map_err(|_| "URL rejected: bad redirect".to_string())?;
        url_guard::check_redirect(&checked.url, &final_parsed)?;
        if final_parsed.scheme() == "http" {
            return Err("URL rejected: refusing https-to-http downgrade".to_string());
        }
        let status = response.status();
        if !status.is_success() {
            return Err(format!("Network request failed: HTTP {status}"));
        }
        let body = read_bounded(response, limits::MAX_JSON_BODY_BYTES, "Response body")?;
        Ok(FetchResult { body, final_url })
    }

    fn head_len(&self, url: &str) -> Result<Option<u64>, String> {
        if url.starts_with("ftp://") || url.starts_with("FTP://") {
            return ftp_size(url);
        }
        let checked = url_guard::validate(url, false, false)?;
        let (host, port) = host_port(&checked.url)?;
        let client = client_for(&host, port)?;
        let response = client
            .head(checked.url.clone())
            .send()
            .map_err(|e| format!("Network request failed: {e}"))?;
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

    fn download_bounded(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
        if url.starts_with("ftp://") || url.starts_with("FTP://") {
            return ftp_download(url, max_bytes);
        }
        let checked = url_guard::validate(url, false, false)?;
        let (host, port) = host_port(&checked.url)?;
        let client = client_for(&host, port)?;
        let response = client
            .get(checked.url.clone())
            .send()
            .map_err(|e| format!("Network request failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Network request failed: HTTP {}",
                response.status()
            ));
        }
        let cap = max_bytes.min(64 * 1024 * 1024 * 1024) as usize;
        read_bounded(response, cap, "Download")
    }
}

// ---- Legacy FTP (explicit opt-in only) ------------------------------------
// Minimal passive-mode RETR/SIZE. Plaintext: callers must warn.

fn ftp_warning() -> String {
    "WARNING: FTP is insecure plaintext transport; prefer https".to_string()
}

fn ftp_parse_url(url: &str) -> Result<(String, u16, String), String> {
    let checked = url_guard::validate(url, true, false)?;
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

fn ftp_control(host: &str, port: u16) -> Result<std::net::TcpStream, String> {
    let stream = std::net::TcpStream::connect_timeout(
        &pinned_ip(host, port).map(|ip| std::net::SocketAddr::new(ip, port))?,
        Duration::from_millis(10_000),
    )
    .map_err(|e| format!("FTP connection failed: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(limits::NETWORK_TIMEOUT_MS)))
        .map_err(|e| format!("FTP connection failed: {e}"))?;
    Ok(stream)
}

fn ftp_exchange(stream: &mut std::net::TcpStream, command: &str) -> Result<String, String> {
    use std::io::Write;
    stream
        .write_all(format!("{command}\r\n").as_bytes())
        .map_err(|e| format!("FTP command failed: {e}"))?;
    let mut reader = std::io::BufReader::new(
        stream
            .try_clone()
            .map_err(|e| format!("FTP command failed: {e}"))?,
    );
    let mut line = String::new();
    use std::io::BufRead;
    reader
        .read_line(&mut line)
        .map_err(|e| format!("FTP command failed: {e}"))?;
    Ok(line)
}

pub fn ftp_size(url: &str) -> Result<Option<u64>, String> {
    let (host, port, path) = ftp_parse_url(url)?;
    let mut control = ftp_control(&host, port)?;
    let _ = ftp_exchange(&mut control, "USER anonymous")?;
    let _ = ftp_exchange(&mut control, "PASS goshaim@example.com")?;
    let _ = ftp_exchange(&mut control, "TYPE I")?;
    let reply = ftp_exchange(&mut control, &format!("SIZE {path}"))?;
    let _ = ftp_exchange(&mut control, "QUIT");
    match reply.strip_prefix("213") {
        Some(size) => size
            .trim()
            .parse::<u64>()
            .map(Some)
            .map_err(|_| "FTP SIZE parse failed".to_string()),
        None => Ok(None),
    }
}

pub fn ftp_download(url: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let (host, port, path) = ftp_parse_url(url)?;
    let mut control = ftp_control(&host, port)?;
    let _ = ftp_exchange(&mut control, "USER anonymous")?;
    let _ = ftp_exchange(&mut control, "PASS goshaim@example.com")?;
    let _ = ftp_exchange(&mut control, "TYPE I")?;
    let pasv = ftp_exchange(&mut control, "PASV")?;
    let data_addr = parse_pasv(&pasv)?;
    control
        .write_all(format!("RETR {path}\r\n").as_bytes())
        .map_err(|e| format!("FTP download failed: {e}"))?;
    let data = std::net::TcpStream::connect_timeout(&data_addr, Duration::from_millis(10_000))
        .map_err(|e| format!("FTP data connection failed: {e}"))?;
    data.set_read_timeout(Some(Duration::from_millis(limits::NETWORK_TIMEOUT_MS)))
        .map_err(|e| format!("FTP data connection failed: {e}"))?;
    let cap = max_bytes.min(64 * 1024 * 1024 * 1024) as usize;
    let body = read_bounded(data, cap, "FTP download")?;
    let _ = ftp_exchange(&mut control, "QUIT");
    Ok(body)
}

fn parse_pasv(reply: &str) -> Result<std::net::SocketAddr, String> {
    let start = reply
        .find('(')
        .ok_or_else(|| "FTP PASV parse failed".to_string())?;
    let end = reply
        .find(')')
        .ok_or_else(|| "FTP PASV parse failed".to_string())?;
    let nums: Vec<u16> = reply[start + 1..end]
        .split(',')
        .map(|s| s.trim().parse::<u16>())
        .collect::<Result<_, _>>()
        .map_err(|_| "FTP PASV parse failed".to_string())?;
    if nums.len() != 6 {
        return Err("FTP PASV parse failed".to_string());
    }
    let ip = std::net::Ipv4Addr::new(nums[0] as u8, nums[1] as u8, nums[2] as u8, nums[3] as u8);
    Ok(std::net::SocketAddr::new(
        std::net::IpAddr::V4(ip),
        nums[4] * 256 + nums[5],
    ))
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
    fn get(&self, url: &str, _headers: &[(String, String)]) -> Result<FetchResult, String> {
        self.calls.lock().unwrap().push(url.to_string());
        // Credentials fail closed before any canned match.
        url_guard::validate(url, false, false)?;
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

    fn head_len(&self, url: &str) -> Result<Option<u64>, String> {
        self.calls.lock().unwrap().push(format!("HEAD {url}"));
        url_guard::validate(url, false, false)?;
        for (key, size) in self.head_sizes.lock().unwrap().iter() {
            if url.contains(key) {
                return Ok(Some(*size));
            }
        }
        Ok(None)
    }

    fn download_bounded(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
        let result = self.get(url, &[])?;
        if result.body.len() as u64 > max_bytes {
            return Err(format!("Download exceeds size bound ({max_bytes} bytes)"));
        }
        Ok(result.body)
    }
}
