//! Shared bridge plumbing: typed errors, panic containment, the operation
//! registry (cancellation and the Tasks page), and the core task queue.
//!
//! Operations that the old GUI ran on a worker register here. The Dart side
//! names each operation with its own id, so it can cancel it while the call is
//! still in flight.

use std::collections::HashMap;
use std::fmt;
use std::panic::UnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use goshaim_core::controller::AppController;
use goshaim_core::tasks::TaskQueue;
use goshaim_core::types::{TaskKind, UpdatePhase};

/// Result type for every bridge function.
pub type Res<T> = Result<T, CoreError>;

/// The failure kinds the front end distinguishes (design doc, section 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    UserInput,
    Validation,
    NotFound,
    Permission,
    CorruptData,
    Network,
    /// A network operation ran out of time. Kept apart from `Network` because
    /// the Updates page says the status is unknown rather than failed.
    Timeout,
    Process,
    Failure,
    Internal,
}

/// A failure the front end cannot turn into an operation outcome. `message` is
/// user-safe; `details` is for the log only.
#[derive(Debug, Clone)]
pub struct CoreError {
    pub kind: ErrorKind,
    pub message: String,
    pub details: String,
}

impl CoreError {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            details: String::new(),
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CoreError {}

/// Map a core failure string to a typed kind. The core still reports failures
/// as text (defect D-20). This is the single place that reads that text.
pub(crate) fn classify(message: String) -> CoreError {
    let lower = message.to_lowercase();
    let kind = if lower.contains("no installed app")
        || lower.contains("cannot read") && lower.contains("no such file")
    {
        ErrorKind::NotFound
    } else if lower.contains("not an owned")
        || lower.contains("protected path")
        || lower.contains("permission denied")
    {
        ErrorKind::Permission
    } else if lower.contains("timed out") {
        ErrorKind::Timeout
    } else if lower.contains("dns resolution") || lower.contains("network") {
        ErrorKind::Network
    } else if lower.contains("not valid json") || lower.contains("corrupt") {
        ErrorKind::CorruptData
    } else if lower.contains("elf")
        || lower.contains("url rejected")
        || lower.contains("not allowed")
        || lower.contains("not valid")
        || lower.contains("too many")
        || lower.contains("not usable")
        || lower.contains("not a valid")
    {
        ErrorKind::Validation
    } else {
        ErrorKind::Failure
    };
    CoreError {
        kind,
        message,
        details: String::new(),
    }
}

/// Whether a core failure message is a timeout. See `classify`.
pub(crate) fn is_timeout(message: &str) -> bool {
    classify(message.to_string()).kind == ErrorKind::Timeout
}

/// Run a bridge body, turning a Rust panic into a typed `internal` error so it
/// never ends the process (design doc, section 4.4).
pub(crate) fn guard<T>(body: impl FnOnce() -> Res<T> + UnwindSafe) -> Res<T> {
    match std::panic::catch_unwind(body) {
        Ok(result) => result,
        Err(payload) => Err(CoreError {
            kind: ErrorKind::Internal,
            message: "Something went wrong inside the core. Details were written to the log."
                .to_string(),
            details: panic_message(payload),
        }),
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        return (*text).to_string();
    }
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    "non-string panic payload".to_string()
}

/// Build a fresh controller for one call. Each call reads the settings and the
/// registry from disk, so no lock is held between calls. That keeps long jobs
/// from blocking the others (defect D-03).
pub(crate) fn controller() -> Res<AppController> {
    AppController::new().map_err(|error| CoreError {
        kind: ErrorKind::Failure,
        message: error,
        details: String::new(),
    })
}

fn tasks_guard() -> MutexGuard<'static, TaskQueue> {
    static TASKS: OnceLock<Mutex<TaskQueue>> = OnceLock::new();
    TASKS
        .get_or_init(|| Mutex::new(TaskQueue::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Run a closure against the shared task queue.
pub(crate) fn with_tasks<T>(body: impl FnOnce(&mut TaskQueue) -> T) -> T {
    let mut queue = tasks_guard();
    body(&mut queue)
}

struct Operation {
    cancel: Arc<AtomicBool>,
    task_id: String,
}

fn operations() -> MutexGuard<'static, HashMap<String, Operation>> {
    static OPERATIONS: OnceLock<Mutex<HashMap<String, Operation>>> = OnceLock::new();
    OPERATIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Owns one registered operation. Dropping it without `finish` records the
/// operation as failed, so a panic cannot leave a task stuck in "running".
pub(crate) struct OperationGuard {
    op_id: String,
    cancel: Arc<AtomicBool>,
    finished: bool,
}

impl OperationGuard {
    /// Start an operation: show it on the Tasks page and make it cancellable.
    pub(crate) fn begin(op_id: &str, kind: TaskKind, title: &str, target: &str) -> Self {
        let task_id = with_tasks(|queue| queue.begin(kind, title, target, false));
        let cancel = Arc::new(AtomicBool::new(false));
        operations().insert(
            op_id.to_string(),
            Operation {
                cancel: cancel.clone(),
                task_id,
            },
        );
        Self {
            op_id: op_id.to_string(),
            cancel,
            finished: false,
        }
    }

    /// The flag the core polls to stop early.
    pub(crate) fn cancel_flag(&self) -> &AtomicBool {
        &self.cancel
    }

    /// Report progress for the Tasks page.
    pub(crate) fn progress(&self, percent: i32, status: &str) {
        if let Some(task_id) = task_id_of(&self.op_id) {
            with_tasks(|queue| queue.progress(&task_id, percent, status));
        }
    }

    /// Name the app this operation concerns, once it is known.
    pub(crate) fn named(&self, name: &str) {
        if let Some(task_id) = task_id_of(&self.op_id) {
            with_tasks(|queue| queue.set_target(&task_id, name));
        }
    }

    /// Record the versions this operation moves between.
    pub(crate) fn versions(&self, from_version: &str, to_version: &str) {
        if let Some(task_id) = task_id_of(&self.op_id) {
            with_tasks(|queue| queue.set_versions(&task_id, from_version, to_version));
        }
    }

    /// Record whether this removal deletes permanently (true) or moves the app
    /// to the Trash (false).
    pub(crate) fn set_permanent(&self, permanent: bool) {
        if let Some(task_id) = task_id_of(&self.op_id) {
            with_tasks(|queue| queue.set_permanent(&task_id, permanent));
        }
    }

    /// Record the update stage and the bytes moved in it.
    pub(crate) fn phase(&self, phase: UpdatePhase, done: u64, total: u64) {
        if let Some(task_id) = task_id_of(&self.op_id) {
            with_tasks(|queue| queue.set_phase(&task_id, phase, done, total));
        }
    }

    /// Finish the operation with its outcome.
    pub(crate) fn finish(mut self, outcome: Result<(), String>) {
        self.complete(outcome);
    }

    fn complete(&mut self, outcome: Result<(), String>) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some(operation) = operations().remove(&self.op_id) {
            let cancelled = operation.cancel.load(Ordering::Relaxed);
            with_tasks(|queue| queue.finish(&operation.task_id, outcome, cancelled));
        }
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        // This runs while a panic may be unwinding through the operation. A
        // panic that escapes a destructor during an unwind aborts the process,
        // so nothing in this path may leave it.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.complete(Err("The operation stopped unexpectedly.".to_string()));
        }));
    }
}

