# MIGRATION_AUDIT.md — Gosh AppImage Manager 3.0.0

Phase 1 (audit) of the "rewrite to Flutter + Dart + Rust" brief. This file
records how the existing application actually behaves, as established from
source, build output, and runtime checks on 2026-10-07. It does not start the
migration and does not authorize one.

- Baseline: `main` at `54e1c97`, clean working tree at the start.
- Changes made by this audit: this file only (untracked). No source,
  packaging, or existing documentation was edited. Nothing was committed,
  pushed, tagged, or published.
- Environment side effects: a stable Rust toolchain was installed with rustup
  into `~/.cargo` and `~/.rustup` (`--no-modify-path`; no shell profile was
  changed). A GUI test instance was started and stopped. Every runtime test
  used an isolated `HOME` in a temporary directory outside the repository. The owner's real
  `~/AppImages` folder was not read or written.

Status vocabulary follows the brief: WORKING, PARTIAL, BROKEN, DEAD, UNKNOWN,
NOT IMPLEMENTED, MIGRATED, VERIFIED. "VERIFIED" here means observed in this
audit. "WORKING" means supported by source reading only.

Evidence labels used throughout: **VERIFIED** (or **RUN** in section 13) =
observed by running the binary or a command in this audit; **READ** = confirmed
by reading the cited code in this audit; **SRC** = stated by a source-reading
review and not independently confirmed; **UNVERIFIED** = not checked.

## 0. Summary

1. **What the app is.** One Rust 3.0.0 binary (`gosh-appimage-manager`). The
   CLI is always compiled. The libcosmic GUI is behind `--features gui`. The
   core is the library crate `goshaim_core`, whose `controller` module exposes
   coarse operations. The app manages Linux AppImage files: inspect, integrate
   (copy into a managed folder and write a freedesktop menu entry), launch,
   update from six source types, and remove (to Trash by default).
2. **Build and test health (VERIFIED).** Debug CLI build, GUI build, test
   build, `cargo test --no-fail-fast` (160 passed, 0 failed), `cargo fmt
   --check`, `cargo clippy --all-targets -- -D warnings` (default and
   `--features gui`), `--self-test` (`SELF_TEST_OK`), desktop-file-validate,
   and `appstreamcli validate --pedantic` (passes; one pedantic note).
3. **Core CLI behavior (VERIFIED).** Integrate, conflict refusal, keep-both,
   Trash removal, and permanent delete behave as documented, in an isolated
   home (section 3).
4. **GUI behavior (PARTIAL).** The GUI launches and renders the Library and
   Inspect pages in dark mode under XWayland. Keyboard input reaches the app.
   Pointer input does not: synthetic clicks did not change the page in
   repeated tests. Settings, theme switching, light mode, dialogs, and every
   pointer-driven control are NOT VERIFIED in this audit.
5. **Findings that matter for the migration.**
   - The unsafe `--appimage-extract` fallback cannot execute anything. The
     Settings dialog still promises a "per-file confirmation" that does not
     exist (section 13, D-12).
   - DwarFS binaries are bundled and pinned in the Flatpak, but no code path
     invokes them (section 13, D-21).
   - The Flatpak grants `--talk-name=org.freedesktop.Flatpak`, which permits
     host execution (`flatpak-spawn --host`). The in-code allowlist is the only
     restriction (section 8).
   - An AppImage is a Linux executable format. Launching, desktop-entry
     integration, and the update path are Linux concepts. Windows and macOS
     have no direct equivalent of the core product (section 18).
6. **Decisions (section 19).** Direction chosen after the audit on 2026-10-07:
   Flutter UI on the Rust core, Linux first (`docs/flutter/ARCHITECTURE.md`,
   a proposal). `AGENTS.md` still mandates Rust + libcosmic, `cargo` + `just`,
   and the freedesktop 23.08 Flatpak SDK, so it must be amended at cut-over.
   The brief originally contradicted it. The product meaning on Windows and
   macOS needs an owner decision. Several verification prerequisites are
   absent here: Flutter/Dart, `flatpak-builder`, sudo, Windows and macOS hosts,
   and CI runners on those platforms.
7. **Defect register (section 13).** The source review and runtime checks
   recorded 50 defects: 6 High, 19 Medium, 23 Low and 2 informational. The High
   items are: a failed replace that restores the wrong desktop entry and icon
   (D-01); a rollback step that can delete user entries named `.gosh-*` (D-02);
   one lock held across long GUI jobs, so Cancel cannot work until the job ends
   (D-03); settings that probably fail to save inside the Flatpak (D-04,
   unverified); a vulnerable `rustls` (D-05); and embedded update information
   for GitHub-released AppImages, which never works (D-40, reproduced).

## 1. Environment and method

| Item | Observed value |
|---|---|
| Host | Fedora Linux Asahi Remix 44 (Workstation), aarch64, kernel 7.1.13 (16k pages) |
| Desktop session | GNOME Shell 50.5 on Wayland, with XWayland (`DISPLAY=:0`) |
| Rust toolchain (installed for this audit) | rustc / cargo 1.99.0 stable, clippy 0.1.99, rustfmt 1.10.0 |
| Declared MSRV | `rust-version = "1.89"` (`Cargo.toml`). Not tested by this audit. Flatpak pins Rust 1.90.0 |
| Flatpak tooling | `flatpak` 1.18.4 present; `flatpak-builder` absent |
| Flutter / Dart | Absent |
| GTK 3 development files | 3.24.52 present |
| `appstreamcli`, `desktop-file-validate` | Present |
| Privilege | `sudo` requires a password; no system packages could be installed |
| GUI automation | `Xvfb`, `xdotool`, `wtype`, `ydotool` absent. GNOME Shell screenshot D-Bus call denied. AT-SPI disabled at desktop level (`toolkit-accessibility=false`), so the app is not in the accessibility tree |

The project's own GUI smoke test (`tools/gui-smoke.sh`) needs Xvfb and
xdotool, so it could not run here.

### 1.1 Commands run and results

| Check | Command | Result |
|---|---|---|
| CLI build | `cargo build --locked` | Pass (30 s) |
| GUI build | `cargo build --locked --features gui` | Pass; libcosmic v0.12 (rev `9c62f19e`); no warnings |
| Test build | `cargo test --locked --no-run` | Pass |
| Full tests | `cargo test --locked --no-fail-fast` | 160 passed, 0 failed, 0 ignored (7 unit tests in `src/`, 153 in `tests/`) |
| Format | `cargo fmt --check` | Pass |
| Lint, default | `cargo clippy --locked --all-targets -- -D warnings` | Pass |
| Lint, GUI | `cargo clippy --locked --features gui --all-targets -- -D warnings` | Pass |
| Self-test | `gosh-appimage-manager --self-test` (isolated HOME) | `SELF_TEST_OK`, exit 0 |
| Desktop entry | `desktop-file-validate data/com.goshapps.AppImageManager.desktop` | Pass |
| AppStream | `appstreamcli validate --pedantic --no-net …metainfo.xml` | Pass; one pedantic note: `cid-contains-uppercase-letter` on `com.goshapps.AppImageManager`. That ID is also the Flatpak app ID, so it is recorded and not changed |
| Flatpak build | `just flatpak-x86_64` | Not run: `flatpak-builder` absent |
| Release pipeline | `.github/workflows/release.yml` | Not run (tag-driven; publishes a GitHub Release) |

## 2. Test-fixture method

No real AppImage was used. Synthetic files were created in a temporary
directory outside the repository, following the repository's own fixture generator
(`inspector::make_test_elf`, `tests/common.rs::write_fixture`):

- `Demo-app-1.0.0-aarch64.AppImage`: 128-byte ELF64 (little-endian, machine
  `EM_AARCH64`) with `AI\x02` at offsets 8–10. It is never executed by the
  tests, and the CLI runs below never execute it either.
- `not-an-appimage.txt`: plain text, for the rejection path.
- `Upd-app-1.0.0-aarch64.AppImage`: 360-byte AArch64 type-2 file with an
  `.upd_info` ELF section containing
  `gh-releases-zsync|example-user|example-app|latest|…AppImage.zsync`. Used to
  test embedded update information (section 3.1).

## 3. Verified runtime behavior (CLI, isolated HOME)

Every command below ran with `HOME` and `GOSHAIM_HOME` set to a fresh scratch
directory and `stdin` closed.

| Step | Command | Observed | Exit |
|---|---|---|---|
| Probe | `--probe-inspect <fixture>` | JSON with `"type":"type-2"`, `"architecture":"aarch64"`, `"magic_valid":true`, SHA-256, `"name":""`, and the marker line `INSPECT_NO_EXECUTION` | 0 |
| Integrate | `--integrate <fixture> --yes` | `Integrated <HOME>/AppImages/Demo-app-1.0.0-aarch64.AppImage`. The source file is left in place (copy, not move) | 0 |
| List | `--list-installed --json` | `schema_version: 1`, one `installed` entry with `owned: true`, a UUID, and `desktop_id` `gosh-appimage-<uuid>.desktop` | 0 |
| Conflict | integrate the same file again | `Name conflict requires keep-both or replace` | 6 |
| Non-ELF | `--integrate not-an-appimage.txt --yes` | `Not an ELF file` | **1** (the README lists 6 as "validation") |
| Keep both | `--integrate <fixture> --keep-both --yes` | `…-aarch64-1.AppImage` created; second registry row | 0 |
| Remove (default) | `--remove <path> --yes` | File moved to `~/.local/share/Trash/files/` with a `.trashinfo` entry | 0 |
| Remove again | same path | `Not an owned managed AppImage` | 1 |
| Permanent delete | `--remove <path> --yes --delete` | File deleted with no second prompt; registry row removed | 0 |

Final state of the isolated home: registry empty; `Trash/files` holds the
trashed copy; the generated `.desktop` files were removed with their apps; and
`~/.local/share/applications/mimeinfo.cache` remained. That file is created by
`update-desktop-database`, which the integrate step runs and which nothing
removes afterwards.

Other probes (isolated HOME): `--version` → `3.0.0`; `--list-update-managers`
→ `static github gitlab codeberg forgejo ftp`; `--list-installed --json` on an
empty registry → `{"installed":[],"schema_version":1}`; `--probe-host` →
`HOST_PROBE_OK` with `in_flatpak=false`.

No-argument run in a CLI-only build prints `This build has no GUI` and exits
2, as the `gui-smoke.sh` guard expects.

### 3.1 Additional runtime checks (CLI, isolated home)

| Check | Command | Observed | Exit |
|---|---|---|---|
| Legacy import, valid and empty-UUID entries | `--list-installed --json` after writing a 2.x `registry.json` | Valid entry imported with its UUID; empty-UUID entry skipped; second run shows one entry; legacy file checksum unchanged | 0 |
| Destructive command without a TTY or `--yes` | `--remove <path>` with stdin closed | `Refusing destructive action without a TTY; pass --yes`; file kept | 5 |
| Loopback static source over plain http | `--set-update-source … url=http://127.0.0.1:9/… allow_local_network=true` | `URL rejected: plain http is not allowed; use https` | 6 |
| Loopback static source over https, with the opt-in | same, `https://127.0.0.1:9/…` | `URL rejected: local-network destinations need an explicit opt-in`. The opt-in is ignored at set time (D-09) | 6 |
| Check failure reported by `--list-updates` | static source at `https://nonexistent.invalid/…` (DNS failure) | `update check failed (static): DNS resolution failed …` | 8 |
| Same check failure under `--update --all --yes` | same | No message at all | **0** (D-08) |
| Embedded `gh-releases-zsync` update information | integrate a fixture whose `.upd_info` names a GitHub release, then `--list-updates` and `--update` | `update check failed (gh-releases-zsync): Unknown update manager: gh-releases-zsync`; `--update` prints `Unknown update manager: gh-releases-zsync` | 8 and 1 (D-40) |
| Login-check entry, non-mutating | `--probe-autostart` | Rendered entry with `Exec=<binary> --fetch-updates`; `autostart_installed=false`; no file written | 0 |

## 4. Verified GUI behavior (partial)

The GUI was built with `--features gui` and started with `HOME` and
`GOSHAIM_HOME` isolated. Two launch modes were tried:

- **Native Wayland** (default in this session): the process started and ran,
  but no X window existed, so nothing could be captured. Whether a Wayland
  window appeared on the desktop was not verified.
- **X11 backend under XWayland** (`WINIT_UNIX_BACKEND=x11`, `WAYLAND_DISPLAY`
  unset): the window `Gosh AppImage Manager — Library`, 2048×1536, was
  captured with ImageMagick `import`.

Observed on the X11 path:

