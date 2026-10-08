// Gosh AppImage Manager 3.0.0 — GUI launcher. Made by Gosh.
// GPL-3.0-or-later.
//
// The Rust binary is the one user-facing entry point. A run with no CLI
// command hands the window to the Flutter GUI by replacing this process, so
// the GUI owns signals and the exit status from then on. Nothing here reads
// stdin, and nothing goes through a shell: the program path and its arguments
// are passed as an argument array.

use std::ffi::OsStr;
use std::fmt;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::types::ExitCode;

/// Development override: an absolute path to a GUI executable. Unset or empty
/// means "use the installed GUI".
pub const GUI_OVERRIDE_ENV: &str = "GOSH_APPIMAGE_GUI";

/// Directory holding the Flutter bundle, relative to the binary's install root
/// (the parent of the directory that contains the binary).
pub const GUI_LIBEXEC_DIR: &str = "libexec/gosh-appimage-manager";

/// Name of the Flutter executable inside [`GUI_LIBEXEC_DIR`].
pub const GUI_BINARY_NAME: &str = "gosh-appimage-manager-gui";

/// Why the GUI could not be started. Each variant renders as one line, and
/// [`LaunchError::hint`] is the second line the user sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// The override is set but is not an absolute path.
    OverrideNotAbsolute(PathBuf),
    /// No regular file at the resolved path, or it cannot be read.
    Missing(PathBuf),
    /// A file is present but has no execute permission bit.
    NotExecutable(PathBuf),
    /// Execute bit set, but the file is neither an ELF image nor a `#!` script.
    /// Handing it to exec would make libc fall back to running it with
    /// `/bin/sh`, which this launcher never does.
    NotAProgram(PathBuf),
    /// The location of this binary is unknown, so the bundle cannot be found.
    NoExecutablePath,
}

impl LaunchError {
    /// The one-line hint printed under the error.
    pub fn hint(&self) -> &'static str {
        match self {
            LaunchError::OverrideNotAbsolute(_) => {
                "Hint: pass a full path in GOSH_APPIMAGE_GUI, or unset it to use the installed GUI."
            }
            LaunchError::Missing(_) | LaunchError::NotExecutable(_) => {
                "Hint: install the GUI bundle next to this binary, or set GOSH_APPIMAGE_GUI to an absolute path. CLI commands work without it; see --help."
            }
            LaunchError::NotAProgram(_) => {
                "Hint: point GOSH_APPIMAGE_GUI at a built GUI executable, or reinstall the GUI bundle."
            }
            LaunchError::NoExecutablePath => {
                "Hint: set GOSH_APPIMAGE_GUI to the absolute path of the GUI executable."
            }
        }
    }
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::OverrideNotAbsolute(value) => write!(
                f,
                "Cannot start the GUI: {GUI_OVERRIDE_ENV} must be an absolute path, got {}.",
                value.display()
            ),
            LaunchError::Missing(path) => {
                write!(
                    f,
                    "Cannot start the GUI: no executable at {}.",
                    path.display()
                )
            }
            LaunchError::NotExecutable(path) => {
                write!(
                    f,
                    "Cannot start the GUI: {} is not executable.",
                    path.display()
                )
            }
            LaunchError::NotAProgram(path) => write!(
                f,
                "Cannot start the GUI: {} is neither an ELF program nor a #! script.",
                path.display()
            ),
            LaunchError::NoExecutablePath => {
                write!(
                    f,
                    "Cannot start the GUI: the location of this binary is unknown."
                )
            }
        }
    }
}

