//! Updates: check every app, apply one update, or apply all of them.

use std::sync::atomic::Ordering;

use goshaim_core::types::TaskKind;

use crate::api::common::{controller, guard, CoreError, OperationGuard};
use crate::api::dto::{BatchDto, OutcomeDto, UpdateFailureDto, UpdateOfferDto, UpdateScanDto};
use crate::api::library::find_app;

/// Check every app that has an update source. Apps without one are skipped,
/// and apps that could not be checked are listed as failures.
pub fn check_updates(op_id: String) -> Result<UpdateScanDto, CoreError> {
    guard(move || {
        let op = OperationGuard::begin(&op_id, TaskKind::CheckUpdate, "Checking for updates", "");
        let controller = controller()?;
        let scan = controller.scan_updates(op.cancel_flag());
        let dto = UpdateScanDto {
            offers: scan.offers.iter().map(UpdateOfferDto::from_core).collect(),
            failures: scan.failures.iter().map(failure_dto).collect(),
            skipped: scan.skipped as i64,
            checked: scan.checked as i64,
            cancelled: scan.cancelled,
        };
        op.finish(Ok(()));
        Ok(dto)
    })
}

/// Apply one app's update. A running app is refused unless `force` is set.
pub fn apply_update(op_id: String, uuid: String, force: bool) -> Result<OutcomeDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        let op = OperationGuard::begin(&op_id, TaskKind::Update, "Updating", &app.name);
        let result = controller.apply_update(&app, force, op.cancel_flag());
        let outcome = OutcomeDto::from_integrate(&result);
        op.finish(if result.ok {
            Ok(())
        } else {
            Err(result.error.clone())
        });
        Ok(outcome)
    })
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
                });
            }
        }
        let check_failures: Vec<UpdateFailureDto> = scan.failures.iter().map(failure_dto).collect();
        op.progress(100, "");
        op.finish(if failed.is_empty() && check_failures.is_empty() {
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
    }
}

fn percent(done: usize, total: usize) -> i32 {
    (done * 100).checked_div(total).unwrap_or(100) as i32
}
