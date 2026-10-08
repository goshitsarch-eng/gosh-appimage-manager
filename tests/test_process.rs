// Audit R6-01: a child the runner is watching can be reaped by someone else.
//
// The runner used to wait with the wait-timeout crate. That crate keeps one
// process-wide SIGCHLD handler and a shared map of the children it watches.
// When a watched child was reaped elsewhere first, the handler's try_wait
// failed with ECHILD and panicked while the map was locked. The unwind poisoned
// the map, and the crate's drop of its waiter then panicked during the unwind,
// which aborted the whole process. The runner must report such a child as
// unwatchable and carry on.
//
// The scenario runs in a re-executed copy of this test binary, because the old
// behaviour was an abort that would take the test process down with it.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use goshaim_core::process::{ProcessRequest, ProcessRunner, SystemRunner};

const SCENARIO_DIR_ENV: &str = "GOSH_TEST_REAPED_CHILD_DIR";
const TEST_NAME: &str = "a_child_reaped_while_it_is_watched_does_not_abort_the_process";

extern "C" {
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
}

#[test]
fn a_child_reaped_while_it_is_watched_does_not_abort_the_process() {
    if let Some(dir) = std::env::var_os(SCENARIO_DIR_ENV) {
        run_scenario(Path::new(&dir));
        return;
    }
    let dir = tempfile::tempdir().expect("a scratch directory");
    let output = Command::new(std::env::current_exe().expect("the test binary"))
        .args(["--exact", TEST_NAME, "--test-threads=1"])
        .env(SCENARIO_DIR_ENV, dir.path())
        .output()
        .expect("the scenario process starts");
    assert!(
        output.status.success(),
        "the process ended abnormally ({}) when a watched child was reaped elsewhere:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Run the runner on a child that another thread reaps while the runner watches it.
fn run_scenario(dir: &Path) {
    let pid_file = dir.join("pid");
    let reaper = {
        let pid_file = pid_file.clone();
        std::thread::spawn(move || reap_once_started(&pid_file))
    };
    // The script is fixed. The pid file's path is passed as an argument, never
    // spliced into the script text.
    let request = ProcessRequest {
        program: "sh".to_string(),
        args: vec![
            "-c".to_string(),
            "echo $$ > \"$1\"; sleep 1".to_string(),
            "sh".to_string(),
            pid_file.to_string_lossy().into_owned(),
        ],
        timeout_ms: 30_000,
        ..Default::default()
    };
    // Whether the runner reaps the child itself or finds it already gone, it
    // must come back with a result.
    let result = SystemRunner::new().run(&request);
    reaper.join().expect("the reaper thread finishes");
    eprintln!(
        "the runner returned: exit {} refused {}",
        result.exit_code, result.refused
    );
}

/// Wait for the child to report its pid, then reap it from this thread. waitpid
/// blocks until the child exits, so the runner is already watching it by then.
fn reap_once_started(pid_file: &Path) {
    let deadline = Instant::now() + Duration::from_secs(20);
    let pid = loop {
        let parsed = fs::read_to_string(pid_file).ok().and_then(|text| {
            text.strip_suffix('\n')
                .and_then(|digits| digits.parse::<i32>().ok())
        });
        if let Some(pid) = parsed {
            break pid;
        }
        assert!(
            Instant::now() < deadline,
            "the child never reported its pid"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut status = 0;
    // SAFETY: waitpid on a child this process started, with a valid status pointer.
    unsafe {
        waitpid(pid, &mut status, 0);
    }
}
