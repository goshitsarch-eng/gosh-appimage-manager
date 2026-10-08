#![allow(dead_code)]
// Shared test harness: isolated dirs + shared-state fake seams.
// Tests never touch a real home, execute an AppImage, launch an app,
// or call live update APIs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use goshaim_core::controller::AppController;
use goshaim_core::network::{FetchResult, Local, NetworkClient};
use goshaim_core::process::{ProcessRequest, ProcessResult, ProcessRunner};
use goshaim_core::proctable::ProcessTable;
use goshaim_core::settings::Dirs;
use goshaim_core::trash::TrashSink;
use goshaim_core::types::{AppImageType, Architecture};

type CannedOutputs = Arc<Mutex<HashMap<String, (i32, Vec<u8>)>>>;
type SpawnLog = Arc<Mutex<Vec<(String, Vec<String>)>>>;

/// A hook that can observe a request and optionally answer it outright.
///
/// Extraction tools do not just print: they write files into `-d <dest>`.
/// Canned stdout alone cannot model that, so a hook may plant the files the
/// real tool would have produced. Returning None falls through to the canned
/// table.
type RunHook = Box<dyn Fn(&ProcessRequest) -> Option<ProcessResult> + Send + Sync>;

#[derive(Clone)]
pub struct SharedRunner {
    pub outputs: CannedOutputs,
    pub spawned: SpawnLog,
    pub fail_start: Arc<Mutex<bool>>,
    /// Exit code when no canned output matches (models a missing tool).
    pub default_exit: Arc<Mutex<i32>>,
    hooks: Arc<Mutex<Vec<RunHook>>>,
}

