//! System state: versions, the Tasks page, and cancellation.

use flutter_rust_bridge::frb;
use goshaim_core::tasks::TaskQueue;
use goshaim_core::types::TaskState;

use crate::api::common::{cancel_operation, with_tasks};
use crate::api::dto::TaskDto;

/// The bridge's own version. Synchronous, so it doubles as a cheap round-trip check.
#[frb(sync)]
pub fn bridge_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// The application version, as the core reports it.
pub fn app_version() -> String {
    goshaim_core::limits::VERSION.to_string()
}

/// Running tasks first, then the finished ones, as the Tasks page lists them.
pub fn list_tasks() -> Vec<TaskDto> {
    with_tasks(|queue| task_list(queue))
}

/// Each task is listed once. The history holds every task, running ones
/// included, so the running ones are split out of it rather than added again.
fn task_list(queue: &TaskQueue) -> Vec<TaskDto> {
    let (running, finished): (Vec<_>, Vec<_>) = queue
        .history()
        .into_iter()
        .partition(|task| matches!(task.state, TaskState::Queued | TaskState::Running));
    running
        .iter()
        .chain(finished.iter())
        .map(TaskDto::from_core)
        .collect()
}

/// Remove finished tasks from the Tasks page.
pub fn clear_finished_tasks() {
    with_tasks(|queue| queue.clear_finished());
}

/// Ask a running operation to stop. The argument is the operation's own id or
/// the task id the Tasks page lists for it. Returns false when no running
/// operation has that id.
pub fn cancel_task(op_id: String) -> bool {
    cancel_operation(&op_id)
}

#[cfg(test)]
mod tests {
    use goshaim_core::tasks::TaskQueue;
    use goshaim_core::types::TaskKind;

    use super::task_list;

    #[test]
    fn a_running_task_is_listed_once_and_before_the_finished_ones() {
        let mut queue = TaskQueue::new();
        let finished = queue.begin(TaskKind::Update, "Updating", "Quill Notes", false);
        queue.finish(&finished, Ok(()), false);
        let running = queue.begin(TaskKind::Update, "Updating", "Atlas Viewer", false);

        let listed = task_list(&queue);
        assert_eq!(listed.len(), 2, "a running task must not be listed twice");
        assert_eq!(listed[0].id, running);
        assert_eq!(listed[1].id, finished);
    }
}

#[cfg(test)]
mod cancel_tests {
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use goshaim_core::types::TaskKind;

    use super::{cancel_task, list_tasks};
    use crate::api::common::OperationGuard;
    use crate::api::dto::{TaskDto, TaskStateDto};

    /// Start an operation on a worker thread. It checks its cancel flag between
    /// steps, as the core does, and reports whether it was cancelled once it has
    /// stopped.
    fn start_operation(op_id: &'static str, target: &'static str) -> mpsc::Receiver<bool> {
        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || {
            let op = OperationGuard::begin(op_id, TaskKind::Inspect, "Testing cancel", target);
            let deadline = Instant::now() + Duration::from_secs(20);
            while !op.cancel_flag().load(Ordering::Relaxed) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            let cancelled = op.cancel_flag().load(Ordering::Relaxed);
            op.finish(if cancelled {
                Err("The operation was cancelled.".to_string())
            } else {
                Ok(())
            });
            let _ = done_tx.send(cancelled);
        });
        done_rx
    }

    /// The Tasks-page entry of the running operation with this target.
    fn running_task(target: &str) -> TaskDto {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let found = list_tasks()
                .into_iter()
                .find(|task| task.target == target && matches!(task.state, TaskStateDto::Running));
            if let Some(task) = found {
                return task;
            }
            assert!(
                Instant::now() < deadline,
                "the operation is listed as running"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn final_state(target: &str) -> TaskStateDto {
        list_tasks()
            .into_iter()
            .find(|task| task.target == target)
            .map(|task| task.state)
            .expect("the operation has a Tasks entry")
    }

    /// The Tasks page cancels by the task id it lists. The operation must stop
    /// and finish as cancelled, as it does for the op id.
    #[test]
    fn cancelling_by_task_id_stops_the_operation_and_finishes_it_as_cancelled() {
        let target = "QA3-006 by task id";
        let done = start_operation("r7-cancel-by-task-id", target);
        let task = running_task(target);

        assert!(
            cancel_task(task.id.clone()),
            "the task id {} names a running operation",
            task.id
        );
        let cancelled = done
            .recv_timeout(Duration::from_secs(5))
            .expect("the operation stops promptly");
        assert!(cancelled, "the operation saw its cancel flag");
        assert!(
            matches!(final_state(target), TaskStateDto::Cancelled),
            "a cancelled operation ends as cancelled"
        );
    }

    /// The status bar cancels by the op id, which must keep working.
    #[test]
    fn cancelling_by_op_id_stops_the_operation_and_finishes_it_as_cancelled() {
        let target = "QA3-006 by op id";
        let done = start_operation("r7-cancel-by-op-id", target);
        let _ = running_task(target);

        assert!(cancel_task("r7-cancel-by-op-id".to_string()));
        let cancelled = done
            .recv_timeout(Duration::from_secs(5))
            .expect("the operation stops promptly");
        assert!(cancelled, "the operation saw its cancel flag");
        assert!(matches!(final_state(target), TaskStateDto::Cancelled));
    }
}
