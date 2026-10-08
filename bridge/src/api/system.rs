//! System state: versions, the Tasks page, and cancellation.

use flutter_rust_bridge::frb;

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
    with_tasks(|queue| {
        let mut items = queue.active();
        items.extend(queue.history());
        items.iter().map(TaskDto::from_core).collect()
    })
}

/// Remove finished tasks from the Tasks page.
pub fn clear_finished_tasks() {
    with_tasks(|queue| queue.clear_finished());
}

/// Ask a running operation to stop. Returns false when it is not running.
pub fn cancel_task(op_id: String) -> bool {
    cancel_operation(&op_id)
}