impl Default for SharedRunner {
    fn default() -> Self {
        Self {
            outputs: Arc::new(Mutex::new(HashMap::new())),
            spawned: Arc::new(Mutex::new(Vec::new())),
            fail_start: Arc::new(Mutex::new(false)),
            default_exit: Arc::new(Mutex::new(1)),
            hooks: Arc::new(Mutex::new(Vec::new())),
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

    /// Register a hook run before the canned table is consulted.
    pub fn on_run(&self, hook: RunHook) {
        self.hooks.lock().unwrap().push(hook);
    }
}

impl ProcessRunner for SharedRunner {
    fn run(&self, req: &ProcessRequest) -> ProcessResult {
        // Exercise the same host policy the real runner enforces, so tests
        // cannot pass on a request production would refuse.
        if let Err(error) = goshaim_core::process::host_spawn_permitted(req) {
            return ProcessResult {
                program: req.program.clone(),
                refused: true,
                stderr: error.into_bytes(),
                ..Default::default()
            };
        }
        for hook in self.hooks.lock().unwrap().iter() {
            if let Some(result) = hook(req) {
                return result;
            }
        }
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
        goshaim_core::process::host_spawn_permitted(req)?;
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
    fn get(
        &self,
        url: &str,
        _headers: &[(String, String)],
        local: Local,
    ) -> Result<FetchResult, String> {
        self.calls.lock().unwrap().push(url.to_string());
        goshaim_core::url_guard::validate(url, false, local.allowed())?;
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

    fn head_len(&self, url: &str, local: Local) -> Result<Option<u64>, String> {
        self.calls.lock().unwrap().push(format!("HEAD {url}"));
        goshaim_core::url_guard::validate(url, false, local.allowed())?;
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
        dest: &Path,
        max_bytes: u64,
        cancel: &AtomicBool,
        local: Local,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<u64, String> {
        // Exercise the real streaming writer so the tests cover the same code
        // path the production client uses to land bytes on disk.
        let body = self.download_bounded(url, max_bytes, local)?;
        let expected = body.len() as u64;
        goshaim_core::network::stream_to_file_reporting(
            body.as_slice(),
            dest,
            max_bytes,
            cancel,
            expected,
            progress,
        )
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

/// A real squashfs image (made with mksquashfs 4.6.1) holding `quill-notes.desktop`
/// (Name=Quill Notes), `quill-notes.png` and an `AppRun` that is never run. It is
/// 4096 bytes, so it is the payload for the AppImage-layout tests.
pub const LAYOUT_PAYLOAD: &[u8] = include_bytes!("fixtures/appimage-layout-payload.sqsh");

/// Layout-only ELF64 little-endian x86_64 header with the AppImage Type-2 magic.
///
/// `loads` are PT_LOAD program headers as `(p_offset, p_filesz)`. `sections` are
/// section headers as `(sh_type, sh_offset, sh_size)`, written as a table at
/// `shoff`. The returned bytes end at the end of whatever tables were written,
/// so a caller appends the payload right after them. Nothing here is executed.
pub fn elf_layout(shoff: usize, sections: &[(u32, u64, u64)], loads: &[(u64, u64)]) -> Vec<u8> {
    let len = (shoff + sections.len() * 64).max(64 + loads.len() * 56);
    let mut d = vec![0u8; len];
    d[0..4].copy_from_slice(b"\x7fELF");
    d[4] = 2; // ELFCLASS64
    d[5] = 1; // ELFDATA2LSB
    d[6] = 1; // EV_CURRENT
    d[8] = b'A';
    d[9] = b'I';
    d[10] = 2; // AppImage type 2
    d[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    d[18..20].copy_from_slice(&0x3Eu16.to_le_bytes()); // EM_X86_64
    d[20..24].copy_from_slice(&1u32.to_le_bytes()); // EV_CURRENT
    let phoff: u64 = if loads.is_empty() { 0 } else { 64 };
    d[32..40].copy_from_slice(&phoff.to_le_bytes());
    d[40..48].copy_from_slice(&(shoff as u64).to_le_bytes());
    d[52..54].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize
    d[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
    d[56..58].copy_from_slice(&(loads.len() as u16).to_le_bytes());
    d[58..60].copy_from_slice(&64u16.to_le_bytes()); // e_shentsize
    d[60..62].copy_from_slice(&(sections.len() as u16).to_le_bytes());
    for (i, (offset, filesz)) in loads.iter().enumerate() {
        let base = 64 + i * 56;
        d[base..base + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
        d[base + 8..base + 16].copy_from_slice(&offset.to_le_bytes());
        d[base + 32..base + 40].copy_from_slice(&filesz.to_le_bytes());
        d[base + 40..base + 48].copy_from_slice(&filesz.to_le_bytes());
    }
    for (i, (kind, offset, size)) in sections.iter().enumerate() {
        let base = shoff + i * 64;
        d[base + 4..base + 8].copy_from_slice(&kind.to_le_bytes());
        d[base + 24..base + 32].copy_from_slice(&offset.to_le_bytes());
        d[base + 32..base + 40].copy_from_slice(&size.to_le_bytes());
    }
    d
}

/// Whether the squashfs tool the inspector shells out to is installed.
pub fn unsquashfs_on_path() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join("unsquashfs").is_file())
    })
}

/// Every call a fake runner received, as argument vectors.
pub type RunLog = Arc<Mutex<Vec<Vec<String>>>>;

/// Stand in for an AppImage's own `--appimage-extract` (never a real AppImage). When
/// the core runs `--appimage-extract` through the fake runner, this writes a
/// squashfs-root tree into the working directory it is given: `demo.desktop`
/// (Name=Unpacked, version 9.9, Icon=demo) and `demo.png`. Every call is recorded.
pub fn plant_self_extraction(h: &Harness) -> RunLog {
    let calls: RunLog = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    h.runner.on_run(Box::new(move |req| {
        seen.lock().unwrap().push(req.args.clone());
        if !req.args.iter().any(|a| a == "--appimage-extract") {
            return None;
        }
        let root = std::path::Path::new(&req.work_dir).join("squashfs-root");
        std::fs::create_dir_all(&root).ok()?;
        std::fs::write(
            root.join("demo.desktop"),
            b"[Desktop Entry]\nName=Unpacked\nX-AppImage-Version=9.9\nIcon=demo\nExec=demo\n",
        )
        .ok()?;
        std::fs::write(root.join("demo.png"), b"\x89PNG\r\n\x1a\n").ok()?;
        Some(ProcessResult {
            program: req.program.clone(),
            exit_code: 0,
            ..Default::default()
        })
    }));
    calls
}

/// Whether the recorded calls include a self-extraction.
pub fn ran_self_extraction(log: &RunLog) -> bool {
    log.lock()
        .unwrap()
        .iter()
        .any(|args| args.iter().any(|a| a == "--appimage-extract"))
}