/// Choose the GUI executable.
///
/// The environment value and the binary's own path are passed in rather than
/// read here, so the rules are testable against a scratch directory. Order:
/// a non-empty `override_value` (which must be absolute and must not fall back
/// silently when it is wrong), otherwise `<install root>/libexec/...` where the
/// install root is the parent of the directory containing `current_exe`.
pub fn resolve_gui(
    override_value: Option<&OsStr>,
    current_exe: Option<&Path>,
) -> Result<PathBuf, LaunchError> {
    if let Some(value) = override_value.filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return Err(LaunchError::OverrideNotAbsolute(path));
        }
        return check_executable(path);
    }
    let exe = current_exe.ok_or(LaunchError::NoExecutablePath)?;
    let install_root = exe
        .parent()
        .and_then(Path::parent)
        .ok_or(LaunchError::NoExecutablePath)?;
    check_executable(install_root.join(GUI_LIBEXEC_DIR).join(GUI_BINARY_NAME))
}

/// Accept `path` only if it is a regular file (following symlinks), has an
/// execute bit set, and starts with an ELF header or a `#!` line. This is a
/// usability check, not a security boundary: the kernel still decides whether
/// the file can run.
fn check_executable(path: PathBuf) -> Result<PathBuf, LaunchError> {
    let Ok(metadata) = std::fs::metadata(&path) else {
        return Err(LaunchError::Missing(path));
    };
    if !metadata.is_file() {
        return Err(LaunchError::Missing(path));
    }
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(LaunchError::NotExecutable(path));
    }
    match is_program_image(&path) {
        Ok(true) => Ok(path),
        Ok(false) => Err(LaunchError::NotAProgram(path)),
        Err(_) => Err(LaunchError::Missing(path)),
    }
}

/// True when the first bytes are an ELF magic or a `#!` line, the two formats
/// `execve` runs directly. Anything else would trigger the `execvp` fallback.
fn is_program_image(path: &Path) -> std::io::Result<bool> {
    let mut head = Vec::with_capacity(4);
    std::fs::File::open(path)?.take(4).read_to_end(&mut head)?;
    Ok(head.starts_with(b"\x7fELF") || head.starts_with(b"#!"))
}

/// Start the GUI in place of this process.
///
/// On success this does not return. It returns only when the GUI could not be
/// started; the reason has then been written to `stderr` and the returned code
/// is what the binary should exit with.
pub fn launch_gui(args: &[String], stderr: &mut dyn Write) -> ExitCode {
    let override_value = std::env::var_os(GUI_OVERRIDE_ENV);
    let current_exe = std::env::current_exe().ok();
    match resolve_gui(override_value.as_deref(), current_exe.as_deref()) {
        Ok(program) => exec_gui(&program, args, stderr),
        Err(error) => {
            report(stderr, &error.to_string(), error.hint());
            ExitCode::Failure
        }
    }
}

/// Replace the process with `program`. Returns only when the exec failed.
fn exec_gui(program: &Path, args: &[String], stderr: &mut dyn Write) -> ExitCode {
    let error = gui_command(program, args).exec();
    report(
        stderr,
        &format!("Cannot start the GUI at {}: {error}.", program.display()),
        "Hint: the file must be a runnable GUI build for this machine's architecture.",
    );
    ExitCode::Failure
}

/// The command that replaces this process. The program path and each argument
/// stay separate entries; nothing is split or interpreted by a shell.
fn gui_command(program: &Path, args: &[String]) -> Command {
    let mut command = Command::new(program);
    command.args(args);
    command
}

