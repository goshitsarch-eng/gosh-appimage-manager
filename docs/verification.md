# Verification record (Rust core with a Flutter GUI, 3.0.0)

Latest pass: 2026-10-07, on an aarch64 Linux host (Fedora 44 kernel),
rustc and cargo 1.99.0, just 1.58.0, Flutter 3.47.6. `flatpak-builder` is not
installed on this host, so this pass ran no Flatpak build. Commands were run
for real; anything not re-verified says so. The crate declares
`rust-version = "1.89"`, the floor the locked dependency graph compiles on.

The GUI is Flutter (`flutter/`) on the Rust core through the bridge in
`bridge/`. The earlier GUI and its `gui` feature were removed. Every result
in sections 1 to 4 comes from the tree after that removal. Section 5 keeps
the earlier build result, marked as superseded.

Tests use fake process, network, process-table, and trash seams with
synthetic ELF and AppImage fixtures. They never touch a real home, execute an
AppImage, or call a live update API. The one exception was an early
`just self-test` run before that recipe isolated `HOME` (see section 4).

## 1. Test suite

```
cargo test --offline --no-fail-fast
```

175 passed; 0 failed; 0 ignored, across the 26 test binaries and the doc
tests. Sixteen of those are the launcher unit tests in `src/launcher.rs`
(GUI resolution, overrides, executable checks, one-line errors, argument
handling, and exec failure). The i18n test that read `src/gui.rs` was
removed with that file.

Two suites (`test_ssrf`, `test_ftp`) exercise hostname resolution through the
real resolver. Where `probe.example.test` does not resolve they report a skip
rather than passing vacuously.

## 2. Build, lint, and format gates

```
cargo build --offline                        # no warnings
cargo clippy --all-targets -- -D warnings    # clean
cargo fmt --check                            # clean
SKIP_FLATPAK=1 ./scripts/verify.sh           # 7 passed, 1 skipped (Flatpak)
```

The removed GUI toolkit is absent from the dependency graph: `cargo tree -i`
on its package name reports no match (exit 101), and neither lock file
mentions it.

`verify.sh` ran cargo build, cargo test, clippy, fmt, the isolated-`HOME`
self-test, `desktop-file-validate`, and `appstreamcli validate --pedantic`.
AppStream reported one pedantic note, `cid-contains-uppercase-letter` for the
application ID `com.goshapps.AppImageManager`. That ID is fixed, and the note
predates this change.

## 3. Launcher

The binary with no command starts the GUI from
`../libexec/gosh-appimage-manager/gosh-appimage-manager-gui`, or from the path
in `GOSH_APPIMAGE_GUI`. Checked by hand on the debug build:

- No GUI installed: one error line and one hint on stderr, exit 1.
- Relative `GOSH_APPIMAGE_GUI`: refused, exit 1.
- Missing absolute `GOSH_APPIMAGE_GUI`: refused, exit 1, no fallback.
- `GOSH_APPIMAGE_GUI=/usr/bin/echo` with the arguments `"a b" c`: exec'd and
  printed `a b c`, exit 0. The argument array is passed through unchanged.
- Stdin held open by `sleep`: the binary returned at once. It never reads stdin.
- `--version` and `--help`: unchanged.

Not verified: the real Flutter GUI started through the launcher. No bundle is
installed on this host.

## 4. Offline self-test

```
just self-test          # HOME and GOSHAIM_HOME point at a scratch directory
```

Prints `SELF_TEST_OK`, exit 0.

Two writes to the real home were found during this pass:

- `~/.local/share/gosh-appimage-manager/registry.sqlite` was created at
  19:15 by an early `just self-test`, before the recipe isolated `HOME`. It
  was left in place and can be deleted.
- `~/.config/gosh-appimage-manager/settings.json` was created at 18:53. It
  does not come from the root crate's tests, whose harness is isolated. The
  bridge's `controller()` uses the real `HOME` unless `GOSHAIM_HOME` is set,
  so a bridge or Flutter run without it is the likely source. That work is
  outside this change.

## 5. Flatpak

Not built for this change, and the manifest has not been built. What was
checked:

- The manifest parses as YAML. The core module builds `--release` with no
  feature flags. The `gosh-appimage-manager-gui` module builds the Flutter
  bundle and installs it to `/app/libexec/gosh-appimage-manager/`.
