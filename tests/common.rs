#![allow(dead_code)]
// Shared test harness: isolated dirs + shared-state fake seams.
// Tests never touch a real home, execute an AppImage, launch an app,
// or call live update APIs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use goshaim_core::controller::AppController;
use goshaim_core::network::{FetchResult, NetworkClient};
use goshaim_core::process::{ProcessRequest, ProcessResult, ProcessRunner};
use goshaim_core::proctable::ProcessTable;
use goshaim_core::settings::Dirs;
use goshaim_core::trash::TrashSink;
use goshaim_core::types::{AppImageType, Architecture};

type CannedOutputs = Arc<Mutex<HashMap<String, (i32, Vec<u8>)>>>;
type SpawnLog = Arc<Mutex<Vec<(String, Vec<String>)>>>;

#[derive(Clone)]
pub struct SharedRunner {
    pub outputs: CannedOutputs,
    pub spawned: SpawnLog,
    pub fail_start: Arc<Mutex<bool>>,
    /// Exit code when no canned output matches (models a missing tool).
    pub default_exit: Arc<Mutex<i32>>,
}

impl Default for SharedRunner {
    fn default() -> Self {
        Self {
            outputs: Arc::new(Mutex::new(HashMap::new())),
            spawned: Arc::new(Mutex::new(Vec::new())),
            fail_start: Arc::new(Mutex::new(false)),
            default_exit: Arc::new(Mutex::new(1)),
        }
    }
}

impl SharedRunner {
    pub fn canned(&self, program_sub: &str, exit: i32, stdout: &[u8]) {
        self.outputs
            .lock()
            .unwrap()
            .insert(program_sub.to_string(), (exit, stdout.to_vec()));
    }
}

impl ProcessRunner for SharedRunner {
    fn run(&self, req: &ProcessRequest) -> ProcessResult {
        let mut result = ProcessResult {
            program: req.program.clone(),
            exit_code: *self.default_exit.lock().unwrap(),
            ..Default::default()
        };
        for (key, (exit, out)) in self.outputs.lock().unwrap().iter() {
            if req.program.contains(key) {
                result.exit_code = *exit;
                result.stdout = out.clone();
                if out.len() > goshaim_core::limits::MAX_PROCESS_OUTPUT_BYTES {
                    result.stderr = b"Archive listing exceeded output bound".to_vec();
                    result.exit_code = 1;
                }
                return result;
            }
        }
        result
    }

    fn start_detached(&self, req: &ProcessRequest) -> Result<(), String> {
        if *self.fail_start.lock().unwrap() {
            return Err("Cannot start: fake spawn failure".to_string());
        }
        self.spawned
            .lock()
            .unwrap()
            .push((req.program.clone(), req.args.clone()));
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct SharedNetwork {
    pub bodies: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub head_sizes: Arc<Mutex<HashMap<String, u64>>>,
    pub calls: Arc<Mutex<Vec<String>>>,
}

impl SharedNetwork {
    pub fn canned_body(&self, url_sub: &str, body: &[u8]) {
        self.bodies
            .lock()
            .unwrap()
            .insert(url_sub.to_string(), body.to_vec());
    }

    pub fn canned_size(&self, url_sub: &str, size: u64) {
        self.head_sizes
            .lock()
            .unwrap()
            .insert(url_sub.to_string(), size);
    }
}

impl NetworkClient for SharedNetwork {
    fn get(&self, url: &str, _headers: &[(String, String)]) -> Result<FetchResult, String> {
        self.calls.lock().unwrap().push(url.to_string());
        goshaim_core::url_guard::validate(url, false, false)?;
        for (key, body) in self.bodies.lock().unwrap().iter() {
            if url.contains(key) {
                if body.len() > goshaim_core::limits::MAX_JSON_BODY_BYTES {
                    return Err(format!(
                        "Response body exceeds size bound ({} bytes)",
                        goshaim_core::limits::MAX_JSON_BODY_BYTES
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
        goshaim_core::url_guard::validate(url, false, false)?;
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

#[derive(Clone, Default)]
pub struct SharedTable {
    pub running: Arc<Mutex<HashMap<String, Vec<u32>>>>,
}

impl SharedTable {
    pub fn mark_running(&self, executable: &str) {
        self.running
            .lock()
            .unwrap()
            .insert(executable.to_string(), vec![4242]);
    }
}

impl ProcessTable for SharedTable {
    fn pids_for_executable(&self, executable: &str) -> Vec<u32> {
        self.running
            .lock()
            .unwrap()
            .get(executable)
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone, Default)]
pub struct SharedTrash {
    pub fail_next: Arc<Mutex<bool>>,
    pub trashed: Arc<Mutex<Vec<String>>>,
}

impl TrashSink for SharedTrash {
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

pub struct Harness {
    pub tmp: tempfile::TempDir,
    pub runner: SharedRunner,
    pub network: SharedNetwork,
    pub table: SharedTable,
    pub trash: SharedTrash,
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness {
    pub fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().expect("tempdir"),
            runner: SharedRunner::default(),
            network: SharedNetwork::default(),
            table: SharedTable::default(),
            trash: SharedTrash::default(),
        }
    }

    pub fn dirs(&self) -> Dirs {
        Dirs::under(self.tmp.path())
    }

    pub fn controller(&self) -> AppController {
        AppController::with_seams(
            Box::new(self.runner.clone()),
            Box::new(self.network.clone()),
            Box::new(self.table.clone()),
            Box::new(self.trash.clone()),
            self.dirs(),
        )
        .expect("controller")
    }

    pub fn cancel() -> AtomicBool {
        AtomicBool::new(false)
    }
}

/// Write a synthetic Type-2 x86_64 ELF fixture (never a real AppImage).
pub fn write_fixture(dir: &Path, name: &str) -> PathBuf {
    write_fixture_arch(dir, name, Architecture::X86_64, AppImageType::Type2)
}

pub fn write_fixture_arch(
    dir: &Path,
    name: &str,
    arch: Architecture,
    app_type: AppImageType,
) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(
        &path,
        goshaim_core::inspector::make_test_elf(arch, app_type),
    )
    .expect("write fixture");
    path
}

pub fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}