- **Library** (dark; the session's colour scheme is `prefer-dark` and the app's Appearance was left at its default, System): left nav with six
  entries (Library, Inspect, Updates, Tasks, Settings, About). Header with a
  logo button. Page title, Refresh button, search field ("Search by name,
  version or path"), and sort chips Name / Version / Updates first. Empty
  state "No AppImages yet" with the copy "Open one from the Inspect page.
  Opening a file never integrates or executes it."
- **Inspect** (reached through keyboard focus, see below): title "Inspect an
  AppImage", subtitle "Opening a file only inspects it. Nothing is integrated
  or executed.", a "Path to .AppImage" field, "Browse…" and "Inspect" buttons.
  The header logo button shows a focus ring.
- **Keyboard**: after F5 and then Tab, the rendered page was Inspect. Which of
  the two keys caused the change was not isolated. A later Shift press was
  not sent (its keycode lookup failed).
- **Pointer**: a warped pointer produced hover highlights, but held
  press/release events (XTEST) did not change the page. One click on
  "Library" while Inspect was active left the frame byte-identical (ImageMagick
  `compare`: 0 differing pixels). Pointer-driven controls therefore remain
  NOT VERIFIED.

Not observed in this audit: Updates, Tasks, Settings, About, the light theme,
dialogs, drag and drop, the Integrate path, the Details page, and the native
Wayland window.

## 5. Architecture and source tree

One Cargo package, `gosh-appimage-manager` 3.0.0 (edition 2021,
`rust-version = "1.89"`), with two targets: the library `goshaim_core`
(`src/lib.rs`) and the binary `gosh-appimage-manager` (`src/main.rs`).

| Feature set | Contents |
|---|---|
| default (empty) | CLI only. No GUI dependencies compiled |
| `gui` | libcosmic (git tag `v0.12`, locked at `9c62f19e`), tokio. The Flatpak ships this feature |

`Cargo.toml` also pins `cosmic-text` to crates.io `=0.13.2` through `[patch]`,
because the libcosmic branch it tracks has moved API.

### 5.1 Source map (line counts measured in this audit)

Total: `src/` 12,845 lines in 29 files; `tests/` 4,552 lines in 24 files.

| File | Lines | Role | UI-coupled |
|---|---:|---|---|
| `gui.rs` | 2,704 | libcosmic application: pages, dialogs, messages, background jobs | Yes (feature `gui`) |
| `cli.rs` | 1,010 | CLI commands, JSON output, probes, self-test, exit codes | No (text output) |
| `updates_sources.rs` | 965 | six update managers, config validation, embedded descriptors | No |
| `inspector.rs` | 855 | inspection pipeline, extraction through `unsquashfs` and `7zz`, unsafe-fallback gate | No |
| `network.rs` | 716 | HTTP and FTP clients, DNS pinning, redirect and size limits | No |
| `integration.rs` | 634 | integrate transaction: copy or move, conflicts, desktop and icon, rollback | No |
| `registry.rs` | 629 | SQLite registry; legacy `registry.json` import | No |
| `updates_service.rs` | 511 | check and apply updates; staging and verification | No |
| `controller.rs` | 488 | `AppController`: the coarse operations the frontends call | No |
| `types.rs` | 446 | shared types: `InstalledApp`, requests, results, `ExitCode`, fail points | No |
| `process.rs` | 427 | process runner; host-helper allowlist; `flatpak-spawn` | No |
| `desktop.rs` | 396 | `.desktop` writer and parser; ownership markers; `Exec` building; file-name sanitising | No |
| `safe_fs.rs` | 391 | atomic write; private temp directories; rollback temps | No |
| `settings.rs` | 346 | settings store (JSON); path roots (`Dirs`) | No |
| `i18n.rs` | 315 | catalogs and locale resolution | No |
| `elf.rs` | 316 | ELF and AppImage magic parsing | No |
| `proctable.rs` | 256 | running-process detection (`/proc`, host `pgrep`) | No |
| `removal.rs` | 222 | remove: Trash or permanent; protected paths | No |
| `library.rs` | 217 | discovery and adoption | No |
| `url_guard.rs` | 171 | URL policy: scheme, credentials, local-network rules | No |
| `tasks.rs` | 175 | task queue model (running and history) | Partly (state only) |
| `limits.rs` | 78 | size, count and timeout constants | No |
| `diagnostics.rs` | 77 | stderr diagnostics behind a switch | No |
| `drop_queue.rs` | 80 | planning for files opened or dropped into the window | Partly |
| `main.rs` | 186 | entry point: version, help, probes, CLI dispatch, GUI start | No |
| `launch.rs` | 100 | launch service: detached start, environment filter, NixOS shim | No |
| `trash.rs` | 63 | trash sink (freedesktop Trash through the `trash` crate) | No |
| `notifier.rs` | 38 | desktop notification after `--fetch-updates` | No |
| `lib.rs` | 33 | module list; `gui` module behind the feature | No |

### 5.2 Entry points

- `main.rs:61-68`: `--version`/`-V` and `--help`/`-h` exit first.
- `main.rs:70-146`: probes and `--self-test` (`--self-test` overrides other
  flags).
- `main.rs:76-115`: any CLI command selects the CLI (`cli.rs`). The first
  matching command wins, in this order: `--list-update-managers`,
  `--list-installed`, `--list-updates`, `--list-discovered`, `--adopt`,
  `--integrate`, `--update`, `--remove-all`, `--remove`,
  `--set-update-source`, `--fetch-updates`. No match prints "Unknown command"
  and exits 2 (`cli.rs:774`).
- `main.rs:149`: every other argument is a file path for the GUI. `--bogus` is
  therefore opened as a path (READ; not run).
- Default build without `gui`: prints the version, a no-GUI notice and help,
  then exits 2 (`main.rs:158-186`). VERIFIED by running (section 3).
- GUI start: `gui.rs:48` calls `cosmic::app::run::<App>(settings, Flags {
  initial_files })`. Failures print to stderr and exit 1 (`gui.rs:48-54`).

### 5.3 Core operations exposed by `AppController` (`controller.rs`)

The frontends call these public methods. They are coarse (one call per user
action), which suits a Flutter bridge.

`inspect_file`, `inspect_with`, `integrate`, `remove_app`, `adopt_external`,
`discover`, `reveal_in_file_manager`, `refresh_metadata`,
`set_arguments_and_environment`, `running_uuids`, `is_running`, `check_updates`,
`scan_updates`, `apply_update`, `set_update_source`, `unset_update_source`,
`sync_autostart`, `render_autostart_entry`, `readiness`, `models_ready`, plus
accessors (`settings`, `registry`, `tasks`, `notifier`, and so on).

Side effects go through injectable traits: `ProcessRunner`, `ProcessTable`,
`TrashSink`, `NetworkClient`. `AppController::with_seams` injects fakes, and the
test suite depends on that seam. Those traits are the reason the core can be
tested without a desktop.

### 5.4 State model

| State | Where it lives | Owner |
|---|---|---|
| Installed apps (records) | SQLite `apps` table (section 6) | `registry.rs` |
| Settings | `settings.json` (section 6) | `settings.rs` |
| Desktop entries, icons | XDG-style data directories, outside the registry | `desktop.rs`, `integration.rs` |
| Managed AppImages | managed folder (default `~/AppImages`) | `integration.rs`, `removal.rs` |
| Controller | `Arc<Mutex<AppController>>` shared by the UI and every worker (`gui.rs:274`, `gui.rs:300-306`) | `gui.rs` |
| GUI state | fields of `App` (`gui.rs:269-297`): page, detail selection, library cache, inspect result, dialog, queue, status, the single `busy` slot | `gui.rs` |
| UI messages | `Message` enum (`gui.rs:157-217`); `Outcome` enum for results (`gui.rs:113-155`) | `gui.rs` |

The GUI has no independent copy of registry data. It reloads the library from
the controller (`load_library`), so the registry is the single source of truth.

### 5.5 Threading and concurrency

- GUI jobs run on `tokio::task::spawn_blocking` (`gui.rs:313-336`). Each job
  gets an `Arc<AtomicBool>` cancel flag and returns `Message::Finished`.
- Each worker takes the controller mutex and keeps it for the whole job
  (inspect: `gui.rs:446-470`; integrate: `gui.rs:487-507`; update:
  `gui.rs:1441-1451`). The UI thread takes the same mutex for Cancel,
  Launch, Settings views and job spawn. Section 13 (D-03, D-16) describes the
  consequences.
- Core operations are synchronous and report no progress. The task model has
  a progress field that nothing updates (section 13, D-27).
- The CLI is single-threaded.

### 5.6 Dependencies (from `Cargo.toml`)

| Crate | Use |
|---|---|
| `serde`, `serde_json` | settings file; CLI JSON |
| `sha2`, `hex` | SHA-256 for inspection and update verification |
| `rusqlite` 0.31 (`bundled`) | registry |
| `reqwest` 0.12 (`blocking`, `rustls-tls`, no default features) | HTTP updates |
| `url`, `percent-encoding`, `uuid` (v4) | URLs; identifiers |
| `trash` 5 | freedesktop Trash |
| `notify-rust` 4 | desktop notifications (through `zbus`, verified in `Cargo.lock` by the CLI review) |
| `wait-timeout` 0.2 | process timeouts |
| `libcosmic` (optional, `gui`) | GUI toolkit: `tokio`, `winit`, `wgpu`, `a11y`, `xdg-portal`, `single-instance` |
| `tokio` (optional, `gui`) | blocking pool for GUI jobs |
| `tempfile` (dev) | tests |

Helper programs bundled into the Flatpak only (section 10): `unsquashfs`
(squashfs-tools 4.6.1, built from source), `7zz` (7-Zip 26.00), and
`dwarfsextract`/`dwarfsck` (DwarFS 0.15.3). The last two are never invoked (D-21).

## 6. Data formats and persistence

Paths below come from `settings.rs:18-32` (`Dirs::from_env`). `$HOME` is
`GOSHAIM_HOME` if set, then `HOME`, then `/root`. The data, config and cache
roots can be overridden only by `GOSHAIM_XDG_DATA_HOME`,
`GOSHAIM_XDG_CONFIG_HOME` and `GOSHAIM_XDG_CACHE_HOME`. The standard
`XDG_*_HOME` variables are not read. That contradicts the header comment at
`settings.rs:1` ("XDG-aware"); the README is correct on this point.

| Item | Path | Mode | Status |
|---|---|---|---|
| Managed folder | `~/AppImages` (setting `ManagedFolder`) | folder 0700 | VERIFIED: copy created, source left in place |
| Registry | `<data>/gosh-appimage-manager/registry.sqlite` | dir 0700, file 0600 | VERIFIED: created; file mode 600; journal mode `delete` |
| Legacy registry | `<data>/gosh-appimage-manager/registry.json` | not modified | VERIFIED: imported once; file checksum unchanged |
| Settings | `<config>/gosh-appimage-manager/settings.json` | 0600 | WORKING (source). Not created by any run here, because no setting was changed |
| Desktop entries | `<data>/applications/gosh-appimage-<uuid>.desktop` | 0644 | VERIFIED: created on integrate, removed on remove |
| Icons | `<data>/icons/hicolor/256x256/apps/gosh-appimage-<uuid>.{png,svg}` | 0644 | UNVERIFIED: the fixture has no icon |
| Login-check entry | `<config>/autostart/com.goshapps.AppImageManager-updates.desktop` | 0644 | VERIFIED: rendered by `--probe-autostart`, and no file was written |
| Trash | freedesktop Trash (`Trash/files`, `Trash/info/*.trashinfo`) | per trash crate | VERIFIED: trashed file and `.trashinfo` created |
| Extraction work | `$TMPDIR/gosh-appimage-manager/…` | private 0700 | WORKING (source) |
| Cache | `<cache>/gosh-appimage-manager` | — | Defined, never used (`settings.rs:228-230`) |

### 6.1 Registry schema (SQLite, `registry.rs:13-46`)

```text
meta(key TEXT PRIMARY KEY, value TEXT)          -- one row: schema_version = 1
apps(uuid TEXT PRIMARY KEY, name, version, comment, managed_path,
     desktop_id, desktop_path, icon_path, sha256, app_type, architecture,
     size, arguments, default_arguments, environment, update_manager,
     update_config, embedded_update, last_update_check, available_version,
     available_url, available_size, update_available, digest,
     reduced_verification, external_folder, owned, adopted, website,
     terminal, actions)                           -- 33 columns, all NOT NULL with defaults
```

- `uuid` is the primary key. `managed_path` is a lookup key with no UNIQUE
  constraint. No CHECK, foreign key, index or trigger exists (source).
- List-valued and map-valued fields (arguments, environment, update config,
  actions) are JSON text. Corrupt JSON in a row silently becomes empty
  (`registry.rs:86-100`, source).
- No `PRAGMA` and no busy timeout are set anywhere in `src/` (grep). The bundled
  SQLite defaults apply (rollback journal, observed as `delete`).
- The `last_update_check`, `available_*`, `update_available` and `digest`
  columns are mostly unused. The CLI review and the older inventory both flag
  them as vestigial.
- Fields that the `InstalledApp` struct carries but the schema does not store:
  `categories`, `mime_types`, `startup_wm_class`, `running` (source,
  `types.rs:285-290`). That gap causes defect D-13.

### 6.2 Legacy import (2.x `registry.json`)

- Read only when `registry.sqlite` does not exist yet (`registry.rs:277-293`).
  Verified: the import kept the valid entry's UUID and path, skipped an entry
  with an empty UUID, did not duplicate on a second run, and left
  `registry.json` byte-identical.
- Format: `{"schema_version": 1, "apps": [ {uuid, name, managed_path,
  desktop_id, owned, …} ]}`. Rejected: empty UUID or path, duplicate UUID,
  bodies over 2 MiB, and `schema_version` above 1 (`registry.rs:302-327`,
  source).
- Errors from the import are discarded (`registry.rs:291`). Once any mutation
  creates `registry.sqlite`, the legacy file is never imported again (D-35).

### 6.3 Settings file (`settings.rs:303-334`)

PascalCase keys, pretty-printed and sorted:

| Key | Type, default | Validation or note |
|---|---|---|
| `ManagedFolder` | string, `~/AppImages` | any string; absolute-path check only in the GUI (D-39) |
| `MoveSource` | bool, false | — |
| `ManageOutsideFolder` | bool, false | — |
| `TerminalOmitSuffix` | bool, false | — |
| `BackgroundUpdateChecks` | bool, false | read only by the GUI (D-14) |
| `UnsafeExtractionFallback` | bool, false | inert (section 14) |
| `DebugLogging` | bool, false | — |
| `Appearance` | `system`, `light` or `dark`; default `system` | unknown values become `system` (`types.rs:415-421`) |
| `MaxAppImageBytes` | integer, 8 GiB | clamped to [1 MiB, 32 GiB] (`settings.rs:293-295`; `limits.rs:48-50`) |

Corrupt or unreadable files are not handled safely: see defect D-07.

### 6.4 Desktop entries, icons and marker

- The desktop entry is written by `integration.rs` and `desktop.rs`. It carries a
  Desktop Entry body built from argument tokens, plus an ownership marker,
  `X-Gosh-AppImage-Id` (`limits.rs:43`, source). Ownership is checked on removal
  by UUID (`removal.rs:81-95`).
- Filename: `gosh-appimage-<uuid>.desktop`. The icon name follows the same UUID
  pattern. Names are UUID-based, so accidental collisions are unlikely (source;
  D-43).
- `update-desktop-database` runs after integration and creates
  `applications/mimeinfo.cache` (verified). Nothing removes that file later
  (D-37).

### 6.5 Managed folder contents on this machine (not touched)

The owner's real managed folder, `/home/gosh/AppImages`, holds three AppImages
(`claude.appimage`, `cursor.appimage`, `grok_bot.appimage`, about 680 MB in
total) and a `.icons/` subfolder. No `~/.local/share/gosh-appimage-manager`
and no `~/.config/gosh-appimage-manager` exist, so this account has no
registry or settings from the app. This audit listed the folder's metadata
only. It never opened, copied, executed or wrote any file in it.

## 7. Platform assumptions and Linux-specific integrations

The core is Linux-specific in more places than the code's structure suggests.
Windows and macOS would need a separate platform layer for each row below.

| Assumption | Where | Consequence off Linux |
|---|---|---|
| AppImage is a Linux ELF executable; magic and type detection | `elf.rs`, `inspector.rs` | The file format itself is Linux-specific. Launching cannot work elsewhere |
| XDG-style data and config directories under `HOME` | `settings.rs:18-32`, `desktop.rs` | Needs Windows Known Folders and macOS `Application Support` |
| freedesktop `.desktop` entries and hicolor icon theme | `integration.rs:512-560`, `desktop.rs` | No equivalent of menu entries on Windows or macOS |
| `update-desktop-database` refresh | `integration.rs:552-560` | Linux-only tool |
| freedesktop Trash, plus `gio trash` on the host as fallback | `trash.rs`, `removal.rs:37-52` | Trash differs per OS |
| `/proc` scan for running executables | `proctable.rs:60-140` | Linux-only; needs native process APIs elsewhere |
| `pgrep -a -f .AppImage` on the host, through `flatpak-spawn` | `proctable.rs:92-108` | Linux-only, and Flatpak-specific |
| `flatpak-spawn --host` for launch and host checks | `process.rs:116-145` (`in_flatpak` checks `/.flatpak-info` or `FLATPAK_ID`) | Flatpak-specific |
| `xdg-open` to reveal a folder | `controller.rs:200-214` | Opens the parent folder, not the file; per-OS equivalents needed |
| `/etc/NIXOS` check and `appimage-run` shim | `launch.rs:24-56` | NixOS-specific workaround |
| Login-item entry (`autostart`) | `controller.rs:410-443`, `settings.rs:240-243` | Needs LaunchAgents (macOS) and a Run key or Startup folder (Windows) |
| D-Bus notifications (`notify-rust` → `zbus`) | `notifier.rs`, `Cargo.toml` | Needs per-OS notification APIs |
| Fixed i18n search paths `/app/share/...`, `/usr/share/...`, relative `i18n` | `i18n.rs:192-201` | Linux install layout; the relative path depends on the working directory (D-30) |
| Desktop file: `Exec=gosh-appimage-manager %F`, MIME list, `StartupWMClass` | `data/com.goshapps.AppImageManager.desktop` | freedesktop-only metadata |
| libcosmic features `winit`, `wgpu`, `xdg-portal`, `a11y`, `single-instance` | `Cargo.toml` | Portal-based file dialogs are Flatpak-friendly; other OSes need their own dialogs |
| CI and release on Ubuntu runners only | `.github/workflows/*.yml` | No Windows or macOS build exists (section 10) |

## 8. External processes and command execution

Every external program the core can start is listed below. All requests use
the `ProcessRunner` trait with an argument array, never a shell.

- A search for `sh` and `bash` found no shell invocation (verified). The only
  process spawn is `Command::new(&program)` (`process.rs:171`, source).
- The host-helper allowlist is `HOST_HELPERS` (`process.rs:37-46`): `gio`,
  `pgrep`, `xdg-open`, `update-desktop-database`, `appimage-run`, `true`.
  Anything else requested with `HostSpawn::Helper` is refused.
- Process hygiene (`process.rs:182-201, 345-368`): stdin is null, each child
  calls `setsid` before exec, a reaper thread collects exit status, and output
  is capped. Launch is start-only: the manager never waits for or kills the
  app (`launch.rs`, source).

| Program | Caller | Arguments | Purpose | Status and notes |
|---|---|---|---|---|
| The managed AppImage itself, `--appimage-extract` | `inspector.rs:205-206` (`extract_via_appimage`) | `[--appimage-extract]`, cwd in a private temp dir | Unsafe fallback for metadata | UNREACHABLE: needs `confirm_unsafe_extract`, which no production code sets to true (`inspector.rs:162`; `gui.rs:456`; `cli.rs:908`; `integration.rs:63`). READ |
| `unsquashfs` (`-l`, then extraction of the icon and desktop file) | `inspector.rs:365-367, 413-415` | argv | Metadata for type-2 AppImages | WORKING by tests with canned output. Bundled in the Flatpak; not in the tarball (D-22) |
| `7zz` | `inspector.rs:369-371, 423-425` | argv | Metadata for type-1 AppImages | WORKING by tests with canned output. Bundled in the Flatpak; not in the tarball |
| `dwarfsextract` | `inspector.rs:276-284` (dispatch) | — | DwarFS | BROKEN: no lister arm, so it always fails with "No safe lister" (D-21) |
| `appimage-run` | `launch.rs:41-42` | probe | NixOS launch shim | Probe with a 3 s limit (source) |
| The managed AppImage (launch) | `launch.rs:57-85` | `arguments` from the registry; environment filtered | Start the app | WORKING by tests. Start-only and detached. In the Flatpak the start goes through `flatpak-spawn --host` (`process.rs:130-145`) |
| `update-desktop-database` | `integration.rs:552-560` | argv | Refresh the desktop database | Best effort. VERIFIED: creates `mimeinfo.cache` |
| `gio trash <path>` | `removal.rs:37-52` | argv, host helper | Trash fallback | Runs only after native trash fails. Not exercised at runtime |
| `xdg-open <parent folder>` | `controller.rs:200-214` | argv | Reveal in file manager | Opens the folder; does not select the file (D-33) |
| `pgrep -a -f .AppImage` | `proctable.rs:92-108` | argv, host helper, 5 s limit | Running detection from inside the Flatpak | Output parsed by exact path match |
| `pgrep` (second call) | `proctable.rs:167-168` | argv | Running detection, other path | Source |
| `true` | `cli.rs:869-870` (`--probe-host`) | none | Verifies host spawning | VERIFIED: `host_spawn_exit=0`, `HOST_PROBE_OK` |

Notes on the Flatpak grant behind all host execution:

- The manifest grants `--talk-name=org.freedesktop.Flatpak`, which lets the app
  run programs on the host. The in-code allowlist above is the only limit. The
  README says so (section "Sandbox grants"). The host-spawn gate checks only that
  the path is absolute and is a regular file (`process.rs:77-95`, source). It
  does not check that the path is a registered AppImage (D-50).
- Trust: a registry row can point at any absolute file. Rows come from the
  user's own registry, adoption, or the legacy import. Exploiting this needs a
  local attacker who can already edit the user's data, so the impact is
  limited. It still means the sandbox does not confine host execution.

## 9. Networking and update sources

Evidence for this section comes from the source review of `network.rs`,
`url_guard.rs`, `updates_sources.rs`, `updates_service.rs`, `inspector.rs` and
`elf.rs`, plus the runtime checks named below. No live download was performed.
The only network activity in this audit was a DNS lookup of a `.invalid` name.

### 9.1 HTTP and FTP clients (`network.rs`)

- `reqwest` 0.12 in blocking mode, `rustls-tls`, default features off
  (`Cargo.toml:32`). Certificates come from bundled webpki roots, not the OS trust
  store. Environment proxies are honoured by default (source).
- Timeouts: a 30 s total timeout on the HTTP client (`network.rs:192`,
  `limits::NETWORK_TIMEOUT_MS`) and a 10 s connect timeout (`network.rs:193`).
  The total timeout covers the response body, so a download that runs past 30 s
  fails, even though the size cap allows files up to 8 GiB (D-46).
- Redirects: at most 3 hops. Each hop passes the redirect policy
  (`url_guard::check_redirect`), and the final URL is checked again
  (`network.rs:161-183, 237-246`, source).
- DNS pinning: the first address that DNS returns for the origin host is checked,
  then pinned for that connection (`network.rs:138-153`, read). Redirect targets
  are checked but not pinned, so reqwest resolves them again at connect time.
  Environment proxies bypass the pin.
- Local-network opt-in: `allow_local_network` is read only at
  `network.rs:43-48` (`true`, `yes` or `1`). The embedded metadata cannot set it
  (source, `updates_service.rs:110-115`). Set-time validation of static URLs
  ignores it (`updates_sources.rs:134`), so the documented opt-in cannot be used
  to configure a loopback static source (VERIFIED at runtime, D-09).
- Size caps: JSON responses 2 MiB (`network.rs:330`); zsync files 256 KiB; HTTP
  downloads stream and are capped during the transfer. FTP downloads are buffered
  in memory up to the same cap (`network.rs:390-391`, source; D-49).
- FTP is plaintext, with a warning on stderr only. It logs in anonymously with a
  fixed password (`network.rs:527`, source). The data connection is pinned to the
  control peer (`network.rs:576-585`, source).

### 9.2 URL policy (`url_guard.rs`)

- `https` is accepted. `http` is accepted only when the caller passes
  `allow_http` (`url_guard.rs:26-40`, read). Every HTTP caller passes `false`;
  only the FTP path passes `true` (`network.rs:419`, read).
- `ftp` is accepted by `validate` for every caller. The `ftp` arm has no rejection
  (`url_guard.rs:40-44`, read). A release document or zsync file that names an
  `ftp://` download URL is therefore accepted (D-41).
- Credentials in URLs and control characters are refused (`url_guard.rs:34-52`,
  read). Local-network destinations are refused unless the caller opts in
  (`url_guard.rs:56-66`, read).
- Private and reserved ranges (`url_guard.rs:77-130`, source): IPv4 loopback,
  link-local, RFC 1918, unspecified, broadcast, documentation, 100.64/10,
  192.0.0.0/24, 198.18/15; IPv6 loopback, unspecified, `fc00::/7`, `fe80::/10`,
  mapped and NAT64 forms; `localhost`, `*.localhost`, `*.local`, `*.internal`.
  Not covered: multicast, 240/4, 0/8, `fec0::/10`, 6to4 and Teredo.

### 9.3 Update managers (`updates_sources.rs`)

| Manager | Required keys | Endpoint pattern | Asset hosts | Notes |
|---|---|---|---|---|
| `static` | `url` (https); `version` in config, or a `Version:` line in the zsync file | the URL itself | any https URL, validated with `allow_private = false` at set time | Set-time loopback refusal (D-09) |
| `github` | `username`, `repo`, `filename` | `api.github.com/repos/{user}/{repo}/releases/latest` | `github.com`, `*.githubusercontent.com` | Assets matched by filename glob, limited to 256 and 512 characters |
| `gitlab` | `project`; `host` defaults to `gitlab.com` | `{host}/api/v4/projects/{project}/releases`, with a package fallback | the forge host | Private hosts refused unconditionally (`updates_sources.rs:498-500`); package results carry an empty version and are never offered (D-44) |
| `codeberg` | `owner`, `repo` | `{host}/api/v1/repos/{owner}/{repo}/releases?limit=1` | the forge host | — |
| `forgejo` | `host`, `owner`, `repo` | same shape as codeberg | the configured host | The default host `forgejo` is not a real host (inventory note) |
| `ftp` | `url` (`ftp://`), `version` | the URL | `ftp` (plaintext) | Legacy; credentials refused (`url_guard.rs`) |

The manager list and keys come from `updates_sources.rs:92-965` (source). The
`--list-update-managers` output lists all six names (VERIFIED).

### 9.4 Embedded update information (`.upd_info`)

- The reader takes the `.upd_info` ELF section, inside the first 1 MiB and under
  4 KiB, when it starts with `gh-releases-zsync|`, `zsync|` or `bintray-zsync|`
  (`elf.rs:188-238`, source). Other prefixes are dropped silently (source).
- `gh-releases-zsync|user|repo|release|filename` is classified with the hint
  `gh-releases-zsync` (`inspector.rs:762-781`, read), and that hint is stored as
  the app's update manager name (`integration.rs:262-263`, read). The factory has
  no manager of that name (`updates_sources.rs:919-937`, read), so every check
  fails with `Unknown update manager: gh-releases-zsync` (`updates_service.rs:109`,
  read). **VERIFIED at runtime** with a synthetic AArch64 AppImage whose
  `.upd_info` names a GitHub release (section 3.1; D-40). The README says embedded
  information is "picked up automatically"; that is false for this, the most common
  GitHub form.
- `zsync|URL` is classified as `static` with the second field as the URL
  (`inspector.rs:781-790`, source). It works when the zsync file or the config
  supplies a version.
- `bintray-zsync|…` passes the whole string as the URL (`inspector.rs:797`,
  source), so the check fails to parse it.
- `gitlab`, `codeberg` and `forgejo` embedded forms: `config_from_embedded` reads
  keys that `parse_upd_info` never writes (it writes `field0…n`), so those forms
  are configurable by the user only (source).
- The zsync parser reads `Filename:`, `Version:` and the first URL only. There is
  no `Length`, `Blocksize` or SHA-1 handling, so no delta updates and no zsync
  checksum (`updates_sources.rs:228-247`, source).

### 9.5 Update check and apply (`updates_service.rs`)

- Version comparison is string inequality (`updates_service.rs:262-269`,
  `updates_sources.rs:190, 214`, read). A downgrade is offered and applied when the
  strings differ. `v1.2.3` and `1.2.3` compare unequal, so the offer repeats
  forever. GitHub, GitLab and Forgejo set `available = true` unconditionally
  (source; D-44).
- Apply sequence (`updates_service.rs:237-453`, source): owned check → check →
  version present and different → running check (refused unless forced) → stage
  `sibling_temp(live, ".gosh-upd-")` in the same directory → download, capped at
  `max_bytes` → `chmod 0755` → ELF header, type and architecture checks (the
  architecture must be supported and equal to the installed one) → SHA-256 when a
  digest is advertised → hard-link backup → `rename(2)` over the live file →
  registry and desktop-entry rewrite → registry upsert → backup removed. Failures
  after the swap move the backup back and restore the registry snapshot; a failed
  restore is ignored.
- Staging name: predictable, and created `0600` without `O_EXCL` or `O_NOFOLLOW`
  (`network.rs:90-96`, source; D-24).
- Ownership: apply trusts the registry `owned` flag, and legacy rows default to
  `true` (`registry.rs:190`, read). The desktop entry is rewritten without
  `verify_ownership` (`updates_service.rs:421-428`, source; D-43). AGENTS.md
  requires verified ownership before any overwrite.
- Running check (read): `is_running` returns a bool built from
  `pids_for_executable`. In the Flatpak, if the host probe is refused, times out or
  exits with status 2 or more, the code falls back to a `/proc` scan. That scan is
  blind inside the sandbox, so the result is "not running", and the apply proceeds
  without `--force` (`proctable.rs:186-200` and `updates_service.rs:272-276`, both
  read; D-42).
- Digests: `sha256:<64 hex>` and bare 64-hex are accepted. A mismatch fails. A
  non-SHA-256 digest fails closed (`updates_service.rs:58-78, 354-376`, source).
  Without a digest, the update proceeds with reduced verification, and the flag is
  stored (source). The digest comes from the same response as the URL, so it
  detects corruption but not a malicious release. Payload checks stop at the ELF
  header and architecture; the payload's own version is never compared (D-45).

### 9.6 What was verified for this area

| Claim | Status |
|---|---|
| `gh-releases-zsync|` embedded updates fail on every check | VERIFIED at runtime (D-40) |
| Documented `allow_local_network=true` rejected for static sources at set time | VERIFIED at runtime (D-09) |
| Check failures reported by `--list-updates` (exit 8) | VERIFIED at runtime |
| `--update --all` exits 0 when every check failed | VERIFIED at runtime (D-08) |
| `ftp://` accepted by the URL policy for every caller | READ (D-41) |
| Running check falls open inside the Flatpak | READ (D-42) |
| Spawn failure reported as exit 0 (helper missing) | READ: `process.rs:243-249` sets `refused` and leaves `exit_code` at 0; `inspector.rs:389` checks only `exit_code` (D-22) |
| Live downloads, redirects, FTP transfers, digest flows | NOT RUN |
| The 30 s total timeout on real downloads | NOT RUN (D-46) |

## 10. Packaging, CI and release

### 10.1 Flatpak manifest (`packaging/com.goshapps.AppImageManager.yml`, read)

- Runtime `org.freedesktop.Platform` 23.08 with SDK `org.freedesktop.Sdk` and
  the `org.freedesktop.Sdk.Extension.rust-stable` extension.
- Modules: `rust-gosh` (Rust 1.90.0, SHA-256 pinned per architecture, removed from
  the final image); `unsquashfs` (squashfs-tools 4.6.1 built from source); `7zip`
  (7-Zip 26.00 binaries per architecture); `dwarfs` (0.15.3 binaries for
  `dwarfsextract` and `dwarfsck`, neither used by the code, D-21); and
  `gosh-appimage-manager` (offline cargo build with `--features gui`, installs the
  desktop file, metainfo, icons, catalogs and licences, and runs `--self-test`
  during the build).
- `finish-args`: `--share=ipc`, `--socket=fallback-x11`, `--socket=wayland`,
  `--device=dri`, `--share=network`, `--talk-name=org.freedesktop.Flatpak`,
  `--talk-name=org.freedesktop.portal.Desktop`,
  `--talk-name=org.freedesktop.Notifications`,
  `--own-name=com.goshapps.AppImageManager`, `--filesystem=~/AppImages:create`,
  `--filesystem=~/.local/share/applications:create`,
  `--filesystem=~/.local/share/icons:create`,
  `--filesystem=~/.local/share/gosh-appimage-manager:create`,
  `--filesystem=xdg-config/autostart:create`.
- Not granted: `~/.config/gosh-appimage-manager`, where settings are written
  (D-04). No `host` or `home` filesystem access.
- `--talk-name=org.freedesktop.Flatpak` allows host execution. The in-code
  allowlist is the only limit (section 8).

### 10.2 Tarball and install script

- `scripts/package-release.sh` writes
  `gosh-appimage-manager-<ver>-linux-<arch>.tar.gz`. It contains the binary,
  desktop file, metainfo, icons, catalogs, `COPYING`, the third-party licence
  texts, `README.md` and `install.sh` (read).
- The tarball contains no extraction helpers (`unsquashfs`, `7zz`). Without them,
  metadata extraction fails and the failure is reported as "No desktop entry
  found" (D-22). README says the tarball holds "the binary, desktop integration
  files, icons, licenses, and an install.sh".
- `packaging/install.sh` installs under `PREFIX` (default `/usr/local`) and does
  not verify the architecture (read).

### 10.3 CI and release workflows (read)

- `ci.yml` (push to `main`, pull requests, dispatch): runner `ubuntu-24.04` only.
  Steps: `cargo fmt --check`; clippy for default and `--features gui` with
  `-D warnings`; `cargo build --features gui`; `cargo test`; `--self-test`.
- `flatpak.yml` (push to `main`, pull requests, dispatch, `workflow_call`): x86_64
  on `ubuntu-24.04`, aarch64 on `ubuntu-24.04-arm`. Builds the bundle and uploads
  it as an artifact.
- `release.yml` (tags `v*`): version check (`scripts/check-version.sh` compares the
  tag with `Cargo.toml` and the newest metainfo release); tarballs for both
  architectures; the Flatpak through the reusable workflow; a GitHub Release
  created or updated with `gh release create` and `upload --clobber`; and
  verification with `scripts/verify-release.sh`.
- Recent runs (read-only `gh run list`): CI, Flatpak and Release all report
  `success`; the latest are dated 2026-09-13 (VERIFIED).
- No Windows or macOS runner, and no installer or bundle format for either
  platform (`.app`, `.dmg`, MSI, MSIX) is produced (READ of the workflows).

### 10.4 Published release (read-only check)

- `v3.0.0` was published 2026-09-13T22:13:28Z, marked Latest, not a draft and not a
  prerelease (VERIFIED with `gh release view`).
- Assets: `gosh-appimage-manager-3.0.0-linux-x86_64.flatpak`,
  `…-linux-aarch64.flatpak`, `…-linux-x86_64.tar.gz`, `…-linux-aarch64.tar.gz`,
  and `SHA256SUMS`. No other platform artifact is attached.
- The aarch64 Flatpak was downloaded for this audit and its SHA-256 matches the
  published `SHA256SUMS` line (VERIFIED). The bundle was not installed (section 19).

### 10.5 Build gates and validators (VERIFIED)

| Gate | Result |
|---|---|
| `desktop-file-validate` | pass |
| `appstreamcli validate --pedantic --no-net` | pass; one pedantic note: uppercase letters in the component ID, which is also the Flatpak app ID, so it was left unchanged |
| `just build`, `build-gui`, `lint`, `fmt-check`, `validate` (just 1.58.0) | all pass |
| `just self-test` | not run. The recipe writes to the real data directory. The same binary was run with an isolated `HOME` (section 3) |
| `cargo audit` | Before the fix: 1 vulnerability (RUSTSEC-2026-0285, `rustls` 0.23.43; D-05) and 9 unmaintained and 3 unsound warnings (D-34). After the fix (section 22): 0 vulnerabilities, same warnings |

### 10.6 Not performed here

`flatpak-builder` (absent); the sandbox run of the bundle (the 23.08 runtime, SDK
and rust-stable extension are not installed); a tarball install test; the release
workflow (it publishes; an audit must not run it); Windows and macOS builds (no
hosts available).

## 11. Tests

- `cargo test --locked --no-fail-fast`: 160 passed, 0 failed, 0 ignored (VERIFIED).
  Count by `#[test]`: 7 in `src/` and 153 in `tests/`.
- Harness: `tests/common.rs` (310 lines) provides a temporary directory, shared
  fakes for the process runner, process table, trash and network, and a synthetic
  ELF writer. The tests do not touch the real home directory (source).
- Covered (test headers and source review): CLI exit codes and JSON schema;
  probes; registry round trip and legacy import; integration conflicts, ownership
  and rollback at injected failure points; removal (Trash first, protected paths,
  partial results); update flows against fake HTTP (digests, architecture refusal,
  oversize, cancel cleanup, running guard); SSRF checks for hostnames that resolve
  to loopback (conditional); FTP against an in-process RFC 959 server; i18n
  catalogs and the pseudolocale; inspector gating; detached launch with no zombies;
  settings corruption (checks only the state before any change).
- Not covered (VERIFIED or source): the GUI (`gui.rs` has no tests; only
  `drop_queue.rs` has unit tests); real extractor binaries (canned output only);
  live HTTP endpoints; redirect hops and the HTTP DNS pin; the replace-rollback
  defect (the two fixtures are byte-identical, D-01); the `--update --all` exit code
  (D-08); the set-time opt-in (D-09); `gh-releases-zsync` embedded metadata (D-40,
  found by running, not by any test); the settings overwrite after a load error
  (D-07); Flatpak; Windows; macOS.
- Test quality notes (source): `test_ssrf.rs` returns early and passes vacuously
  unless the probe name resolves to loopback (D-47). `test_inspector.rs` sets
  `confirm = true` with a fake runner, while a code comment says tests never set it
  (D-48).
- A perf-bound test, `bulk_removal_is_linear`, flaked on an earlier machine
  (`docs/migration/PARITY_BASELINE.md`). It passed in this run.

## 12. Feature inventory

Columns: **Status** (VERIFIED = observed in this audit; WORKING = supported by
tests or source reading only; PARTIAL; BROKEN; DEAD; UNVERIFIED = not checked;
NOT IMPLEMENTED). **Source** gives file and line. **Known issues** point to
section 13. **Proposed split** is a first-pass assignment for the migration
(Flutter = presentation and platform plugins; Rust = core). It is a proposal,
not a decision. **Migration status** is NOT STARTED for every row.

### 12.1 Shell and GUI (`gui` feature)

| ID | Feature | Status | Source | Known issues | Proposed split | Cross-platform |
|---|---|---|---|---|---|---|
| F-01 | Window and application shell (libcosmic; default 1024×768; minimum 420×420) | VERIFIED (launch and render under XWayland); native Wayland UNVERIFIED | `gui.rs:38-55, 631` | No window-state persistence; no single instance (D-28) | Flutter: window shell and persistence | Window APIs differ per OS |
| F-02 | Six-page navigation: Library, Inspect, Updates, Tasks, Settings, About | VERIFIED for Library and Inspect; WORKING for the rest | `gui.rs:61-69, 643-660, 737-740` | Keyboard navigation not checked | Flutter: routing | — |
| F-03 | Library list with search (name, version, path) and sort (name, version, updates first) | PARTIAL: empty state VERIFIED; list not exercised | `gui.rs:403-425, 1697-1723` | Version sort is string-based (D-26) | Flutter: list widgets. Rust: search and sort in one call | — |
| F-04 | Library row actions: Launch, Details, Trash | WORKING (source) | `gui.rs:897, 785, 925` | Launch on the UI thread (D-16); Trash without a running check (D-10) | Flutter: actions. Rust: launch and remove | Launch semantics differ per OS |
| F-05 | Detail page: provenance, argument and environment editor, update-source editor, reveal, refresh metadata, permanent delete | WORKING (source) | `gui.rs:1865-2021` | Single-line inputs cannot carry newlines (paste merges lines); reveal opens the parent folder (D-33); refresh drops desktop categories and MIME types (D-13) | Flutter: forms. Rust: record, validation, persistence | Desktop-entry parts are Linux-only |
| F-06 | Inspect page: path field, Browse (portal), Inspect (read-only) | Layout VERIFIED; inspection VERIFIED through the CLI; Browse UNVERIFIED | `gui.rs:2023-2094, 803-838` | Inspection hashes under the controller lock (D-03); Browse has no AppImage filter (`gui.rs:809`); Browse cancel shows an error | Flutter: file picker through a plugin. Rust: inspection | Pickers differ per OS; the Flatpak needs the portal |
| F-07 | Integrate after inspection; copy by default, move as an option | WORKING (copy VERIFIED through the CLI; move UNVERIFIED) | `gui.rs:475-507, 880-881`; `integration.rs:441-465` | Integrates whatever is in the path field without a second look; move trashes the original without a warning | Flutter: confirmation. Rust: transaction | Linux desktop integration only |
| F-08 | Conflict resolution: keep both, replace (owned rows only) | Keep-both VERIFIED; replace WORKING (source) but defective | `integration.rs:136-183`; `gui.rs:889-893` | Replace rollback is wrong (D-01); rollback clean-up is unsafe (D-02) | Flutter: dialog. Rust: policy and transaction | — |
| F-09 | Files opened from the command line or the file manager (`%F`), queued one at a time | WORKING (source) | `main.rs:149`; `gui.rs:709-729`; `drop_queue.rs` | Queue stops after an inspection that is not integrated (D-15) | Flutter: startup arguments and queue state. Rust: per-file operations | Argument conventions differ per OS |
| F-10 | Drag and drop of files onto the window | UNVERIFIED (code present) | `gui.rs:760-770`; `drop_queue.rs:19-43` | Starts only when idle; otherwise stalls like F-09 | Flutter: drop plugin (to evaluate). Rust: none | URI and path conversion differ per OS; Flatpak portal behaviour unknown |
| F-11 | Keyboard shortcuts: Ctrl+O browse, Ctrl+R and F5 refresh, Ctrl+F update check (not find), Esc dismiss | F5 input reached the app (VERIFIED); mapping matches the README (READ) | `gui.rs:744-758` | Lowercase letters only (D-25) | Flutter: Shortcuts and Actions | macOS needs Command mappings |
| F-12 | Updates page: check now, update one, update all, reduced-verification notes, running-app prompt | WORKING (source); not exercised | `gui.rs:1079-1155, 2096-2197` | Update all has no confirmation; applied offers stay listed (`self.updates` is written only by scan, `gui.rs:1523`) | Flutter: presentation. Rust: check and apply | — |
| F-13 | Tasks page: running and history, cancel, clear finished, progress | PARTIAL (source) | `gui.rs:2199-2242`; `tasks.rs` | Progress never moves (D-27); Cancel waits on the lock (D-03); several operations ignore the cancel flag | Flutter: task list. Rust: job model with progress and cancellation | — |
| F-14 | Appearance: System, Light, Dark; saved and applied live | WORKING (source); System tracking UNVERIFIED | `gui.rs:1271-1277, 364-371`; `settings.rs` | No selected-state indication | Flutter: ThemeMode. Persistence: decide between Dart and Rust | Native theme detection per OS |
| F-15 | Managed-folder setting | WORKING (source) | `settings.rs:282-285`; `gui.rs:1282-1291` | Any absolute path is accepted, including the home folder (D-39) | Flutter: folder picker. Rust: validation | Path rules per OS |
| F-16 | Maximum AppImage size setting (1–32768 MB; default 8192 MB) | WORKING (source; clamp) | `settings.rs:207-213, 293-295`; `limits.rs` | — | Flutter: input. Rust: limit | — |
| F-17 | Move-original toggle | WORKING (source) | `gui.rs:1222`; `integration.rs:441-465` | Trashes the source without a warning | Flutter: toggle and warning. Rust: transaction | — |
| F-18 | Discovery of AppImages outside the managed folder (opt-in) | WORKING (tests); GUI adoption skips inspection | `library.rs:63-157`; `gui.rs:1010` | Frontends differ (D-18) | Rust: discovery. Flutter: list | Linux desktop-entry discovery |
| F-19 | Drop the `.AppImage` suffix for terminal apps | WORKING (source) | `gui.rs:1234`; `controller.rs:255-260` | Takes effect only at the next desktop-entry write | Rust: entry generation | Linux only |
| F-20 | Verbose diagnostics on stderr | WORKING (tests) | `diagnostics.rs`; `gui.rs:1240-1247` | One switch and no levels; raw FTP URLs can be printed (D-38) | Rust: logging. Flutter: toggle | — |
| F-21 | Background update checks (notify only) toggle | PARTIAL | `gui.rs:1195, 2369-2371` | `--fetch-updates` ignores the setting (D-14) | Rust: checks and notification. Flutter: toggle | Notification mechanism differs per OS |
| F-22 | Run update checks at login (autostart entry) | VERIFIED (rendered entry, no write); write and removal WORKING (source) | `controller.rs:410-443`; `gui.rs:1175-1200` | Exec path frozen at write time (D-36) | Rust: autostart entry. Flutter: toggle | Login items differ per OS (LaunchAgents, Run key, Startup folder) |
| F-23 | Unsafe extraction fallback: setting and dialog | DEAD (unreachable from every shipped caller) | `inspector.rs:150-188`; `gui.rs:456, 2523-2539` | Dialog promises a per-file confirmation that does not exist (D-12) | Decide: remove, or implement with a real confirmation. Rust: policy. Flutter: dialog | — |
| F-24 | Confirmation dialogs: trash, permanent delete, adopt, conflict, update of a running app | WORKING (source) | `gui.rs:2452-2569` | Frontends hold the confirmation policy (D-11); no running check on removal (D-10) | Flutter: dialogs. Rust: policy as request types | — |
| F-25 | About page | WORKING (source) | `gui.rs:2409-2450` | — | Flutter | — |
| F-26 | Status messages and dismissal | WORKING (source) | `gui.rs:1386-1392, 1295, 1315` | A success message is hidden while another status is shown | Flutter | — |
| F-27 | Header logo button (focusable, icon only) | Present (VERIFIED by focus ring); behaviour UNVERIFIED | libcosmic header | No tooltip or accessible name (D-31) | Flutter | — |

### 12.2 Command-line interface

| ID | Feature | Status | Source | Known issues | Proposed split | Cross-platform |
|---|---|---|---|---|---|---|
| F-28 | Inspect without execution (`--probe-inspect`) | VERIFIED | `cli.rs:913-965` | stdout is JSON followed by a plain marker line (D-32) | Rust: keep the contract | — |
| F-29 | Integrate: copy by default; `--keep-both`; `--replace`; `--yes` | VERIFIED: copy, conflict refusal (exit 6), keep-both (`-1` suffix); replace WORKING but defective | `cli.rs`; `integration.rs` | D-01; the exit code for a non-ELF file is 1, not 6 (D-20) | Rust | — |
| F-30 | List installed apps (text and JSON, schema v1) | VERIFIED | `cli.rs:117-123` | `available_version` and `download_size` are empty (vestigial) | Rust: keep the schema | — |
| F-31 | List updates; exit 8 when a check failed | VERIFIED (exit 8 with a DNS failure) | `cli.rs:286-290` | — | Rust | — |
| F-32 | Update one app or all (`--update`, `--update --all`, `--force`) | PARTIAL: `--update --all` exits 0 when every check failed (VERIFIED) | `cli.rs:500-545` | D-08; running-app refusal is string-matched (D-20) | Rust | — |
| F-33 | Remove (Trash by default; `--delete` permanent; `--yes`); remove-all | VERIFIED: Trash, permanent delete, refusal without a TTY (exit 5). remove-all UNVERIFIED | `cli.rs:590-671`; `removal.rs` | D-10, D-11 | Rust | Trash semantics differ per OS |
| F-34 | Set or unset an update source (`--set-update-source`, `key=value`, `--unset`) | PARTIAL: the loopback opt-in is rejected for static sources (VERIFIED) | `cli.rs:691-700`; `updates_sources.rs:134` | D-09 | Rust | — |
| F-35 | List update managers | VERIFIED | `cli.rs` | — | Rust | — |
| F-36 | Adopt an external file; list discovered files | WORKING (tests); runtime UNVERIFIED | `cli.rs:352-367`; `library.rs` | D-18 | Rust | — |
| F-37 | Fetch updates (notify only; exit 8 on failures) | PARTIAL: exit-8 path shared with list-updates (VERIFIED); notification UNVERIFIED | `cli.rs:735-767`; `notifier.rs` | D-14 | Rust | Notification backend per OS |
| F-38 | Probes: `--probe-host`, `--probe-autostart`, `--probe-inspect` | VERIFIED (all three) | `cli.rs:869-1010` | Flatpak-specific probe behaviour UNVERIFIED | Rust | — |
| F-39 | Self-test (`SELF_TEST_OK`) | VERIFIED | `cli.rs:780-865` | Writes a row to the live registry and removes it; the JSON-schema check is tautological (source); does not exercise probes or notifications | Rust | — |
| F-40 | Version and help (`--version`, `-h`) | `--version` VERIFIED; help text not checked | `main.rs:61-68` | README omits `-V` and `-h` | Rust | — |

### 12.3 Integration, removal, persistence and launch

| ID | Feature | Status | Source | Known issues | Proposed split | Cross-platform |
|---|---|---|---|---|---|---|
| F-41 | Managed copy into the managed folder (folder mode 0700) | VERIFIED: copy created, source kept | `integration.rs:106-212` | Name collisions handled (source); no protected-path check on the folder (D-39) | Rust | Path rules per OS |
| F-42 | Desktop entry generation (`gosh-appimage-<uuid>.desktop`) | VERIFIED: created and removed | `desktop.rs:58-74, 221-266` | Exec quoting partial (D-19); categories lost on rewrite (D-13) | Rust | Linux only |
| F-43 | Icon installation (`hicolor/256x256`) | UNVERIFIED (fixture has no icon) | `integration.rs:512-521` | xpm icons saved with a `.png` name (source) | Rust | Linux only |
| F-44 | Desktop database refresh (`update-desktop-database`) | VERIFIED: runs; leaves `mimeinfo.cache` | `integration.rs:552-560` | D-37 | Rust | Linux only |
| F-45 | Registry (SQLite: `apps` table and `meta` with schema version 1) | VERIFIED: created; mode 0600 | `registry.rs:13-46` | No PRAGMAs or busy timeout (D-23); corrupt file not quarantined (D-35) | Rust | Location per OS |
| F-46 | Legacy `registry.json` import (one-time, fail-soft) | VERIFIED: imported; no duplicates on a second run; source file unchanged | `registry.rs:277-293` | Import errors discarded (D-35) | Rust | — |
| F-47 | Settings file (`settings.json`; PascalCase keys; mode 0600) | WORKING (source) | `settings.rs:303-334` | Overwritten after a load error (D-07); Flatpak location not granted (D-04) | Rust: store. Flutter: UI | Config directory per OS |
| F-48 | Trash-first removal (native trash, then `gio trash` on the host) | VERIFIED for native trash; `gio` path UNVERIFIED | `removal.rs:117-120`; `trash.rs` | An orphan desktop entry can remain after a partial failure (D-06) | Rust | Trash per OS |
| F-49 | Permanent delete (explicit mode; protected paths and symlinks refused) | VERIFIED: CLI delete with `--delete --yes`; protection covered by tests | `removal.rs:100-116, 188-213` | Frontends hold the confirmation (D-11) | Rust | — |
| F-50 | Launch (detached, start-only, environment filter, arguments) | WORKING (tests); not run here | `launch.rs:57-100`; `process.rs:182-201` | Launch on the UI thread (D-16); no working directory is set | Rust: spawn. Flutter: button | Process spawning per OS |
| F-51 | Running-process detection (`/proc`; host `pgrep` inside the Flatpak) | VERIFIED: `running:false` for a non-running app; host probe passes. Flatpak path UNVERIFIED | `proctable.rs`; `process.rs:116-145` | Fails open in the Flatpak (D-42) | Rust | Linux only (`/proc`) |
| F-52 | Desktop notifications (`notify-rust` over D-Bus) | UNVERIFIED | `notifier.rs:25-37` | English literal text; send errors ignored | Rust | Per OS |
| F-53 | Localization machinery (catalogs; pseudolocale) | WORKING (tests); no translation ships | `i18n.rs`; `i18n/qps.json` (`{}`) | Several strings bypass the catalog (D-29, D-30; CLI and core errors) | Dart (intl) or Rust (catalogs): decide | — |
| F-54 | Debug logging (stderr switch) | WORKING (tests) | `diagnostics.rs` | No levels (D-38) | Rust | — |

### 12.4 Inspection, updates and networking

| ID | Feature | Status | Source | Known issues | Proposed split | Cross-platform |
|---|---|---|---|---|---|---|
| F-55 | Inspection pipeline: magic check, size bounds, symlink rule, bounded reads | VERIFIED for type-2 through the CLI; WORKING (tests) for the rest | `inspector.rs:30-192`; `elf.rs:78-118` | A missing helper is reported as "No desktop entry found" (D-22) | Rust | AppImage is Linux-only |
| F-56 | Metadata extraction through `unsquashfs` (type 2) and `7zz` (type 1), with member validation and bounds | WORKING (tests with canned output); real helpers not run here | `inspector.rs:365-430`; `safe_fs.rs:358-391` | D-22; no check that helper binaries are the pinned versions | Rust | Linux helpers |
| F-57 | DwarFS metadata | BROKEN: detected, classified, and never readable | `inspector.rs:276-284` | `dwarfsextract` and `dwarfsck` are shipped but unused (D-21) | Rust | — |
| F-58 | Embedded update information: `zsync|URL` | WORKING (source) | `inspector.rs:781-790`; `updates_sources.rs:178-186` | Needs a version in the zsync file or the config | Rust | — |
| F-59 | Embedded update information: `gh-releases-zsync|…` (the common GitHub form) | BROKEN (VERIFIED at runtime) | `inspector.rs:762-781`; `integration.rs:262-263`; `updates_service.rs:109` | D-40 | Rust | — |
| F-60 | Embedded update information: `bintray-zsync|`, and the gitlab, codeberg and forgejo forms | BROKEN or UNVERIFIED (source) | `inspector.rs:797`; `updates_sources.rs` | Keys do not match what the parser writes (source) | Rust | — |
| F-61 | Update managers: static, GitHub, GitLab, Codeberg, Forgejo, FTP (keys, URL shapes) | WORKING (tests); live hosts not contacted | `updates_sources.rs:92-965` | D-09; D-41; D-44 | Rust | — |
| F-62 | Update check: version comparison and offers | PARTIAL (source) | `updates_service.rs:210, 262-269`; `updates_sources.rs:190, 214` | Downgrades offered; string inequality (D-44) | Rust | — |
| F-63 | Update apply: staged download, size cap, ELF and architecture checks, SHA-256 when advertised, atomic rename, rollback | WORKING (tests with fake network); partly defective | `updates_service.rs:237-453` | Running check fails open (D-42); predictable staging name (D-24); 30 s total timeout (D-46); ownership not rechecked (D-43) | Rust | Linux binaries only |
| F-64 | Digest verification (`sha256:` or bare hex); reduced-verification flag | WORKING (tests) | `updates_service.rs:58-78, 354-376` | Optional digest (D-45) | Rust | — |
| F-65 | URL policy: https only; no credentials; local-network opt-in; redirects re-checked; DNS pin | PARTIAL: opt-in unusable for static (VERIFIED); `ftp://` accepted for every caller (READ) | `url_guard.rs`; `network.rs:138-183` | D-09; D-41; D-47 | Rust | — |
| F-66 | Zsync metadata | PARTIAL: parses Filename, Version and the first URL only | `updates_sources.rs:228-247` | Full download every time, no deltas (documented limitation) | Rust | — |
| F-67 | FTP source (legacy, explicit) | PARTIAL: plaintext with a stderr warning; whole file buffered in memory | `network.rs:389-391, 414-416` | D-41; D-49 | Rust | — |
| F-68 | Unsafe fallback: `--appimage-extract` execution | DEAD (unreachable) | `inspector.rs:150-188` | See F-23 | — | — |

### 12.5 Packaging, CI and release

| ID | Feature | Status | Source | Known issues | Proposed split | Cross-platform |
|---|---|---|---|---|---|---|
| F-69 | Flatpak build: freedesktop 23.08, rust-stable extension, x86_64 and aarch64 | Artifacts published; aarch64 checksum VERIFIED; build NOT RUN here | `packaging/com.goshapps.AppImageManager.yml` | D-04 possible; host execution grant (section 8) | — | Linux only |
| F-70 | Flatpak sandbox behaviour (portals, host spawn, filesystem grants) | UNVERIFIED | manifest `finish-args` | Runtime not installed | — | Linux only |
| F-71 | Tarball and `install.sh` | Published; contents read; helper binaries absent (D-22) | `scripts/package-release.sh`; `packaging/install.sh` | D-22 | — | Linux only |
| F-72 | CI (fmt, clippy with and without `gui`, build, test, self-test) | Recent runs VERIFIED green (read-only `gh run list`) | `.github/workflows/ci.yml` | Ubuntu runners only; no Windows or macOS (section 10) | — | Add runners (decision) |
| F-73 | Release automation (tag-driven; publishes a GitHub Release) | v3.0.0 published with four artifacts and `SHA256SUMS` (VERIFIED, read-only) | `.github/workflows/release.yml`; `scripts/verify-release.sh` | Not run here (correct: an audit must not publish) | — | Linux only |
| F-74 | Desktop file and AppStream metadata | VERIFIED: both validators pass (one pedantic note) | `data/*` | Uppercase letters in the component ID (pedantic) | — | freedesktop only |

## 13. Defects and risks

Severity: **High** = data loss or corruption, a security exposure, or a
documented feature that fails for a common case. **Medium** = incorrect or
misleading behaviour with a workaround, silent failure in scripts, or a
safety-relevant misstatement. **Low** = correctness or hygiene with small
impact. **Info** = note only. Evidence: **RUN** = reproduced in this audit;
**READ** = confirmed by reading the cited code; **SRC** = reported by a source
review, not independently confirmed.

### 13.1 Summary

- High (6): D-01, D-02, D-03, D-04, D-05, D-40.
- Medium (19): D-06, D-07, D-08, D-09, D-10, D-11, D-12, D-13, D-14, D-15, D-17,
  D-22, D-31, D-35, D-39, D-41, D-42, D-44, D-46.
- Low (23) and Info (2): the remaining items. D-04 is High only if the Flatpak
  behaviour is confirmed.

### 13.2 Register

| ID | Sev. | Defect | Evidence |
|---|---|---|---|
| D-01 | High | A failed **replace** restores the new desktop entry and icon, not the old ones. The backups are copied after `install_desktop_and_icon` has overwritten the live files (`integration.rs:291-296` before `322-344`; rollback at `404-417`). The test fixtures are byte-identical, so the test cannot detect it (`tests/test_rollback.rs:176-180`) | READ |
| D-02 | High | `rollback_temps` deletes every entry in the managed folder whose name starts with `.gosh-`, recursively for directories, on each integration rollback (`safe_fs.rs:239-263`; `integration.rs:420`). A user file with that prefix is lost. The managed folder can be any absolute path (D-39), which widens the reach | READ |
| D-03 | High | GUI: the controller mutex is held for a whole inspect, integrate or update job (`gui.rs:446-470, 487-507, 1441-1451`). The UI thread takes the same lock for Cancel, Launch, Settings and job spawn (`gui.rs:300-306, 896-912, 1160-1166`). Cancel cannot take effect until the job releases the lock, and inspection hashes the whole file (up to 8 GiB) under the lock | READ (not timed) |
| D-04 | High (unverified) | Flatpak: settings are written to `~/.config/gosh-appimage-manager`, but the manifest grants only `xdg-config/autostart`. Settings writes probably fail in the sandbox (`settings.rs:29`; manifest `finish-args`) | READ; UNVERIFIED in the sandbox |
| D-05 | High (security) | Dependency `rustls` 0.23.43 was affected by RUSTSEC-2026-0285 (TLS 1.3 handshake messages accepted across encryption-level boundaries). **Fixed after the audit** in the working tree: `rustls` 0.23.45 in `Cargo.lock`, `packaging/cargo-sources.json` updated, `cargo audit` reports zero vulnerabilities (section 22). Not committed. Exploitability was not assessed | RUN (`cargo audit`, before and after) |
| D-06 | Medium | Removal deletes the registry row even when desktop or icon cleanup failed. The result is reported as partial, but the desktop file is left behind with no managed record (`removal.rs:136-160`; `remove_uuid` runs whether or not `artifacts_ok`) | READ |
| D-07 | Medium | Settings: after a corrupt or unreadable `settings.json`, defaults load and `load_error` is set, but `save()` ignores both `loaded` and `load_error`. The next setting change replaces the file with no backup (`settings.rs:245-273` against `303-334`; the comment at `250-251` says otherwise). The GUI shows the error once at startup; the CLI never shows it | READ |
| D-08 | Medium | `--update --all` exits 0 and prints nothing when every update check failed. `--list-updates` reports the same failure and exits 8 (`cli.rs:505-545`; `controller.rs:349-354`) | RUN |
| D-09 | Medium | The documented opt-in cannot configure a static source. Set-time validation passes `allow_private = false` (`updates_sources.rs:134`), so `--set-update-source` rejects loopback and private hosts with exit 6 even with `allow_local_network=true`. Other managers were not tested at set time | RUN (static, loopback https); READ (line) |
| D-10 | Medium | Removal of a running AppImage is not refused or warned about, in the GUI (`gui.rs:925-937`), in the core (`removal.rs:81-160`) or in the CLI | READ |
| D-11 | Medium | Destructive confirmation is enforced in the frontends only. `assume_yes` on `RemovalRequest` and `IntegrateRequest` is set by the CLI and GUI and never read by the core (`types.rs:308, 344`). A caller of the library can permanently delete without a confirmation step | READ (grep) |
| D-12 | Medium | The unsafe-extraction dialog promises that "each file still has to be confirmed separately" (`gui.rs:2525-2530`). No confirmation flow exists. The inspector only adds a warning (`inspector.rs:184-188`) | READ |
| D-13 | Medium | Desktop entries are rewritten from registry rows, which do not store `Categories`, `MimeType` or `StartupWMClass` (the schema has no such columns; `registry.rs:13-46`). The writer then emits `Categories=Utility;` and no MIME line (`desktop.rs:251-264`). Argument saves and metadata refresh therefore drop the original metadata (`controller.rs:255-266, 316-327`) | READ (schema and writer); runtime not reproduced, because the fixtures carry no metadata |
| D-14 | Medium | The background-checks setting has no effect on `--fetch-updates`. Only the GUI reads it (`gui.rs:2371, 2591`). The login entry is separate. The README calls the feature "check for updates in the background (notify only)" | READ (grep) |
| D-15 | Medium | Queue stall: the next queued file starts only after a successful integration (`gui.rs:1494-1510`). After a failed or declined inspection, queued files never start | READ |
| D-16 | Low | Launch runs on the UI thread inside the controller lock (`gui.rs:896-912`). The NixOS shim probe can take up to 3 s (`launch.rs:40-50`) | READ; SRC for the probe time |
| D-17 | Medium | Integration half-states: an icon failure after the desktop entry is installed leaves an orphan desktop entry with no row (`integration.rs:291-298`). A crash between install and upsert leaves a marked orphan that the library skips (`library.rs:135-137`) | READ (first case); SRC (crash case) |
| D-18 | Low | The frontends disagree on adoption: the GUI adopts without inspecting (`gui.rs:1010`); the CLI inspects first (`cli.rs:352-367`) | SRC |
| D-19 | Low | `Exec` quoting is partial. Arguments and environment values are quoted for spaces and reserved characters, but backslashes and newlines are not escaped at the string level (`desktop.rs:14-40, 58-74`). The input is the user's own arguments and environment | READ |
| D-20 | Low | Exit codes come from substring matches on error text (`cli.rs:488, 516, 576`). The non-ELF rejection returns 1, while the README lists 6 for validation failures | RUN (exit 1); READ |
| D-21 | Low | DwarFS: the type is detected and then never readable. `dwarfsextract` and `dwarfsck` are shipped in the Flatpak but never invoked (`inspector.rs:276-284`) | READ |
| D-22 | Medium | A helper that cannot be started is reported as success with exit code 0 (`process.rs:243-249` sets `refused` and leaves `exit_code` at 0; `inspector.rs:389` checks only `exit_code`). The user then sees "No desktop entry found" instead of "helper not installed". The tarball ships no helper (section 10.2) | READ |
| D-23 | Low | The registry sets no busy timeout and no PRAGMAs. The GUI and CLI running together can hit `SQLITE_BUSY`; whether the error reaches the user is UNVERIFIED | READ (grep) |
| D-24 | Low | Hardening gaps: no `fsync` anywhere; no `O_NOFOLLOW`; predictable temporary names, including the update staging name `.gosh-upd-` (`safe_fs.rs:96-129`; `updates_service.rs:283`). The threat is limited because it needs same-user access. The module header overstates the crash safety it provides | READ (grep); SRC |
| D-25 | Low | Shortcuts match lowercase letters only. Shift or Caps Lock disables Ctrl+O, Ctrl+R and Ctrl+F (`gui.rs:744-758`) | READ |
| D-26 | Low | Version sorting is a string comparison ("10" sorts before "9") (`gui.rs:417`) | SRC |
| D-27 | Low | The Tasks progress bars never advance. `progress()` is called only from tests and `run_task` (`tasks.rs`) | SRC |
| D-28 | Low | Single instance is not used. The app calls `cosmic::app::run`, not the single-instance runner, although the libcosmic `single-instance` feature is enabled (`gui.rs:48-55`; `Cargo.toml`) | READ |
| D-29 | Low | `LC_ALL=C` does not force the C locale, because the loop moves on to `LANG` (`i18n.rs:44-53`). Negligible while only English ships | READ |
| D-30 | Low | The catalog search includes a relative `i18n` directory (`i18n.rs:199`), so lookup depends on the working directory | READ |
| D-31 | Medium | No control has a tooltip or an accessible name. The icon-only header button has neither (grep of `gui.rs`). The brief requires both | READ (grep) |
| D-32 | Low | `--probe-inspect` prints JSON followed by a plain marker line, so stdout is not one JSON document. `main.rs:50-51` says otherwise | RUN |
| D-33 | Low | Reveal opens the parent folder with `xdg-open` and does not select the file (`controller.rs:200-214`) | READ |
| D-34 | Low | Dependency advisories: 9 unmaintained and 3 unsound transitive crates (`ttf-parser`, `rustybuzz`, `derivative`, `paste`, `proc-macro-error`, `instant`, `lru`, `memmap2`), mostly from the GUI stack | RUN (`cargo audit`) |
| D-35 | Medium | Registry startup: import errors are discarded (`registry.rs:291`), and once any mutation creates `registry.sqlite` the legacy file is never imported again. A corrupt `registry.sqlite` makes the app fail to open, with no quarantine or recovery path (`controller.rs:57`; `registry.rs:358-359`) | SRC |
| D-36 | Low | The login entry records the executable path when it is written (`Exec=<path> --fetch-updates`). If the app moves, the entry breaks until the setting is saved again | RUN (rendered entry); READ |
| D-37 | Info | Removal leaves `applications/mimeinfo.cache`, which `update-desktop-database` created | RUN |
| D-38 | Low | Logging hygiene: the FTP parser prints the raw URL, query string included (`network.rs:433`). Diagnostics do not strip control characters from names | SRC |
| D-39 | Medium | The managed folder accepts any absolute path, including the home folder. The GUI checks only that the path is absolute (`settings.rs:282-285`; `gui.rs:1288-1291`), so no protected-path check applies. This widens D-02 | SRC |
| D-40 | High | Embedded `gh-releases-zsync|` updates fail on every check with `Unknown update manager: gh-releases-zsync`. The README says embedded information is picked up automatically (`integration.rs:262-263`; `updates_service.rs:109`) | RUN |
| D-41 | Medium | `url_guard::validate` accepts `ftp://` for every caller (`url_guard.rs:40-44`). Release documents and zsync files can therefore name plaintext FTP download URLs, which the apply path then fetches (`network.rs:389-391`) | READ (guard); SRC (apply path) |
| D-42 | Medium | Running check fails open inside the Flatpak. When the host probe fails, the `/proc` fallback is blind in the sandbox and reads as "not running", so an update can overwrite a running app without `--force` (`proctable.rs:186-200`; `updates_service.rs:272-276`) | READ |
| D-43 | Low | Ownership is not rechecked before rewriting a desktop entry during apply, refresh or argument saves. The registry `owned` flag is trusted (`updates_service.rs:421-428`; `controller.rs:254-266, 316-326`). The autostart file is written and deleted by fixed name without a marker (`controller.rs:413-423`) | SRC; READ (apply path) |
| D-44 | Medium | Version comparison is string inequality. Downgrades are offered and applied; `v1.2.3` and `1.2.3` produce a permanent offer; GitHub, GitLab and Forgejo set `available = true` unconditionally; GitLab package results have an empty version and are never offered (`updates_service.rs:262-269`; `updates_sources.rs:190, 214, 335, 563, 581, 654, 757`) | SRC |
| D-45 | Low | The digest is optional. Without one the update proceeds with reduced verification. The payload's own version is never compared (`updates_service.rs:354-376`) | SRC |
| D-46 | Medium | The 30 s total timeout applies to the whole response, including the body (`network.rs:192`; `limits.rs`). Downloads that take longer fail, although the size cap allows files up to 8 GiB. Reqwest's documented behaviour was read from the crate source | READ (line); SRC (crate behaviour) |
| D-47 | Low | `test_ssrf.rs` returns early and passes unless the probe name resolves to loopback (lines 20-25). Nothing tests the HTTP DNS pin or the redirect-hop checks | SRC |
| D-48 | Info | `test_inspector.rs` sets `confirm = true` with a fake runner, while `inspector.rs:4, 155` says tests never set it. Production paths are unaffected | SRC |
| D-49 | Low | FTP downloads are buffered in memory up to `max_bytes` (`network.rs:390-391, 594-595`), against the design note at `network.rs:61-69` | SRC |
| D-50 | Low | The host-spawn gate checks only that the path is absolute and a regular file (`process.rs:77-95`). It does not check that the path is a registered AppImage, although the comment at `process.rs:17-21` says it does | SRC |

### 13.3 Claims checked and rejected or corrected

| Claim | Outcome |
|---|---|
| Adopted apps cannot be updated (`updates_service.rs:245-246`) | Rejected. Adoption sets `owned = true` and `adopted = true` (`registry.rs:615-624`), and the gate refuses only rows where `owned` is false. The README statement holds |
| The unsafe fallback can run from the GUI | Rejected for production paths. `gui.rs:456` passes `confirm = false`. The dialog text is still misleading (D-12) |
| The Flatpak maps `HOME` to `~/.var/app` (README) | Not supported by the code. `settings.rs:23-30` uses `HOME`, and the manifest grants real-home paths. Corrected in section 16 |
| `--probe-inspect` keeps stdout valid JSON (`main.rs:50-51`) | Corrected. The marker line follows the JSON (D-32) |
| Settings are "XDG-aware" (`settings.rs:1`) | Corrected. `XDG_*_HOME` is not read (section 6) |
| Permanent delete asks again (README) | Partly true. The GUI asks once; the CLI `--delete --yes` does not ask again (RUN) |
| Exit code 6 is validation (README) | Corrected. A rejected file returns 1 (D-20) |
| Embedded `.upd_info` is "picked up automatically" (README) | Corrected for `gh-releases-zsync` (D-40). Works for `zsync|` (static) |
| `allow_local_network=true` applies to static sources (README) | Corrected at set time (D-09). The check-time path does honour it (`network.rs:43-48`) |

## 14. Dead, partial and unreachable features

- **Unsafe extraction fallback (F-23, F-68).** Implemented, tested with a fake
  runner, and unreachable from every shipped caller (`inspector.rs:162`). The
  settings toggle, the dialog and the README describe a flow that cannot run.
- **DwarFS (F-57).** Detected, then always fails. The bundled binaries are unused
  (D-21).
- **Task progress (F-13, D-27).** Shown in the UI, never updated.
- **Background checks (F-21, D-14).** The GUI toggle changes only the display; the
  login entry is separate.
- **Single instance (D-28).** The libcosmic feature is compiled in and not used.
- **Dead code (grep and source review).** `ExitCode::NotFound` (never returned);
  `AppController::models_ready` (no callers); the cache directory
  (`settings.rs:228-230`, no callers); `SettingsStore::loaded` (tests only);
  `assume_yes` on the requests (never read by the core); `TaskState::Queued`
  (never assigned); the `Outcome::Simple(Ok)` arm (only a panic path produces
  it); `download_bounded` (no production caller); `DEFAULT_PROCESS_TIMEOUT_MS`
  (unused); the Type-2 arm that selects `dwarfsextract` (shadowed by the ELF
  check); the GitHub `gh-releases-zsync` handler (`updates_sources.rs:259`, shadowed
  at `919-923`); `is_rtl` (no UI consumer); `i18n::set_active_for_test` and
  `Catalog::source_only` (test-only).
- **Vestigial registry columns.** `last_update_check`, `available_*`,
  `update_available` and `digest` are cleared or copied, but no check writes them
  (section 6.1).

## 15. Undocumented or under-documented behaviour

- `--probe-inspect` prints a provenance marker, `INSPECT_NO_EXECUTION`, after its
  JSON (VERIFIED). `INSPECT_EXECUTED_UNSAFE` exists in code and is unreachable
  (`cli.rs:962`).
- `--self-test` writes a row to the live registry, then removes it. It is
  documented as fixture-only (README), but it touches the data directory (VERIFIED
  in an isolated home).
- Login-check notifications use English text that is not translatable
  (`notifier.rs:25`).
- Reveal opens the parent folder and does not select the file (D-33).
- Removing an adopted app trashes the file at its original location, outside the
  managed folder (README, adopt section).
- Legacy `registry.json` is ignored once `registry.sqlite` exists (D-35), so a
  user who restores the old file after the first mutation gets no import.
- `-V`, `-h` and `-y` aliases exist; the usage text omits `-V` and `-h`.
- Cache directory settings exist and do nothing.

## 16. Documentation drift

| Where | Claim | Observed | Status |
|---|---|---|---|
| README, "Upgrading" and "Where things live" | Sandbox paths land in `~/.var/app` | The code uses `HOME`; the manifest grants real-home paths (`settings.rs:23-30`) | Incorrect (D-04) |
| README, Safety | Exit code 6 is validation | A rejected file exits 1 (VERIFIED) | Incorrect (D-20) |
| README, Using the app | Embedded `.upd_info` is "picked up automatically" | `gh-releases-zsync` fails on every check (VERIFIED) | Incorrect (D-40) |
| README, Settings | Background checks "notify only" | The toggle does not affect `--fetch-updates` (READ) | Incorrect (D-14) |
| README, CLI | `allow_local_network=true` opts a source into loopback | Rejected at set time for static sources (VERIFIED) | Incorrect at set time (D-09) |
| README, Safety | "Permanent delete asks again" | GUI asks once; CLI `--delete --yes` does not ask again (VERIFIED) | Partly true |
| README, Safety and Settings | Unsafe fallback: "Enabling it in Settings only adds a warning" | True for code paths, but the dialog promises a confirmation (`gui.rs:2525-2530`) | Misleading (D-12) |
| README, Using the app | Adopted apps "can be updated and removed here" | True (adoption sets `owned = true`) | Accurate |
| README, Development | Keyboard shortcuts | Matches `gui.rs:744-758`, with lowercase-only matching (D-25) | Accurate, incomplete |
| README, Limitations | "The GUI has been rendered and driven on a headless X server" | Not re-run here (Xvfb and xdotool absent) | Not re-verified |
| `docs/verification.md` | "134 tests" | 160 tests pass (VERIFIED) | Stale |
| `docs/audit/FEATURES.md` | Unsafe fallback: "GUI toggle + per-file confirm" | No per-file confirmation exists | Incorrect |
| `docs/audit/FEATURES.md`, `docs/documentation/APP-INVENTORY.md` | Adopted rows: remove and update "work (verified live)" | Confirmed by reading (`owned = true`) | Accurate |
| `docs/documentation/APP-INVENTORY.md` section 18 | Metainfo has no translation claim | Metainfo has none (READ) | Accurate |
| `data/com.goshapps.AppImageManager.metainfo.xml` | `<control>touch</control>` | Desktop app; touch is not a designed input (READ) | Questionable |
| `settings.rs:1` | "XDG-aware" | `XDG_*_HOME` not read | Incorrect |
| `main.rs:50-51`, `cli.rs:284-285` | stdout stays valid JSON | Marker line follows the probe JSON (VERIFIED) | Incorrect (D-32) |
| `cli.rs:973-979` | The probe renders into a private temp directory | It does not (READ) | Incorrect (source) |
| `inspector.rs:4, 155` | Tests never set `confirm` | `test_inspector.rs` does (D-48) | Incorrect (source) |
| `i18n.rs:42-43`, `i18n/README.md:25` | `LC_ALL=C` selects C | `LANG` wins (D-29) | Incorrect (READ) |
| `i18n.rs:190-191` | Binary-relative catalog lookup | Not implemented; relative `i18n` used instead (D-30) | Incorrect (READ) |
| `Cargo.toml` `rust-version = "1.89"` | Minimum supported Rust | Not tested here; the toolchain used was 1.99.0 | UNVERIFIED |

## 17. AGENTS.md compliance

| Rule | Verdict | Evidence |
|---|---|---|
| Treat every external input as untrusted | PARTIAL | Inspection and downloads are bounded. Desktop quoting (D-19), log hygiene (D-38) and the update source policy (D-41) fall short |
| Never execute an AppImage to inspect metadata by default | COMPLIANT | `--probe-inspect` marks no execution (VERIFIED); the only execution is gated (`inspector.rs:162`) |
| Unsafe legacy extraction: opt-in, clearly warned, disabled by default | PARTIAL | Off by default (`settings.rs:107`, read). The warning text is misleading and the confirmation step does not exist (D-12) |
| Never construct shell command strings | COMPLIANT | Argument arrays only; no `sh` or `bash` found; the one spawn site is `process.rs:171` |
| Never delete user data when Trash fails | PARTIAL: removal complies; rollback does not | `removal.rs:117-120` (read); `rollback_temps` deletes user entries named `.gosh-*` (D-02) |
| Permanent deletion requires explicit destructive confirmation | PARTIAL | The CLI requires `--delete` and `--yes` or a TTY answer. The GUI uses one dialog. The core does not enforce it (D-11) |
| Never overwrite without verified ownership and explicit replace | VIOLATED | Settings are overwritten after a load error (D-07); desktop entries are rewritten without ownership checks (D-43); the update path trusts the registry flag (D-43) |
| Mutations transactional and recoverable; rollback material kept until success | PARTIAL | Replace rollback restores the wrong content (D-01); removal leaves orphans (D-06); half-states exist (D-17) |
| Validate downloads before atomic replacement; keep rollback material | PARTIAL | Size, ELF, architecture and digest checks run before the swap (source); the swap and backup logic has the gaps listed above; the digest is optional (D-45) |
| Build and test with cargo and just | COMPLIANT | `just build`, `build-gui`, `lint`, `fmt-check`, `validate` and `cargo test` all pass (VERIFIED) |
| Flatpak on freedesktop 23.08, x86_64 and aarch64, one manifest | COMPLIANT (manifest); runtime not verified here | Manifest reviewed; artifacts for both architectures published (VERIFIED) |
| Real tests, package builds, packaged smoke checks, documented evidence | PARTIAL | 160 real tests pass; the packaged `--self-test` runs during the Flatpak build (source); no sandbox run here |
| No publishing or signed-channel enrollment from implementation workers | COMPLIANT | This audit published nothing and changed no release |

## 18. Migration analysis

### 18.1 What can be reused

- The core library (`goshaim_core`) already separates the operations from the
  GUI, behind a controller with about twenty public methods and four injectable
  seams (section 5.3). That is the right boundary for a bridge.
- The registry schema and the legacy import are stable and tested. Keep them.
- The CLI's JSON contract (`schema_version: 1`) is what scripts depend on. Keep
  it stable.
- 160 tests give a regression baseline for the core.

### 18.2 What is coupled to the GUI or to Linux

- `gui.rs` (2,704 lines) holds policy that belongs in the core: confirmation
  rules, the queue, and which operations run in which order (D-11, D-15).
- The core also holds Linux-specific mechanisms: desktop entries, the hicolor icon
  theme, `update-desktop-database`, `gio trash`, `/proc`, `flatpak-spawn`,
  `xdg-open`, the NixOS shim, D-Bus notifications and the `XDG`-style paths
  (section 7). Windows and macOS would need a separate platform layer for each.
- The GUI and the core share one mutex, and the GUI holds it during long jobs
  (D-03). Any new front end inherits that unless the core is redesigned to run
  jobs off the lock.

### 18.3 Proposed split (for discussion, not decided)

- **Flutter (Dart):** windows, navigation, forms, lists, dialogs, theme, keyboard
  shortcuts, status messages, file pickers and drag and drop through plugins,
  and the presentation state for each page.
- **Rust core:** inspection, validation, extraction, the integration transaction,
  registry, settings store, removal, launch, running detection, update checks,
  apply and rollback, networking and URL policy, notifications, autostart entry
  generation, and the CLI.
- **Bridge:** coarse request and response types, one call per user action, and
  progress as a stream for long jobs. No per-field calls. `flutter_rust_bridge`
  and direct FFI have not been evaluated; no selection is made here.
- **Confirmation policy in the core:** each destructive operation takes an
  explicit confirmation token, so no caller can skip it (D-11).
- **Typed errors:** replace the substring matching behind the exit codes and the
  running-app refusal (D-20) with error kinds.

### 18.4 Product fit (the main risk)

- An AppImage is a Linux executable. The core's purpose is to integrate, launch and
  update Linux AppImages through freedesktop mechanisms. On Windows and macOS those
  mechanisms do not exist, and the AppImages cannot run.
- So "the same application on Windows and macOS" would be a different product:
  at most an inspector and update-checker for files. The brief's requirements for
  installers, `.app` bundles, notarisation and Windows CI would apply to that
  smaller product, not to the current one.
- A Flutter UI on Linux would also replace the libcosmic look, which the project
  describes as part of its identity (`README.md`, `docs/rewrite-3.0.0.md`).

### 18.5 Data compatibility requirements

- Keep `registry.sqlite` schema version 1 readable and writable, and keep the
  legacy import.
- Keep `settings.json` keys and their meanings. Fix the overwrite (D-07) first.
- Keep the desktop entry and icon names (`gosh-appimage-<uuid>.desktop`), so
  existing menu entries keep working.
- The owner's managed folder (three AppImages and `.icons/`) must be recognised
  and left in place. It was not modified in this audit.

### 18.6 Verification prerequisites (absent here)

- Flutter SDK and Dart (absent). Flutter's Linux embedder needs GTK 3, which is
  installed (3.24.52).
- The freedesktop 23.08 runtime, SDK and rust-stable extension, for Flatpak tests
  (absent).
- Windows and macOS hosts, or CI runners for them (absent).
- A Flutter GUI test strategy: pointer input does not reach iced widgets in this
  environment (section 4), so GUI tests would need a different approach.

## 19. Decisions required from the owner

1. **Direction.** Three options:
   - **A. Keep the Rust and libcosmic application.** Fix the defects in section 13.
     This matches `AGENTS.md`, the 3.0.0 release (published 2026-09-13), and the
     green CI. It is the lowest-risk path.
   - **B. Build a Flutter front end on the Linux core.** This needs `AGENTS.md`
     amended (it names Rust, libcosmic, `cargo` and `just`), a bridge decision,
     a Flutter build inside the 23.08 Flatpak, and a plan for the GTK 3 embedder.
   - **C. Full Windows and macOS parity.** Not feasible for the current feature set
     (section 18.4). It needs a product decision first.
   - **Recommendation:** choose A now. Revisit B only after the high-severity
     defects are fixed. C requires a new product definition.
2. **Windows and macOS scope**, if B or C is chosen: inspect-only, or not offered.
3. **Verification environment.** Approve, or decline, the following:
   - a user-level install of the 23.08 Flatpak runtime, SDK and rust-stable
     extension (size not measured here), so the published bundle can be tested in
     its sandbox (section 10.6). Nothing was installed for this.
   - a Flutter SDK, if B is chosen;
   - Windows and macOS runners in GitHub Actions, which the repository would then
     depend on.
4. **Immediate fixes.** Approve, or decline, these repository changes:
   - bump `rustls` to 0.23.45 or later in `Cargo.lock`, and regenerate
     `packaging/cargo-sources.json` with `just vendor` (it needs a checkout of
     flatpak-builder-tools) (D-05);
   - the High and Medium defects in section 13, each with a regression test
     written first, as the brief requires.
5. **Publishing.** No release, tag or GitHub change has been made. Any new release
   needs an explicit instruction, because the release workflow publishes.

## 20. Proposed next steps (no changes made)

1. Owner decision on section 19, items 1 and 2.
2. Patch the security dependency (D-05) and re-run `cargo audit`, `cargo test`
   and the Flatpak CI path.
3. Fix the data-safety defects in this order, each with a test that fails first:
   D-02 (scope and content of rollback clean-up), D-01 (backups taken before the
   overwrite, with fixtures that differ), D-06 (keep the row if artifacts remain),
   D-07 (refuse to save after a load error, or keep a backup), D-43 (verify
   ownership before rewriting), D-17 (complete rollback on partial failure).
4. Fix the functional gaps: D-40 (accept `gh-releases-zsync` as a GitHub source;
   test with a real `.upd_info`), D-09 (the set-time opt-in), D-08 (exit status of
   bulk update), D-13 (keep categories and MIME types in the registry or re-read
   them), D-14 (wire or relabel the background setting).
5. Fix the responsiveness defect D-03 (run long jobs without holding the
   controller lock; make Cancel non-blocking), then D-16.
6. Decide on the unsafe fallback (F-23): implement a real confirmation or remove
   the feature and its text (D-12).
7. Verify the Flatpak settings path (D-04) once the sandbox can be run.
8. Accessibility: add labels and tooltips to icon-only controls (D-31).
9. Do not begin Flutter work until section 19 is decided.

## 21. Reproduction and artifacts

- Repository changes made by this audit: `MIGRATION_AUDIT.md` only (untracked).
  No other file was edited. Nothing was committed, pushed, tagged or published.
- Tools installed for this audit, all in the user's cargo and rustup directories:
  rustup stable toolchain (rustc and cargo 1.99.0, clippy, rustfmt), `just` 1.58.0,
  and `cargo-audit` 0.22.2. No shell profile was changed.
- Scratch material (synthetic fixtures, isolated home directories, screenshots,
  command logs) was kept outside the repository, in the session's temporary
  directory. That directory is no longer available.
- Source-review notes came from four read-only review passes. Their claims are
  marked SRC, or READ where this audit checked them.
- Owner data was not read or written. The three AppImages in `~/AppImages` were
  listed only.
- Commands that reproduce the results in sections 1, 3, 10.5 and 13:
  `cargo build --locked`, `cargo build --locked --features gui`,
  `cargo test --locked --no-fail-fast`, `cargo fmt --check`,
  `cargo clippy --locked --all-targets -- -D warnings` (both feature sets),
  `just build`, `just build-gui`, `just lint`, `just fmt-check`, `just validate`,
  `desktop-file-validate data/com.goshapps.AppImageManager.desktop`,
  `appstreamcli validate --pedantic --no-net data/com.goshapps.AppImageManager.metainfo.xml`,
  `cargo audit`. Runtime checks used an isolated `HOME` and `GOSHAIM_HOME`.
- GUI checks: `WINIT_UNIX_BACKEND=x11` with `WAYLAND_DISPLAY` unset, screenshots
  taken with ImageMagick `import` against the window id, keys sent through XTEST.

## 22. Changes after the audit (2026-10-07)

- **Direction.** The owner chose "Flutter UI on the Rust core, Linux first".
  `docs/flutter/ARCHITECTURE.md` records the proposal: the layers, the
  responsibility split, the bridge choice (`flutter_rust_bridge` 2.13.0,
  provisional until a spike passes), the coarse API, the error and concurrency
  rules, the validation plan, and the phases. Nothing is adopted yet.
  `AGENTS.md` is unchanged and must be amended at cut-over.
- **Security fix (D-05), approved by the owner and applied in the working tree:**
  - `Cargo.lock`: `rustls` 0.23.43 → 0.23.45 (checksum updated). A dev-only
    dependency edge (`tempfile` → `getrandom`) moved from 0.4.3 to 0.3.4 during
    re-resolution. The lockfile format moved from version 3 to version 4, which
    Rust 1.89 (the declared minimum) and the Flatpak's Rust 1.90.0 both read.
  - `packaging/cargo-sources.json`: the two `rustls` entries replaced (archive and
    `.cargo-checksum.json`). The archive `sha256` equals the lockfile checksum.
    The official generator (flatpak-builder-tools, latest) also rewrote 24 inline
    `Cargo.toml` entries for git-sourced crates, differing only in blank lines.
    Those were not taken, so the diff covers only the `rustls` entries.
  - Verification: `cargo build --locked` (CLI and `gui`), `cargo test --locked`
    (160 passed, 0 failed), clippy with `-D warnings` (both feature sets),
    `cargo fmt --check`, and `cargo audit` (0 vulnerabilities; the same 9
    unmaintained and 3 unsound warnings as before).
  - Not verified: a Flatpak build with the new sources. `flatpak-builder` and the
    23.08 SDK are not installed here.
  - Not committed and not published.
- **Flutter feasibility on this host (verified).**
  - The official Flutter release feed lists 742 releases and none is an ARM64
    Linux SDK.
  - x86-64 binaries cannot run here without root. The binfmt handler hands them to
    `/usr/bin/binfmt-dispatcher`, which reports "Will attempt to install missing
    requirements for FEX" and blocks. A test binary (hand-assembled, no libc) timed
    out without output.
  - `flutter_rust_bridge_codegen` 2.13.0 installs and runs on aarch64. The
    2.14.0-beta.2 release is a pre-release and was not used.
  - **Correction (2026-10-08).** The conclusion above was wrong. The official
    release archives are x86-64 only, but the Flutter SDK runs on this aarch64
    host when it is bootstrapped from the git repository at tag `3.47.6`. The
    tool then fetches the arm64 Dart SDK and, through `flutter precache --linux`,
    the `linux-arm64` engine artifacts. Section 23 records the arm64 build and
    test results. The x86-64 build runs only in CI, because x86-64 binaries
    cannot execute here without root.

## 23. Flutter bridge spike on aarch64 (2026-10-08)

The owner asked for arm64 builds as well as x86-64. The spike was built and
tested on this host. Its files are `bridge/` (a Rust crate that depends on the
core by path), `flutter/` (the Flutter app with the cargokit plugin), and
`docs/flutter/ARCHITECTURE.md`. None of it is committed.

Results (aarch64, VERIFIED):

- Rust: in `bridge/`, `cargo fmt --check`, `cargo clippy --all-targets -- -D
  warnings` and `cargo build` pass.
- Codegen: `flutter_rust_bridge_codegen` 2.13.0 generates the Dart and Rust glue.
  A second run changes nothing. It warns that `cargo-expand` is absent, and
  generation still succeeds.
- Dart: `dart format` is clean and `flutter analyze` reports no issues.
  `flutter test` passes 7 of 7: the bridge version, inspection of a synthetic
  AArch64 type-2 AppImage, `not_found`, `user_input`, `validation` with the core
  message, stream order, and a panic mapped to `internal` while the bridge keeps
  working.
- Build: `flutter build linux`, debug and release, produces AArch64 bundles. The
  bridge library `libgoshaim_bridge.so` sits next to the executable. The release
  bundle is 23 MB.
- Runtime: the app ran under XWayland (`GDK_BACKEND=x11`). The screenshot shows
  the bridge version, the inspection summary (size, SHA-256, type, architecture,
  the core's metadata warning), the stream 0 to 4, and the panic mapped to
  `internal`.

Limits and open items:

- x86-64 has not been built or tested. The build runs only in CI, and the CI
  workflow `.github/workflows/flutter.yml` is not pushed.
- The CI workflow has not run. Its bindings-drift check and its two-architecture
  matrix are unverified until a run completes.
- The spike API includes a deliberate panic, `panic_for_contract_test`. Remove it
  before real pages use the bridge.
- Cancellation of a long operation is untested. It waits for `check_updates` in a
  later phase.
- The Flatpak build of the Flutter app is not designed. Vendoring the Flutter SDK,
  pub packages and cargo crates for an offline build is open.
- Tools installed in the user's home for this work: the Flutter SDK (3.47.6) at
  `~/development/flutter`, `flutter_rust_bridge_codegen` 2.13.0 in `~/.cargo/bin`,
  and the pub cache in `~/.pub-cache`.

## 24. Flutter front end: implementation and verification (2026-10-08)

Scope: the Flutter application now carries every page, dialog and workflow of the
libcosmic GUI, on the bridge. The libcosmic GUI still ships, and nothing has been
cut over. The per-feature mapping, the checks run, and the known differences are
in `docs/flutter/PARITY.md`. This section records what changed in the bridge and
the Flutter tree, and what the checks showed.

What changed:

- Bridge: `OutcomeDto` carries `conflict_uuid` and `conflict_name`. The replace
  candidate is computed the way the GUI computes it, and only when one
  installation is implicated. The spike module (`count_ticks`,
  `panic_for_contract_test`) is removed. Panic containment is covered by a Rust
  unit test on `guard`.
- Flutter: `lib/core` (the `CoreApi` seam, with a fake for tests), `lib/state`
  (the `AppModel` workflow, its status texts, the pure format and drop helpers),
  `lib/theme` (COSMIC palettes derived by `cosmic-theme`, Fira Sans), and
  `lib/ui` (shell, rail, the six pages, dialogs, shared widgets).
- Linux runner: 1024 by 768 default size, 420 by 420 minimum, the application
  title, a header bar that carries the navigation toggle and takes its colours
  from Dart, and the `gosh/window` channel for the page title.
- Tests: 77 Dart tests in `just flutter-check`, of which 8 are bridge integration
  tests that refuse to run without a scratch `GOSHAIM_HOME`. Golden baselines for
  each page, the condensed layout and two dialogs live in `flutter/test/goldens/`.

What the checks showed:

- The libcosmic GUI and the Flutter build were run side by side on the same
  scratch home. The Library matches in layout, colour and type (captures in
  `docs/flutter/parity/`). Two fixes came from that comparison: the selected
  navigation label is drawn in the accent, and the condensed layout keeps
  buttons at their natural width.
- Input automation could not reach either window from this host. Pages other
  than Library were checked by widget tests and golden renders only.
- Both `flutter build linux` modes build, and the release bundle launches and
  shows the page title.

Open items: the cut-over and its decisions (application ID, binary name, AGENTS.md
wording, the About page's COSMIC wording, removal of the libcosmic GUI), the
translation catalogs, the Flatpak manifest for the Flutter build, the portal
colour-scheme query, and the x86-64 run in CI, which is blocked by the account
billing lock. Each is listed in `docs/flutter/PARITY.md`, section 5.