fn report(stderr: &mut dyn Write, line: &str, hint: &str) {
    // Best effort: a closed stderr must not turn into a panic.
    let _ = writeln!(stderr, "{line}\n{hint}");
    let _ = stderr.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;

    /// Create `path` (and its parents) with `content` and the given mode.
    fn write_with(path: &Path, mode: u32, content: &[u8]) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create dirs");
        fs::write(path, content).expect("write file");
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod");
    }

    /// Create `path` as a stand-in GUI. It starts with an ELF magic so it passes
    /// the program check. The tests locate it and never run it.
    fn write_file(path: &Path, mode: u32) {
        write_with(
            path,
            mode,
            b"\x7fELF stand-in, never executed by the tests\n",
        );
    }

    /// `<root>/bin/gosh-appimage-manager`: the binary's own path. The file
    /// itself is never needed; only its location matters.
    fn binary_in(root: &Path) -> PathBuf {
        root.join("bin").join("gosh-appimage-manager")
    }

    fn installed_gui(root: &Path) -> PathBuf {
        root.join("libexec")
            .join("gosh-appimage-manager")
            .join("gosh-appimage-manager-gui")
    }

    #[test]
    fn installed_gui_is_found_next_to_the_binary() {
        let root = tempfile::tempdir().expect("tempdir");
        let gui = installed_gui(root.path());
        write_file(&gui, 0o755);

        let resolved = resolve_gui(None, Some(&binary_in(root.path()))).expect("resolves");
        assert_eq!(resolved, gui);
    }

    #[test]
    fn override_wins_over_the_installed_gui() {
        let root = tempfile::tempdir().expect("tempdir");
        write_file(&installed_gui(root.path()), 0o755);
        let dev = root.path().join("dev-build").join("gui");
        write_file(&dev, 0o755);

        let resolved =
            resolve_gui(Some(dev.as_os_str()), Some(&binary_in(root.path()))).expect("resolves");
        assert_eq!(resolved, dev);
    }

    #[test]
    fn empty_override_counts_as_unset() {
        let root = tempfile::tempdir().expect("tempdir");
        let gui = installed_gui(root.path());
        write_file(&gui, 0o755);

        let empty = OsString::new();
        let resolved =
            resolve_gui(Some(empty.as_os_str()), Some(&binary_in(root.path()))).expect("resolves");
        assert_eq!(resolved, gui);
    }

    #[test]
    fn relative_override_is_rejected_instead_of_falling_back() {
        let root = tempfile::tempdir().expect("tempdir");
        write_file(&installed_gui(root.path()), 0o755);
        let relative = OsString::from("build/gui");

        let err = resolve_gui(Some(relative.as_os_str()), Some(&binary_in(root.path())))
            .expect_err("relative override must fail");
        assert_eq!(
            err,
            LaunchError::OverrideNotAbsolute(PathBuf::from("build/gui"))
        );
    }

    #[test]
    fn missing_override_does_not_fall_back_to_the_installed_gui() {
        let root = tempfile::tempdir().expect("tempdir");
        write_file(&installed_gui(root.path()), 0o755);
        let absent = root.path().join("nowhere").join("gui");

        let err = resolve_gui(Some(absent.as_os_str()), Some(&binary_in(root.path())))
            .expect_err("missing override must fail");
        assert_eq!(err, LaunchError::Missing(absent));
    }

    #[test]
    fn override_without_execute_bit_is_refused() {
        let root = tempfile::tempdir().expect("tempdir");
        let dev = root.path().join("gui");
        write_file(&dev, 0o644);

        let err = resolve_gui(Some(dev.as_os_str()), Some(&binary_in(root.path())))
            .expect_err("non-executable override must fail");
        assert_eq!(err, LaunchError::NotExecutable(dev));
    }

    #[test]
    fn missing_installed_gui_reports_the_expected_path() {
        let root = tempfile::tempdir().expect("tempdir");

        let err = resolve_gui(None, Some(&binary_in(root.path()))).expect_err("nothing installed");
        assert_eq!(err, LaunchError::Missing(installed_gui(root.path())));
    }

    #[test]
    fn installed_gui_without_execute_bit_is_refused() {
        let root = tempfile::tempdir().expect("tempdir");
        let gui = installed_gui(root.path());
        write_file(&gui, 0o600);

        let err = resolve_gui(None, Some(&binary_in(root.path()))).expect_err("not executable");
        assert_eq!(err, LaunchError::NotExecutable(gui));
    }

    #[test]
    fn a_directory_in_place_of_the_gui_is_missing() {
        let root = tempfile::tempdir().expect("tempdir");
        let gui = installed_gui(root.path());
        fs::create_dir_all(&gui).expect("create dir");

        let err = resolve_gui(None, Some(&binary_in(root.path()))).expect_err("a directory");
        assert_eq!(err, LaunchError::Missing(gui));
    }

    #[test]
    fn unknown_binary_location_is_reported() {
        let err = resolve_gui(None, None).expect_err("no current_exe");
        assert_eq!(err, LaunchError::NoExecutablePath);
    }

    #[test]
    fn binary_at_filesystem_root_has_no_install_root() {
        // No filesystem writes: "/bin" has no parent to hold a libexec dir.
        let err = resolve_gui(None, Some(Path::new("/gosh-appimage-manager")))
            .expect_err("root has no install root");
        assert_eq!(err, LaunchError::NoExecutablePath);
    }

    #[test]
    fn every_error_is_one_line_with_a_hint() {
        let errors = [
            LaunchError::OverrideNotAbsolute(PathBuf::from("rel")),
            LaunchError::Missing(PathBuf::from("/x/gui")),
            LaunchError::NotExecutable(PathBuf::from("/x/gui")),
            LaunchError::NotAProgram(PathBuf::from("/x/gui")),
            LaunchError::NoExecutablePath,
        ];
        for error in errors {
            let line = error.to_string();
            assert!(!line.contains('\n'), "multi-line error: {line}");
            assert!(line.starts_with("Cannot start the GUI"), "got: {line}");
            let hint = error.hint();
            assert!(hint.starts_with("Hint:"), "got: {hint}");
            assert!(!hint.contains('\n'), "multi-line hint: {hint}");
        }
    }

    #[test]
    fn arguments_stay_an_argument_array() {
        let program = Path::new("/app/libexec/gosh-appimage-manager/gosh-appimage-manager-gui");
        let args = vec![
            "/home/me/My Apps/tool.AppImage".to_string(),
            "--flag; rm -rf ~".to_string(),
            "$(id)".to_string(),
        ];

        let command = gui_command(program, &args);
        assert_eq!(command.get_program(), program.as_os_str());
        let forwarded: Vec<&OsStr> = command.get_args().collect();
        let expected: Vec<&OsStr> = args.iter().map(|a| OsStr::new(a.as_str())).collect();
        assert_eq!(forwarded, expected);
    }

    #[test]
    fn text_with_execute_bit_is_not_a_program() {
        let root = tempfile::tempdir().expect("tempdir");
        let dev = root.path().join("gui");
        write_with(&dev, 0o755, b"not a program\n");

        let err = resolve_gui(Some(dev.as_os_str()), Some(&binary_in(root.path())))
            .expect_err("text is not a program");
        assert_eq!(err, LaunchError::NotAProgram(dev));
    }

    #[test]
    fn shebang_scripts_are_accepted() {
        let root = tempfile::tempdir().expect("tempdir");
        let dev = root.path().join("gui-wrapper");
        write_with(&dev, 0o755, b"#!/bin/sh\nexit 0\n");

        let resolved =
            resolve_gui(Some(dev.as_os_str()), Some(&binary_in(root.path()))).expect("resolves");
        assert_eq!(resolved, dev);
    }

    #[test]
    fn an_exec_failure_is_reported_and_returned() {
        // The interpreter does not exist, so exec fails with ENOENT and returns
        // here instead of replacing the test process. (A file with no valid
        // format would instead hit the execvp fallback to /bin/sh.)
        let root = tempfile::tempdir().expect("tempdir");
        let gui = root.path().join("gui");
        write_with(
            &gui,
            0o755,
            b"#!/nonexistent/gosh-appimage-manager-test-interpreter\n",
        );

        let mut stderr: Vec<u8> = Vec::new();
        let code = exec_gui(&gui, &["file.AppImage".to_string()], &mut stderr);
        assert_eq!(code, ExitCode::Failure);
        let text = String::from_utf8(stderr).expect("utf8");
        assert!(text.starts_with("Cannot start the GUI at "), "got: {text}");
        assert_eq!(text.lines().count(), 2, "got: {text}");
    }
}
