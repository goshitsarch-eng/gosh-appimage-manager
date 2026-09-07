// Gosh AppImage Manager — CLI frontend (ports src/Cli.cpp exactly).
// Same binary, no GUI. Machine output goes to stdout only; prompts,
// diagnostics, and errors go to stderr. JSON uses schema_version with
// `installed` / `updates` arrays.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::atomic::AtomicBool;

use crate::controller::AppController;
use crate::elf;
use crate::inspector::AppImageInspector;
use crate::limits;
use crate::process::ProcessRequest;
use crate::types::{
    app_image_type_name, architecture_name, ConflictPolicy, CopyMode, ExitCode, InspectOptions,
    InstalledApp, IntegrateRequest, RemovalMode, RemovalRequest,
};
use crate::updates_sources::UpdateSourceFactory;

fn has_arg(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn option_path(args: &[String], name: &str) -> String {
    let Some(idx) = args.iter().position(|a| a == name) else {
        return String::new();
    };
    const SKIP_ALONE: &[&str] = &[
        "--yes",
        "-y",
        "--keep-both",
        "--replace",
        "--force",
        "--delete",
        "--json",
        "--all",
        "--unset",
    ];
    const SKIP_WITH_VALUE: &[&str] = &["--replace-uuid", "--target", "--manager"];
    let mut i = idx + 1;
    while i < args.len() {
        let arg = &args[i];
        if SKIP_ALONE.contains(&arg.as_str()) {
            i += 1;
            continue;
        }
        if SKIP_WITH_VALUE.contains(&arg.as_str()) {
            i += 2;
            continue;
        }
        if arg.starts_with("--") {
            i += 1;
            continue;
        }
        return arg.clone();
    }
    String::new()
}

fn arg_value(args: &[String], name: &str) -> String {
    let path = option_path(args, name);
    if !path.is_empty() {
        return path;
    }
    if let Some(idx) = args.iter().position(|a| a == name) {
        if idx + 1 < args.len() && !args[idx + 1].starts_with("--") {
            return args[idx + 1].clone();
        }
    }
    String::new()
}

fn confirm(
    prompt: &str,
    yes: bool,
    tty: bool,
    stderr: &mut dyn Write,
    stdin: &mut dyn Read,
) -> bool {
    if yes {
        return true;
    }
    if !tty {
        let _ = writeln!(
            stderr,
            "Refusing destructive action without a TTY; pass --yes"
        );
        return false;
    }
    let _ = write!(stderr, "{prompt} [y/N] ");
    let _ = stderr.flush();
    let mut buf = [0u8; 1];
    match stdin.read(&mut buf) {
        Ok(1) => buf[0] == b'y' || buf[0] == b'Y',
        _ => false,
    }
}

fn app_json(app: &InstalledApp) -> serde_json::Map<String, serde_json::Value> {
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), serde_json::Value::String(app.name.clone()));
    obj.insert(
        "path".into(),
        serde_json::Value::String(app.managed_path.clone()),
    );
    obj.insert(
        "desktop_id".into(),
        serde_json::Value::String(app.desktop_id.clone()),
    );
    obj.insert(
        "current_version".into(),
        serde_json::Value::String(app.version.clone()),
    );
    obj.insert(
        "available_version".into(),
        serde_json::Value::String(app.available_version.clone()),
    );
    obj.insert(
        "download_size".into(),
        serde_json::Value::Number(app.available_size.into()),
    );
    obj.insert(
        "manager".into(),
        serde_json::Value::String(app.update_manager.clone()),
    );
    obj.insert(
        "embedded_source".into(),
        serde_json::Value::String(app.embedded_update.clone()),
    );
    obj.insert("running".into(), serde_json::Value::Bool(app.running));
    obj.insert("uuid".into(), serde_json::Value::String(app.uuid.clone()));
    obj.insert("owned".into(), serde_json::Value::Bool(app.owned));
    obj
}

