//! Inspect: read a file's facts without installing or executing it.

use std::path::Path;

use goshaim_core::controller::AppController;
use goshaim_core::types::{InspectOptions, TaskKind};

use crate::api::common::{controller, guard, CoreError, ErrorKind, OperationGuard};
use crate::api::dto::InspectDto;

/// Largest icon the bridge copies across. Icons are small; the limit keeps a
/// hostile file from pushing megabytes through the boundary.
const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;

/// Inspect one AppImage. A file that cannot be read or parsed comes back with
/// `error` set; only an empty path is an error of the call itself.
pub fn inspect_path(
    op_id: String,
    path: String,
    confirm_unsafe: bool,
) -> Result<InspectDto, CoreError> {
    guard(move || {
        if path.trim().is_empty() {
            return Err(CoreError::new(
                ErrorKind::UserInput,
                "Choose an AppImage to inspect.",
            ));
        }
        let controller = controller()?;
        inspect_on(&controller, &op_id, &path, confirm_unsafe)
    })
}

/// The body of `inspect_path`, run on a controller the caller provides, so a test
/// can run it on scratch seams. A cancelled inspection finishes as cancelled.
fn inspect_on(
    controller: &AppController,
    op_id: &str,
    path: &str,
    confirm_unsafe: bool,
) -> Result<InspectDto, CoreError> {
    let op = OperationGuard::begin(op_id, TaskKind::Inspect, "Inspecting", path);
    let existing = controller
        .registry()
        .by_path(path)
        .map(|app| app.uuid)
        .unwrap_or_default();
    let options = inspect_options(controller.settings().max_appimage_bytes(), confirm_unsafe);
    let result = controller.inspect_with(
        path,
        &options,
        op.cancel_flag(),
        if existing.is_empty() {
            None
        } else {
            Some(existing.as_str())
        },
    );
    let icon_bytes = read_icon(&result.metadata.extracted_icon_path);
    let dto = InspectDto::from_core(&result, icon_bytes);
    result.discard_staging();
    op.finish(if dto.error.is_empty() {
        Ok(())
    } else {
        Err(dto.error.clone())
    });
    Ok(dto)
}

fn read_icon(path: &str) -> Option<Vec<u8>> {
    if path.is_empty() {
        return None;
    }
    let path = Path::new(path);
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ICON_BYTES {
        return None;
    }
    std::fs::read(path).ok()
}

/// The options one bridge inspection runs with. The confirmation is the user's
/// answer for this file; the core gate still decides whether the fallback may run.
pub(crate) fn inspect_options(max_bytes: i64, confirm_unsafe: bool) -> InspectOptions {
    InspectOptions {
        max_bytes,
        confirm_unsafe,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_confirmation_reaches_the_inspection_options() {
        assert!(inspect_options(1024, true).confirm_unsafe);
        assert!(!inspect_options(1024, false).confirm_unsafe);
        assert_eq!(inspect_options(1024, true).max_bytes, 1024);
    }
}

#[cfg(test)]
mod cancel_tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use goshaim_core::controller::AppController;
    use goshaim_core::inspector::make_test_elf;
    use goshaim_core::network::ReqwestClient;
    use goshaim_core::process::{ProcessRequest, ProcessResult, ProcessRunner};
    use goshaim_core::proctable::SysTable;
    use goshaim_core::settings::Dirs;
    use goshaim_core::trash::FakeTrash;
    use goshaim_core::types::{AppImageType, Architecture};

    use super::inspect_on;
    use crate::api::dto::TaskStateDto;
    use crate::api::system::{cancel_task, list_tasks};

    const LISTING: &[u8] = b"squashfs-root/Quill.desktop\n";
    const DESKTOP: &[u8] = b"[Desktop Entry]\nType=Application\nName=Quill Notes\nExec=quill\n";

    /// Stands in for the archive reader. Its listing is busy until the test
    /// releases it, as a long read is.
    struct ArchiveReader {
        entered: Mutex<Option<mpsc::Sender<()>>>,
        released: Arc<AtomicBool>,
    }

    impl ProcessRunner for ArchiveReader {
        fn run(&self, req: &ProcessRequest) -> ProcessResult {
            if req.program != "unsquashfs" {
                return ProcessResult {
                    program: req.program.clone(),
                    refused: true,
                    ..Default::default()
                };
            }
            if req.args.iter().any(|a| a == "-l") {
                if let Some(tx) = self.entered.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                let deadline = Instant::now() + Duration::from_secs(10);
                while !self.released.load(Ordering::Relaxed) && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(5));
                }
                return ProcessResult {
                    program: req.program.clone(),
                    exit_code: 0,
                    stdout: LISTING.to_vec(),
                    ..Default::default()
                };
            }
            if let Some(i) = req.args.iter().position(|a| a == "-d") {
                let _ = fs::write(Path::new(&req.args[i + 1]).join("Quill.desktop"), DESKTOP);
            }
            ProcessResult {
                program: req.program.clone(),
                exit_code: 0,
                ..Default::default()
            }
        }

        fn start_detached(&self, _req: &ProcessRequest) -> Result<(), String> {
            Err("not used by this test".to_string())
        }
    }

    /// A scratch home for one test, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("gosh-r7-inspect-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("a scratch home");
            Self(root)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Audit QA3-009: a cancelled inspection must not come back as a valid result.
    /// It must be cancelled on the Tasks page too.
    #[test]
    fn a_cancelled_inspection_comes_back_failed_and_its_task_ends_cancelled() {
        let scratch = Scratch::new("cancel");
        let path = scratch.0.join("Quill.AppImage");
        fs::write(
            &path,
            make_test_elf(Architecture::X86_64, AppImageType::Type2),
        )
        .expect("the fixture is written");
        let path_text = path.to_string_lossy().into_owned();
        let (entered_tx, entered_rx) = mpsc::channel();
        let released = Arc::new(AtomicBool::new(false));
        let reader = ArchiveReader {
            entered: Mutex::new(Some(entered_tx)),
            released: Arc::clone(&released),
        };
        let root = scratch.0.clone();
        let worker_path = path_text.clone();
        let worker = thread::spawn(move || {
            let controller = AppController::with_seams(
                Box::new(reader),
                Box::new(ReqwestClient::new()),
                Box::new(SysTable::new()),
                Box::new(FakeTrash::new()),
                Dirs::under(&root),
            )
            .expect("a controller rooted in the scratch home");
            inspect_on(&controller, "r7-inspect-cancel", &worker_path, false)
        });

        entered_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the archive reader started");
        assert!(
            cancel_task("r7-inspect-cancel".to_string()),
            "the inspection is running"
        );
        released.store(true, Ordering::Relaxed);
        let dto = worker
            .join()
            .expect("the inspection returns")
            .expect("a cancelled inspection still returns its result");

        assert!(
            !dto.magic_valid,
            "a cancelled inspection is not a valid AppImage"
        );
        assert!(
            dto.error.to_lowercase().contains("cancel"),
            "the error says the inspection was cancelled: {:?}",
            dto.error
        );
        assert!(
            dto.name.is_empty(),
            "a cancelled inspection carries no metadata"
        );
        let state = list_tasks()
            .into_iter()
            .find(|task| task.target == path_text)
            .map(|task| task.state)
            .expect("the inspection has a Tasks entry");
        assert!(
            matches!(state, TaskStateDto::Cancelled),
            "a cancelled inspection ends as cancelled, not {state:?}"
        );
    }
}
