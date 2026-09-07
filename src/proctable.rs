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

    /// Which of `executables` are running.
    ///
    /// Asking one at a time means re-enumerating every process for every app:
    /// listing a library of N apps cost N walks of /proc and N readlinks per
    /// process. Implementations that can answer for all of them in one pass
    /// override this; the default keeps the one-at-a-time behaviour for seams
    /// where a batch is no cheaper.
    fn running_among(&self, executables: &[String]) -> std::collections::HashSet<String> {
        executables
            .iter()
            .filter(|exe| self.is_running(exe))
            .cloned()
            .collect()
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

    /// One pass over `/proc` answering for every executable at once.
    fn scan_proc_among(executables: &[String]) -> std::collections::HashSet<String> {
        use std::collections::HashSet;
        let wanted: HashSet<&str> = executables.iter().map(String::as_str).collect();
        let target = std::fs::read_link("/proc/self/exe").ok();
        let self_pid = std::process::id();
        let mut found = HashSet::new();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return found;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(pid): Result<u32, _> = name.parse() else {
                continue;
            };
            if pid == self_pid {
                continue;
            }
            let Ok(exe) = std::fs::read_link(format!("/proc/{pid}/exe")) else {
                continue;
            };
            if Some(&exe) == target.as_ref() {
                continue;
            }
            let text = exe.to_string_lossy();
            if let Some(hit) = wanted.get(text.as_ref()) {
                found.insert((*hit).to_string());
            }
        }
        found
    }

    /// Ask the host once for the whole set.
    fn host_running_among(
        &self,
        executables: &[String],
    ) -> Option<std::collections::HashSet<String>> {
        let runner = self.host.as_ref()?;
        // `pgrep -a` prints "<pid> <command line>"; matching the full path
        // against each candidate keeps this to one host round trip.
        let result = runner.run(&ProcessRequest {
            program: "pgrep".to_string(),
            args: vec!["-a".to_string(), "-f".to_string(), ".AppImage".to_string()],
            host: true,
            timeout_ms: 5_000,
            ..Default::default()
        });
        if result.refused || result.timed_out || !matches!(result.exit_code, 0 | 1) {
            return None;
        }
        let text = String::from_utf8_lossy(&result.stdout);
        let mut found = std::collections::HashSet::new();
        for line in text.lines().take(4096) {
            // Skip the pid, then match the command against each candidate.
            let Some((_, command)) = line.trim().split_once(' ') else {
                continue;
            };
            for exe in executables {
                if command == exe || command.starts_with(&format!("{exe} ")) {
                    found.insert(exe.clone());
                }
            }
        }
        Some(found)
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

    fn running_among(&self, executables: &[String]) -> std::collections::HashSet<String> {
        if executables.is_empty() {
            return std::collections::HashSet::new();
        }
        if crate::process::in_flatpak() {
            // One host round trip for the whole set rather than one per app.
            if let Some(running) = self.host_running_among(executables) {
                return running;
            }
        }
        Self::scan_proc_among(executables)
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

/// Canonical managed paths for a set of apps, for a batch running check.
pub fn executables_for(apps: &[crate::types::InstalledApp]) -> Vec<String> {
    apps.iter()
        .map(|app| {
            crate::removal::canonical_existing(&app.managed_path)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| app.managed_path.clone())
        })
        .collect()
}