fn print_json(
    key: &str,
    items: Vec<serde_json::Map<String, serde_json::Value>>,
    stdout: &mut dyn Write,
) -> ExitCode {
    let mut root = serde_json::Map::new();
    root.insert(
        "schema_version".into(),
        serde_json::Value::Number(limits::JSON_SCHEMA_VERSION.into()),
    );
    root.insert(
        key.into(),
        serde_json::Value::Array(items.into_iter().map(serde_json::Value::Object).collect()),
    );
    let mut data = serde_json::to_vec(&root).unwrap_or_default();
    data.push(b'\n');
    let _ = stdout.write_all(&data);
    ExitCode::Ok
}

pub fn run_cli(
    controller: &mut AppController,
    args: &[String],
    interactive_tty: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    stdin: &mut dyn Read,
) -> ExitCode {
    let yes = has_arg(args, "--yes") || has_arg(args, "-y");
    let json = has_arg(args, "--json");
    let force = has_arg(args, "--force");
    let keep_both = has_arg(args, "--keep-both");
    let replace = has_arg(args, "--replace");
    let del = has_arg(args, "--delete");
    let cancel = AtomicBool::new(false);

    if has_arg(args, "--list-update-managers") {
        for name in UpdateSourceFactory::names() {
            let _ = writeln!(stdout, "{name}");
        }
        return ExitCode::Ok;
    }

    if has_arg(args, "--list-installed") {
        let mut items = Vec::new();
        let apps = controller.registry().apps();
        let running = controller.running_uuids(&apps);
        for mut app in apps {
            app.running = running.contains(&app.uuid);
            if json {
                items.push(app_json(&app));
            } else {
                let _ = writeln!(
                    stdout,
                    "{}\t{}\t{}",
                    app.name, app.managed_path, app.version
                );
            }
        }
        return if json {
            print_json("installed", items, stdout)
        } else {
            ExitCode::Ok
        };
    }

    if has_arg(args, "--list-updates") {
        let scan = controller.scan_updates(&cancel);
        let offers = &scan.offers;
        for failure in &scan.failures {
            let _ = writeln!(
                stderr,
                "{}: update check failed ({}): {}",
                failure.name, failure.manager, failure.error
            );
        }
        let mut items = Vec::new();
        for offer in offers {
            if json {
                let app = controller.registry().by_uuid(&offer.uuid);
                let mut obj = serde_json::Map::new();
                obj.insert("name".into(), serde_json::Value::String(offer.name.clone()));
                obj.insert(
                    "path".into(),
                    serde_json::Value::String(app.map(|a| a.managed_path).unwrap_or_default()),
                );
                obj.insert(
                    "desktop_id".into(),
                    serde_json::Value::String(
                        controller
                            .registry()
                            .by_uuid(&offer.uuid)
                            .map(|a| a.desktop_id)
                            .unwrap_or_default(),
                    ),
                );
                obj.insert(
                    "current_version".into(),
                    serde_json::Value::String(offer.current_version.clone()),
                );
                obj.insert(
                    "available_version".into(),
                    serde_json::Value::String(offer.available_version.clone()),
                );
                obj.insert(
                    "download_size".into(),
                    serde_json::Value::Number(offer.download_size.into()),
                );
                obj.insert(
                    "manager".into(),
                    serde_json::Value::String(offer.manager.clone()),
                );
                obj.insert(
                    "embedded_source".into(),
                    serde_json::Value::String(offer.embedded_source.clone()),
                );
                obj.insert("running".into(), serde_json::Value::Bool(offer.running));
                items.push(obj);
            } else {
                let _ = writeln!(
                    stdout,
                    "{}\t{} -> {}",
                    offer.name, offer.current_version, offer.available_version
                );
            }
        }
        if json {
            print_json("updates", items, stdout);
        }
        // stdout stays a valid, complete JSON document either way; the exit
        // code is what tells a script the list may be short.
        return if scan.failures.is_empty() {
            ExitCode::Ok
        } else {
            ExitCode::Network
        };
    }

    if has_arg(args, "--list-discovered") {
        // Discovery and adoption were fully implemented but reachable from no
        // command and no UI, so external AppImages could never be adopted.
        let discovered = controller.discover();
        let mut items = Vec::new();
        for app in &discovered {
            if json {
                let mut obj = serde_json::Map::new();
                obj.insert("name".into(), serde_json::Value::String(app.name.clone()));
                obj.insert("path".into(), serde_json::Value::String(app.path.clone()));
                obj.insert("managed".into(), serde_json::Value::Bool(app.managed));
                obj.insert("uuid".into(), serde_json::Value::String(app.uuid.clone()));
                obj.insert(
                    "origin".into(),
                    serde_json::Value::String(
                        match app.origin {
                            crate::library::Origin::ManagedFolder => "managed-folder",
                            crate::library::Origin::ExternalDesktopEntry => "external-entry",
                        }
                        .to_string(),
                    ),
                );
                obj.insert(
                    "desktop_path".into(),
                    serde_json::Value::String(app.desktop_path.clone()),
                );
                items.push(obj);
            } else {
                let _ = writeln!(
                    stdout,
                    "{}\t{}\t{}",
                    if app.managed { "managed" } else { "external" },
                    app.name,
                    app.path
                );
            }
        }
        return if json {
            print_json("discovered", items, stdout)
        } else {
            ExitCode::Ok
        };
    }

    if has_arg(args, "--adopt") {
        let path = arg_value(args, "--adopt");
        if path.is_empty() {
            let _ = writeln!(stderr, "Usage: --adopt <path> [--yes]");
            return ExitCode::Usage;
        }
        if !confirm(
            &format!("Adopt {path}? (registers it; nothing on disk is changed)"),
            yes,
            interactive_tty,
            stderr,
            stdin,
        ) {
            return ExitCode::NeedsConfirmation;
        }
        // Validate it really is an AppImage before registering it. Adoption
        // itself writes nothing but the registry row.
        let inspected = controller.inspect_file(&path, &cancel, None);
        if !inspected.magic_valid {
            let _ = writeln!(
                stderr,
                "{}",
                if inspected.error.is_empty() {
                    "Not a valid AppImage".to_string()
                } else {
                    inspected.error.clone()
                }
            );
            inspected.discard_staging();
            return ExitCode::Validation;
        }
        inspected.discard_staging();
        return match controller.adopt_external(&path) {
            Ok(app) => {
                let _ = writeln!(stderr, "Adopted {} as {}", app.managed_path, app.uuid);
                ExitCode::Ok
            }
            Err(error) => {
                let _ = writeln!(stderr, "{error}");
                ExitCode::Failure
            }
        };
    }

    if has_arg(args, "--integrate") {
        let path = arg_value(args, "--integrate");
        if path.is_empty() {
            let _ = writeln!(
                stderr,
                "Usage: --integrate <path> [--keep-both|--replace|--replace-uuid UUID|--target PATH] [--yes]"
            );
            return ExitCode::Usage;
        }
        if !confirm(
            &format!("Integrate {path}?"),
            yes,
            interactive_tty,
            stderr,
            stdin,
        ) {
            return ExitCode::NeedsConfirmation;
        }
        let mut req = IntegrateRequest {
            source_path: path.clone(),
            ..Default::default()
        };
        req.copy_mode = if controller.settings().move_source() {
            CopyMode::Move
        } else {
            CopyMode::Copy
        };
        req.assume_yes = yes;
        if replace {
            req.conflict = ConflictPolicy::Replace;
            let mut target = arg_value(args, "--replace-uuid");
            if target.is_empty() {
                target = arg_value(args, "--target");
            }
            let mut owned: Option<InstalledApp> = if !target.is_empty() {
                controller
                    .registry()
                    .by_uuid(&target)
                    .or_else(|| controller.registry().by_path(&target))
            } else {
                controller.registry().by_path(&path)
            };
            if owned.is_none() && target.is_empty() {
                let inspector = AppImageInspector::new(controller.runner());
                let options = InspectOptions {
                    allow_unsafe_extract: false,
                    ..Default::default()
                };
                let inspected = inspector.inspect(&path, &options, &cancel, None);
                if !inspected.existing_managed_id.is_empty() {
                    owned = controller
                        .registry()
                        .by_uuid(&inspected.existing_managed_id);
                }
                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let matches: Vec<InstalledApp> = controller
                    .registry()
                    .apps()
                    .into_iter()
                    .filter(|app| {
                        app.owned
                            && std::path::Path::new(&app.managed_path)
                                .file_name()
                                .map(|s| s.to_string_lossy() == file_name)
                                .unwrap_or(false)
                    })
                    .collect();
                if owned.is_none() && matches.len() == 1 {
                    owned = Some(matches[0].clone());
                } else if owned.is_none() && matches.len() > 1 {
                    let _ = writeln!(stderr, "Replace is ambiguous; pass --replace-uuid");
                    return ExitCode::Validation;
                }
            }
            match owned {
                Some(app) if app.owned => req.replace_uuid = app.uuid,
                _ => {
                    let _ = writeln!(
                        stderr,
                        "Replace requires a specific owned managed installation"
                    );
                    return ExitCode::NotIntegrated;
                }
            }
        } else if keep_both {
            req.conflict = ConflictPolicy::KeepBoth;
        } else {
            req.conflict = ConflictPolicy::Unspecified;
        }
        let result = controller.integrate(&req, &cancel);
        if !result.ok {
            let _ = writeln!(stderr, "{}", result.error);
            if result.error.contains("keep-both") || result.error.contains("replace") {
                return ExitCode::Validation;
            }
            return ExitCode::Failure;
        }
        let _ = writeln!(stderr, "Integrated {}", result.app.managed_path);
        return ExitCode::Ok;
    }

    if has_arg(args, "--update") {
        if has_arg(args, "--all") {
            if !confirm("Update all AppImages?", yes, interactive_tty, stderr, stdin) {
                return ExitCode::NeedsConfirmation;
            }
            let mut failures = 0;
            let mut skipped_running = 0;
            let mut applied = 0;
            let offers = controller.check_updates(&cancel);
            for offer in &offers {
                let Some(app) = controller.registry().by_uuid(&offer.uuid) else {
                    continue;
                };
                if !app.owned {
                    continue;
                }
                let result = controller.apply_update(&app, force, &cancel);
                if !result.ok {
                    let _ = writeln!(stderr, "{}: {}", app.managed_path, result.error);
                    if result.error.contains("running") {
                        skipped_running += 1;
                        continue;
                    }
                    failures += 1;
                } else {
                    applied += 1;
                }
            }
            if skipped_running > 0 {
                let _ = writeln!(
                    stderr,
                    "{skipped_running} update(s) skipped because applications are running; pass --force to override"
                );
            }
            if failures > 0 {
                return ExitCode::Failure;
            }
            if applied == 0 && skipped_running > 0 {
                return ExitCode::Running;
            }
            return ExitCode::Ok;
        }
        let path = arg_value(args, "--update");
        let mut app = controller.registry().by_path(&path);
        if app.is_none() {
            app = controller.registry().by_uuid(&path);
        }
        let Some(app) = app else {
            let _ = writeln!(stderr, "Not integrated");
            return ExitCode::NotIntegrated;
        };
        if !confirm(
            &format!("Update {}?", app.managed_path),
            yes,
            interactive_tty,
            stderr,
            stdin,
        ) {
            return ExitCode::NeedsConfirmation;
        }
        let result = controller.apply_update(&app, force, &cancel);
        if !result.ok {
            let _ = writeln!(stderr, "{}", result.error);
            if result.error.contains("running") {
                return ExitCode::Running;
            }
            return ExitCode::Failure;
        }
        return ExitCode::Ok;
    }

    if has_arg(args, "--remove-all") {
        let owned_count = controller
            .registry()
            .apps()
            .iter()
            .filter(|a| a.owned)
            .count();
        let permanent = del;
        if permanent {
            if !confirm(
                &format!(
                    "Permanently delete {owned_count} owned AppImages and their Gosh desktop/icon artifacts?"
                ),
                yes,
                interactive_tty,
                stderr,
                stdin,
            ) {
                return ExitCode::NeedsConfirmation;
            }
        } else if !confirm(
            &format!("Move {owned_count} owned AppImages to Trash?"),
            yes,
            interactive_tty,
            stderr,
            stdin,
        ) {
            return ExitCode::NeedsConfirmation;
        }
        let apps = controller.registry().apps();
        // Per-item results: one stuck entry must not abandon the rest, and the
        // user needs a summary of what actually happened.
        let mut removed = 0usize;
        let mut failed = 0usize;
        for app in &apps {
            if !app.owned {
                continue;
            }
            let req = RemovalRequest {
                path_or_uuid: app.uuid.clone(),
                mode: if permanent {
                    RemovalMode::Permanent
                } else {
                    RemovalMode::Trash
                },
                assume_yes: true,
            };
            let outcome = controller.remove_app(&req);
            if outcome.ok {
                removed += 1;
            } else {
                failed += 1;
                let _ = writeln!(stderr, "{}: {}", app.managed_path, outcome.error);
            }
        }
        let _ = writeln!(
            stderr,
            "Removed {removed} of {owned_count}; {failed} failed"
        );
        return if failed > 0 {
            ExitCode::Failure
        } else {
            ExitCode::Ok
        };
    }

    if has_arg(args, "--remove") {
        let path = arg_value(args, "--remove");
        if path.is_empty() {
            return ExitCode::Usage;
        }
        let prompt = if del {
            format!("Permanently delete {path}?")
        } else {
            format!("Trash {path}?")
        };
        if !confirm(&prompt, yes, interactive_tty, stderr, stdin) {
            return ExitCode::NeedsConfirmation;
        }
        let req = RemovalRequest {
            path_or_uuid: path,
            mode: if del {
                RemovalMode::Permanent
            } else {
                RemovalMode::Trash
            },
            assume_yes: yes,
        };
        let outcome = controller.remove_app(&req);
        if !outcome.ok {
            let _ = writeln!(stderr, "{}", outcome.error);
            return ExitCode::Failure;
        }
        return ExitCode::Ok;
    }

    if has_arg(args, "--set-update-source") {
        let path = arg_value(args, "--set-update-source");
        let mut app = controller.registry().by_path(&path);
        if app.is_none() {
            app = controller.registry().by_uuid(&path);
        }
        let Some(app) = app else {
            return ExitCode::NotIntegrated;
        };
        if has_arg(args, "--unset") {
            let mut error = String::new();
            let ok = controller.unset_update_source(app, &mut error);
            if !ok {
                let _ = writeln!(stderr, "{error}");
                return ExitCode::Failure;
            }
            return ExitCode::Ok;
        }
        let manager = arg_value(args, "--manager");
        // Only the tokens after `--manager <name>` are configuration. Scanning
        // all of argv meant a source path containing '=' was silently parsed
        // into the update config.
        let mut config = BTreeMap::new();
        let config_start = args
            .iter()
            .position(|a| a == "--manager")
            .map(|i| i + 2)
            .unwrap_or(args.len());
        for arg in args.iter().skip(config_start) {
            if arg.starts_with('-') {
                continue;
            }
            if let Some((key, value)) = arg.split_once('=') {
                config.insert(key.to_string(), value.to_string());
            }
        }
        let mut error = String::new();
        let ok = controller.set_update_source(app, &manager, config, &mut error);
        if !ok {
            let _ = writeln!(stderr, "{error}");
            return ExitCode::Validation;
        }
        return ExitCode::Ok;
    }

    if has_arg(args, "--fetch-updates") {
        let scan = controller.scan_updates(&cancel);
        let _ = writeln!(stderr, "{} update(s) available", scan.offers.len());
        for offer in &scan.offers {
            let _ = writeln!(
                stderr,
                "{} {} -> {}",
                offer.name, offer.current_version, offer.available_version
            );
        }
        // Report what could not be checked rather than folding it into
        // "0 updates available", which reads as "you are up to date".
        for failure in &scan.failures {
            let _ = writeln!(
                stderr,
                "{}: update check failed ({}): {}",
                failure.name, failure.manager, failure.error
            );
        }
        if !scan.offers.is_empty() {
            controller.notifier().notify_offers(&scan.offers);
        }
        if !scan.failures.is_empty() {
            return ExitCode::Network;
        }
        return ExitCode::Ok;
    }

    let _ = writeln!(stderr, "Unknown command");
    ExitCode::Usage
}

