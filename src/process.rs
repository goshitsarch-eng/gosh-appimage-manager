// Gosh AppImage Manager — argument-array process runner (ports ProcessRunner).
// Programs are spawned from argv arrays only. Shell strings are never built.
// Output is bounded; wall-clock timeouts are enforced; failures fail closed.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::limits;

#[derive(Debug, Clone, Default)]
pub struct ProcessRequest {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Run on the host via flatpak-spawn when sandboxed.
    pub host: bool,
    pub timeout_ms: u64,
    pub work_dir: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessResult {
    pub program: String,
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// True when the runner refused to run (used by probes).
    pub refused: bool,
    pub timed_out: bool,
}

pub trait ProcessRunner: Send + Sync {
    fn run(&self, req: &ProcessRequest) -> ProcessResult;
    /// Start-only detached launch; never waits, never kills.
    fn start_detached(&self, req: &ProcessRequest) -> Result<(), String>;
}

pub fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
        || std::env::var("FLATPAK_ID").as_deref() == Ok(limits::APP_ID)
}

/// Resolve the argv actually spawned: host requests inside a sandbox go
/// through arg-safe `flatpak-spawn --host` (no shell involved).
pub fn resolve_argv(req: &ProcessRequest) -> (String, Vec<String>) {
    if req.host && in_flatpak() {
        let mut args = vec!["--host".to_string(), req.program.clone()];
        args.extend(req.args.iter().cloned());
        ("flatpak-spawn".to_string(), args)
    } else {
        (req.program.clone(), req.args.clone())
    }
}

pub struct SystemRunner;

impl Default for SystemRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemRunner {
    pub fn new() -> Self {
        Self
    }

    fn spawn_command(
        &self,
        req: &ProcessRequest,
        detached: bool,
    ) -> Result<std::process::Child, String> {
        let (program, args) = resolve_argv(req);
        if program.is_empty() || program.contains('\0') {
            return Err("Refused to run empty program".to_string());
        }
        let mut cmd = Command::new(&program);
        cmd.args(&args);
        for (key, value) in &req.env {
            if key.is_empty() || key.contains('\0') || value.contains('\0') {
                continue;
            }
            cmd.env(key, value);
        }
        if !req.work_dir.is_empty() {
            cmd.current_dir(&req.work_dir);
        }
        if detached {
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
        } else {
            cmd.stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
        }
        // Detach from our process group so the child outlives us.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    libc_setsdt();
                    Ok(())
                });
            }
        }
        cmd.spawn()
            .map_err(|e| format!("Cannot start {}: {e}", req.program))
    }
}

#[cfg(unix)]
fn libc_setsdt() {
    unsafe { libc_setsid() };
}

#[cfg(unix)]
unsafe fn libc_setsid() {
    extern "C" {
        fn setsid() -> i32;
    }
    setsid();
}

impl ProcessRunner for SystemRunner {
    fn run(&self, req: &ProcessRequest) -> ProcessResult {
        let mut result = ProcessResult {
            program: req.program.clone(),
            ..Default::default()
        };
        let mut child = match self.spawn_command(req, false) {
            Ok(child) => child,
            Err(e) => {
                result.refused = true;
                result.stderr = e.into_bytes();
                return result;
            }
        };
        let timeout = Duration::from_millis(req.timeout_ms.max(1));
        let mut stdout_taken = child.stdout.take();
        let mut stderr_taken = child.stderr.take();
        // Bounded output readers.
        let stdout_handle = std::thread::spawn(move || {
            let mut out = Vec::new();
            if let Some(reader) = stdout_taken.as_mut() {
                use std::io::Read;
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if out.len() + n > limits::MAX_PROCESS_OUTPUT_BYTES + 1 {
                                out.extend_from_slice(&buf[..n]);
                                break;
                            }
                            out.extend_from_slice(&buf[..n]);
                        }
                        Err(_) => break,
                    }
                }
            }
            out
        });
        let stderr_handle = std::thread::spawn(move || {
            let mut out = Vec::new();
            if let Some(reader) = stderr_taken.as_mut() {
                use std::io::Read;
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if out.len() + n > limits::MAX_PROCESS_OUTPUT_BYTES + 1 {
                                out.extend_from_slice(&buf[..n]);
                                break;
                            }
                            out.extend_from_slice(&buf[..n]);
                        }
                        Err(_) => break,
                    }
                }
            }
            out
        });
        let exit = {
            use wait_timeout::ChildExt;
            match child.wait_timeout(timeout) {
                Ok(Some(status)) => Some(status),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    result.timed_out = true;
                    None
                }
                Err(_) => {
                    let _ = child.kill();
                    result.timed_out = true;
                    None
                }
            }
        };
        result.stdout = stdout_handle.join().unwrap_or_default();
        result.stderr = stderr_handle.join().unwrap_or_default();
        if result.stdout.len() > limits::MAX_PROCESS_OUTPUT_BYTES
            || result.stderr.len() > limits::MAX_PROCESS_OUTPUT_BYTES
        {
            result.refused = false;
            result.stderr = b"Archive listing exceeded output bound".to_vec();
            result.exit_code = 1;
            return result;
        }
        match exit {
            Some(status) => result.exit_code = status.code().unwrap_or(1),
            None => {
                result.exit_code = 1;
                if result.stderr.is_empty() {
                    result.stderr = b"Process timed out".to_vec();
                }
            }
        }
        result
    }

    fn start_detached(&self, req: &ProcessRequest) -> Result<(), String> {
        let child = self.spawn_command(req, true)?;
        // Forget the child: start-only semantics, never wait, never kill.
        std::mem::forget(child);
        Ok(())
    }
}

/// In-memory fake runner for tests: canned stdout per program substring.
#[derive(Debug, Default)]
pub struct FakeRunner {
    pub outputs: HashMap<String, (i32, Vec<u8>)>,
    pub fail_start: bool,
    pub spawned: std::sync::Mutex<Vec<(String, Vec<String>)>>,
}

impl FakeRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn canned(mut self, program_sub: &str, exit: i32, stdout: &[u8]) -> Self {
        self.outputs
            .insert(program_sub.to_string(), (exit, stdout.to_vec()));
        self
    }
}

impl ProcessRunner for FakeRunner {
    fn run(&self, req: &ProcessRequest) -> ProcessResult {
        let mut result = ProcessResult {
            program: req.program.clone(),
            ..Default::default()
        };
        for (key, (exit, out)) in &self.outputs {
            if req.program.contains(key) {
                result.exit_code = *exit;
                result.stdout = out.clone();
                if out.len() > limits::MAX_PROCESS_OUTPUT_BYTES {
                    result.stderr = b"Archive listing exceeded output bound".to_vec();
                    result.exit_code = 1;
                }
                return result;
            }
        }
        result
    }

    fn start_detached(&self, req: &ProcessRequest) -> Result<(), String> {
        if self.fail_start {
            return Err("Cannot start: fake spawn failure".to_string());
        }
        self.spawned
            .lock()
            .unwrap()
            .push((req.program.clone(), req.args.clone()));
        Ok(())
    }
}
