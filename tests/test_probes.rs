mod common;

use common::{args, plant_self_extraction, ran_self_extraction, Harness};

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
    let code = goshaim_core::cli::run_inspect_probe(
        &c,
        path.to_str().unwrap(),
        false,
        &mut out,
        &mut Vec::new(),
    );
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
    let code = goshaim_core::cli::run_inspect_probe(
        &c,
        "/nonexistent/x.AppImage",
        false,
        &mut out,
        &mut Vec::new(),
    );
    assert_eq!(code as i32, goshaim_core::types::ExitCode::Failure as i32);
    assert!(String::from_utf8_lossy(&out).contains("INSPECT_NO_EXECUTION"));
}

/// Audit finding C-13. The probe verifies the autostart entry it *would*
/// install. It must not install it, and must not enable background update
/// checks: the brief requires diagnostic probes to be non-mutating, and this
/// one used to opt the user into a login-time network task as a side effect.
#[test]
fn autostart_probe_verifies_without_mutating() {
    let h = Harness::new();
    let mut c = h.controller();
    let autostart = c.autostart_desktop_path();
    let background_before = c.settings().background_update_checks();

    let mut out = Vec::new();
    let code = goshaim_core::cli::run_autostart_probe(&mut c, &mut out);
    let text = String::from_utf8_lossy(&out);

    // It still proves the Exec line is right.
    assert!(text.contains("autostart_path="), "got: {text}");
    assert!(text.contains("--fetch-updates"), "got: {text}");
    assert!(text.contains("AUTOSTART_OK"), "got: {text}");
    assert_eq!(code as i32, goshaim_core::types::ExitCode::Ok as i32);

    // And it leaves nothing behind.
    assert!(
        !autostart.exists(),
        "the probe must not install the autostart entry"
    );
    assert_eq!(
        c.settings().background_update_checks(),
        background_before,
        "the probe must not change the background-checks setting"
    );
    assert!(
        text.contains("autostart_installed=false"),
        "the probe should report whether an entry is actually installed: {text}"
    );
}

#[test]
fn version_flag_prints_3_0_0() {
    assert_eq!(goshaim_core::limits::VERSION, "3.0.0");
    let _ = args(&["--version"]);
}

/// `--probe-inspect` takes the stored setting like every other inspection. With it
/// on, the fallback runs, and the probe says that the AppImage executed.
#[test]
fn inspect_probe_reports_an_unsafe_run_when_the_setting_is_on() {
    let h = Harness::new();
    let mut c = h.controller();
    let log = plant_self_extraction(&h);
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let mut out = Vec::new();

    let code = goshaim_core::cli::run_inspect_probe(
        &c,
        path.to_str().unwrap(),
        true,
        &mut out,
        &mut Vec::new(),
    );

    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(ran_self_extraction(&log), "the fallback did not run");
    assert!(
        text.contains("INSPECT_EXECUTED_UNSAFE"),
        "probe output: {text}"
    );
    assert_eq!(code as i32, goshaim_core::types::ExitCode::Failure as i32);
}

/// Without the flag, a probe that would need the fallback runs nothing, says why
/// on stderr, and exits as needing confirmation.
#[test]
fn inspect_probe_stops_at_a_pending_read_and_names_the_flag() {
    let h = Harness::new();
    let mut c = h.controller();
    let log = plant_self_extraction(&h);
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code =
        goshaim_core::cli::run_inspect_probe(&c, path.to_str().unwrap(), false, &mut out, &mut err);

    let stderr = String::from_utf8_lossy(&err).into_owned();
    assert!(
        !ran_self_extraction(&log),
        "the AppImage ran before confirmation"
    );
    assert_eq!(
        code as i32,
        goshaim_core::types::ExitCode::NeedsConfirmation as i32
    );
    assert!(stderr.contains("--allow-unsafe"), "stderr: {stderr}");
}

/// QA2-018: a fallback that ran but produced no metadata must still be reported as
/// run. The probe says so on stdout and stderr, and its JSON lists the warnings.
#[test]
fn a_probe_reports_a_fallback_that_ran_even_when_it_produced_no_metadata() {
    let h = Harness::new();
    let mut c = h.controller();
    // The stand-in runs when asked and leaves a marker, but writes no squashfs-root.
    let marker = h.tmp.path().join("ran.marker");
    let marker_for_hook = marker.clone();
    h.runner.on_run(Box::new(move |req| {
        if !req.args.iter().any(|a| a == "--appimage-extract") {
            return None;
        }
        std::fs::write(&marker_for_hook, b"ran").ok()?;
        Some(goshaim_core::process::ProcessResult {
            program: req.program.clone(),
            exit_code: 0,
            ..Default::default()
        })
    }));
    c.settings_mut()
        .set_unsafe_extraction_fallback(true)
        .expect("the owner can turn the fallback on");
    let path = common::write_fixture(h.tmp.path(), "Demo.AppImage");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code =
        goshaim_core::cli::run_inspect_probe(&c, path.to_str().unwrap(), true, &mut out, &mut err);

    let text = String::from_utf8_lossy(&out).into_owned();
    let stderr = String::from_utf8_lossy(&err).into_owned();
    assert!(marker.exists(), "the AppImage did not run");
    assert!(
        text.contains("INSPECT_EXECUTED_UNSAFE"),
        "probe output: {text}"
    );
    assert_eq!(code as i32, goshaim_core::types::ExitCode::Failure as i32);
    let json: serde_json::Value = serde_json::from_str(text.lines().next().unwrap_or(""))
        .expect("the probe prints JSON first");
    assert_eq!(
        json["unsafe_fallback"],
        serde_json::json!(true),
        "json: {json}"
    );
    let warnings = json["warnings"].as_array().cloned().unwrap_or_default();
    assert!(
        warnings.iter().any(|w| w
            .as_str()
            .is_some_and(|w| w.contains("Unsafe extraction fallback used"))),
        "warnings in json: {json}"
    );
    assert!(
        stderr.contains("Unsafe extraction fallback used"),
        "stderr: {stderr}"
    );
}