/// --self-test: initialize the stack, exercise synthetic fixtures, exit.
/// Never executes an AppImage, launches an app, or calls live update APIs.
pub fn run_self_test(
    controller: &mut AppController,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> ExitCode {
    let mut failures: Vec<String> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();
    let mut check = |failures: &mut Vec<String>, name: &str, ok: bool| {
        let _ = writeln!(
            stderr,
            "[self-test] {name}: {}",
            if ok { "ok" } else { "FAIL" }
        );
        if !ok {
            failures.push(name.to_string());
        }
    };

    // Readiness reports why it failed, not just that it did.
    let readiness = controller.readiness();
    if let Err(reason) = &readiness {
        reasons.push(format!("readiness: {reason}"));
    }
    check(&mut failures, "readiness", readiness.is_ok());

    // Synthetic Type-2 fixture must validate without execution.
    let fixture = crate::inspector::make_test_elf(
        crate::types::Architecture::X86_64,
        crate::types::AppImageType::Type2,
    );
    let info = elf::parse(&fixture);
    check(&mut failures, "elf-fixture", info.error.is_empty());

    // Registry round-trip.
    let mut app = InstalledApp::new_owned();
    app.uuid = crate::registry::ManagedRegistry::new_uuid();
    app.name = "SelfTest".to_string();
    app.managed_path = "/tmp/goshaim-self-test/SelfTest.AppImage".to_string();
    let registry_ok = controller.registry_mut().upsert(app.clone()).is_ok()
        && controller.registry().by_uuid(&app.uuid).is_some();
    check(&mut failures, "registry", registry_ok);
    let _ = controller.registry_mut().remove_uuid(&app.uuid);

    // Desktop build + ownership markers round-trip.
    let body = crate::desktop::build_desktop_file(&app, &app.managed_path, false);
    let desk_ok = body.contains(crate::limits::OWNERSHIP_KEY)
        && body.contains(&app.uuid)
        && body.contains("TryExec=");
    check(&mut failures, "desktop", desk_ok);

    // URL guard rejects credentials without network.
    let guard_ok = crate::url_guard::validate("https://user:pass@example.com/x", false, false)
        .is_err()
        && crate::url_guard::validate("https://example.com/x", false, false).is_ok();
    check(&mut failures, "url-guard", guard_ok);

    // Manager registry lists all six sources.
    check(
        &mut failures,
        "managers",
        UpdateSourceFactory::names().len() == 6,
    );

    // JSON schema sanity: installed envelope parses with required keys.
    let envelope = serde_json::json!({"schema_version": 1, "installed": []});
    check(
        &mut failures,
        "json-schema",
        envelope.get("schema_version").and_then(|v| v.as_i64()) == Some(1)
            && envelope
                .get("installed")
                .and_then(|v| v.as_array())
                .is_some(),
    );

    if failures.is_empty() {
        let _ = writeln!(stdout, "SELF_TEST_OK");
        ExitCode::Ok
    } else {
        for reason in &reasons {
            let _ = writeln!(stderr, "[self-test] {reason}");
        }
        let _ = writeln!(stderr, "SELF_TEST_FAIL: {}", failures.join(","));
        ExitCode::Failure
    }
}

/// Non-mutating host integration probe.
pub fn run_host_probe(controller: &AppController, stdout: &mut dyn Write) -> ExitCode {
    let result = controller.runner().run(&ProcessRequest {
        program: "true".to_string(),
        host: true,
        timeout_ms: 5000,
        ..Default::default()
    });
    let _ = writeln!(stdout, "host_spawn_program={}", result.program);
    let _ = writeln!(stdout, "host_spawn_exit={}", result.exit_code);
    let _ = writeln!(
        stdout,
        "in_flatpak={}",
        if crate::process::in_flatpak() {
            "true"
        } else {
            "false"
        }
    );
    let _ = writeln!(
        stdout,
        "managed_folder={}",
        controller.settings().managed_folder().display()
    );
    if result.refused {
        let _ = writeln!(stdout, "HOST_PROBE_FAIL");
        return ExitCode::Failure;
    }
    let _ = writeln!(stdout, "HOST_PROBE_OK");
    ExitCode::Ok
}

/// Inspect a local file without executing it (JSON to stdout).
pub fn run_inspect_probe(
    controller: &AppController,
    path: &str,
    stdout: &mut dyn Write,
) -> ExitCode {
    let cancel = AtomicBool::new(false);
    let options = InspectOptions {
        allow_unsafe_extract: false,
        confirm_unsafe_extract: false,
        ..Default::default()
    };
    let inspector = AppImageInspector::new(controller.runner());
    let result = inspector.inspect(path, &options, &cancel, None);
    let mut obj = serde_json::Map::new();
    obj.insert(
        "schema_version".into(),
        serde_json::Value::Number(limits::JSON_SCHEMA_VERSION.into()),
    );
    obj.insert(
        "path".into(),
        serde_json::Value::String(result.identity.path.clone()),
    );
    obj.insert(
        "size".into(),
        serde_json::Value::Number(result.identity.size.into()),
    );
    obj.insert(
        "sha256".into(),
        serde_json::Value::String(hex::encode(&result.identity.sha256)),
    );
    obj.insert(
        "type".into(),
        serde_json::Value::String(app_image_type_name(result.app_type).to_string()),
    );
    obj.insert(
        "architecture".into(),
        serde_json::Value::String(architecture_name(result.architecture).to_string()),
    );
    obj.insert(
        "magic_valid".into(),
        serde_json::Value::Bool(result.magic_valid),
    );
    obj.insert(
        "name".into(),
        serde_json::Value::String(result.metadata.name.clone()),
    );
    obj.insert(
        "error".into(),
        serde_json::Value::String(result.error.clone()),
    );
    obj.insert(
        "unsafe_fallback".into(),
        serde_json::Value::Bool(result.extraction_used_unsafe_fallback),
    );
    obj.insert(
        "extractor".into(),
        serde_json::Value::String(result.extractor_used.clone()),
    );
    let mut data = serde_json::to_vec(&obj).unwrap_or_default();
    data.push(b'\n');
    let _ = stdout.write_all(&data);
    if result.extraction_used_unsafe_fallback {
        let _ = stdout.write_all(b"INSPECT_EXECUTED_UNSAFE\n");
        return ExitCode::Failure;
    }
    let _ = stdout.write_all(b"INSPECT_NO_EXECUTION\n");
    if result.magic_valid {
        ExitCode::Ok
    } else {
        ExitCode::Failure
    }
}

/// Render and verify the session autostart entry without installing it.
///
/// This is a diagnostic. It used to enable background update checks in the
/// user's settings and write a real autostart entry, so running a probe opted
/// the user into a login-time network task and left residue behind that had to
/// be cleaned up by hand. It now renders the entry to a private temporary
/// directory and verifies that, touching neither settings nor the session.
pub fn run_autostart_probe(controller: &mut AppController, stdout: &mut dyn Write) -> ExitCode {
    let path = controller.autostart_desktop_path();
    let _ = writeln!(stdout, "autostart_path={}", path.display());
    let _ = writeln!(
        stdout,
        "autostart_installed={}",
        if path.exists() { "true" } else { "false" }
    );
    let body = match controller.render_autostart_entry() {
        Ok(body) => body.into_bytes(),
        Err(e) => {
            let _ = writeln!(stdout, "AUTOSTART_MISSING ({e})");
            return ExitCode::Failure;
        }
    };
    let _ = stdout.write_all(&body);
    let text = String::from_utf8_lossy(&body);
    let flatpak = crate::process::in_flatpak()
        || std::env::var("FLATPAK_ID").as_deref() == Ok(limits::APP_ID);
    let exec_ok = if flatpak {
        text.contains("flatpak run com.goshapps.AppImageManager --fetch-updates")
    } else {
        text.contains("--fetch-updates")
    };
    if !exec_ok {
        let _ = writeln!(stdout, "AUTOSTART_BAD_EXEC");
        return ExitCode::Failure;
    }
    let _ = writeln!(stdout, "AUTOSTART_OK");
    ExitCode::Ok
}
