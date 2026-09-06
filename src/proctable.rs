// Gosh AppImage Manager — process table seam (ports ProcessTable).
// Used to block updates while the app is running and to badge Library rows.

use std::collections::HashMap;
use std::sync::Mutex;

pub trait ProcessTable: Send + Sync {
    fn pids_for_executable(&self, executable: &str) -> Vec<u32>;
    fn is_running(&self, executable: &str) -> bool {
        !self.pids_for_executable(executable).is_empty()
    }
}

/// Real table: scans /proc for exact exe matches (never matches self).
pub struct SysTable;

impl Default for SysTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SysTable {
    pub fn new() -> Self {
        Self
    }
}

impl ProcessTable for SysTable {
    fn pids_for_executable(&self, executable: &str) -> Vec<u32> {
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
