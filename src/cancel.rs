// Gosh AppImage Manager — calls that a cancel can stop.
//
// A blocked socket read or a child process that is still running cannot look at
// a flag. So the blocking call runs on a helper thread, and the caller stops
// waiting for it as soon as the operation is cancelled. The helper is left to
// finish on its own: its requests are bounded by their timeouts, and its result
// is dropped. Nothing it does after the cancel is reported to the caller.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use crate::network::{FetchResult, Local, NetworkClient};
use crate::proctable::ProcessTable;

/// How often a caller that is waiting on a helper checks its cancel flag.
const CANCEL_POLL: Duration = Duration::from_millis(50);

/// The message a cancelled network call reports. The download path uses the same text.
pub const CANCELLED: &str = "Cancelled";

/// The message for a helper that could not start, or that ended without a result.
const STOPPED: &str = "The network request stopped unexpectedly";

/// Run `work` on a helper thread and return its result. If `cancel` is set
/// first, return `cancelled` at once and stop waiting. `stopped` is returned when
/// the helper cannot start or ends without a result.
pub fn run_unless_cancelled<T: Send + 'static>(
    cancel: &AtomicBool,
    work: impl FnOnce() -> T + Send + 'static,
    cancelled: T,
    stopped: T,
) -> T {
    if cancel.load(Ordering::Relaxed) {
        return cancelled;
    }
    let (sender, receiver) = mpsc::channel();
    let started = std::thread::Builder::new()
        .name("goshaim-cancellable".to_string())
        .spawn(move || {
            let _ = sender.send(work());
        });
    if started.is_err() {
        return stopped;
    }
    loop {
        match receiver.recv_timeout(CANCEL_POLL) {
            Ok(value) => return value,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if cancel.load(Ordering::Relaxed) {
                    return cancelled;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return stopped,
        }
    }
}

fn cancelled_error<T>() -> Result<T, String> {
    Err(CANCELLED.to_string())
}

fn stopped_error<T>() -> Result<T, String> {
    Err(STOPPED.to_string())
}

/// A network client whose calls return as soon as `cancel` is set.
///
/// The requests themselves are unchanged. Each one runs on a helper thread, and
/// the caller returns `Err("Cancelled")` as soon as the flag is set. A download
/// also sets the helper's own stop flag, so the helper removes what it wrote
/// once its blocked read returns.
pub struct CancellableNetwork<'a> {
    inner: Arc<dyn NetworkClient>,
    cancel: &'a AtomicBool,
}

impl<'a> CancellableNetwork<'a> {
    pub fn new(inner: Arc<dyn NetworkClient>, cancel: &'a AtomicBool) -> Self {
        Self { inner, cancel }
    }
}

