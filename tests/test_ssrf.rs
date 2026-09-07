// Audit findings S-1 and S-2, verified against a real socket rather than a
// fake seam: url_guard can only see the literal host string, so the guard that
// matters is the one applied to the address the resolver actually returns.
//
// `probe.example.test` must resolve to 127.0.0.1 for this to be meaningful.
// When it does not (a clean CI box), the test reports that and skips rather
// than passing vacuously.

use std::io::Write;
use std::net::{TcpListener, ToSocketAddrs};

fn resolves_to_loopback(host: &str) -> bool {
    (host, 80u16)
        .to_socket_addrs()
        .map(|mut a| a.any(|s| s.ip().is_loopback()))
        .unwrap_or(false)
}

#[test]
fn hostname_resolving_to_loopback_is_refused_before_connecting() {
    const HOST: &str = "probe.example.test";
    if !resolves_to_loopback(HOST) {
        eprintln!("skipping: {HOST} does not resolve to loopback in this environment");
        return;
    }

    // A listener standing in for an internal service. If the guard works, it
    // never sees a connection.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Ok((mut sock, _)) = listener.accept() {
            let _ = tx.send(());
            let _ = sock.write_all(b"220 internal service\r\n");
        }
    });

    // The URL passes the literal-host guard: `probe.example.test` is an
    // ordinary public-looking name with a dot and no local suffix.
    let url = format!("ftp://{HOST}:{port}/file.AppImage");
    assert!(
        goshaim_core::url_guard::validate(&url, true, false).is_ok(),
        "the literal host is deliberately innocuous; that is the point"
    );

    let result = goshaim_core::network::ftp_size(&url, goshaim_core::network::Local::Denied);
    assert!(
        result.is_err(),
        "a hostname resolving to loopback must be refused, got {result:?}"
    );
    let message = result.unwrap_err();
    assert!(
        message.contains("local-network"),
        "expected a local-network rejection, got: {message}"
    );
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(300))
            .is_err(),
        "the guard must refuse before any packet reaches the internal service"
    );
}
