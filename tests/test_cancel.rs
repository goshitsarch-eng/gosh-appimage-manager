// Audit INV-093 follow-up: a helper thread that outlives a cancelled download
// must not change the managed folder or the registry, and a cancel that arrives
// before the swap must stop the swap.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use goshaim_core::cancel::CANCELLED;
use goshaim_core::controller::AppController;
use goshaim_core::inspector::make_test_elf;
use goshaim_core::network::{
    stream_to_file_reporting, FakeNetwork, FetchResult, Local, NetworkClient,
};
use goshaim_core::process::FakeRunner;
use goshaim_core::proctable::SysTable;
use goshaim_core::settings::Dirs;
use goshaim_core::trash::FakeTrash;
use goshaim_core::types::{AppImageType, ApplyEvent, Architecture, InstalledApp, UpdatePhase};

const UUID: &str = "demo-uuid";

/// The new build: a well-formed AppImage, so it would pass every check and be
/// installed if nothing stopped it.
fn new_build() -> Vec<u8> {
    make_test_elf(Architecture::X86_64, AppImageType::Type2)
}

/// Holds a download's first read until the test releases it, as a stalled
/// connection does.
#[derive(Default)]
struct Gate {
    state: Mutex<GateState>,
    changed: Condvar,
}

#[derive(Default)]
struct GateState {
    entered: bool,
    released: bool,
}

impl Gate {
    fn enter_and_wait(&self) {
        let mut state = self.state.lock().unwrap();
        state.entered = true;
        self.changed.notify_all();
        while !state.released {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn wait_entered(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let mut state = self.state.lock().unwrap();
        while !state.entered {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            state = self.changed.wait_timeout(state, left).unwrap().0;
        }
        true
    }

    fn release(&self) {
        self.state.lock().unwrap().released = true;
        self.changed.notify_all();
    }
}

/// The body of a download: the first read waits for the gate, then the bytes arrive.
struct Body {
    gate: Option<Arc<Gate>>,
    payload: Vec<u8>,
    delivered: bool,
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(gate) = self.gate.take() {
            gate.enter_and_wait();
        }
        if self.delivered {
            return Ok(0);
        }
        self.delivered = true;
        let n = buf.len().min(self.payload.len());
        buf[..n].copy_from_slice(&self.payload[..n]);
        Ok(n)
    }
}

/// A network whose download is the real streaming writer reading a body that may
/// be held. It reports when the download has returned.
struct Network {
    gate: Option<Arc<Gate>>,
    finished: Option<mpsc::Sender<()>>,
}

impl NetworkClient for Network {
    fn get(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        _local: Local,
    ) -> Result<FetchResult, String> {
        Err("unused".to_string())
    }

    fn head_len(&self, _url: &str, _local: Local) -> Result<Option<u64>, String> {
        Ok(Some(new_build().len() as u64))
    }

    fn download_bounded(
        &self,
        _url: &str,
        _max_bytes: u64,
        _local: Local,
    ) -> Result<Vec<u8>, String> {
        Err("unused".to_string())
    }

    fn download_to_file(
        &self,
        _url: &str,
        dest: &Path,
        max_bytes: u64,
        cancel: &AtomicBool,
        _local: Local,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<u64, String> {
        let payload = new_build();
        let expected = payload.len() as u64;
        let body = Body {
            gate: self.gate.clone(),
            payload,
            delivered: false,
        };
        let result = stream_to_file_reporting(body, dest, max_bytes, cancel, expected, progress);
        if let Some(finished) = &self.finished {
            let _ = finished.send(());
        }
        result
    }
}

fn controller_on(home: &Path, network: Box<dyn NetworkClient>) -> AppController {
    AppController::with_seams(
        Box::new(FakeRunner::new()),
        network,
        Box::new(SysTable::new()),
        Box::new(FakeTrash::new()),
        Dirs::under(home),
    )
    .expect("a controller on the scratch home")
}

/// One owned app, one version behind its source, with the old build installed.
/// Returns the path of the installed file.
fn seed(home: &Path) -> PathBuf {
    let folder = home.join("AppImages");
    fs::create_dir_all(&folder).unwrap();
    let live = folder.join("Demo.AppImage");
    fs::write(&live, b"old-build").unwrap();
    let mut config = BTreeMap::new();
    config.insert(
        "url".to_string(),
        "https://mirror.example/Demo.AppImage".to_string(),
    );
    config.insert("version".to_string(), "2.0".to_string());
    let app = InstalledApp {
        uuid: UUID.to_string(),
        name: "Demo".to_string(),
        version: "1.0".to_string(),
        managed_path: live.to_string_lossy().into_owned(),
        update_manager: "static".to_string(),
        update_config: config,
        ..InstalledApp::new_owned()
    };
    let mut controller = controller_on(home, Box::new(FakeNetwork::new()));
    controller
        .registry_mut()
        .upsert(app)
        .expect("the registry accepts the app");
    live
}

/// Every entry in the managed folder, with its bytes, in name order.
fn managed_folder(home: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries: Vec<(String, Vec<u8>)> = fs::read_dir(home.join("AppImages"))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let bytes = fs::read(entry.path()).unwrap();
            (entry.file_name().to_string_lossy().into_owned(), bytes)
        })
        .collect();
    entries.sort();
    entries
}

