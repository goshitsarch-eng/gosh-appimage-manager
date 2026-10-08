//! Updates: check every app, check one app, apply one update, or apply all.

use std::sync::atomic::{AtomicBool, Ordering};

use goshaim_core::types::{ApplyEvent, TaskKind, UpdatePhase};

use goshaim_core::controller::AppController;

use crate::api::common::{controller, guard, is_timeout, CoreError, OperationGuard};
use crate::api::dto::{
    BatchDto, OutcomeDto, UpdateCheckDto, UpdateFailureDto, UpdateOfferDto, UpdateScanDto,
};
use crate::api::library::find_app;

/// Check one app's update source for "Check for update" on its Detail page.
/// It reports what the source offers and never downloads or applies anything:
/// the installed file and its version stay as they are. Applying is the
/// separate Update action. A check is not listed on the Tasks page.
pub fn check_one_update(uuid: String) -> Result<UpdateCheckDto, CoreError> {
    guard(move || {
        let controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        let never_cancelled = AtomicBool::new(false);
        let checked = controller.check_one_update(&app, &never_cancelled);
        Ok(UpdateCheckDto::from_core(&app, &checked))
    })
}

/// Check every app that has an update source. Apps without one are skipped,
/// and apps that could not be checked are listed as failures.
pub fn check_updates(op_id: String) -> Result<UpdateScanDto, CoreError> {
    guard(move || {
        let controller = controller()?;
        check_updates_on(&controller, &op_id)
    })
}

/// The body of `check_updates`, run on a controller the caller provides, so a
/// test can run it on scratch seams.
fn check_updates_on(controller: &AppController, op_id: &str) -> Result<UpdateScanDto, CoreError> {
    let op = OperationGuard::begin(op_id, TaskKind::CheckUpdate, "Checking for updates", "");
    let scan = controller.scan_updates(op.cancel_flag());
    let dto = UpdateScanDto {
        offers: scan.offers.iter().map(UpdateOfferDto::from_core).collect(),
        failures: scan.failures.iter().map(failure_dto).collect(),
        skipped: scan.skipped as i64,
        checked: scan.checked as i64,
        cancelled: scan.cancelled,
    };
    // A cancelled check ends as cancelled on the Tasks page, not as a success.
    op.finish(if scan.cancelled {
        Err("The check was cancelled.".to_string())
    } else {
        Ok(())
    });
    Ok(dto)
}

/// Apply one app's update. A running app is refused unless `force` is set.
pub fn apply_update(op_id: String, uuid: String, force: bool) -> Result<OutcomeDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        apply_update_on(&mut controller, &op_id, uuid, force)
    })
}

/// The body of `apply_update`, run on a controller the caller provides, so a
/// test can run it on scratch seams.
fn apply_update_on(
    controller: &mut AppController,
    op_id: &str,
    uuid: String,
    force: bool,
) -> Result<OutcomeDto, CoreError> {
    let app = find_app(controller, &uuid)?;
    let op = OperationGuard::begin(op_id, TaskKind::Update, "Updating", &app.name);
    let result =
        controller.apply_update_with_progress(&app, force, op.cancel_flag(), &mut |event| {
            report_apply_event(&op, event)
        });
    let outcome = OutcomeDto::from_integrate(&result);
    op.finish(if result.ok {
        Ok(())
    } else {
        Err(result.error.clone())
    });
    Ok(outcome)
}

