// Gosh AppImage Manager — process table seam (ports ProcessTable).
// Used to block updates while the app is running and to badge Library rows.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::process::{ProcessRequest, ProcessRunner};

pub trait ProcessTable: Send + Sync {
    fn pids_for_executable(&self, executable: &str) -> Vec<u32>;
    fn is_running(&self, executable: &str) -> bool {
        !self.pids_for_executable(executable).is_empty()
    }
}

/// Real table: scans `/proc` for exact exe matches (never matches self).
///
/// Inside a Flatpak sandbox `/proc` is the sandbox's own PID namespace, so it
/// contains this process and nothing else. Scanning it there does not find a
/// running AppImage -- it finds nothing, every time -- which silently turned
/// the "running apps block updates" guarantee into "updates are never
/// blocked" in the only configuration we ship. When sandboxed, the lookup is
/// therefore delegated to the host through the existing argument-safe
/// `flatpak-spawn --host` path.
pub struct SysTable {
    host: Option<Box<dyn ProcessRunner>>,
}

impl Default for SysTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SysTable {
    pub fn new() -> Self {
        Self { host: None }
    }

    /// Give the table a runner so it can ask the host when sandboxed.
    pub fn with_host_runner(runner: Box<dyn ProcessRunner>) -> Self {
        Self { host: Some(runner) }
    }

    /// Read the local `/proc`. Correct outside a sandbox; blind inside one.
    fn scan_proc(executable: &str) -> Vec<u32> {
        let target = std::fs::read_link("/proc/self/exe").ok();
        let self_pid = std::process::id();
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return out;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(pid): Result<u32, _> = name.parse() else {
                continue;
            };
            if pid == self_pid {
                continue;
            }
            let exe_link = format!("/proc/{pid}/exe");
            let Ok(exe) = std::fs::read_link(&exe_link) else {
                continue;
            };
            // Compare canonical paths textually (bounded seam, no shell).
            if exe.to_string_lossy() == executable && Some(exe) != target {
                out.push(pid);
            }
        }
        out
    }

    /// Ask the host which PIDs are running `executable`.
    ///
    /// `pgrep -x -f` takes the path as a literal argv element, so nothing is
    /// shell-parsed. A runner that refuses, times out, or is missing yields
    /// None, and the caller treats that as "cannot tell" rather than
    /// "not running" -- the guard must not be weakened by a failed probe.
    #[doc(hidden)]
    pub fn host_pids_for_test(&self, executable: &str) -> Option<Vec<u32>> {
        self.host_pids(executable)
    }

    fn host_pids(&self, executable: &str) -> Option<Vec<u32>> {
        let runner = self.host.as_ref()?;
        let result = runner.run(&ProcessRequest {
            program: "pgrep".to_string(),
            args: vec!["-x".to_string(), "-f".to_string(), executable.to_string()],
            host: true,
            timeout_ms: 5_000,
            ..Default::default()
        });
        if result.refused || result.timed_out {
            return None;
        }
        // pgrep exits 1 when nothing matched, which is a real answer.
        match result.exit_code {
            0 | 1 => Some(
                String::from_utf8_lossy(&result.stdout)
                    .lines()
                    .filter_map(|line| line.trim().parse::<u32>().ok())
                    .take(4096)
                    .collect(),
            ),
            _ => None,
        }
    }
}

impl ProcessTable for SysTable {
    fn pids_for_executable(&self, executable: &str) -> Vec<u32> {
        if crate::process::in_flatpak() {
            // The sandbox's /proc cannot answer this; the host can.
            if let Some(pids) = self.host_pids(executable) {
                return pids;
            }
            // Fall through to the local scan rather than claiming "not
            // running": it is still the honest best effort available.
        }
        Self::scan_proc(executable)
    }
}

#[derive(Debug, Default)]
pub struct FakeTable {
    pub running: Mutex<HashMap<String, Vec<u32>>>,
}

impl FakeTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_running(&self, executable: &str) {
        self.running
            .lock()
            .unwrap()
            .insert(executable.to_string(), vec![4242]);
    }
}

impl ProcessTable for FakeTable {
    fn pids_for_executable(&self, executable: &str) -> Vec<u32> {
        self.running
            .lock()
            .unwrap()
            .get(executable)
            .cloned()
            .unwrap_or_default()
    }
}
