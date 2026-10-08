//! Integrate: copy or move an AppImage into the managed folder and write its
//! menu entry. The core re-inspects the file, so the Inspect result is not
//! needed here.

use goshaim_core::controller::AppController;
use goshaim_core::types::{ConflictPolicy, CopyMode, InstalledApp, IntegrateRequest, TaskKind};

use crate::api::common::{controller, guard, CoreError, OperationGuard};
use crate::api::dto::OutcomeDto;

/// How to resolve a name conflict. `Automatic` means the caller has not chosen;
/// the core then refuses and reports the conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    Automatic,
    KeepBoth,
    Replace,
}

/// The core request for one integration. The fallback runs only when
/// `confirm_unsafe` says the user confirmed this file.
pub(crate) fn integrate_request(
    source_path: String,
    conflict: ConflictChoice,
    replace_uuid: String,
    move_source: bool,
    confirm_unsafe: bool,
) -> IntegrateRequest {
    IntegrateRequest {
        source_path,
        conflict: match conflict {
            ConflictChoice::Automatic => ConflictPolicy::Unspecified,
            ConflictChoice::KeepBoth => ConflictPolicy::KeepBoth,
            ConflictChoice::Replace => ConflictPolicy::Replace,
        },
        replace_uuid,
        copy_mode: if move_source {
            CopyMode::Move
        } else {
            CopyMode::Copy
        },
        assume_yes: true,
        confirm_unsafe,
    }
}

/// Integrate one file. `replace_uuid` is used only with `Replace`. With
/// `move_source`, the original goes to the Trash after a verified copy.
pub fn integrate_app(
    op_id: String,
    source_path: String,
    conflict: ConflictChoice,
    replace_uuid: String,
    move_source: bool,
    confirm_unsafe: bool,
) -> Result<OutcomeDto, CoreError> {
    guard(move || {
        let op = OperationGuard::begin(&op_id, TaskKind::Integrate, "Integrating", &source_path);
        let mut controller = controller()?;
        // Resolve what a Replace would target before asking, so the conflict
        // dialog can name it instead of demanding a UUID.
        let candidate = conflict_candidate(&controller, &source_path);
        let request = integrate_request(
            source_path,
            conflict,
            replace_uuid,
            move_source,
            confirm_unsafe,
        );
        let result = controller.integrate(&request, op.cancel_flag());
        let mut outcome = OutcomeDto::from_integrate(&result);
        if outcome.conflict {
            if let Some((uuid, name)) = candidate {
                outcome.conflict_uuid = uuid;
                outcome.conflict_name = name;
            }
        }
        // The Tasks entry reads "Integrated <name> <version>" once it is done.
        if result.ok {
            op.named(&result.app.name);
        }
        op.versions("", &result.app.version);
        op.finish(if result.ok {
            Ok(())
        } else {
            Err(result.error.clone())
        });
        Ok(outcome)
    })
}

/// Which managed installation a Replace would target for this source. Only a
/// single implicated installation is offered; more than one is ambiguous and
/// the user has to pick from the Library.
pub(crate) fn conflict_candidate(
    controller: &AppController,
    source: &str,
) -> Option<(String, String)> {
    let file_name = std::path::Path::new(source).file_name()?.to_string_lossy();
    let base = goshaim_core::desktop::sanitize_file_base(&file_name);
    let candidate = controller.settings().managed_folder().join(&base);
    let matches: Vec<InstalledApp> = controller
        .registry()
        .apps()
        .into_iter()
        .filter(|app| {
            app.owned
                && std::path::Path::new(&app.managed_path)
                    .file_stem()
                    .map(|s| goshaim_core::desktop::sanitize_file_base(&s.to_string_lossy()))
                    .as_deref()
                    == candidate
                        .file_stem()
                        .map(|s| s.to_string_lossy())
                        .as_deref()
        })
        .collect();
    if matches.len() == 1 {
        Some((matches[0].uuid.clone(), matches[0].name.clone()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_confirmation_reaches_the_integration_request() {
        let confirmed = integrate_request(
            "/x/Demo.AppImage".into(),
            ConflictChoice::Automatic,
            String::new(),
            false,
            true,
        );
        assert!(confirmed.confirm_unsafe);
        assert_eq!(confirmed.source_path, "/x/Demo.AppImage");
        let unconfirmed = integrate_request(
            "/x/Demo.AppImage".into(),
            ConflictChoice::Automatic,
            String::new(),
            false,
            false,
        );
        assert!(!unconfirmed.confirm_unsafe);
    }
}
