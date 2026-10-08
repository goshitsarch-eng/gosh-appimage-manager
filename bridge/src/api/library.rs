//! Library operations: the list, one app's details, launch, reveal, arguments
//! and environment, update sources, adopt, metadata refresh, and removal.

use std::collections::BTreeMap;

use goshaim_core::controller::AppController;
use goshaim_core::types::{EnvPair, InstalledApp, RemovalMode, RemovalRequest, TaskKind};

use crate::api::common::{classify, controller, guard, CoreError, ErrorKind, OperationGuard};
use crate::api::dto::{AppDto, DiscoveredDto, EnvVarDto, KeyValueDto, OutcomeDto};

/// Everything the Library page shows, in one call.
#[derive(Debug, Clone)]
pub struct LibraryDto {
    pub apps: Vec<AppDto>,
    pub discovered: Vec<DiscoveredDto>,
}

/// The registered apps, which of them are running, and what discovery finds.
pub fn list_library() -> Result<LibraryDto, CoreError> {
    guard(|| {
        let controller = controller()?;
        let apps = controller.registry().apps();
        let running = controller.running_uuids(&apps);
        let apps = apps
            .iter()
            .map(|app| AppDto::from_core(app, running.contains(&app.uuid)))
            .collect();
        let discovered = controller
            .discover()
            .iter()
            .map(DiscoveredDto::from_core)
            .collect();
        Ok(LibraryDto { apps, discovered })
    })
}

/// One app with its current running state.
pub fn get_app(uuid: String) -> Result<AppDto, CoreError> {
    guard(move || {
        let controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        let running = controller.is_running(&app);
        Ok(AppDto::from_core(&app, running))
    })
}

/// Start the app, detached. The manager never waits for it or stops it.
pub fn launch_app(uuid: String) -> Result<(), CoreError> {
    guard(move || {
        let controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        controller.launch_service().launch(&app).map_err(classify)
    })
}

/// Open the file manager at the app's folder.
pub fn reveal_app(uuid: String) -> Result<(), CoreError> {
    guard(move || {
        let controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        controller
            .reveal_in_file_manager(&app.managed_path)
            .map_err(classify)
    })
}

/// Replace the app's launch arguments and environment, and rewrite its entry.
pub fn save_arguments_and_environment(
    uuid: String,
    arguments: Vec<String>,
    environment: Vec<EnvVarDto>,
) -> Result<AppDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        let pairs: Vec<EnvPair> = environment
            .into_iter()
            .map(|var| EnvPair {
                name: var.name,
                value: var.value,
            })
            .collect();
        controller
            .set_arguments_and_environment(&uuid, arguments, pairs)
            .map_err(classify)?;
        let app = find_app(&controller, &uuid)?;
        let running = controller.is_running(&app);
        Ok(AppDto::from_core(&app, running))
    })
}

/// Choose how the app checks for updates. The config is the manager's keys.
pub fn set_update_source(
    uuid: String,
    manager: String,
    config: Vec<KeyValueDto>,
) -> Result<AppDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        let config: BTreeMap<String, String> = config
            .into_iter()
            .map(|pair| (pair.key, pair.value))
            .collect();
        let mut error = String::new();
        if !controller.set_update_source(app, &manager, config, &mut error) {
            return Err(classify(error));
        }
        let app = find_app(&controller, &uuid)?;
        let running = controller.is_running(&app);
        Ok(AppDto::from_core(&app, running))
    })
}

/// Remove the app's update source, so it is no longer checked.
pub fn unset_update_source(uuid: String) -> Result<AppDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        let mut error = String::new();
        if !controller.unset_update_source(app, &mut error) {
            return Err(classify(error));
        }
        let app = find_app(&controller, &uuid)?;
        let running = controller.is_running(&app);
        Ok(AppDto::from_core(&app, running))
    })
}

/// Register an external AppImage. Nothing on disk changes.
pub fn adopt_path(op_id: String, path: String) -> Result<AppDto, CoreError> {
    guard(move || {
        let op = OperationGuard::begin(&op_id, TaskKind::Adopt, "Adopting", &path);
        let mut controller = controller()?;
        match controller.adopt_external(&path) {
            Ok(app) => {
                op.finish(Ok(()));
                Ok(AppDto::from_core(&app, false))
            }
            Err(error) => {
                op.finish(Err(error.clone()));
                Err(classify(error))
            }
        }
    })
}

/// Re-read the app's metadata and rewrite its menu entry.
pub fn refresh_metadata(op_id: String, uuid: String) -> Result<AppDto, CoreError> {
    guard(move || {
        let op = OperationGuard::begin(
            &op_id,
            TaskKind::RefreshMetadata,
            "Refreshing metadata",
            &uuid,
        );
        let mut controller = controller()?;
        match controller.refresh_metadata(&uuid, op.cancel_flag()) {
            Ok(_name) => {
                let app = find_app(&controller, &uuid)?;
                op.finish(Ok(()));
                let running = controller.is_running(&app);
                Ok(AppDto::from_core(&app, running))
            }
            Err(error) => {
                op.finish(Err(error.clone()));
                Err(classify(error))
            }
        }
    })
}

/// Remove an app: to the Trash by default, or permanently when asked.
pub fn remove_app(op_id: String, uuid: String, permanent: bool) -> Result<OutcomeDto, CoreError> {
    guard(move || {
        let mut controller = controller()?;
        let app = find_app(&controller, &uuid)?;
        let op = OperationGuard::begin(&op_id, TaskKind::Remove, "Removing", &app.name);
        let request = RemovalRequest {
            path_or_uuid: uuid,
            mode: if permanent {
                RemovalMode::Permanent
            } else {
                RemovalMode::Trash
            },
            assume_yes: true,
        };
        let result = controller.remove_app(&request);
        let outcome = OutcomeDto::from_removal(&result);
        op.finish(if result.ok {
            Ok(())
        } else {
            Err(result.error.clone())
        });
        Ok(outcome)
    })
}

pub(crate) fn find_app(controller: &AppController, uuid: &str) -> Result<InstalledApp, CoreError> {
    controller
        .registry()
        .by_uuid(uuid)
        .ok_or_else(|| CoreError::new(ErrorKind::NotFound, "No installed app with that id."))
}
