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
///
/// Environment pairs have to be forwarded explicitly with `--env=K=V`.
/// Setting them on the child process sets them on `flatpak-spawn`, not on the
/// program it starts on the host, so a user's custom variables were silently
/// dropped in the Flatpak -- the only configuration we ship. Each pair is one
/// argv element, so a value containing spaces or quotes needs no escaping and
/// cannot be re-split.
pub fn resolve_argv(req: &ProcessRequest) -> (String, Vec<String>) {
    if req.host && in_flatpak() {
        let mut args = vec!["--host".to_string()];
        for (key, value) in &req.env {
            if key.is_empty() || key.contains('\0') || key.contains('=') || value.contains('\0') {
                continue;
            }
            args.push(format!("--env={key}={value}"));
        }
        args.push(req.program.clone());
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

/// Join an output-reader thread, giving up if it is stuck on a pipe held open
/// by a grandchild rather than blocking the caller forever.
fn join_bounded(handle: std::thread::JoinHandle<Vec<u8>>) -> Vec<u8> {
    let deadline = std::time::Instant::now() + Duration::from_millis(READER_JOIN_GRACE_MS);
    while !handle.is_finished() {
        if std::time::Instant::now() >= deadline {
            // Leave it parked on the pipe; it holds nothing the caller needs
            // and exits when the last writer closes.
            return Vec::new();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    handle.join().unwrap_or_default()
}

const READER_JOIN_GRACE_MS: u64 = 2_000;

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
                    // Reap here too; the previous code killed without waiting
                    // and left a zombie behind on this path.
                    let _ = child.wait();
                    result.timed_out = true;
                    None
                }
            }
        };
        // Killing the child closes our copies of the pipe ends, but a
        // grandchild that inherited them keeps the reader threads blocked. An
        // unconditional join would then hang the caller indefinitely and
        // defeat the timeout entirely -- and on the GUI that caller is the
        // thread drawing the window. Take whatever the readers have collected
        // by the deadline and let any stragglers finish detached.
        result.stdout = join_bounded(stdout_handle);
        result.stderr = join_bounded(stderr_handle);
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
        let mut child = self.spawn_command(req, true)?;
        // Start-only semantics: report success once the process has started,
        // never wait for it and never kill it.
        //
        // The child still has to be reaped, though. setsid() makes it a
        // session leader but does not reparent it, so it stays our direct
        // child and becomes a zombie the moment it exits. `mem::forget` used
        // to be used here to express "don't touch it", which is exactly what
        // left the zombie: a long-lived GUI session accumulated one per launch
        // until it hit the per-user process limit.
        //
        // Hand the reap to a detached thread instead. It blocks in wait()
        // for as long as the app runs -- which costs one parked thread and
        // nothing else -- and neither the caller nor the launched app waits
        // on anything.
        std::thread::Builder::new()
            .name("goshaim-reap".to_string())
            .spawn(move || {
                let _ = child.wait();
            })
            .map_err(|e| format!("Cannot start {}: {e}", req.program))?;
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
