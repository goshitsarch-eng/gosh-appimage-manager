// Updates driven by an AppImage's own embedded update information.
//
// Almost every AppImage that can update itself carries a string like
//   gh-releases-zsync|owner|repo|latest|App-*x86_64.AppImage.zsync
// The earlier tests only exercised hand-written GitHub sources with an exact
// asset name, so none of them noticed that this form never produced an update:
//
// * `detect_embedded` asked the static source first, and the static source
//   claims any hint containing "zsync" -- which includes "gh-releases-zsync".
//   The check then failed with "Static source needs a url".
// * Once routed to GitHub, the pattern still ends in `.zsync`, so it matched
//   the release's control file rather than the AppImage itself.
// * Release tags are usually "v1.2.3" while the AppImage says "1.2.3", so an
//   app that was already current looked outdated.
//
// The network is the shared fake; no AppImage is ever executed.

mod common;

use common::{write_fixture, Harness};
use goshaim_core::controller::AppController;
use goshaim_core::types::InstalledApp;
use goshaim_core::updates_sources::UpdateSourceFactory;

const EMBEDDED: &str = "gh-releases-zsync|example-org|quill|latest|Quill-*x86_64.AppImage.zsync";

/// A release as GitHub's API returns it: the AppImage, its control file, and an
/// AppImage for another architecture.
fn release_json(tag: &str, digest: Option<&str>) -> String {
    let digest = digest
        .map(|d| format!(r#","digest":"sha256:{d}""#))
        .unwrap_or_default();
    format!(
        r#"{{"tag_name":"{tag}","assets":[
          {{"name":"Quill-{tag}-aarch64.AppImage","size":2048,"browser_download_url":"https://github.com/example-org/quill/releases/download/{tag}/Quill-{tag}-aarch64.AppImage"}},
          {{"name":"Quill-{tag}-x86_64.AppImage.zsync","size":512,"browser_download_url":"https://github.com/example-org/quill/releases/download/{tag}/Quill-{tag}-x86_64.AppImage.zsync"}},
          {{"name":"Quill-{tag}-x86_64.AppImage","size":4096,"browser_download_url":"https://github.com/example-org/quill/releases/download/{tag}/Quill-{tag}-x86_64.AppImage"{digest}}}
        ]}}"#
    )
}

/// An owned app that has only its embedded update information: no source was
/// ever chosen, which is how an integrated or adopted app starts out.
fn seed(c: &mut AppController, dir: &std::path::Path, version: &str) -> InstalledApp {
    let path = write_fixture(dir, "Quill.AppImage");
    let mut app = InstalledApp::new_owned();
    app.uuid = goshaim_core::registry::ManagedRegistry::new_uuid();
    app.name = "Quill".to_string();
    app.version = version.to_string();
    app.managed_path = path.to_string_lossy().into_owned();
    app.embedded_update = EMBEDDED.to_string();
    app.architecture = goshaim_core::types::Architecture::X86_64;
    c.registry_mut().upsert(app.clone()).unwrap();
    app
}

#[test]
fn a_gh_releases_zsync_string_selects_the_github_source() {
    let source = UpdateSourceFactory::detect_embedded(EMBEDDED).expect("a source is chosen");
    assert_eq!(source.name(), "github");
}

#[test]
fn the_other_embedded_forms_still_select_their_own_source() {
    for (raw, expected) in [
        ("zsync|https://example.org/Quill.AppImage.zsync", "static"),
        ("bintray-zsync|user|repo|pkg|Quill*.zsync", "static"),
        (
            "gh-releases-direct|example-org|quill|latest|Quill*",
            "github",
        ),
        ("gitlab-releases-zsync|group/project|Quill*", "gitlab"),
        ("ftp|ftp://example.org/Quill.AppImage", "ftp"),
    ] {
        let source = UpdateSourceFactory::detect_embedded(raw)
            .unwrap_or_else(|| panic!("no source for {raw}"));
        assert_eq!(source.name(), expected, "for {raw}");
    }
}

#[test]
fn an_embedded_github_pattern_offers_the_appimage_not_its_control_file() {
    let h = Harness::new();
    let mut c = h.controller();
    let app = seed(&mut c, h.tmp.path(), "1.0.0");
    h.network
        .canned_body("api.github.com", release_json("v1.1.0", None).as_bytes());

    let checked = c.check_one_update(&app, &Harness::cancel());

    assert!(checked.ok, "the check failed: {}", checked.error);
    assert!(checked.available);
    assert_eq!(checked.manager, "github");
    assert_eq!(checked.version, "v1.1.0");
    assert!(
        checked.url.ends_with("Quill-v1.1.0-x86_64.AppImage"),
        "downloads {} instead of the AppImage",
        checked.url
    );
    assert_eq!(checked.size, 4096);
}