/// Check every app, then apply each offer in turn. Running apps are skipped
/// unless `force` is set. Progress is reported to the Tasks page per app.
pub fn apply_all_updates(op_id: String, force: bool) -> Result<BatchDto, CoreError> {
    guard(move || {
        let op = OperationGuard::begin(&op_id, TaskKind::Update, "Updating all", "");
        let mut controller = controller()?;
        let scan = controller.scan_updates(op.cancel_flag());
        let total = scan.offers.len();
        let mut applied = Vec::new();
        let mut failed = Vec::new();
        let mut skipped_running = Vec::new();
        let mut cancelled = scan.cancelled;
        for (index, offer) in scan.offers.iter().enumerate() {
            if op.cancel_flag().load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            let Some(app) = controller.registry().by_uuid(&offer.uuid) else {
                continue;
            };
            if !app.owned {
                continue;
            }
            op.progress(percent(index, total), &format!("Updating {}", app.name));
            let result = controller.apply_update(&app, force, op.cancel_flag());
            if result.ok {
                applied.push(app.name.clone());
            } else if result.error.to_lowercase().contains("running") {
                skipped_running.push(app.name.clone());
            } else {
                failed.push(UpdateFailureDto {
                    uuid: app.uuid.clone(),
                    name: app.name.clone(),
                    manager: app.update_manager.clone(),
                    error: result.error.clone(),
                    timed_out: is_timeout(&result.error),
                });
            }
        }
        let check_failures: Vec<UpdateFailureDto> = scan.failures.iter().map(failure_dto).collect();
        op.progress(100, "");
        op.finish(if cancelled {
            Err("The update was cancelled.".to_string())
        } else if failed.is_empty() && check_failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{} update(s) failed, {} app(s) could not be checked",
                failed.len(),
                check_failures.len()
            ))
        });
        Ok(BatchDto {
            applied,
            failed,
            skipped_running,
            check_failures,
            cancelled,
        })
    })
}

fn failure_dto(failure: &goshaim_core::updates_service::UpdateCheckFailure) -> UpdateFailureDto {
    UpdateFailureDto {
        uuid: failure.uuid.clone(),
        name: failure.name.clone(),
        manager: failure.manager.clone(),
        error: failure.error.clone(),
        timed_out: is_timeout(&failure.error),
    }
}

/// Show one update's stages on its Tasks entry. The bar follows the download
/// bytes; the verify and swap-in stages hold it at 100.
fn report_apply_event(op: &OperationGuard, event: ApplyEvent) {
    match event {
        ApplyEvent::Planned {
            from_version,
            to_version,
            ..
        } => op.versions(&from_version, &to_version),
        ApplyEvent::Phase { phase, done, total } => {
            let percent = match phase {
                UpdatePhase::Download if total > 0 => percent(done as usize, total as usize),
                UpdatePhase::Download => 0,
                UpdatePhase::Verify | UpdatePhase::SwapIn => 100,
            };
            op.phase(phase, done, total);
            op.progress(percent, phase.label());
        }
    }
}

fn percent(done: usize, total: usize) -> i32 {
    (done * 100).checked_div(total).unwrap_or(100) as i32
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::api::common::OperationGuard;
    use crate::api::dto::TaskStateDto;
    use crate::api::system::list_tasks;
    use goshaim_core::types::{ApplyEvent, UpdatePhase};

    /// A single update's Tasks entry moves while it downloads, before it
    /// finishes: the bar follows the bytes, and the entry names its versions
    /// and stage.
    #[test]
    fn a_download_moves_the_task_bar_before_the_update_finishes() {
        let target = "Bar progress test app";
        let op = OperationGuard::begin("bar-progress-op", TaskKind::Update, "Updating", target);
        report_apply_event(
            &op,
            ApplyEvent::Planned {
                from_version: "1.0".to_string(),
                to_version: "2.0".to_string(),
                download_size: 200,
            },
        );
        report_apply_event(
            &op,
            ApplyEvent::Phase {
                phase: UpdatePhase::Download,
                done: 50,
                total: 200,
            },
        );

        let mine = || {
            list_tasks()
                .into_iter()
                .find(|task| task.target == target)
                .expect("the update has a Tasks entry")
        };
        let running = mine();
        assert!(matches!(running.state, TaskStateDto::Running));
        assert_eq!(running.progress, 25, "50 of 200 bytes is 25 percent");
        assert_eq!(running.phase_index, 1);
        assert_eq!(running.phase, "Download");
        assert_eq!((running.bytes_done, running.bytes_total), (50, 200));
        assert_eq!(
            (running.from_version.as_str(), running.to_version.as_str()),
            ("1.0", "2.0")
        );
        assert!(running.started_at > 0 && running.finished_at == 0);

        report_apply_event(
            &op,
            ApplyEvent::Phase {
                phase: UpdatePhase::SwapIn,
                done: 0,
                total: 0,
            },
        );
        op.finish(Ok(()));
        let done = mine();
        assert!(matches!(done.state, TaskStateDto::Succeeded));
        assert!(done.finished_at >= done.started_at && done.finished_at > 0);
        assert_eq!(done.phase, "Swap in");
    }

    #[test]
    fn a_timeout_is_recognised_so_the_page_says_unknown() {
        assert!(crate::api::common::is_timeout(
            "Network request failed: timed out"
        ));
        assert!(!crate::api::common::is_timeout(
            "Network request failed: DNS resolution"
        ));
    }
}

