// Gosh AppImage Manager — argument-array process runner (ports ProcessRunner).
// Programs are spawned from argv arrays only. Shell strings are never built.
// Output is bounded; wall-clock timeouts are enforced; failures fail closed.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::limits;

/// Whether a request may leave the sandbox, and as what.
///
/// The Flatpak manifest grants `--talk-name=org.freedesktop.Flatpak` so the
/// manager can run an AppImage on the host. That grant is arbitrary host
/// command execution, and it cannot be given up without giving up launching.
/// What it *can* be given is a narrow definition of what is allowed through
/// it, enforced at the one place every spawn passes: a fixed set of helper
/// programs, plus AppImages the caller resolved from the registry. Anything
/// else is refused before a process is created, so influencing a
/// ProcessRequest is not enough to run something arbitrary on the host.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HostSpawn {
    /// Stay inside the sandbox.
    #[default]
    No,
    /// One of the fixed helper programs in `HOST_HELPERS`.
    Helper,
    /// An AppImage this application manages, named by absolute path.
    ManagedAppImage,
}

/// Helper programs the manager is allowed to run on the host.
///
/// Each is here because a specific feature needs it: trashing a file the
/// sandbox cannot reach, finding running applications in the host's PID
/// namespace, opening a file manager, refreshing the desktop database, the
/// NixOS AppImage shim, and the no-op used by `--probe-host`.
pub const HOST_HELPERS: &[&str] = &[
    "gio",
    "pgrep",
    "xdg-open",
    "update-desktop-database",
    "appimage-run",
    "true",
];

#[derive(Debug, Clone, Default)]
pub struct ProcessRequest {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Whether this may run on the host via flatpak-spawn, and as what.
    pub host: HostSpawn,
    pub timeout_ms: u64,
    pub work_dir: String,
    /// Start the child with a minimal environment: nothing inherited, only PATH
    /// (fixed), LANG=C, and HOME and TMPDIR set to `work_dir`. Used for code the
    /// manager does not trust, which must not see the manager's environment.
    pub minimal_env: bool,
}

/// The PATH a minimal-environment child gets: fixed, not inherited.
const MINIMAL_PATH: &str = "/usr/bin:/bin";

/// Is this request allowed to run on the host at all?
///
/// Applied whether or not we are sandboxed, so the same rule is exercised in
/// development and in the Flatpak rather than only in the configuration that
/// is hardest to test.
pub fn host_spawn_permitted(req: &ProcessRequest) -> Result<(), String> {
    match req.host {
        HostSpawn::No => Ok(()),
        HostSpawn::Helper => {
            if HOST_HELPERS.contains(&req.program.as_str()) {
                Ok(())
            } else {
                Err(format!(
                    "Refusing to run {} on the host: not a known helper",
                    req.program
                ))
            }
        }
        HostSpawn::ManagedAppImage => {
            let path = Path::new(&req.program);
            if !path.is_absolute() {
                return Err(format!(
                    "Refusing to run {} on the host: managed applications are named by \
                     absolute path",
                    req.program
                ));
            }
            // The caller resolved this from the registry; confirm it is still
            // a real file rather than trusting the string it handed us.
            match std::fs::metadata(path) {
                Ok(meta) if meta.is_file() => Ok(()),
                _ => Err(format!(
                    "Refusing to run {} on the host: not a regular file",
                    req.program
                )),
            }
        }
    }
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
    if req.host != HostSpawn::No && in_flatpak() {
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
        // Enforce the host policy before anything is spawned.
        host_spawn_permitted(req)?;
        let (program, args) = resolve_argv(req);
        if program.is_empty() || program.contains('\0') {
            return Err("Refused to run empty program".to_string());
        }
        let mut cmd = Command::new(&program);
        cmd.args(&args);
        if req.minimal_env {
            cmd.env_clear();
            cmd.env("PATH", MINIMAL_PATH);
            cmd.env("LANG", "C");
            if !req.work_dir.is_empty() {
                cmd.env("HOME", &req.work_dir);
                cmd.env("TMPDIR", &req.work_dir);
            }
        }
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
        spawn_retrying_busy(&mut cmd).map_err(|e| format!("Cannot start {}: {e}", req.program))
    }
}

