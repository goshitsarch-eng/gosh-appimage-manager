mod common;

use common::{args, Harness};

#[test]
fn host_probe_reports_spawn_and_folder() {
    let h = Harness::new();
    h.runner.canned("true", 0, b"");
    let c = h.controller();
    let mut out = Vec::new();
    let code = goshaim_core::cli::run_host_probe(&c, &mut out);
    assert_eq!(code as i32, goshaim_core::types::ExitCode::Ok as i32);
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("host_spawn_program=true"));
    assert!(text.contains("host_spawn_exit=0"));
    assert!(text.contains("in_flatpak="));
    assert!(text.contains("managed_folder="));
    assert!(text.contains("HOST_PROBE_OK"));
}

#[test]
fn inspect_probe_json_marks_no_execution() {
    let h = Harness::new();
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let c = h.controller();
    let mut out = Vec::new();
    let code = goshaim_core::cli::run_inspect_probe(&c, path.to_str().unwrap(), &mut out);
    assert_eq!(code as i32, goshaim_core::types::ExitCode::Ok as i32);
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("INSPECT_NO_EXECUTION"));
    assert!(!text.contains("INSPECT_EXECUTED_UNSAFE"));
    let first_line = text.lines().next().unwrap();
    let doc: serde_json::Value = serde_json::from_str(first_line).unwrap();
    assert_eq!(doc.get("schema_version").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(doc.get("magic_valid").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(doc.get("type").and_then(|v| v.as_str()), Some("type-2"));
    assert_eq!(
        doc.get("architecture").and_then(|v| v.as_str()),
        Some("x86_64")
    );
    assert_eq!(
        doc.get("unsafe_fallback").and_then(|v| v.as_bool()),
        Some(false)
    );
}

#[test]
fn inspect_probe_invalid_file_fails() {
    let h = Harness::new();
    let c = h.controller();
    let mut out = Vec::new();
    let code = goshaim_core::cli::run_inspect_probe(&c, "/nonexistent/x.AppImage", &mut out);
    assert_eq!(code as i32, goshaim_core::types::ExitCode::Failure as i32);
    assert!(String::from_utf8_lossy(&out).contains("INSPECT_NO_EXECUTION"));
}

#[test]
fn autostart_probe_writes_and_verifies_entry() {
    let h = Harness::new();
    let mut c = h.controller();
    let mut out = Vec::new();
    // Outside flatpak the Exec line is the current exe + --fetch-updates.
    let code = goshaim_core::cli::run_autostart_probe(&mut c, &mut out);
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("autostart_path="));
    if goshaim_core::process::in_flatpak()
        || std::env::var("FLATPAK_ID").as_deref() == Ok("com.goshapps.AppImageManager")
    {
        assert!(text.contains("AUTOSTART_OK"));
    } else {
        assert!(text.contains("AUTOSTART_OK"), "got: {text}");
    }
    assert!(text.contains("--fetch-updates"));
    assert_eq!(code as i32, goshaim_core::types::ExitCode::Ok as i32);
}

#[test]
fn version_flag_prints_3_0_0() {
    assert_eq!(goshaim_core::limits::VERSION, "3.0.0");
    let _ = args(&["--version"]);
}
