// Gosh AppImage Manager — task queue (ports TaskQueue).
// Conflicting mutations run serially; reads stay cancellable.
// History is bounded; progress callbacks are rate-limited by callers.

use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;

use crate::limits;
use crate::types::{task_kind_name, TaskItem, TaskKind, TaskState};

pub struct TaskQueue {
    history: VecDeque<TaskItem>,
    next_id: u64,
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
            next_id: 1,
        }
    }

    pub fn history(&self) -> Vec<TaskItem> {
        self.history.iter().cloned().collect()
    }

    /// Tasks that have started and not yet finished.
    ///
    /// This could never return anything before: `run_task` recorded a task
    /// only once it had already completed, so by the time an entry existed its
    /// state was terminal. Nothing could show work in progress, which is why
    /// the Tasks page the brief asks for had nothing to display.
    pub fn active(&self) -> Vec<TaskItem> {
        self.history
            .iter()
            .filter(|t| matches!(t.state, TaskState::Queued | TaskState::Running))
            .cloned()
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<TaskItem> {
        self.history.iter().find(|t| t.id == id).cloned()
    }

    /// Record a task that has just started, and return its id.
    ///
    /// The caller runs the work elsewhere -- on a worker thread, so the UI
    /// stays responsive -- and reports back through `progress` and `finish`.
    pub fn begin(&mut self, kind: TaskKind, title: &str, target: &str, retryable: bool) -> String {
        let id = format!("task-{}", self.next_id);
        self.next_id += 1;
        let item = TaskItem {
            id: id.clone(),
            kind,
            state: TaskState::Running,
            title: title.to_string(),
            target: target.to_string(),
            progress: 0,
            status_text: task_kind_name(kind).to_string(),
            error: String::new(),
            retryable,
        };
        self.push(item);
        id
    }

    /// Update a running task's progress and status line.
    pub fn progress(&mut self, id: &str, percent: i32, status: &str) {
        if let Some(item) = self.history.iter_mut().find(|t| t.id == id) {
            if matches!(item.state, TaskState::Running | TaskState::Queued) {
                item.progress = percent.clamp(0, 100);
                item.status_text = status.to_string();
            }
        }
    }

    /// Mark a running task as cancelling, so the UI can say so while the
    /// worker notices the flag.
    pub fn mark_cancelling(&mut self, id: &str) {
        if let Some(item) = self.history.iter_mut().find(|t| t.id == id) {
            if matches!(item.state, TaskState::Running | TaskState::Queued) {
                item.state = TaskState::Cancelling;
                item.status_text = "cancelling".to_string();
            }
        }
    }

    /// Record the outcome of a task started with `begin`.
    pub fn finish(&mut self, id: &str, outcome: Result<(), String>, cancelled: bool) {
        if let Some(item) = self.history.iter_mut().find(|t| t.id == id) {
            match outcome {
                Ok(()) => {
                    item.state = TaskState::Succeeded;
                    item.progress = 100;
                    item.status_text = "done".to_string();
                }
                Err(error) => {
                    item.state = if cancelled {
                        TaskState::Cancelled
                    } else {
                        TaskState::Failed
                    };
                    item.status_text = if cancelled { "cancelled" } else { "failed" }.to_string();
                    item.error = error;
                }
            }
        }
    }

    /// Run one task to completion inline, recording it in bounded history.
    ///
    /// Kept for callers with nothing else to do while they wait (the CLI).
    /// Anything with a user interface should use `begin`/`finish` around work
    /// on another thread.
    pub fn run_task(
        &mut self,
        kind: TaskKind,
        title: &str,
        target: &str,
        retryable: bool,
        cancel: &AtomicBool,
        op: impl FnOnce(&mut dyn FnMut(i32, &str), &AtomicBool) -> Result<(), String>,
    ) -> TaskItem {
        let id = self.begin(kind, title, target, retryable);
        let mut progress: Vec<(i32, String)> = Vec::new();
        let outcome = if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            Err("Cancelled".to_string())
        } else {
            let mut record = |percent: i32, status: &str| {
                progress.push((percent, status.to_string()));
            };
            op(&mut record, cancel)
        };
        if let Some((percent, status)) = progress.last() {
            self.progress(&id, *percent, status);
        }
        let cancelled = cancel.load(std::sync::atomic::Ordering::Relaxed);
        self.finish(&id, outcome, cancelled);
        self.get(&id).expect("task was just recorded")
    }

    fn push(&mut self, item: TaskItem) {
        self.history.push_back(item);
        // Bound history, but never drop a task that is still running.
        while self.history.len() > limits::MAX_TASK_HISTORY {
            let removable = self
                .history
                .iter()
                .position(|t| !matches!(t.state, TaskState::Running | TaskState::Queued));
            match removable {
                Some(index) => {
                    self.history.remove(index);
                }
                None => break,
            }
        }
    }

    /// Drop completed tasks, keeping anything still in flight.
    pub fn clear_finished(&mut self) {
        self.history.retain(|t| {
            matches!(
                t.state,
                TaskState::Queued | TaskState::Running | TaskState::Cancelling
            )
        });
    }
}