/// Start a child, retrying briefly when the executable is "text file busy".
///
/// That error (ETXTBSY) means the file was still open for writing, or a concurrent
/// fork still holds its write descriptor. A file that was just written and is about
/// to run hits it for a few milliseconds, so waiting clears it. Any other error is
/// returned at once.
fn spawn_retrying_busy(cmd: &mut Command) -> std::io::Result<std::process::Child> {
    #[cfg(unix)]
    const ETXTBSY: i32 = 26;
    const ATTEMPTS: u32 = 50;
    let mut attempt = 0;
    loop {
        match cmd.spawn() {
            #[cfg(unix)]
            Err(e) if e.raw_os_error() == Some(ETXTBSY) && attempt < ATTEMPTS => {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(10));
            }
            other => return other,
        }
    }
}

/// Read a child's output pipe to its end, bounded by the output limit.
fn spawn_pipe_reader<R>(pipe: Option<R>) -> std::io::Result<std::thread::JoinHandle<Vec<u8>>>
where
    R: std::io::Read + Send + 'static,
{
    std::thread::Builder::new().spawn(move || {
        let mut out = Vec::new();
        if let Some(mut reader) = pipe {
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
    })
}

/// Stop a child that started but cannot be watched, and report why.
fn abandon_child(
    mut child: std::process::Child,
    mut result: ProcessResult,
    program: &str,
    error: std::io::Error,
) -> ProcessResult {
    kill_process_group(child.id());
    let _ = child.kill();
    let _ = child.wait();
    result.refused = true;
    result.exit_code = 1;
    result.stderr = format!("Cannot watch {program} while it runs: {error}").into_bytes();
    result
}

/// How often the runner checks a running child while it waits for it.
const EXIT_POLL: Duration = Duration::from_millis(5);

/// Wait up to `timeout` for `child` to exit. `Ok(None)` means it was still
/// running when the time was up.
///
/// This polls `try_wait` instead of waiting on a SIGCHLD handler. A handler is
/// one per process, and the state it shares can be disturbed by any other
/// reaper of children. The wait-timeout crate did that, and a watched child
/// that was reaped elsewhere aborted the whole process. See
/// tests/test_process.rs.
fn wait_for_exit<W: ExitPoll + ?Sized>(
    child: &mut W,
    timeout: Duration,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    let started = std::time::Instant::now();
    loop {
        match child.poll_exit() {
            Ok(Some(status)) => return Ok(Some(status)),
            Ok(None) => {}
            // An interrupted or would-block poll says nothing about the child.
            // Ask again, rather than reporting a child that is still running as
            // one that cannot be watched.
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                ) => {}
            Err(error) => return Err(error),
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            return Ok(None);
        }
        std::thread::sleep(EXIT_POLL.min(timeout - elapsed));
    }
}

/// What the runner asks of a child while it waits: has it exited yet?
trait ExitPoll {
    fn poll_exit(&mut self) -> std::io::Result<Option<std::process::ExitStatus>>;
}

impl ExitPoll for std::process::Child {
    fn poll_exit(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.try_wait()
    }
}

#[cfg(all(test, unix))]
mod wait_tests {
    use std::collections::VecDeque;
    use std::io;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::time::Duration;

    use super::{wait_for_exit, ExitPoll};

    /// Answers a scripted sequence of polls, then reports that the child runs.
    struct Scripted(VecDeque<io::Result<Option<ExitStatus>>>);

    impl ExitPoll for Scripted {
        fn poll_exit(&mut self) -> io::Result<Option<ExitStatus>> {
            self.0.pop_front().unwrap_or(Ok(None))
        }
    }

