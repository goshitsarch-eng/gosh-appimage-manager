# Verification record (Rust 3.0.0)

Latest pass: 2026-09-13, in a Fedora distrobox (`gosh-os-next-dev`) with
rustc/cargo 1.98.0 and flatpak-builder 1.4.10. Commands were run for real;
anything not re-verified says so. The crate declares `rust-version = "1.89"`
— the floor the locked dependency graph actually compiles on.

This is the native Rust + libcosmic implementation. There is no Qt/KDE,
CMake, QML, CTest, or ECM anywhere in the tree;
`docs/implementation-brief.md` is the original mission statement and is
superseded on stack by `docs/rewrite-3.0.0.md`.

No command below mutated a real `~/AppImages`, desktop entry, icon, or
configuration. Tests use fake process/network/table/trash seams and
synthetic ELF/AppImage fixtures; they never touch a real home, execute an
AppImage, or call a live update API. Live CLI probes ran with an isolated
`HOME` (`GOSHAIM_HOME` is also honoured).

## 1. Full test suite

```
cargo test --no-fail-fast
```

```
160 passed; 0 failed (23 suites + fixture test)
```

Two suites (`test_ssrf`, `test_ftp`) exercise hostname→loopback resolution
through the real resolver; where `probe.example.test` does not resolve they
report the skip rather than passing vacuously. Adding
`127.0.0.1 probe.example.test` to `/etc/hosts` exercises them fully — in
this environment the test passed without the hosts entry.

## 2. Lint and format gates

```
cargo clippy --all-targets -- -D warnings
cargo clippy --features gui --all-targets -- -D warnings
cargo fmt --check
```

All pass with zero warnings.

## 3. GUI build

```
cargo check --features gui
cargo build --features gui
```

Clean against the pinned libcosmic v0.12 tag (1m51s). Needs the Wayland and
XKB development headers; on Debian/Ubuntu:

```
apt-get install libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev
```

The GUI binary was also launched on a real X display (`DISPLAY=:1`) with an
isolated `HOME` and ran without errors until killed — it opens on X11; a
driven COSMIC/Wayland session remains unverified (§8).

## 4. Offline self-test through the real binary

```
cargo build
HOME=/tmp/gosh-aim-qa-home ./target/debug/gosh-appimage-manager --self-test
```

```
[self-test] readiness: ok
[self-test] elf-fixture: ok
[self-test] registry: ok
[self-test] desktop: ok
[self-test] url-guard: ok
[self-test] managers: ok
[self-test] json-schema: ok
SELF_TEST_OK
```

Exit 0, isolated HOME. `readiness` checks real things (registry path, data
dir, settings load errors, managed folder, all six managers resolve).

## 5. Live CLI probes (isolated HOME, all exit 0)

```
--version                -> 3.0.0
--list-installed --json  -> {"installed":[],"schema_version":1}
--list-updates --json    -> {"schema_version":1,"updates":[]}
--list-update-managers   -> static github gitlab codeberg forgejo ftp
--probe-host             -> host_spawn_program=true, in_flatpak=false, HOST_PROBE_OK
--probe-autostart        -> autostart_installed=false, AUTOSTART_OK (renders, never writes)
--probe-inspect <file>   -> JSON + INSPECT_NO_EXECUTION
```

End-to-end against a synthetic 128-byte ELF+AI\x02 fixture:
`--integrate --yes` copies to `~/AppImages/` (0700 dir, executable file),
writes `~/.local/share/applications/gosh-appimage-<uuid>.desktop` with the
ownership markers, and registers a row; `--list-installed --json` reports
it `owned:true`; `--remove --yes` trashes the file (freedesktop mount-top
`.Trash-<uid>`), removes the entry, and deletes the row. `--adopt` on an
external file registers it `owned:true` with no disk changes; a later
`--remove` trashes it at its original location. Confirmation refusal
without a TTY exits 5; `--update` on an unknown path exits 4.

## 6. Flatpak build (x86_64)

```
flatpak-builder --user --force-clean --disable-rofiles-fuse \
  build-dir packaging/com.goshapps.AppImageManager.yml --arch=x86_64
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml \
  gosh-appimage-manager --self-test
```

Full build passed: pinned Rust 1.90.0 toolchain, unsquashfs from source,
per-arch 7zz/dwarfs binaries, vendored cargo deps offline, in-builder
`--self-test` ran during the build, appstreamcli compose succeeded, exports
clean. The packaged `--self-test` re-printed `SELF_TEST_OK` under
`flatpak-builder --run`. aarch64 is covered by CI (qemu-user).

## 7. Localizability

```
GOSHAIM_LOCALE_DIR=./i18n LC_ALL=qps cargo run --features gui
```

`qps` is a pseudolocale: `i18n.rs` accents every letter and pads each
string by about a third, so anything still in plain ASCII was never routed
through the catalog and anything clipped is a layout that only fits
English. `i18n/qps.json` is an empty marker file — the transform lives in
code, keyed on the locale name.

## 8. Not verified

Stated plainly rather than implied:

- **No driven pass on a real compositor.** The GUI launches and renders on
  real X11 (§3) and is driven/measured on a headless X server by
  `tools/gui-smoke.sh`, but no one has used it through a COSMIC or Wayland
  session. Window-manager behaviour — minimum size, tiling, fractional
  scaling — is the compositor's and remains unverified.
- **No aarch64 local build.** Covered by CI (qemu-user) only.
- **No real AppImage was executed or integrated.** All fixtures are
  synthetic ELF files; extraction tools are driven through the process
  seam in tests (the real `unsquashfs`/`7zz` paths are exercised only in
  the packaged build).
- **GUI smoke test not run here** — Xvfb/xdotool/ImageMagick are absent;
  `scripts/verify.sh` skips that stage loudly.

## 9. Findings history

`AUDIT.md` records the 2026-09-07 end-to-end audit and its fixes;
`docs/audit/` the 2026-09-12 hardening round; `docs/migration/` the parity
round. `docs/documentation/` holds the 2026-09-13 documentation audit
(`APP-INVENTORY.md`, `AUDIT.md`, `PLAN.md`) this record was refreshed
alongside.
