// Gosh AppImage Manager 3.0.1 — entry point. Made by Gosh.
// One binary: the CLI, plus the launcher that starts the Flutter GUI.

use std::io::{IsTerminal, Write};

use goshaim_core::cli;
use goshaim_core::controller::AppController;
use goshaim_core::launcher;
use goshaim_core::limits;
use goshaim_core::types::ExitCode;

const CLI_COMMANDS: &[&str] = &[
    "--integrate",
    "--update",
    "--remove",
    "--remove-all",
    "--list-installed",
    "--list-updates",
    "--list-update-managers",
    "--set-update-source",
    "--fetch-updates",
    "--list-discovered",
    "--adopt",
    "--probe-host",
    "--probe-inspect",
    "--probe-autostart",
];

fn is_cli_command(args: &[String]) -> bool {
    args.iter().any(|a| CLI_COMMANDS.contains(&a.as_str()))
}

fn print_help(stdout: &mut dyn Write) {
    let _ = writeln!(
        stdout,
        "Gosh AppImage Manager {version} — Made by Gosh\n\
         Inspect, integrate, launch, organize, update, and remove AppImages.\n\
         Opening an AppImage never integrates or executes it.\n\
         \n\
         Usage:\n  \
         gosh-appimage-manager [--integrate <path> [--keep-both|--replace] [--replace-uuid UUID|--target PATH] [--yes] [--allow-unsafe]]\n  \
         gosh-appimage-manager [--update <path>|--all [--yes] [--force]]\n  \
         gosh-appimage-manager [--remove <path> [--yes] [--delete]] [--remove-all [--yes]]\n  \
         gosh-appimage-manager [--list-installed [--json]] [--list-updates [--json]]\n  \
         gosh-appimage-manager [--list-discovered [--json]] [--adopt <path> [--yes]]\n  \
         gosh-appimage-manager [--list-update-managers] [--set-update-source <path> --manager <name> key=value... | --unset]\n  \
         gosh-appimage-manager [--fetch-updates [--background]] [--self-test]\n  \
         gosh-appimage-manager [--probe-host] [--probe-inspect <path> [--allow-unsafe]] [--probe-autostart]\n  \
         gosh-appimage-manager [files...]  (open in GUI)\n\
         \n\
         --allow-unsafe runs the unsafe extraction fallback for that one file. The Settings\n\
         switch must be on as well; without the flag a file that needs it is left pending.\n\
         JSON lists use schema_version 1 with installed/updates arrays.\n\
         Diagnostics go to stderr so stdout stays valid JSON.",
        version = limits::VERSION
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stdout = std::io::stdout();
    let mut out_lock = stdout.lock();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        let _ = writeln!(out_lock, "{}", limits::VERSION);
        std::process::exit(ExitCode::Ok as i32);
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help(&mut out_lock);
        std::process::exit(ExitCode::Ok as i32);
    }

    let self_test = args.iter().any(|a| a == "--self-test");
    let probe_host = args.iter().any(|a| a == "--probe-host");
    let probe_inspect = args.iter().any(|a| a == "--probe-inspect");
    let probe_autostart = args.iter().any(|a| a == "--probe-autostart");
    let allow_unsafe = args.iter().any(|a| a == "--allow-unsafe");
    let cli = is_cli_command(&args);

    if cli && !self_test {
        let mut controller = match AppController::new() {
            Ok(controller) => controller,
            Err(e) => {
                eprintln!("Cannot start: {e}");
                std::process::exit(ExitCode::Failure as i32);
            }
        };
        if probe_host {
            let code = cli::run_host_probe(&controller, &mut out_lock);
            std::process::exit(code as i32);
        }
        if probe_inspect {
            let idx = args.iter().position(|a| a == "--probe-inspect");
            let path = idx
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or_default();
            let code = cli::run_inspect_probe(
                &controller,
                &path,
                allow_unsafe,
                &mut out_lock,
                &mut std::io::stderr(),
            );
            std::process::exit(code as i32);
        }
        if probe_autostart {
            let code = cli::run_autostart_probe(&mut controller, &mut out_lock);
            std::process::exit(code as i32);
        }
        let tty = std::io::stdin().is_terminal();
        let stderr = std::io::stderr();
        let mut err_lock = stderr.lock();
        let stdin = std::io::stdin();
        let mut in_lock = stdin.lock();
        let code = cli::run_cli(
            &mut controller,
            &args,
            tty,
            &mut out_lock,
            &mut err_lock,
            &mut in_lock,
        );
        std::process::exit(code as i32);
    }

    if self_test {
        let mut controller = match AppController::new() {
            Ok(controller) => controller,
            Err(e) => {
                eprintln!("Cannot start: {e}");
                std::process::exit(ExitCode::Failure as i32);
            }
        };
        if probe_host {
            let code = cli::run_host_probe(&controller, &mut out_lock);
            std::process::exit(code as i32);
        }
        if probe_inspect {
            let idx = args.iter().position(|a| a == "--probe-inspect");
            let path = idx
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or_default();
            let code = cli::run_inspect_probe(
                &controller,
                &path,
                allow_unsafe,
                &mut out_lock,
                &mut std::io::stderr(),
            );
            std::process::exit(code as i32);
        }
        if probe_autostart {
            let code = cli::run_autostart_probe(&mut controller, &mut out_lock);
            std::process::exit(code as i32);
        }
        let stderr = std::io::stderr();
        let mut err_lock = stderr.lock();
        let code = cli::run_self_test(&mut controller, &mut out_lock, &mut err_lock);
        std::process::exit(code as i32);
    }

    // GUI mode: no CLI command, so the Flutter GUI gets the window and any
    // file arguments. On success this call never returns.
    let code = {
        let stderr = std::io::stderr();
        launcher::launch_gui(&args, &mut stderr.lock())
    };
    std::process::exit(code as i32);
}