#[cfg(test)]
mod cancel_tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc};
    use std::thread;
    use std::time::{Duration, Instant};

    use goshaim_core::controller::AppController;
    use goshaim_core::limits;
    use goshaim_core::network::{FetchResult, Local, NetworkClient};
    use goshaim_core::process::SystemRunner;
    use goshaim_core::proctable::SysTable;
    use goshaim_core::settings::Dirs;
    use goshaim_core::trash::FakeTrash;
    use goshaim_core::types::InstalledApp;

    use super::{apply_update_on, check_updates_on};
    use crate::api::common::CoreError;
    use crate::api::dto::{TaskKindDto, TaskStateDto};
    use crate::api::system::{cancel_task, list_tasks};

    /// The core must stop within this long after a cancel. The defect report
    /// asks for about two seconds.
    const STOP_TARGET: Duration = Duration::from_secs(2);

    /// How long the test waits for an operation to stop at all. It is longer
    /// than the network timeout, so a stall shows its real length.
    const WAIT_LIMIT: Duration = Duration::from_secs(40);

    /// Which call never finishes.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Stall {
        /// The size probe never answers, as a check's source does not.
        Probe,
        /// The probe answers; the download never delivers a byte.
        Download,
    }

    /// A network client whose requests never finish, as a connection that has
    /// gone quiet does. Like a blocked socket read, the calls do not look at
    /// the operation's cancel flag. Each stalled call holds for the network
    /// timeout and then fails, so a stall's real length shows.
    struct StalledNetwork {
        stall: Stall,
        calls: Arc<AtomicUsize>,
    }

    impl StalledNetwork {
        /// Hold for the network timeout, then report it as a timeout.
        fn stalled(&self) -> String {
            thread::sleep(Duration::from_millis(limits::NETWORK_TIMEOUT_MS));
            "Network request failed: timed out".to_string()
        }
    }

    impl NetworkClient for StalledNetwork {
        fn get(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _local: Local,
        ) -> Result<FetchResult, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(self.stalled())
        }

        fn head_len(&self, _url: &str, _local: Local) -> Result<Option<u64>, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            match self.stall {
                Stall::Probe => Err(self.stalled()),
                Stall::Download => Ok(Some(1_048_576)),
            }
        }

        fn download_bounded(
            &self,
            _url: &str,
            _max_bytes: u64,
            _local: Local,
        ) -> Result<Vec<u8>, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(self.stalled())
        }

        fn download_to_file(
            &self,
            _url: &str,
            _dest: &Path,
            _max_bytes: u64,
            _cancel: &std::sync::atomic::AtomicBool,
            _local: Local,
            _progress: &mut dyn FnMut(u64, u64),
        ) -> Result<u64, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(self.stalled())
        }
    }

    /// A scratch home for one test, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("gosh-r6-cancel-{label}-{}", std::process::id()));
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

    /// A controller rooted in `root` whose network stalls as `stall` says.
    fn controller_with(root: &Path, stall: Stall, calls: Arc<AtomicUsize>) -> AppController {
        AppController::with_seams(
            Box::new(SystemRunner::new()),
            Box::new(StalledNetwork { stall, calls }),
            Box::new(SysTable::new()),
            Box::new(FakeTrash::new()),
            Dirs::under(root),
        )
        .expect("a controller rooted in the scratch home")
    }

    /// One owned app whose update source is one version ahead.
    fn seed_app(root: &Path, name: &str) {
        let mut config = BTreeMap::new();
        config.insert(
            "url".to_string(),
            format!("https://mirror.example/{name}.AppImage"),
        );
        config.insert("version".to_string(), "2.0".to_string());
        let app = InstalledApp {
            uuid: format!("{name}-uuid"),
            name: name.to_string(),
            version: "1.0".to_string(),
            managed_path: root
                .join("AppImages")
                .join(format!("{name}.AppImage"))
                .to_string_lossy()
                .into_owned(),
            update_manager: "static".to_string(),
            update_config: config,
            ..InstalledApp::new_owned()
        };
        let mut controller = controller_with(root, Stall::Probe, Arc::new(AtomicUsize::new(0)));
        controller
            .registry_mut()
            .upsert(app)
            .expect("the registry accepts the app");
    }

    /// Wait until the operation is inside a stalled call, cancel it through the
    /// bridge, and time how long it takes to return. Then read the task's state.
    fn cancel_and_time(
        op_id: &str,
        calls: &AtomicUsize,
        in_flight: usize,
        returned: &mpsc::Receiver<Result<(), CoreError>>,
        kind: TaskKindDto,
        target: &str,
    ) -> (Duration, TaskStateDto) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while calls.load(Ordering::Relaxed) < in_flight {
            assert!(
                Instant::now() < deadline,
                "the operation never reached its stalled call"
            );
            thread::sleep(Duration::from_millis(10));
        }
        // Give the operation time to block inside the stalled call.
        thread::sleep(Duration::from_millis(300));

        let requested = Instant::now();
        assert!(
            cancel_task(op_id.to_string()),
            "the operation is still running"
        );
        let outcome = returned.recv_timeout(WAIT_LIMIT);
        let stopped_after = requested.elapsed();
        assert!(
            outcome.is_ok(),
            "the operation did not return within {WAIT_LIMIT:?} of the cancel"
        );
        let state = list_tasks()
            .into_iter()
            .find(|task| task.target == target && task.kind == kind)
            .map(|task| task.state)
            .expect("the operation has a Tasks entry");
        eprintln!(
            "{op_id}: stopped {:.2} s after the cancel, task {state:?}",
            stopped_after.as_secs_f64()
        );
        (stopped_after, state)
    }

    /// A check whose source never answers must stop promptly when cancelled,
    /// and its task must end as cancelled.
    #[test]
    fn a_cancelled_check_stops_within_two_seconds_and_finishes_as_cancelled() {
        let scratch = Scratch::new("check");
        seed_app(&scratch.0, "Cancel Check");
        let calls = Arc::new(AtomicUsize::new(0));
        let (done_tx, done_rx) = mpsc::channel();
        let (root, worker_calls) = (scratch.0.clone(), calls.clone());
        thread::spawn(move || {
            let controller = controller_with(&root, Stall::Probe, worker_calls);
            let outcome = check_updates_on(&controller, "r6-cancel-check").map(|_| ());
            let _ = done_tx.send(outcome);
        });

        let (stopped_after, state) = cancel_and_time(
            "r6-cancel-check",
            &calls,
            1,
            &done_rx,
            TaskKindDto::CheckUpdate,
            "",
        );
        assert!(
            stopped_after <= STOP_TARGET,
            "a cancelled check stopped after {:.2} s; the target is {:.0} s",
            stopped_after.as_secs_f64(),
            STOP_TARGET.as_secs_f64()
        );
        assert!(
            matches!(state, TaskStateDto::Cancelled),
            "a cancelled check ends as cancelled, not {state:?}"
        );
    }

    /// An update whose download never delivers a byte must stop promptly when
    /// cancelled, and its task must end as cancelled.
    #[test]
    fn a_cancelled_update_stops_within_two_seconds_and_finishes_as_cancelled() {
        let scratch = Scratch::new("update");
        seed_app(&scratch.0, "Cancel Update");
        let calls = Arc::new(AtomicUsize::new(0));
        let (done_tx, done_rx) = mpsc::channel();
        let (root, worker_calls) = (scratch.0.clone(), calls.clone());
        thread::spawn(move || {
            let mut controller = controller_with(&root, Stall::Download, worker_calls);
            let uuid = "Cancel Update-uuid".to_string();
            let outcome =
                apply_update_on(&mut controller, "r6-cancel-update", uuid, false).map(|_| ());
            let _ = done_tx.send(outcome);
        });

        let (stopped_after, state) = cancel_and_time(
            "r6-cancel-update",
            &calls,
            2,
            &done_rx,
            TaskKindDto::Update,
            "Cancel Update",
        );
        assert!(
            stopped_after <= STOP_TARGET,
            "a cancelled update stopped after {:.2} s; the target is {:.0} s",
            stopped_after.as_secs_f64(),
            STOP_TARGET.as_secs_f64()
        );
        assert!(
            matches!(state, TaskStateDto::Cancelled),
            "a cancelled update ends as cancelled, not {state:?}"
        );
    }
}
