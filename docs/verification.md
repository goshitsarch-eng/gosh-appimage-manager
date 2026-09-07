# Verification record (Rust 3.0.0)

All commands below were executed on 2026-09-07 UTC in this worktree. Output is
summarised from the real tool transcripts; nothing here is fabricated.

Host: container worktree `/var/home/gosh/Github/gosh-appimage-manager`.
Toolchain: system `cargo`/`rustc` (crate `rust-version = "1.75"`).
This is the native Rust + libcosmic implementation. There is no Qt/KDE,
CMake, QML, CTest, or ECM anywhere in the tree; earlier revisions of this
record that named them described a retired stack.

No command mutated the operator's real `~/AppImages`, desktop entries, icons,
or application configuration. Live CLI probes ran with an isolated `HOME`
(`GOSHAIM_HOME` overrides are also honoured). Tests use fake
process/network/table/trash seams and synthetic ELF/AppImage fixtures; they
never touch a real home, execute an AppImage, or call live update APIs.

## 1. Full test suite

```
cargo test
```

```
test result: ok. 79 passed; 0 failed (15 suites, 0 failures everywhere)
```

Per-suite counts: lib 1, `test_cli` 11, `test_desktop_tasks` 9, `test_elf`
10, `test_inspector` 8, `test_integration` 5, `test_launch` 5,
`test_network` 8, `test_probes` 5, `test_registry` 2, `test_removal` 6,
`test_update` 9.

Coverage includes: ELF/AppImage magic and types, inspector extractors with
fail-closed unsafe paths, transactional integrate (copy/replace/keep-both,
rollback on partial failure), Trash-first removal with permanent-delete
containment, start-only detached launch, URL guards, update-source
set/unset round-trips, digest-mismatch and running-app guards, registry
SQLite round-trips plus one-shot v2 JSON import, CLI JSON schemas
(`schema_version: 1` with `installed`/`updates` arrays), and the
host/inspect/autostart probes.

## 2. Lint and format gates

```
cargo clippy --all-targets -- -D warnings
cargo clippy --offline --features gui --all-targets -- -D warnings
cargo fmt --check
```

All three pass with zero warnings. `cargo check` is warning-free both
without and with `--features gui` (the pinned libcosmic checkout resolves
from the local cargo cache; no network needed).

## 3. GUI compile

```
cargo check --offline --features gui
```

Clean. The libcosmic shell (Library / Inspect / Updates / Settings / About,
conflict/remove/unsafe/force dialogs) type-checks. A headless container has
no display server, so the GUI was verified by compilation plus a
line-by-line audit of every message path, not by screenshots.

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

Exit 0, isolated HOME.

## 5. Non-mutating CLI probes (isolated HOME)

```
HOME=/tmp/gosh-aim-qa-home ./target/debug/gosh-appimage-manager --version
# 3.0.0
HOME=/tmp/gosh-aim-qa-home ./target/debug/gosh-appimage-manager --list-installed --json
# {"installed":[],"schema_version":1}
HOME=/tmp/gosh-aim-qa-home ./target/debug/gosh-appimage-manager --list-updates --json
# {"schema_version":1,"updates":[]}
HOME=/tmp/gosh-aim-qa-home ./target/debug/gosh-appimage-manager --list-update-managers
# static github gitlab codeberg forgejo ftp
HOME=/tmp/gosh-aim-qa-home ./target/debug/gosh-appimage-manager --probe-host
```

```
host_spawn_program=true
host_spawn_exit=0
in_flatpak=false
managed_folder=/tmp/gosh-aim-qa-home/AppImages
HOST_PROBE_OK
```

All exit 0. Diagnostics go to stderr so stdout stays valid JSON.

## 6. GUI audit fixes (this session)

Five defects found by auditing every `Message` path in `src/gui.rs`; all
fixed in that file and re-verified with the gates above:

- Inspect "Replace UUID" input discarded keystrokes (`Noop`); now stored
  via `InspectReplaceUuidChanged` and passed to integrate.
- Settings update-source editor had inputs but no Apply/Remove buttons and
  a caption pointing at nonexistent per-row editors; Library rows now show
  the app UUID and the editor applies/removes sources with unknown-UUID
  feedback.
- Turning "Background update checks" off left the autostart entry behind;
  it now removes the entry first and fails closed on error.
- The autostart toggle displayed the background setting instead of the
  actual entry state; it now reads entry existence.
- The unsafe-extraction "Enable (warned)" button sent `Noop`; a real
  `UnsafeFallbackConfirm` message now enables it after the warning.

## 7. Not run in this environment

- Live GUI interaction/screenshots: no display server in this container.
  Needs one manual pass on a COSMIC session over the five fixed controls.
- Packaged smoke (`flatpak-builder --run build-dir
  packaging/com.goshapps.AppImageManager.yml gosh-appimage-manager
  --self-test`): `flatpak-builder` is not installed here.

## 8. Worktree

`git diff --stat` for this session touches `src/gui.rs`, `src/main.rs`
(dead-import scoping), and this record. `git status` shows no other
modifications; build products (`target/`, `build-dir-*/`, `repo-*/`,
`.flatpak-builder/`) remain ignored.
