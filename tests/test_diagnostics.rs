mod common;

use common::{args, Harness};
use goshaim_core::cli::run_cli;
use goshaim_core::diagnostics::{file_label, format_line, write_if};

#[test]
fn diagnostics_are_silent_unless_enabled() {
    let mut buf = Vec::new();
    write_if(&mut buf, false, "cli", "list-installed count=3");
    assert!(buf.is_empty(), "disabled diagnostics must not write");

    let mut buf = Vec::new();
    write_if(&mut buf, true, "cli", "list-installed count=3");
    let text = String::from_utf8(buf).unwrap();
    assert!(text.contains("[goshaim:cli]"), "got {text:?}");
    assert!(text.contains("count=3"), "got {text:?}");
    assert_eq!(format_line("update", "x"), "[goshaim:update] x");
}

#[test]
fn file_label_never_carries_directories() {
    assert_eq!(
        file_label("/home/someone/AppImages/Foo.AppImage"),
        "Foo.AppImage"
    );
    assert_eq!(file_label(""), "file");
}

#[test]
fn cli_list_emits_diagnostics_only_when_switch_is_on() {
    let h = Harness::new();
    let mut c = h.controller();

    // Off by default: no diagnostics lines.
    let argv = args(&["--list-installed"]);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdin: &[u8] = b"";
    run_cli(&mut c, &argv, false, &mut out, &mut err, &mut stdin);
    let quiet = String::from_utf8(err).unwrap();
    assert!(
        !quiet.contains("[goshaim:"),
        "switch off must stay silent, got {quiet:?}"
    );

    // Flip the persisted switch: diagnostics appear on stderr.
    c.settings_mut()
        .set_debug_logging(true)
        .expect("persist diagnostics preference");
    assert!(c.settings().debug_logging());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdin: &[u8] = b"";
    run_cli(&mut c, &argv, false, &mut out, &mut err, &mut stdin);
    let noisy = String::from_utf8(err).unwrap();
    assert!(
        noisy.contains("[goshaim:cli]"),
        "switch on must emit diagnostics, got {noisy:?}"
    );
    assert!(noisy.contains("list-installed count="), "got {noisy:?}");

    // JSON stdout stays parseable when diagnostics are on.
    let argv = args(&["--list-installed", "--json"]);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdin: &[u8] = b"";
    run_cli(&mut c, &argv, false, &mut out, &mut err, &mut stdin);
    let doc: serde_json::Value = serde_json::from_slice(&out).expect("stdout must stay valid JSON");
    assert!(doc.get("installed").is_some());
    let err_text = String::from_utf8(err).unwrap();
    assert!(err_text.contains("[goshaim:cli]"), "got {err_text:?}");
}
