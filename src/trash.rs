// Gosh AppImage Manager — trash seam (freedesktop Trash via `trash` crate).
// Trash failure NEVER becomes delete; callers leave everything intact.

use std::path::Path;
use std::sync::Mutex;

pub trait TrashSink: Send + Sync {
    fn trash(&self, path: &Path) -> Result<(), String>;
}

pub struct SystemTrash;

impl Default for SystemTrash {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemTrash {
    pub fn new() -> Self {
        Self
    }
}

impl TrashSink for SystemTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        trash::delete(path).map_err(|e| format!("Trash failed: {e}"))
    }
}

/// Fake trash for tests; optionally fails once to exercise partial rollback.
#[derive(Debug, Default)]
pub struct FakeTrash {
    pub fail_next: Mutex<bool>,
    pub trashed: Mutex<Vec<String>>,
}

impl FakeTrash {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failing() -> Self {
        Self {
            fail_next: Mutex::new(true),
            trashed: Mutex::new(Vec::new()),
        }
    }
}

impl TrashSink for FakeTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        if *self.fail_next.lock().unwrap() {
            *self.fail_next.lock().unwrap() = false;
            return Err("Trash failed (fake)".to_string());
        }
        self.trashed
            .lock()
            .unwrap()
            .push(path.to_string_lossy().into_owned());
        Ok(())
    }
}
