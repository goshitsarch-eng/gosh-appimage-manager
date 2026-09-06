// Gosh AppImage Manager — launch service (ports LaunchService).
// Start-only detached spawn via argv arrays (flatpak-spawn --host when
// sandboxed). The manager never waits on, nor kills, the launched app.

use std::path::Path;

use crate::desktop;
use crate::process::{ProcessRequest, ProcessRunner};
use crate::proctable::ProcessTable;
use crate::removal::canonical_existing;
use crate::types::InstalledApp;

pub struct LaunchService<'a> {
    runner: &'a dyn ProcessRunner,
    processes: &'a dyn ProcessTable,
}

impl<'a> LaunchService<'a> {
    pub fn new(runner: &'a dyn ProcessRunner, processes: &'a dyn ProcessTable) -> Self {
        Self { runner, processes }
    }

    pub fn nix_needs_appimage_run() -> bool {
        Path::new("/etc/NIXOS").exists()
    }

    pub fn is_running(&self, app: &InstalledApp) -> bool {
        let canonical = canonical_existing(&app.managed_path)
            .unwrap_or_else(|| Path::new(&app.managed_path).to_path_buf());
        !self
            .processes
            .pids_for_executable(&canonical.to_string_lossy())
            .is_empty()
    }

    pub fn launch(&self, app: &InstalledApp) -> Result<(), String> {
        if app.managed_path.is_empty() {
            return Err("Missing managed path".to_string());
        }
        let (program, mut args) = if Self::nix_needs_appimage_run() {
            let probe = self.runner.run(&ProcessRequest {
                program: "appimage-run".to_string(),
                args: vec!["--version".to_string()],
                host: true,
                timeout_ms: 3000,
                ..Default::default()
            });
            if probe.refused || probe.timed_out {
                return Err("appimage-run is required on NixOS but was not found".to_string());
            }
            (
                "appimage-run".to_string(),
                std::iter::once(app.managed_path.clone())
                    .chain(app.arguments.iter().cloned())
                    .collect::<Vec<_>>(),
            )
        } else {
            (app.managed_path.clone(), app.arguments.clone())
        };
        let mut env: Vec<(String, String)> = Vec::new();
        for pair in &app.environment {
            if desktop::valid_env_name(&pair.name)
                && !is_dangerous_env_name(&pair.name)
                && valid_env_value(&pair.value)
            {
                env.push((pair.name.clone(), pair.value.clone()));
            }
        }
        let _ = &mut args;
        self.runner
            .start_detached(&ProcessRequest {
                program,
                args,
                env,
                host: true,
                timeout_ms: 10_000,
                ..Default::default()
            })
            .map_err(|_| "Failed to start".to_string())
    }
}

fn is_dangerous_env_name(name: &str) -> bool {
    name.starts_with("LD_")
        || name.starts_with("DYLD_")
        || name == "GCONV_PATH"
        || name == "LOCPATH"
}

fn valid_env_value(value: &str) -> bool {
    !value.contains('\0')
        && !value.chars().any(|c| c.is_control())
        && value.len() <= crate::limits::MAX_ARGUMENT_LENGTH
}
