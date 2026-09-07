// Audit finding C-3: the FTP client's replies were one behind the commands.
//
// It never consumed the server's 220 greeting, so it read the greeting as the
// answer to USER, the USER answer as the answer to PASS, and so on. Measured
// against a conformant server before the fix: a server answering `213 4096` to
// SIZE produced Ok(None), and PASV parsing was handed the TYPE reply, so
// ftp_download failed with "FTP PASV parse failed" every time. FTP could
// neither report a size nor download a file at all.
//
// These tests run a real RFC 959 server on loopback. `probe.example.test` must
// resolve to 127.0.0.1; where it does not, they report that and skip rather
// than passing vacuously.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, ToSocketAddrs};

fn loopback_name_available() -> bool {
    ("probe.example.test", 80u16)
        .to_socket_addrs()
        .map(|mut a| a.any(|s| s.ip().is_loopback()))
        .unwrap_or(false)
}

/// A conformant server: greets on connect, and answers SIZE with a real value.
/// `multiline` makes the greeting use the RFC 959 continuation form.
fn spawn_server(multiline: bool, data_port: Option<u16>) -> u16 {
    spawn_server_at(multiline, data_port, "127,0,0,1")
}

/// `pasv_host` is the dotted-quad the server advertises in its 227 reply, so a
/// test can make it name a host other than itself.
fn spawn_server_at(multiline: bool, data_port: Option<u16>, pasv_host: &str) -> u16 {
    let pasv_host = pasv_host.to_string();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let Ok((mut sock, _)) = listener.accept() else {
            return;
        };
        let greeting = if multiline {
            "220-Welcome to the probe server\r\n220-Second banner line\r\n220 Ready\r\n"
        } else {
            "220 probe ftp ready\r\n"
        };
        let _ = sock.write_all(greeting.as_bytes());
        let Ok(peek) = sock.try_clone() else { return };
        let mut reader = BufReader::new(peek);
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            let cmd = line.trim().to_string();
            let reply = if cmd.starts_with("USER") {
                "331 need password\r\n".to_string()
            } else if cmd.starts_with("PASS") {
                "230 logged in\r\n".to_string()
            } else if cmd.starts_with("TYPE") {
                "200 type set\r\n".to_string()
            } else if cmd.starts_with("SIZE") {
                "213 4096\r\n".to_string()
            } else if cmd.starts_with("PASV") {
                let p = data_port.unwrap_or(9);
                format!(
                    "227 Entering Passive Mode ({pasv_host},{},{})\r\n",
                    p / 256,
                    p % 256
                )
            } else if cmd.starts_with("RETR") {
                "150 opening\r\n".to_string()
            } else if cmd.starts_with("QUIT") {
                "221 bye\r\n".to_string()
            } else {
                "500 unknown\r\n".to_string()
            };
            let _ = sock.write_all(reply.as_bytes());
            if cmd.starts_with("QUIT") {
                break;
            }
            line.clear();
        }
    });
    port
}

#[test]
fn size_is_read_from_the_size_reply_not_the_previous_one() {
    if !loopback_name_available() {
        eprintln!("skipping: probe.example.test does not resolve to loopback here");
        return;
    }
    let port = spawn_server(false, None);
    let url = format!("ftp://probe.example.test:{port}/file.AppImage");
    let size = goshaim_core::network::ftp_size(&url, goshaim_core::network::Local::Allowed);
    assert_eq!(
        size,
        Ok(Some(4096)),
        "the server answers `213 4096`; anything else means the stream is out of step"
    );
}

/// Multi-line replies are the other half of staying in sync: a `220-` banner
/// must be consumed whole, not treated as one reply per line.
#[test]
fn multiline_greetings_do_not_desynchronise_the_stream() {
    if !loopback_name_available() {
        eprintln!("skipping: probe.example.test does not resolve to loopback here");
        return;
    }
    let port = spawn_server(true, None);
    let url = format!("ftp://probe.example.test:{port}/file.AppImage");
    assert_eq!(
        goshaim_core::network::ftp_size(&url, goshaim_core::network::Local::Allowed),
        Ok(Some(4096))
    );
}

/// The PASV address is chosen by the server, so it is untrusted. A server that
/// names a host other than itself is trying to use us as a proxy (FTP bounce);
/// the data connection must stay pinned to the control peer.
#[test]
fn pasv_pointing_at_another_host_is_refused() {
    if !loopback_name_available() {
        eprintln!("skipping: probe.example.test does not resolve to loopback here");
        return;
    }
    // A listener standing in for a third-party host the server tries to point
    // us at. Nothing may reach it.
    let victim = TcpListener::bind("127.0.0.1:0").unwrap();
    let vport = victim.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if victim.accept().is_ok() {
            let _ = tx.send(());
        }
    });

    // The server advertises 10.1.2.3 -- not itself.
    let port = spawn_server_at(false, Some(vport), "10,1,2,3");
    let url = format!("ftp://probe.example.test:{port}/file.AppImage");
    let result =
        goshaim_core::network::ftp_download(&url, 1 << 20, goshaim_core::network::Local::Allowed);
    assert!(
        result.is_err(),
        "a server naming a third host must be refused"
    );
    let message = result.unwrap_err();
    assert!(
        message.contains("instead of"),
        "the refusal should name both addresses, got: {message}"
    );
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(300))
            .is_err(),
        "no connection may reach the third-party host"
    );
}

/// A well-behaved server naming itself is followed normally, so the bounce
/// guard is not simply breaking FTP.
#[test]
fn pasv_naming_the_control_host_is_followed() {
    if !loopback_name_available() {
        eprintln!("skipping: probe.example.test does not resolve to loopback here");
        return;
    }
    let data = TcpListener::bind("127.0.0.1:0").unwrap();
    let dport = data.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = data.accept() {
            let _ = s.write_all(b"PAYLOAD");
        }
    });
    let port = spawn_server(false, Some(dport));
    let url = format!("ftp://probe.example.test:{port}/file.AppImage");
    let body =
        goshaim_core::network::ftp_download(&url, 1 << 20, goshaim_core::network::Local::Allowed)
            .expect("a conformant passive transfer should succeed");
    assert_eq!(body, b"PAYLOAD".to_vec());
}

/// A hostile PASV reply must not overflow the port arithmetic. Fields are
/// octets; parsing them as u16 allowed `nums[4] * 256` to overflow, which
/// panics in debug builds and wraps in release.
#[test]
fn hostile_pasv_replies_are_rejected_not_overflowed() {
    use goshaim_core::network::parse_pasv;
    for hostile in [
        "227 Entering Passive Mode (127,0,0,1,999,0)",
        "227 Entering Passive Mode (127,0,0,1,65535,65535)",
        "227 Entering Passive Mode (1,2,3)",
        "227 Entering Passive Mode ()",
        "227 no parentheses here",
        "227 Entering Passive Mode (127,0,0,1,0,0)",
        "227 Entering Passive Mode )backwards(",
    ] {
        assert!(
            parse_pasv(hostile).is_err(),
            "PASV reply {hostile:?} should be rejected"
        );
    }
    // A well-formed reply still parses, port assembled from two octets.
    let addr = parse_pasv("227 Entering Passive Mode (93,184,216,34,20,21)").unwrap();
    assert_eq!(addr.to_string(), "93.184.216.34:5141");
}