#[test]
fn the_asset_for_another_architecture_is_not_chosen() {
    let h = Harness::new();
    let mut c = h.controller();
    let mut app = seed(&mut c, h.tmp.path(), "1.0.0");
    // A looser pattern matches every architecture the release carries.
    app.embedded_update =
        "gh-releases-zsync|example-org|quill|latest|Quill-*.AppImage.zsync".into();
    c.registry_mut().upsert(app.clone()).unwrap();
    h.network
        .canned_body("api.github.com", release_json("v1.1.0", None).as_bytes());

    let checked = c.check_one_update(&app, &Harness::cancel());

    assert!(checked.ok, "the check failed: {}", checked.error);
    assert!(
        checked.url.ends_with("x86_64.AppImage"),
        "an x86_64 install was offered {}",
        checked.url
    );
}

#[test]
fn a_v_prefixed_tag_for_the_installed_version_is_not_an_update() {
    let h = Harness::new();
    let mut c = h.controller();
    let app = seed(&mut c, h.tmp.path(), "1.1.0");
    h.network
        .canned_body("api.github.com", release_json("v1.1.0", None).as_bytes());

    let scan = c.scan_updates(&Harness::cancel());

    assert!(scan.failures.is_empty(), "failures: {:?}", scan.failures);
    assert!(
        scan.offers.is_empty(),
        "an app already at 1.1.0 was offered {:?}",
        scan.offers
            .iter()
            .map(|o| &o.available_version)
            .collect::<Vec<_>>()
    );
    // The single-app check says the same thing.
    let checked = c.check_one_update(&app, &Harness::cancel());
    assert!(!goshaim_core::updates_service::offers_update(
        &app, &checked
    ));
}

#[test]
fn an_older_release_is_never_offered_as_an_update() {
    let h = Harness::new();
    let mut c = h.controller();
    seed(&mut c, h.tmp.path(), "2.0.0");
    h.network
        .canned_body("api.github.com", release_json("v1.9.0", None).as_bytes());

    let scan = c.scan_updates(&Harness::cancel());

    assert!(
        scan.offers.is_empty(),
        "offered a downgrade: {:?}",
        scan.offers.len()
    );
}

#[test]
fn a_newer_release_is_offered_once_with_its_numeric_order() {
    let h = Harness::new();
    let mut c = h.controller();
    seed(&mut c, h.tmp.path(), "1.9.0");
    h.network
        .canned_body("api.github.com", release_json("v1.10.0", None).as_bytes());

    let scan = c.scan_updates(&Harness::cancel());

    assert_eq!(scan.offers.len(), 1, "1.10.0 is newer than 1.9.0");
    assert_eq!(scan.offers[0].available_version, "v1.10.0");
}

#[test]
fn a_named_release_is_fetched_by_tag_not_as_the_latest() {
    let h = Harness::new();
    let mut c = h.controller();
    let mut app = seed(&mut c, h.tmp.path(), "1.0.0");
    app.embedded_update =
        "gh-releases-zsync|example-org|quill|continuous|Quill-*x86_64.AppImage.zsync".into();
    c.registry_mut().upsert(app.clone()).unwrap();
    h.network.canned_body(
        "api.github.com",
        release_json("continuous", None).as_bytes(),
    );

    let checked = c.check_one_update(&app, &Harness::cancel());

    assert!(checked.ok, "the check failed: {}", checked.error);
    let calls = h.network.calls.lock().unwrap().clone();
    assert!(
        calls
            .iter()
            .any(|u| u.contains("/releases/tags/continuous")),
        "asked for {calls:?}"
    );
    assert!(!calls.iter().any(|u| u.ends_with("/releases/latest")));
}

#[test]
fn a_rebuild_under_the_same_tag_is_an_update_when_the_digest_differs() {
    let h = Harness::new();
    let mut c = h.controller();
    let mut app = seed(&mut c, h.tmp.path(), "continuous");
    app.sha256 = vec![0x11; 32];
    app.embedded_update =
        "gh-releases-zsync|example-org|quill|continuous|Quill-*x86_64.AppImage.zsync".into();
    c.registry_mut().upsert(app.clone()).unwrap();
    let other = "22".repeat(32);
    h.network.canned_body(
        "api.github.com",
        release_json("continuous", Some(&other)).as_bytes(),
    );

    let scan = c.scan_updates(&Harness::cancel());
    assert_eq!(scan.offers.len(), 1, "failures: {:?}", scan.failures);

    // The same digest means the installed file is the published one.
    let same = "11".repeat(32);
    h.network.canned_body(
        "api.github.com",
        release_json("continuous", Some(&same)).as_bytes(),
    );
    let scan = c.scan_updates(&Harness::cancel());
    assert!(scan.offers.is_empty(), "offered the file that is installed");
}

