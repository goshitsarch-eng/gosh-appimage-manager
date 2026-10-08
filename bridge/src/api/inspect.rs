//! Inspect: read a file's facts without installing or executing it.

use std::path::Path;

use goshaim_core::types::{InspectOptions, TaskKind};

use crate::api::common::{controller, guard, CoreError, ErrorKind, OperationGuard};
use crate::api::dto::InspectDto;

/// Largest icon the bridge copies across. Icons are small; the limit keeps a
/// hostile file from pushing megabytes through the boundary.
const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;

/// Inspect one AppImage. A file that cannot be read or parsed comes back with
/// `error` set; only an empty path is an error of the call itself.
pub fn inspect_path(op_id: String, path: String) -> Result<InspectDto, CoreError> {
    guard(move || {
        if path.trim().is_empty() {
            return Err(CoreError::new(
                ErrorKind::UserInput,
                "Choose an AppImage to inspect.",
            ));
        }
        let op = OperationGuard::begin(&op_id, TaskKind::Inspect, "Inspecting", &path);
        let controller = controller()?;
        let existing = controller
            .registry()
            .by_path(&path)
            .map(|app| app.uuid)
            .unwrap_or_default();
        let options = InspectOptions {
            allow_unsafe_extract: controller.settings().unsafe_extraction_fallback(),
            confirm_unsafe_extract: false,
            max_bytes: controller.settings().max_appimage_bytes(),
            ..Default::default()
        };
        let result = controller.inspect_with(
            &path,
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
    })
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