    fn exited() -> ExitStatus {
        ExitStatus::from_raw(0)
    }

    #[test]
    fn a_transient_failure_to_poll_is_asked_again_not_reported() {
        let mut child = Scripted(VecDeque::from([
            Err(io::Error::from(io::ErrorKind::WouldBlock)),
            Err(io::Error::from(io::ErrorKind::Interrupted)),
            Ok(None),
            Ok(Some(exited())),
        ]));
        let status = wait_for_exit(&mut child, Duration::from_secs(10))
            .expect("transient failures are not errors");
        assert_eq!(status, Some(exited()), "the exit is still seen");
    }

    #[test]
    fn a_failure_that_is_not_transient_is_returned_to_the_caller() {
        // ECHILD: the child was reaped by someone else.
        let mut child = Scripted(VecDeque::from([Err(io::Error::from_raw_os_error(10))]));
        let error = wait_for_exit(&mut child, Duration::from_secs(10))
            .expect_err("a reaped child cannot be watched");
        assert_eq!(error.raw_os_error(), Some(10));
    }

    #[test]
    fn a_child_still_running_at_the_deadline_is_reported_as_running() {
        let mut child = Scripted(VecDeque::new());
        let outcome = wait_for_exit(&mut child, Duration::from_millis(30))
            .expect("a running child is not an error");
        assert_eq!(outcome, None);
    }
}

/// `ECHILD` on Linux and the BSDs: the child is no longer ours to wait for.
#[cfg(unix)]
const ECHILD: i32 = 10;

/// Report a child the runner can no longer watch. A child that someone else
/// reaped has a pid that may already belong to another process, so it is not
/// signalled or waited for. Any other failure leaves a child that may still be
/// running, which is stopped.
fn unwatchable_child(
    mut child: std::process::Child,
    mut result: ProcessResult,
    program: &str,
    error: std::io::Error,
) -> ProcessResult {
    #[cfg(unix)]
    let reaped_elsewhere = error.raw_os_error() == Some(ECHILD);
    #[cfg(not(unix))]
    let reaped_elsewhere = false;
    if !reaped_elsewhere {
        let _ = child.kill();
        let _ = child.wait();
    }
    result.refused = true;
    result.exit_code = 1;
    result.stderr = format!("Cannot watch {program} while it runs: {error}").into_bytes();
    result
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

/// Kill the process group a child leads. Every child is made a session leader
/// before it runs, so its group id is its own pid, and whatever it started shares
/// that group. Killing only the child would leave those processes running.
#[cfg(unix)]
fn kill_process_group(pid: u32) {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const SIGKILL: i32 = 9;
    if let Ok(pid) = i32::try_from(pid) {
        if pid > 0 {
            // SAFETY: a plain system call on a process group id we created.
            unsafe {
                kill(-pid, SIGKILL);
            }
        }
    }
}

#[cfg(not(unix))]
fn kill_process_group(_pid: u32) {}

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
        // Bounded output readers. If a thread cannot be made, the child is already
        // running, so it is stopped and reported, never left behind.
        let stdout_handle = match spawn_pipe_reader(child.stdout.take()) {
            Ok(handle) => handle,
            Err(e) => return abandon_child(child, result, &req.program, e),
        };
        let stderr_handle = match spawn_pipe_reader(child.stderr.take()) {
            Ok(handle) => handle,
            Err(e) => return abandon_child(child, result, &req.program, e),
        };
        let exit = match wait_for_exit(&mut child, timeout) {
            Ok(Some(status)) => Some(status),
            Ok(None) => {
                kill_process_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
                result.timed_out = true;
                None
            }
            // The child was reaped by someone else, or its state cannot be read.
            // That is not a timeout, and it must not end the process.
            Err(e) => return unwatchable_child(child, result, &req.program, e),
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
        if let Err(error) = host_spawn_permitted(req) {
            result.refused = true;
            result.stderr = error.into_bytes();
            return result;
        }
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
        host_spawn_permitted(req)?;
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