impl NetworkClient for CancellableNetwork<'_> {
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        local: Local,
    ) -> Result<FetchResult, String> {
        let inner = Arc::clone(&self.inner);
        let (url, headers) = (url.to_string(), headers.to_vec());
        run_unless_cancelled(
            self.cancel,
            move || inner.get(&url, &headers, local),
            cancelled_error(),
            stopped_error(),
        )
    }

    fn head_len(&self, url: &str, local: Local) -> Result<Option<u64>, String> {
        let inner = Arc::clone(&self.inner);
        let url = url.to_string();
        run_unless_cancelled(
            self.cancel,
            move || inner.head_len(&url, local),
            cancelled_error(),
            stopped_error(),
        )
    }

    fn download_bounded(&self, url: &str, max_bytes: u64, local: Local) -> Result<Vec<u8>, String> {
        let inner = Arc::clone(&self.inner);
        let url = url.to_string();
        run_unless_cancelled(
            self.cancel,
            move || inner.download_bounded(&url, max_bytes, local),
            cancelled_error(),
            stopped_error(),
        )
    }

    fn download_to_file(
        &self,
        url: &str,
        dest: &Path,
        max_bytes: u64,
        cancel: &AtomicBool,
        local: Local,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<u64, String> {
        if cancel.load(Ordering::Relaxed) {
            return cancelled_error();
        }
        // The helper sees this flag at its next read, once its blocked read
        // returns, and then removes what it wrote.
        let helper_stop = Arc::new(AtomicBool::new(false));
        let helper_flag = Arc::clone(&helper_stop);
        // Progress is reported from this thread, so the helper sends it over a channel.
        let (progress_tx, progress_rx) = mpsc::channel::<(u64, u64)>();
        let (result_tx, result_rx) = mpsc::channel::<Result<u64, String>>();
        let inner = Arc::clone(&self.inner);
        let (url, dest) = (url.to_string(), dest.to_path_buf());
        let started = std::thread::Builder::new()
            .name("goshaim-download".to_string())
            .spawn(move || {
                let mut report = |done: u64, total: u64| {
                    let _ = progress_tx.send((done, total));
                };
                let result = inner.download_to_file(
                    &url,
                    &dest,
                    max_bytes,
                    &helper_flag,
                    local,
                    &mut report,
                );
                let _ = result_tx.send(result);
            });
        if started.is_err() {
            return stopped_error();
        }
        loop {
            while let Ok((done, total)) = progress_rx.try_recv() {
                progress(done, total);
            }
            match result_rx.recv_timeout(CANCEL_POLL) {
                Ok(result) => {
                    // Progress sent before the result is still in the channel.
                    while let Ok((done, total)) = progress_rx.try_recv() {
                        progress(done, total);
                    }
                    return result;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if cancel.load(Ordering::Relaxed) {
                        helper_stop.store(true, Ordering::Relaxed);
                        return cancelled_error();
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return stopped_error(),
            }
        }
    }
}

/// A process table whose probes return as soon as `cancel` is set.
///
/// The host probe can wait on a helper process for seconds. Once the operation
/// is cancelled the caller stops waiting for it. A cancelled probe reports that
/// the executable is running, so a caller that does not check the flag keeps the
/// safe answer.
pub struct CancellableProcesses<'a> {
    inner: Arc<dyn ProcessTable>,
    cancel: &'a AtomicBool,
}

impl<'a> CancellableProcesses<'a> {
    pub fn new(inner: Arc<dyn ProcessTable>, cancel: &'a AtomicBool) -> Self {
        Self { inner, cancel }
    }
}

impl ProcessTable for CancellableProcesses<'_> {
    fn pids_for_executable(&self, executable: &str) -> Vec<u32> {
        let inner = Arc::clone(&self.inner);
        let executable = executable.to_string();
        run_unless_cancelled(
            self.cancel,
            move || inner.pids_for_executable(&executable),
            Vec::new(),
            Vec::new(),
        )
    }

    fn is_running(&self, executable: &str) -> bool {
        let inner = Arc::clone(&self.inner);
        let executable = executable.to_string();
        // A probe that cannot answer must not read as "not running".
        run_unless_cancelled(
            self.cancel,
            move || inner.is_running(&executable),
            true,
            true,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::network::FakeNetwork;

    /// A network whose `get` blocks until the test releases it, as a socket
    /// that has gone quiet does. It does not look at any flag.
    struct Blocked {
        release: Mutex<mpsc::Receiver<()>>,
    }

    impl NetworkClient for Blocked {
        fn get(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _local: Local,
        ) -> Result<FetchResult, String> {
            let _ = self.release.lock().unwrap().recv();
            Err("released".to_string())
        }
        fn head_len(&self, _url: &str, _local: Local) -> Result<Option<u64>, String> {
            Ok(None)
        }
        fn download_bounded(
            &self,
            _url: &str,
            _max: u64,
            _local: Local,
        ) -> Result<Vec<u8>, String> {
            Ok(Vec::new())
        }
        fn download_to_file(
            &self,
            _url: &str,
            _dest: &Path,
            _max: u64,
            _cancel: &AtomicBool,
            _local: Local,
            _progress: &mut dyn FnMut(u64, u64),
        ) -> Result<u64, String> {
            Ok(0)
        }
    }

    #[test]
    fn a_get_that_never_returns_is_left_behind_once_cancelled() {
        let (release_tx, release_rx) = mpsc::channel();
        let network: Arc<dyn NetworkClient> = Arc::new(Blocked {
            release: Mutex::new(release_rx),
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let setter = Arc::clone(&cancel);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            setter.store(true, Ordering::Relaxed);
        });
        let watched = CancellableNetwork::new(network, &cancel);
        let started = Instant::now();
        let outcome = watched.get("https://example.invalid/a", &[], Local::Denied);
        let elapsed = started.elapsed();
        assert_eq!(outcome.err().as_deref(), Some(CANCELLED));
        assert!(
            elapsed < Duration::from_secs(1),
            "a cancelled get returned after {elapsed:?}"
        );
        // Release the helper so the test leaves no thread blocked.
        let _ = release_tx.send(());
    }

    #[test]
    fn a_download_that_is_cancelled_tells_its_helper_to_stop() {
        /// A download that blocks until the helper's own stop flag is set.
        struct Stuck {
            saw_stop: Arc<AtomicBool>,
        }
        impl NetworkClient for Stuck {
            fn get(
                &self,
                _: &str,
                _: &[(String, String)],
                _: Local,
            ) -> Result<FetchResult, String> {
                Err("unused".to_string())
            }
            fn head_len(&self, _: &str, _: Local) -> Result<Option<u64>, String> {
                Ok(None)
            }
            fn download_bounded(&self, _: &str, _: u64, _: Local) -> Result<Vec<u8>, String> {
                Ok(Vec::new())
            }
            fn download_to_file(
                &self,
                _url: &str,
                _dest: &Path,
                _max: u64,
                cancel: &AtomicBool,
                _local: Local,
                _progress: &mut dyn FnMut(u64, u64),
            ) -> Result<u64, String> {
                let deadline = Instant::now() + Duration::from_secs(5);
                while !cancel.load(Ordering::Relaxed) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                self.saw_stop
                    .store(cancel.load(Ordering::Relaxed), Ordering::Relaxed);
                Err("Cancelled".to_string())
            }
        }
        let saw_stop = Arc::new(AtomicBool::new(false));
        let network: Arc<dyn NetworkClient> = Arc::new(Stuck {
            saw_stop: Arc::clone(&saw_stop),
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let setter = Arc::clone(&cancel);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            setter.store(true, Ordering::Relaxed);
        });
        let watched = CancellableNetwork::new(network, &cancel);
        let mut seen = Vec::new();
        let outcome = watched.download_to_file(
            "https://example.invalid/a.AppImage",
            Path::new("/nonexistent-dir/staging"),
            1024,
            &cancel,
            Local::Denied,
            &mut |done, _| seen.push(done),
        );
        assert_eq!(outcome.err().as_deref(), Some(CANCELLED));
        let deadline = Instant::now() + Duration::from_secs(2);
        while !saw_stop.load(Ordering::Relaxed) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            saw_stop.load(Ordering::Relaxed),
            "the helper was told to stop"
        );
    }

    #[test]
    fn a_process_probe_that_never_returns_reports_running_once_cancelled() {
        /// A process table whose probe blocks until released, as a host
        /// `pgrep` that has hung does.
        struct HungProbe {
            release: Mutex<mpsc::Receiver<()>>,
        }
        impl ProcessTable for HungProbe {
            fn pids_for_executable(&self, _executable: &str) -> Vec<u32> {
                let _ = self.release.lock().unwrap().recv();
                Vec::new()
            }
        }
        let (release_tx, release_rx) = mpsc::channel();
        let table: Arc<dyn ProcessTable> = Arc::new(HungProbe {
            release: Mutex::new(release_rx),
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let setter = Arc::clone(&cancel);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            setter.store(true, Ordering::Relaxed);
        });
        let probes = CancellableProcesses::new(table, &cancel);
        let started = Instant::now();
        let running = probes.is_running("/apps/Demo.AppImage");
        let elapsed = started.elapsed();
        assert!(running, "a probe that cannot answer keeps the safe answer");
        assert!(
            elapsed < Duration::from_secs(1),
            "a cancelled probe returned after {elapsed:?}"
        );
        let _ = release_tx.send(());
    }

    #[test]
    fn a_completed_call_returns_its_result_unchanged() {
        let network: Arc<dyn NetworkClient> =
            Arc::new(FakeNetwork::new().canned_body("ok.example", b"fine"));
        let cancel = AtomicBool::new(false);
        let watched = CancellableNetwork::new(network, &cancel);
        let fetched = watched
            .get("https://ok.example/x", &[], Local::Denied)
            .expect("a canned body is returned");
        assert_eq!(fetched.body, b"fine");
    }
}