fn task_id_of(op_id: &str) -> Option<String> {
    operations()
        .get(op_id)
        .map(|operation| operation.task_id.clone())
}

/// Ask a running operation to stop. `id` is either the operation's own id (the
/// status bar names an operation that way) or its Tasks-page task id (the Tasks
/// page names it that way). Returns false when no running operation has that id.
pub(crate) fn cancel_operation(id: &str) -> bool {
    let task_id = {
        let registry = operations();
        // The op id is the registry key, and it is tried first, so an op id that
        // happens to look like a task id still names its own operation. A task
        // id is matched against each operation's task id, the reverse of
        // `task_id_of`.
        let operation = registry
            .get(id)
            .or_else(|| registry.values().find(|operation| operation.task_id == id));
        match operation {
            Some(operation) => {
                operation.cancel.store(true, Ordering::Relaxed);
                operation.task_id.clone()
            }
            None => return false,
        }
    };
    with_tasks(|queue| queue.mark_cancelling(&task_id));
    true
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    #[test]
    fn a_panic_becomes_an_internal_error_and_later_calls_still_work() {
        let error = guard(|| -> Res<()> { panic!("deliberate panic in a test") })
            .expect_err("a panic must not escape the guard");
        assert!(matches!(error.kind, ErrorKind::Internal));
        assert!(error.details.contains("deliberate panic in a test"));
        assert_eq!(guard(|| Ok(7)).expect("a clean call returns its value"), 7);
    }

    /// Audit R6-01: a panic while an operation is registered must not stop the
    /// process or the registry. The operation is recorded as failed, dropped
    /// from the registry, and later operations still run.
    #[test]
    fn a_panic_inside_an_operation_leaves_the_registry_usable() {
        let outcome = std::panic::catch_unwind(|| {
            let _op = OperationGuard::begin(
                "r6-01-panicking",
                TaskKind::Inspect,
                "Inspecting",
                "panicking.AppImage",
            );
            panic!("deliberate panic inside an operation");
        });
        assert!(outcome.is_err(), "the panic reaches the caller");
        assert!(
            !cancel_operation("r6-01-panicking"),
            "the dropped operation is no longer registered"
        );
        let recorded = crate::api::system::list_tasks()
            .into_iter()
            .find(|task| task.target == "panicking.AppImage")
            .expect("the task is still listed");
        assert!(
            matches!(recorded.state, crate::api::dto::TaskStateDto::Failed),
            "the operation is recorded as failed, not left running"
        );

        let op = OperationGuard::begin(
            "r6-01-after",
            TaskKind::Inspect,
            "Inspecting",
            "after.AppImage",
        );
        assert!(
            cancel_operation("r6-01-after"),
            "a later operation registers and can be cancelled"
        );
        op.finish(Ok(()));
    }

    /// Audit R6-01: a panic while the task queue is locked poisons its lock. The
    /// next operation must still start and finish, with no abort.
    #[test]
    fn a_panic_while_the_task_queue_is_locked_does_not_stop_later_operations() {
        let _ = std::panic::catch_unwind(|| {
            with_tasks(|_queue| -> () { panic!("deliberate panic while the task queue is locked") })
        });
        let op = OperationGuard::begin(
            "r6-01-poisoned",
            TaskKind::Inspect,
            "Inspecting",
            "poisoned.AppImage",
        );
        op.finish(Ok(()));
        let recorded = crate::api::system::list_tasks()
            .into_iter()
            .find(|task| task.target == "poisoned.AppImage")
            .expect("the later operation is listed");
        assert!(matches!(
            recorded.state,
            crate::api::dto::TaskStateDto::Succeeded
        ));
    }
}
