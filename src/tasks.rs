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

    pub fn active(&self) -> Vec<TaskItem> {
        self.history
            .iter()
            .filter(|t| matches!(t.state, TaskState::Queued | TaskState::Running))
            .cloned()
            .collect()
    }

    /// Run one task to completion, recording it in bounded history.
    pub fn run_task(
        &mut self,
        kind: TaskKind,
        title: &str,
        target: &str,
        retryable: bool,
        cancel: &AtomicBool,
        op: impl FnOnce(&mut dyn FnMut(i32, &str), &AtomicBool) -> Result<(), String>,
    ) -> TaskItem {
        let id = format!("task-{}", self.next_id);
        self.next_id += 1;
        let mut item = TaskItem {
            id,
            kind,
            state: TaskState::Running,
            title: title.to_string(),
            target: target.to_string(),
            progress: 0,
            status_text: task_kind_name(kind).to_string(),
            error: String::new(),
            retryable,
        };
        let mut progress = |percent: i32, status: &str| {
            item.progress = percent.clamp(0, 100);
            item.status_text = status.to_string();
        };
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            item.state = TaskState::Cancelled;
        } else {
            match op(&mut progress, cancel) {
                Ok(()) => {
                    item.state = TaskState::Succeeded;
                    item.progress = 100;
                }
                Err(error) => {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        item.state = TaskState::Cancelled;
                    } else {
                        item.state = TaskState::Failed;
                    }
                    item.error = error;
                }
            }
        }
        self.history.push_back(item.clone());
        while self.history.len() > limits::MAX_TASK_HISTORY {
            self.history.pop_front();
        }
        item
    }

    pub fn clear_finished(&mut self) {
        self.history.retain(|t| {
            matches!(
                t.state,
                TaskState::Queued | TaskState::Running | TaskState::Cancelling
            )
        });
    }
}