#[test]
fn a_missing_release_is_explained_not_reported_as_a_bare_status() {
    let h = Harness::new();
    let mut c = h.controller();
    let app = seed(&mut c, h.tmp.path(), "1.0.0");
    // No canned body: the fake network answers like an unknown repository.
    let checked = c.check_one_update(&app, &Harness::cancel());
    assert!(!checked.ok);
    assert!(
        !checked.error.contains("Static source"),
        "wrong source reported: {}",
        checked.error
    );
}

#[test]
fn a_zsync_control_file_with_a_relative_url_resolves_against_its_own_address() {
    let h = Harness::new();
    let mut c = h.controller();
    let path = write_fixture(h.tmp.path(), "Quill.AppImage");
    let mut app = InstalledApp::new_owned();
    app.uuid = goshaim_core::registry::ManagedRegistry::new_uuid();
    app.name = "Quill".to_string();
    app.version = "1.0".to_string();
    app.managed_path = path.to_string_lossy().into_owned();
    app.update_manager = "static".to_string();
    app.update_config.insert(
        "url".to_string(),
        "https://updates.example.org/quill/Quill-x86_64.AppImage.zsync".to_string(),
    );
    app.update_config
        .insert("version".to_string(), "2.0".to_string());
    c.registry_mut().upsert(app.clone()).unwrap();
    // zsync writes the target relative to the control file.
    h.network.canned_body(
        "Quill-x86_64.AppImage.zsync",
        b"zsync: 0.6.2\nFilename: Quill-x86_64.AppImage\nBlocksize: 2048\nLength: 4096\nURL: Quill-x86_64.AppImage\n\n",
    );
    h.network.canned_size("Quill-x86_64.AppImage", 4096);

    let checked = c.check_one_update(&app, &Harness::cancel());

    assert!(checked.ok, "the check failed: {}", checked.error);
    assert_eq!(
        checked.url,
        "https://updates.example.org/quill/Quill-x86_64.AppImage"
    );
}

#[test]
fn an_app_with_no_update_source_is_told_how_to_set_one() {
    let h = Harness::new();
    let mut c = h.controller();
    let mut app = seed(&mut c, h.tmp.path(), "1.0.0");
    app.embedded_update.clear();
    c.registry_mut().upsert(app.clone()).unwrap();

    let checked = c.check_one_update(&app, &Harness::cancel());

    assert!(!checked.ok);
    assert!(
        checked.error.contains("Set one on the app's page"),
        "error: {}",
        checked.error
    );
}

#[test]
fn update_information_of_an_unknown_kind_names_the_kind() {
    let h = Harness::new();
    let mut c = h.controller();
    let mut app = seed(&mut c, h.tmp.path(), "1.0.0");
    app.embedded_update = "pling-v1-zsync|1234|Quill-*.AppImage.zsync".to_string();
    c.registry_mut().upsert(app.clone()).unwrap();

    let checked = c.check_one_update(&app, &Harness::cancel());

    assert!(!checked.ok);
    assert!(
        checked.error.contains("pling-v1-zsync"),
        "error: {}",
        checked.error
    );
}

/// The whole update, end to end on the fake network: an embedded GitHub string
/// finds the AppImage beside its control file, downloads it, verifies its
/// digest, swaps it in, and records the version as the release names it.
#[test]
fn an_embedded_github_update_applies_and_records_the_release_version() {
    use sha2::Digest;
    let h = Harness::new();
    let mut c = h.controller();
    let app = seed(&mut c, h.tmp.path(), "1.0.0");
    let payload = goshaim_core::inspector::make_test_elf(
        goshaim_core::types::Architecture::X86_64,
        goshaim_core::types::AppImageType::Type2,
    );
    let digest = hex::encode(sha2::Sha256::digest(&payload));
    h.network.canned_body(
        "api.github.com",
        release_json("v1.1.0", Some(&digest)).as_bytes(),
    );
    h.network
        .canned_body("Quill-v1.1.0-x86_64.AppImage", &payload);

    let result = c.apply_update(&app, true, &Harness::cancel());

    assert!(result.ok, "update failed: {}", result.error);
    let stored = c.registry().by_uuid(&app.uuid).unwrap();
    assert_eq!(stored.version, "1.1.0", "recorded without the tag's v");
    assert_eq!(hex::encode(&stored.sha256), digest);
    // A second check finds nothing newer.
    let scan = c.scan_updates(&Harness::cancel());
    assert!(scan.offers.is_empty(), "offers: {:?}", scan.offers.len());
}