/// The registry's app count and the recorded version of the seeded app.
fn registry_state(home: &Path) -> (usize, String) {
    let controller = controller_on(home, Box::new(FakeNetwork::new()));
    let version = controller
        .registry()
        .by_uuid(UUID)
        .map(|app| app.version)
        .unwrap_or_default();
    (controller.registry().apps().len(), version)
}

/// A download is cancelled while its read is held. The apply returns at once
/// with "Cancelled". Once the held helper finishes, the managed folder and the
/// registry are exactly as they were.
#[test]
fn a_cancelled_download_leaves_the_managed_folder_and_registry_unchanged_once_the_helper_finishes()
{
    let home = tempfile::tempdir().unwrap();
    let live = seed(home.path());
    let folder_before = managed_folder(home.path());
    let registry_before = registry_state(home.path());

    let gate = Arc::new(Gate::default());
    let (finished_tx, finished_rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let (home_path, worker_gate) = (home.path().to_path_buf(), Arc::clone(&gate));
    let worker = thread::spawn(move || {
        let network = Network {
            gate: Some(worker_gate),
            finished: Some(finished_tx),
        };
        let mut controller = controller_on(&home_path, Box::new(network));
        let app = controller
            .registry()
            .by_uuid(UUID)
            .expect("the seeded app")
            .clone();
        controller.apply_update_with_progress(&app, false, &worker_cancel, &mut |_| {})
    });

    assert!(
        gate.wait_entered(Duration::from_secs(10)),
        "the download reached its held read"
    );
    let cancelled_at = Instant::now();
    cancel.store(true, Ordering::Relaxed);
    let result = worker.join().expect("the apply returns");
    assert!(
        cancelled_at.elapsed() < Duration::from_secs(2),
        "the cancelled apply returned after {:?}",
        cancelled_at.elapsed()
    );
    assert!(!result.ok, "a cancelled update is not installed");
    assert_eq!(result.error, CANCELLED);

    gate.release();
    finished_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the held download finishes");
    assert_eq!(
        managed_folder(home.path()),
        folder_before,
        "the managed folder is unchanged once the helper finished"
    );
    assert_eq!(fs::read(&live).unwrap(), b"old-build");
    assert_eq!(
        registry_state(home.path()),
        registry_before,
        "the registry is unchanged once the helper finished"
    );
}

/// A cancel that arrives after the download and verify, just before the swap,
/// stops the update: the installed file is not replaced, no backup or staging
/// file is left, and the registry is unchanged.
#[test]
fn a_cancel_that_arrives_before_the_swap_stops_the_update_without_replacing_the_app() {
    let home = tempfile::tempdir().unwrap();
    let live = seed(home.path());
    let folder_before = managed_folder(home.path());
    let registry_before = registry_state(home.path());

    let cancel = Arc::new(AtomicBool::new(false));
    let (home_path, apply_cancel, events_cancel) = (
        home.path().to_path_buf(),
        Arc::clone(&cancel),
        Arc::clone(&cancel),
    );
    let worker = thread::spawn(move || {
        let mut controller = controller_on(
            &home_path,
            Box::new(Network {
                gate: None,
                finished: None,
            }),
        );
        let app = controller
            .registry()
            .by_uuid(UUID)
            .expect("the seeded app")
            .clone();
        let mut events = |event: ApplyEvent| {
            if let ApplyEvent::Phase {
                phase: UpdatePhase::SwapIn,
                ..
            } = event
            {
                events_cancel.store(true, Ordering::Relaxed);
            }
        };
        controller.apply_update_with_progress(&app, false, &apply_cancel, &mut events)
    });
    let result = worker.join().expect("the apply returns");

    assert!(!result.ok, "a cancel before the swap stops the update");
    assert_eq!(result.error, CANCELLED);
    assert_eq!(
        fs::read(&live).unwrap(),
        b"old-build",
        "the installed file is not replaced"
    );
    assert_eq!(
        managed_folder(home.path()),
        folder_before,
        "no backup or staging file is left behind"
    );
    assert_eq!(registry_state(home.path()), registry_before);
}