- The Flutter tag `3.47.6` resolves on GitHub to commit
  `5fc346839b5d0eef006ed8404392afb4dfae428d`, the commit the local SDK is at.
- Both `rustup-init` SHA-256 values match the published `.sha256` files.

Checked on 2026-10-07 (section 8): the freedesktop 26.08 runtime, with the
`llvm22` and `rust-stable` 26.08 extensions, builds both modules on aarch64.
The Flutter module also uses the network during its build, unlike the
offline Rust module.

Superseded: the 2026-09-13 full x86_64 build of the earlier GUI manifest
passed, including the packaged `--self-test`, and `appstreamcli compose`.
That manifest no longer exists in this form.

## 6. Localizability

`src/i18n.rs` and the `i18n/` catalogs are kept. No interface uses them now,
so the `qps` pseudolocale check has nothing to run against until one does.

## 7. Not verified

- An x86_64 Flatpak build. This host is aarch64, so x86_64 runs in CI only.
- A Flatpak install and run from a repository. Section 8 runs the packaged
  app from the build output with the manifest's permissions.
- The Flutter GUI running through the launcher, on a display or on CI.
- A real AppImage executed or integrated. All fixtures are synthetic ELF
  files, and the real `unsquashfs` and `7zz` paths run only in a packaged build.

## 8. aarch64 run on freedesktop 26.08 (2026-10-07)

Host: Fedora Asahi, aarch64, Wayland session with XWayland. Toolchain: cargo
and rustc 1.99.0, just 1.58.0, Flutter 3.47.6. Every command below ran on this
host and exited 0 unless the result says otherwise.

| Gate | Command | Result |
|---|---|---|
| Build | `just build` | pass |
| Lint | `just lint` (clippy with -D warnings) | pass |
| Format | `just fmt-check` | pass |
| Core tests | `just test` | 199 passed, 0 failed |
| Bridge tests | `cargo test` in `bridge/` | 3 passed, 0 failed |
| Bridge lint | `cargo clippy --all-targets -- -D warnings` in `bridge/` | pass |
| Self-test | `just self-test` | `SELF_TEST_OK` |
| Release build | `cargo build --release` | pass |
| Flutter gate | `just flutter-check` | format: 39 files, 0 changed; analyze: no issues; `flutter test`: 272 passed |
| Linux bundle | `flutter build linux --release` | `gosh-appimage-manager-gui` is an aarch64 ELF |
| GUI smoke | `tools/gui-smoke.sh` with a private `GOSHAIM_HOME` | PASS: window found, stayed up 8 s, stopped cleanly |
| Flatpak build | `just flatpak-aarch64` | exit 0 on runtime 26.08 |
| Packaged version | `gosh-appimage-manager --version`, run in the build | `3.0.0` |
| Packaged self-test | `gosh-appimage-manager --self-test`, run in the build | `SELF_TEST_OK` |
| Packaged GUI | launcher in the build sandbox, with X11 | window shown, stayed up 10 s, stopped cleanly |
| Packaged libraries | `ldd` on the packaged GUI | no library reported missing |

How the Flatpak was run here:

- flatpak-builder is not installed on the host. It came from the
  `org.flatpak.Builder` app (flatpak-builder 1.4.9). A wrapper passes
  `FLATPAK_USER_DIR` and `--user`, so the builder sees the user-installed
  runtimes. The `just` recipe itself is unchanged. On a machine with
  flatpak-builder installed, the recipe runs as written.
- The 23.08 runtime is end-of-life on Flathub. The manifest moved to 26.08,
  with `rust-stable` 26.08 and `llvm22` for clang.
- The first 26.08 attempt failed because `flatpak-builder` downloads
  `rustup-init` without its execute bit. The manifest now runs `chmod +x`
  first. The second attempt built both modules.
- `rustup-init` prints warnings that it ignores because of `-y`: an existing
  Rust at `/usr/lib/sdk/rust-stable`, and a `$HOME` that differs from the
  user's home, since the module sets `HOME`. It still installs the stable
  toolchain that cargokit uses for the bridge.
- The packaged GUI logs two messages that do not stop it: GTK cannot load the
  host's `pk-gtk-module`, and ATK reports a critical error because the sandbox
  has no accessibility bus.
- The packaged GUI was run from the build output with `flatpak build`, which
  applies the manifest's permissions. It was not installed from a repository
  and run with `flatpak run`.

