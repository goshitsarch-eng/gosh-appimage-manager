# Verification record (Rust 3.0.0)

Every command below was executed in this worktree. Output is summarised from
real transcripts; nothing here is fabricated, and claims that could not be
verified are marked as such rather than asserted.

Toolchain: rustc/cargo 1.94.1. The crate declares `rust-version = "1.75"`;
`cargo clippy` enforces that MSRV and passes.

This is the native Rust + libcosmic implementation. There is no Qt/KDE,
CMake, QML, CTest, or ECM anywhere in the tree; earlier revisions of this
record that named them described a retired stack, as does
`docs/implementation-brief.md`, which is kept as the original mission
statement and is superseded on stack by `docs/rewrite-3.0.0.md`.

No command mutated a real `~/AppImages`, desktop entry, icon, or application
configuration. Tests use fake process/network/table/trash seams and synthetic
ELF/AppImage fixtures; they never touch a real home, execute an AppImage, or
call a live update API. Live CLI probes ran with an isolated `HOME`
(`GOSHAIM_HOME` is also honoured).

## 1. Full test suite

```
cargo test
```

```
134 passed; 0 failed (20 suites, 0 failures)
```

Suites: `test_cli`, `test_desktop_tasks`, `test_detail`, `test_elf`,
`test_ftp`, `test_glob`, `test_icon`, `test_inspector`, `test_integration`,
`test_launch`, `test_library`, `test_network`, `test_probes`,
`test_registry`, `test_registry_perf`, `test_removal`, `test_rollback`,
`test_settings`, `test_ssrf`, `test_update`, plus the in-crate fixture test.

Two suites depend on `probe.example.test` resolving to `127.0.0.1`
(`test_ssrf`, `test_ftp`). Where it does not, they print that they are
skipping rather than passing vacuously. Add it to `/etc/hosts` to exercise
them:

```
127.0.0.1 probe.example.test
```

Coverage beyond the previous record: transactional rollback at four injected
failure points (previously untested — the fail-point seam had no caller
anywhere), FTP protocol conformance against a real RFC 959 server, the
resolved-address SSRF guard against a real socket, icon extraction end to
end, external discovery and adoption, settings failure reporting, and
registry write/lookup performance.

## 2. Lint and format gates

```
cargo clippy --all-targets -- -D warnings
cargo clippy --features gui --all-targets -- -D warnings
cargo fmt --check
```

All three pass with zero warnings. The previous record claimed this while it
was failing on `updates_sources.rs` (`clippy::nonminimal_bool`) on any
toolchain from 1.83 onward.

## 3. GUI compile

```
cargo check --features gui
```

Clean against the pinned libcosmic v0.12 tag. This needs the Wayland and XKB
development headers; on Debian/Ubuntu:

```
apt-get install libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev
```

The previous record listed those as unavailable and treated the GUI as
uncompilable here, which is why a line-by-line audit stood in for a build.
They install without trouble, and the GUI has been type-checked since.

## 4. Offline self-test through the real binary

```
cargo build
HOME=/tmp/gosh-aim-qa-home ./target/debug/gosh-appimage-manager --self-test
```

```
[self-test] models-ready: ok
[self-test] elf-fixture: ok
[self-test] registry: ok
[self-test] desktop: ok
[self-test] url-guard: ok
[self-test] managers: ok
[self-test] json-schema: ok
SELF_TEST_OK
```

Exit 0, isolated HOME. Note that `models-ready` returns a constant `true` and
asserts nothing; it is a placeholder, not a check.

## 5. Non-mutating CLI probes (isolated HOME)

```
--version              -> 3.0.0
--list-installed --json -> {"installed":[],"schema_version":1}
--list-updates --json   -> {"schema_version":1,"updates":[]}
--list-update-managers  -> static github gitlab codeberg forgejo ftp
--probe-host            -> host_spawn_program=true, in_flatpak=false, HOST_PROBE_OK
--probe-autostart       -> autostart_installed=false, AUTOSTART_OK
```

All exit 0. Diagnostics go to stderr so stdout stays valid JSON.
`--probe-autostart` now renders and verifies the entry without installing it
or changing any setting; the previous record documents having to delete its
residue by hand afterwards.

Discovery and adoption verified end to end against an isolated HOME with a
foreign desktop entry pointing outside the managed folder: `--list-discovered`
reports it only with the outside-folder setting on, `--adopt` registers it,
and the foreign entry is byte-identical afterwards.

## 6. Measured performance

Release build, three serialized runs each, on the machine used for this
record. These measure the shape of the change, not the box:

| Operation | Before | After |
|---|---|---|
| 300 exact-path registry lookups over 300 rows | ~47 ms | ~0.24 ms |
| 100 single-row registry updates over 400 rows | ~320 ms | ~150 ms |
| 300 registry removals from 300 rows | ~756 ms | ~470 ms |

Asset-name glob matching, release build, against a 50-character asset name —
the previous implementation, showing the blow-up that made a hostile
`.upd_info` able to wedge the background update checker:

| Pattern length | 5 | 9 | 11 | 13 | 15 |
|---|---|---|---|---|---|
| Time | 0.13 ms | 6.7 ms | 57 ms | 507 ms | 3.15 s |

The replacement is linear and answers the 49-character case that used to run
past three minutes in well under a second.

## 7. Not verified

Stated plainly rather than implied:

- **No visual pass.** The GUI compiles and type-checks, and the operations
  behind it are covered by tests, but no screenshots have been taken and no
  frame has been rendered. A manual pass on a COSMIC/Wayland session is still
  outstanding.
- **X11 is broken.** A private Xvfb seat showed the GUI panicking before
  mapping any window:
  `Visual 0x40 does not use softbuffer's pixel format and is unsupported`.
  Visual `0x40` is a 32-bit TrueColor visual, chosen because libcosmic
  requests a transparent window (`cosmic::app::Settings.transparent` defaults
  to true and is `pub(crate)`, so application code cannot turn it off). The
  pinned softbuffer 0.4.1 supports only 16/24-bit X11 visuals. Wayland — the
  primary target — is unaffected. The Flatpak manifest still advertises
  `--socket=fallback-x11`. Upstream fix required; no local workaround applied.
- **No Flatpak build.** `flatpak-builder` is not installed here, so neither
  architecture was built and the packaged probes were not re-run. The manifest
  is unchanged by this work apart from what is noted in
  `docs/rewrite-3.0.0.md`.
- **No AppStream/desktop validation.** `desktop-file-validate` and
  `appstreamcli` are not installed here, so `just validate` was not re-run.
  `data/com.goshapps.AppImageManager.desktop` changed (`%U` → `%F`) and
  should be re-validated where those tools exist.
- **No real AppImage was executed or integrated.** All fixtures are synthetic
  ELF files; the extraction tools are driven through the process seam.

## 8. Findings

`AUDIT.md` records the end-to-end audit this work came out of: what was
found, what was fixed, and what was deliberately not.