/// A network that answers with a fixed failure, for the messages a person reads.
struct Refusing(&'static str);

impl goshaim_core::network::NetworkClient for Refusing {
    fn get(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        _local: goshaim_core::network::Local,
    ) -> Result<goshaim_core::network::FetchResult, String> {
        Err(self.0.to_string())
    }
    fn head_len(
        &self,
        _url: &str,
        _local: goshaim_core::network::Local,
    ) -> Result<Option<u64>, String> {
        Err(self.0.to_string())
    }
    fn download_bounded(
        &self,
        _url: &str,
        _max: u64,
        _local: goshaim_core::network::Local,
    ) -> Result<Vec<u8>, String> {
        Err(self.0.to_string())
    }
    fn download_to_file(
        &self,
        _url: &str,
        _dest: &std::path::Path,
        _max: u64,
        _cancel: &std::sync::atomic::AtomicBool,
        _local: goshaim_core::network::Local,
        _progress: &mut dyn FnMut(u64, u64),
    ) -> Result<u64, String> {
        Err(self.0.to_string())
    }
}

fn check_with_failure(failure: &'static str) -> String {
    let h = Harness::new();
    let mut c = AppController::with_seams(
        Box::new(h.runner.clone()),
        Box::new(Refusing(failure)),
        Box::new(h.table.clone()),
        Box::new(h.trash.clone()),
        h.dirs(),
    )
    .unwrap();
    let app = seed(&mut c, h.tmp.path(), "1.0.0");
    let checked = c.check_one_update(&app, &Harness::cancel());
    assert!(!checked.ok);
    checked.error
}

#[test]
fn a_github_404_names_the_repository_and_what_to_check() {
    let error = check_with_failure("Network request failed: HTTP 404 Not Found");
    assert!(error.contains("example-org/quill"), "error: {error}");
    assert!(error.contains("published release"), "error: {error}");
    assert!(!error.contains("no a "), "error: {error}");
}

#[test]
fn a_github_403_explains_the_usual_causes() {
    let error = check_with_failure("Network request failed: HTTP 403 Forbidden");
    assert!(error.contains("HTTP 403"), "error: {error}");
    assert!(error.contains("hourly limit"), "error: {error}");
}

#[test]
fn a_rate_limited_github_answer_says_so() {
    let error =
        check_with_failure("Network request failed: HTTP 403 Forbidden (rate limit reached)");
    assert!(
        error.contains("request limit was reached"),
        "error: {error}"
    );
}

#[test]
fn a_timeout_stays_recognisable_as_a_timeout() {
    let error = check_with_failure("Network request failed: operation timed out");
    assert!(error.contains("timed out"), "error: {error}");
}

fn config(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// The README says a private or loopback update server needs an explicit
/// `allow_local_network=true` on a source the user created. Saving a static
/// source refused every local address, whatever the config said.
#[test]
fn a_static_source_may_name_a_local_server_only_with_the_opt_in() {
    use goshaim_core::updates_sources::UpdateSource;
    let source = goshaim_core::updates_sources::StaticSource;
    let url = "https://nas.local/updates/Quill.AppImage";
    let denied = config(&[("url", url), ("version", "2.0")]);
    assert!(source.validate_config(&denied).is_err());
    let allowed = config(&[
        ("url", url),
        ("version", "2.0"),
        ("allow_local_network", "true"),
    ]);
    assert!(source.validate_config(&allowed).is_ok());
    // A public address needs nothing.
    let public = config(&[("url", "https://updates.example.org/Quill.AppImage")]);
    assert!(source.validate_config(&public).is_ok());
}

#[test]
fn a_private_gitlab_host_is_allowed_only_with_the_opt_in() {
    use goshaim_core::updates_sources::UpdateSource;
    let source = goshaim_core::updates_sources::GitlabSource;
    let denied = config(&[("project", "team/quill"), ("host", "gitlab.internal")]);
    assert!(source.validate_config(&denied).is_err());
    let allowed = config(&[
        ("project", "team/quill"),
        ("host", "gitlab.internal"),
        ("allow_local_network", "true"),
    ]);
    assert!(source.validate_config(&allowed).is_ok());
}

#[test]
fn embedded_update_information_can_never_opt_itself_into_the_local_network() {
    use goshaim_core::updates_sources::UpdateSource;
    let source = goshaim_core::updates_sources::StaticSource;
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "url".to_string(),
        "https://127.0.0.1/Quill.AppImage".to_string(),
    );
    fields.insert("allow_local_network".to_string(), "true".to_string());
    let derived = source.config_from_embedded(&fields);
    assert!(!derived.contains_key("allow_local_network"));
    assert!(source.validate_config(&derived).is_err());
}