Not verified: the x86_64 bundle and Flatpak (CI only); live input in the
running window (drag, resize, maximize, keyboard focus); screen readers.
Evidence files (logs and screenshots) are in the job's temporary folder and
are not kept in the repository.

## 9. Final verification on the frozen tree (2026-10-08)

The final tree is the round 8 tree. Its CLI, bridge, Flutter and bundle builds
are the ones below. The QA audit that ran against it is recorded in
`docs/qa/AUDIT-2026-10-08.md`.

Gate, host, aarch64 (Fedora Asahi, Wayland with XWayland):

| Gate | Result |
|---|---|
| `cargo build`, `cargo build --release` | pass |
| `cargo test` (root) | 275 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` | pass |
| `just self-test` | `SELF_TEST_OK` |
| `cd bridge`: `cargo test`, clippy, fmt, build | 20 passed, 0 failed; clippy and fmt clean |
| `dart format` (lib, test, integration_test) | clean after one formatting-only fix to the QA walkthrough test |
| `flutter analyze` and `flutter analyze integration_test` | no issues |
| `flutter test` | 360 passed, 0 failed |
| `flutter build linux --release` | `gosh-appimage-manager-gui` is an aarch64 ELF |
| `tools/gui-smoke.sh` on the bundle | PASS: window found, stayed up 8 s, stopped cleanly |

Flatpak, freedesktop 26.08, aarch64 (`just flatpak-aarch64`, with flatpak-builder
from `org.flatpak.Builder`): the build exits 0. The packaged checks run with a
scratch home, `HOME`, and XDG directories, so the real home is not written:

- `gosh-appimage-manager --version` prints `3.0.0`.
- `gosh-appimage-manager --self-test` prints `SELF_TEST_OK`.
- `--probe-inspect` on a real-layout AppImage (ELF header, squashfs at the
  payload offset) reads `Alpha Notes` through `unsquashfs`.
- The packaged GUI starts through the launcher in the sandbox, shows its window,
  stays up for 10 seconds, and stops cleanly. Its only log line is the known
  accessibility-bus warning from the sandbox.

Not verified here: the x86_64 bundle and Flatpak (CI only); a third-party
AppImage executed by the unsafe fallback (the fallback is tested with stand-in
programs, and a read-only probe of a real third-party type-2 AppImage was run
earlier); live pointer and keyboard input to the window.


## 10. Icons and updates fix, release 3.0.1 (2026-10-09)

Recorded in full in `docs/qa/FIX-2026-10-09-icons-and-updates.md`: the causes, how
each was reproduced before the fix, and what was run after. Host: x86_64 Linux,
rustc and cargo 1.97.0, Flutter 3.47.6, `squashfs-tools` 4.6.1 (the version the
Flatpak builds).

| Gate | Result |
|---|---|
| `cargo test` (root) | 332 passed, 0 failed (275 before) |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` (root and `bridge/`) | pass |
| `cd bridge`: `cargo test` | 23 passed, 0 failed (20 before) |
| `flutter_rust_bridge_codegen generate` (2.13.0), run twice | the second run changes nothing |
| `dart format --set-exit-if-changed`, `flutter analyze` | clean |
| `flutter test` | 374 passed, 1 failed (359 and the same 1 before) |
| `flutter build linux --release`, `tools/gui-smoke.sh` | PASS: window found, stayed up 10 s, stopped cleanly |
| `scripts/check-version.sh v3.0.1` | passes; the same script refuses `v3.0.0` |
| `appstreamcli validate --pedantic --no-net`, `desktop-file-validate` | the one pedantic note it already gave (`cid-contains-uppercase-letter`); desktop file clean |
| `gosh-appimage-manager --version` | `3.0.1` |

The one Flutter failure is `pages_golden_test.dart: 02 Empty library`, a 6-pixel
(0.00 %) anti-aliasing difference that fails the same way on the untouched tree on
this host. The earlier rounds ran on aarch64, where it passed.

Run with real tools rather than seams: real AppImages (ELF header, `.upd_info`
section, `mksquashfs` payload) through the real `unsquashfs`; the built CLI
adopting four of them; the release GUI under Xvfb against a registry blanked to
what an older build leaves; and a local HTTPS server (own CA, trusted through
`SSL_CERT_FILE`) against the original code and this tree.

Not verified here: a live GitHub release (api.github.com is blocked by this
session's egress policy); a Flatpak build or any aarch64 run; an IPv6-first
resolver order. DwarFS AppImages are unchanged (still metadata-less).
