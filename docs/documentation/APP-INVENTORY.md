# Gosh AppImage Manager — Application Inventory

Source-of-truth inventory of the repository at `3.0.0`, derived from reading
the implementation, tests, packaging, and metadata — not from README claims.
Every feature below names the code that implements it and its actual state:
**implemented**, **partial**, **stub/dead**, **unreachable**, or
**doc-only**. Where docs and code disagree, the code is described.

Files read in full: all 26 modules under `src/`, `tests/common.rs` and the
test suite list, `Cargo.toml`, `justfile`, `scripts/verify.sh`,
`tools/gui-smoke.sh`, `tools/contrast.py`, `packaging/*.yml`,
`data/*.desktop`, `data/*.metainfo.xml`, `.github/workflows/flatpak.yml`,
`i18n/*`, `README.md`, `AUDIT.md`, `docs/implementation-brief.md`.

---

## 1. Identity and architecture

| Property | Value | Source |
|---|---|---|
| Product | Gosh AppImage Manager | `Cargo.toml:5`, `README.md:1` |
| Version | `3.0.0` | `Cargo.toml:3`, `src/limits.rs:46` (`env!("CARGO_PKG_VERSION")`) |
| Application ID | `com.goshapps.AppImageManager` | `src/limits.rs:40`, `src/gui.rs:631` |
| Executable | `gosh-appimage-manager` | `Cargo.toml:16-18`, `src/limits.rs:41` |
| License | GPL-3.0-or-later | `Cargo.toml:6`, `COPYING` |
| Public identity | Gosh Apps / Gosh-Its-Arch | `Cargo.toml:9` |
| Language | Rust 2021, MSRV `1.75` declared | `Cargo.toml:4,10` |
| GUI toolkit | libcosmic `v0.12` (pinned git tag), iced-based, wgpu renderer | `Cargo.toml:39` |
| Registry | SQLite via rusqlite (bundled) | `Cargo.toml:31`, `src/registry.rs` |
| Settings | JSON file | `src/settings.rs` |
| HTTP | reqwest blocking + rustls (no native TLS) | `Cargo.toml:32` |

**Crate layout** (`Cargo.toml:12-18`): library `goshaim_core` + binary
`gosh-appimage-manager`. One binary serves both CLI and GUI.

**Feature gate**: `gui = ["dep:libcosmic", "dep:tokio"]`, off by default
(`Cargo.toml:20-24`). Without it, `src/gui.rs` is not compiled
(`src/lib.rs` gates `pub mod gui` on the feature) and the binary is CLI-only.
A non-GUI build invoked with no CLI command prints a "This build has no GUI"
notice plus usage and exits with code **2** — it deliberately does not wait
on stdin (`src/main.rs:158-186`).

**Concurrency model** (GUI): `Arc<Mutex<AppController>>`; all disk / process /
hash / network work is dispatched to `tokio::task::spawn_blocking`
(`src/gui.rs:313-336`). Cancellation is an `Arc<AtomicBool>` flipped by the
Cancel button (`src/gui.rs:1160-1169`). The mutex serializes all mutating
work.

**Seams** (`src/controller.rs:23-32`): `ProcessRunner`, `NetworkClient`,
`ProcessTable`, `TrashSink` are boxed traits injected via
`AppController::with_seams` (`controller.rs:48-68`); tests substitute
`FakeRunner`, `FakeNetwork`, `FakeTable`, `FakeTrash`.

**cosmic-text patch**: `Cargo.toml:54-55` pins `cosmic-text = "=0.13.2"`
over libcosmic's git dependency because the vendored iced tracks a moved API.

---

## 2. CLI

Entry: `src/main.rs`. CLI mode is entered when **any** argv token equals one
of the 14 command flags (`main.rs:11-30`). Flags are matched by exact token
anywhere in argv — there is no parser, so position and combination are loose.

### 2.1 Commands

| Command | Behavior | Exit codes |
|---|---|---|
| `--list-update-managers` | Prints the six manager names, one per line (`static`, `github`, `gitlab`, `codeberg`, `forgejo`, `ftp`) — `cli.rs:175-180` | 0 |
| `--list-installed [--json]` | Tab-separated `name\tpath\tversion`, or JSON. Each row's `running` flag is computed live via one process-table pass (`cli.rs:182-209`, `controller.rs:335-343`) | 0 |
| `--list-updates [--json]` | Check-only scan; `name\tcurrent -> available` or JSON. Prints each check failure to stderr; returns **8** if any check failed even though the JSON document is complete (`cli.rs:211-291`) | 0, 8 |
| `--list-discovered [--json]` | Discovered apps: `managed\|external\tname\tpath` or JSON objects `{name,path,managed,uuid,origin,desktop_path}` (`cli.rs:293-335`) | 0 |
| `--adopt <path> [--yes]` | Confirms, inspects (validates magic), then registers without touching disk (`cli.rs:337-385`) | 0, 1, 5, 6; usage → 2 |
| `--integrate <path> [flags]` | Confirm → conflict resolution → `controller.integrate` (`cli.rs:387-495`) | 0, 1, 5, 6, 4 (replace target not found); usage → 2 |
| `--update <path\|uuid> [--yes] [--force]` | Single-app apply; "running" detected by string-matching the error (`cli.rs:545-581`) | 0, 1, 4, 5, 7 |
| `--update --all [--yes] [--force]` | Check all, apply each offer; per-item errors; skipped-running counted (`cli.rs:497-543`) | 0, 1, 5, 7 |
| `--remove <path\|uuid> [--yes] [--delete]` | Confirm (Trash or "Permanently delete") → `remove_app` (`cli.rs:650-689`) | 0, 1, 5; usage → 2 |
| `--remove-all [--yes] [--delete]` | One confirmation for all owned apps; per-item loop with summary `Removed X of Y; Z failed` (`cli.rs:584-648`) | 0, 1, 5 |
| `--set-update-source <path> --manager <name> key=value...` | Only argv tokens after `--manager <name>` are parsed as config (`cli.rs:691-733`) | 0, 4, 6 |
| `--set-update-source <path> --unset` | Clears manager + config (`cli.rs:700-707`) | 0, 1, 4 |
| `--fetch-updates` | Check-only; prints count+offers+failures to **stderr**, sends desktop notification when offers exist (`cli.rs:736-772`) | 0, 8 |
| `--self-test` | Readiness + synthetic ELF + registry round-trip + desktop markers + URL guard + manager count + JSON envelope; prints `SELF_TEST_OK`/`SELF_TEST_FAIL` (`cli.rs:780-865`) | 0, 1 |
| `--probe-host` | Runs `true` through the host-spawn path; prints `host_spawn_program`, `host_spawn_exit`, `in_flatpak`, `managed_folder`, `HOST_PROBE_OK`/`FAIL` (`cli.rs:867-897`) | 0, 1 |
| `--probe-inspect <path>` | Emits a JSON document then `INSPECT_NO_EXECUTION` (or `INSPECT_EXECUTED_UNSAFE` + exit 1 if the fallback ran — unreachable in practice, see §6.4) (`cli.rs:899-971`) | 0, 1 |
| `--probe-autostart` | Renders the autostart entry to stdout and verifies its Exec line; writes nothing (`cli.rs:973-1009`) | 0, 1 |

Also: `--version` / `-V` prints `3.0.0`, exit 0; `--help` / `-h` prints usage,
exit 0 — both short-circuit **before** command detection and win over any
combination (`main.rs:61-68`). `-V`/`-h` are undocumented in README.

### 2.2 Flags and parsing details

- `--yes` / `-y` bypass confirmation. Without it, destructive actions on a
  non-TTY print `Refusing destructive action without a TTY; pass --yes` to
  stderr and exit **5** (`cli.rs:75-99`). On a TTY a single `y`/`Y`
  keystroke confirms; anything else refuses.
- `--integrate` conflicts: `--keep-both`, `--replace`, `--replace-uuid UUID`,
  `--target PATH`. With `--replace` and no explicit target, the CLI
  re-inspects the file for `existing_managed_id`, then falls back to matching
  an owned app by source filename; ambiguity → exit **6** with
  "Replace is ambiguous" (`cli.rs:415-473`).
- Conflict failures after integrate are detected by **substring-matching**
  `result.error` for `"keep-both"`/`"replace"` → exit 6 (`cli.rs:488-489`) —
  fragile string coupling, same pattern used for "running" → exit 7
  (`cli.rs:516,576`).
- `option_path` (`cli.rs:26-60`) skips known flags while scanning for the
  path argument; unknown `--foo` tokens are skipped, so an unrecognized
  option is silently ignored rather than rejected.
- Unknown first-order commands can't happen (main.rs only enters CLI mode on
  known flags); inside `run_cli` an unmatched combination falls to
  "Unknown command" → exit **2** (`cli.rs:774-775`).
- No argument produces usage text *per command* on stderr.

### 2.3 Stream discipline

stdout = machine output only (lists, JSON, probe documents, `SELF_TEST_OK`).
stderr = prompts, errors, progress, diagnostics, FTP warning, `--fetch-updates`
report. `diagnostics::write_if`/`emit_if` emit `cli:`/`update:` lines to
stderr only when `debug_logging` is set (`cli.rs:173,185`, etc.).

### 2.4 Exit codes (`src/types.rs:77-89`)

| Code | Name | Used for |
|---|---|---|
| 0 | Ok | success |
| 1 | Failure | operation failed; controller-init failure (`main.rs:77-83,118-124`) |
| 2 | Usage | missing path argument; unknown command; no-GUI hint (`main.rs:185`) |
| 3 | NotFound | **defined, never returned** — dead enum variant |
| 4 | NotIntegrated | `--update`/`--set-update-source` on an unknown path/uuid; `--replace` with no owned target |
| 5 | NeedsConfirmation | confirmation declined or no TTY |
| 6 | Validation | invalid candidate, ambiguous replace, bad update-source config |
| 7 | Running | update blocked because app is running (single + all-skipped batch) |
| 8 | Network | `--list-updates`/`--fetch-updates` when any check failed |

### 2.5 JSON schemas (`schema_version: 1`, `cli.rs:101-156`)

- `installed`: `{name, path, desktop_id, current_version, available_version,
  download_size, manager, embedded_source, running, uuid, owned}` — note
  `available_version`/`download_size` come from registry columns that are
  never populated (§9.5).
- `updates`: `{name, path, desktop_id, current_version, available_version,
  download_size, manager, embedded_source, running}`.
- `discovered`: `{name, path, managed, uuid, origin, desktop_path}` with
  `origin ∈ {managed-folder, external-entry}`.
- `--probe-inspect`: `{schema_version, path, size, sha256, type,
  architecture, magic_valid, name, error, unsafe_fallback, extractor}`.

### 2.6 Probes and self-test — all non-mutating

`--self-test` performs a real registry upsert+remove round-trip inside the
isolated data dir, parses a synthetic ELF fixture, builds a desktop entry,
checks URL guard reject/accept, counts six managers, and checks the JSON
envelope shape. `--probe-autostart` renders but never writes
(`controller.rs:429-441`, `cli.rs:980-1009`) — fixed after an audit finding
that it used to enable background checks and install a real entry.

---

## 3. GUI (`src/gui.rs`, 2704 lines; feature-gated)

### 3.1 Shell

- Six nav destinations: **Library, Inspect, Updates, Tasks, Settings, About**
  (`gui.rs:62-83`, nav model built at `642-656`).
- Window: 1024×768 default, minimum 420×420 (`gui.rs:38-47`). Condensed
  layout via `core.is_condensed()` stacks rows (`narrow()`, `gui.rs:399`).
- Window title `Gosh AppImage Manager — <page>`; the in-window header title
  is deliberately blanked because it measured 3.38:1 contrast (below WCAG AA)
  (`gui.rs:1636-1651`).
- Startup: loads library, applies saved appearance, opens Inspect on the
  first positional file and queues the rest (`gui.rs:707-730`).
- libcosmic `single-instance` feature is enabled (`Cargo.toml:39`) and the
  desktop file sets `SingleMainWindow=true`; second-launch file forwarding
  depends on libcosmic's D-Bus activation handling — not verified in-tree.

### 3.2 Worker model

`spawn()` (`gui.rs:313-336`) → `tasks_mut().begin()` records a task →
`tokio::task::spawn_blocking` → `Message::Finished` → `handle_finished`
(`gui.rs:1464-1628`) updates state, finishes the task record, and refreshes
the task list. **One `busy` slot**: starting a second worker while one runs
replaces `self.busy`, orphaning the first task's cancel flag — the Cancel
button then cancels only the newest worker (`gui.rs:322`). Workers still
serialize on the controller mutex, so correctness holds; only cancellation
targeting is lossy.

### 3.3 Keyboard and input

`subscription()` (`gui.rs:743-771`):
- **Ctrl+O** → file-chooser dialog (multi-select; `to_file_path` filters out
  non-local URLs; all-local-empty → "Only local files can be inspected").
- **Ctrl+R** and **F5** → refresh library. **Ctrl+F** → refresh updates.
- **Escape** → dismiss dialog.
- Window `FileDropped` events → `Message::FileDropped` → `plan_drop`
  (`src/drop_queue.rs`): first file inspects immediately when idle;
  otherwise everything queues behind current work/an unconfirmed result.
  Drops force-switch the nav to Inspect.

No menu accelerators beyond these; no shortcut for Launch/Remove/Update.

### 3.4 Library page (`view_library`, `gui.rs:1696-1820`)

- Search field filters name/path/version substring (case-insensitive)
  (`visible_library`, `gui.rs:403-428`).
- Sort buttons: Name, Version, Updates first (`SortOrder`, `gui.rs:219-239`).
- Badges per row: `running`, `update available`, `external folder`,
  `adopted` (`badges_for`, `gui.rs:2635-2650`).
- Row actions: **Launch**, **Details**, **Trash**.
- Empty/loading states and no-match state are distinct.
- "Not managed yet" section lists unregistered discoveries with origin text
  ("in the managed folder" / "outside the managed folder · <desktop path>")
  and an **Adopt** button → confirmation dialog (`gui.rs:1774-1817`).

### 3.5 Detail page (`view_detail`, `gui.rs:1865-2021`)

Facts: Path, Desktop ID, SHA-256, Type, Architecture, Version, Size, Update
manager, Embedded source, Provenance ("adopted"/"integrated here").

Actions: Launch; Reveal in file manager (`xdg-open <parent dir>` via
`controller.reveal_in_file_manager`, `controller.rs:200-214` — opens the
containing folder, does not select the file); **Check and update now**
(actually `apply_update` — it re-checks internally then applies);
Refresh metadata; Move to Trash; Delete permanently (destructive).

Editors:
- **Command arguments** — "one per line" caption, but rendered with
  single-line `widget::text_input` (`gui.rs:1965`); newlines can't be typed,
  though pasted multi-line text parses per-line. Saved via
  `set_arguments_and_environment` (`controller.rs:277-329`) which enforces
  `MAX_ARGUMENTS`/`MAX_ARGUMENT_LENGTH`/`MAX_ENV_PAIRS` and valid env names,
  then rewrites the owned desktop file.
- **Environment** — same single-line input limitation; invalid names are
  rejected with a status error listing them (`gui.rs:1047-1057`).
- **Update source** — manager name field + single-line `key=value` field +
  Apply/Reset (`gui.rs:1990-2018`). Because the config field is single-line,
  **only one key=value pair can be entered**, so `github` (needs
  username+repo+filename), `gitlab`, `codeberg`, and `forgejo` configs
  effectively cannot be completed from the GUI — only `static`/`ftp`
  (url-only) or an empty config that inherits embedded fields succeed
  validation. Fields are app-global inputs, not populated from the selected
  app (`gui.rs:294-295`).

### 3.6 Inspect page (`view_inspect`, `gui.rs:2023-2094`)

Path input + Browse + Inspect; queue-count caption; summary rows (Path, Size,
Type, Architecture, SHA-256, Name, Version, Update source, Already managed);
warnings list; **Integrate** button only after a successful inspect. There is
**no per-file unsafe-extraction confirmation UI** anywhere (see §6.4).

### 3.7 Updates page (`view_updates`, `gui.rs:2096-2197`)

Check now / Update all. Check failures are listed **above** any "Everything
is up to date" claim. Offer rows show `current → available (manager)`,
"running — updating replaces the file under a live app", "reduced
verification: no checksum published", download size, and a per-offer Update
button. Update-all runs sequentially in one worker, checks the cancel flag
between items, and reports `Updated N; M failed` (`gui.rs:1090-1133`).

### 3.8 Tasks page (`view_tasks`, `gui.rs:2199-2242`)

Task history from `TaskQueue` (running/cancelling get a progress bar; errors
shown; "Clear finished" prunes). Note: nothing ever calls
`tasks.progress()`, so progress bars sit at 0 until finish — progress is
recorded only by `run_task` (CLI path, which doesn't use it either).

### 3.9 Settings page (`view_settings`, `gui.rs:2244-2407`)

- **Appearance**: System/Light/Dark buttons, applied live via
  `cosmic::app::command::set_theme` and persisted (`gui.rs:364-371,1271-1277`).
- **Integration folder**: managed-folder text field + Apply (must be
  absolute; empty/relative rejected with status error); Max size (MB) field
  parsed by `limits::parse_max_appimage_mb` (strict, 1–32768).
- **Behaviour**: move-instead-of-copy; discover-outside-folder (retriggers a
  library scan); terminal-app `.AppImage` suffix drop; verbose diagnostics
  (emits one line immediately so the switch is observable).
- **Update checks**: background-checks toggle (turning it off also removes
  the autostart entry, `gui.rs:1195-1208`); login autostart toggle reflects
  `autostart_desktop_path().exists()` and calls `sync_autostart`.
- **Unsafe extraction fallback**: toggling on opens a warning dialog
  ("executes untrusted code … each file still has to be confirmed") →
  Enable persists the setting. **But no per-file confirmation flow exists**,
  so the setting can never actually take effect (§6.4).
- A settings load error from startup is shown as an error status
  (`gui.rs:681,704`); failed saves surface via `apply_setting`
  (`gui.rs:379-390`).

### 3.10 About page (`view_about`, `gui.rs:2409-2450`)

Version, "Made by Gosh", description, safety paragraph, Gear Lever
attribution, GPL notice, "No telemetry". **No viewable license text** —
the brief asked for it; only a statement is shown.

### 3.11 Dialogs (`view_dialog`, `gui.rs:2452-2569`)

All presented on the shell's dialog surface: IntegrateConflict (Keep both /
Replace <name> — replace button only when exactly one candidate resolved /
Cancel); Remove (Trash vs permanent wording names the file); UnsafeExtract;
UpdateForce (running app); Adopt. Escape cancels any dialog.

### 3.12 Status bar

"Working: <task>…" + Cancel while busy; otherwise `Note:`/`Done:`/`Error:`
prefixed text + Dismiss — severity is carried by words, not color alone
(`gui.rs:1655-1694`).

---

## 4. Inspection and extraction (`src/inspector.rs`, `src/elf.rs`)

### 4.1 Validation pipeline (`inspector.rs:30-192`)

1. `symlink_metadata` → must be a regular file; symlinks resolved through
   `canonical_bounded` (8-hop limit, `safe_fs.rs:334-354`); the resolved
   target must itself be a regular file.
2. Size: `>0` and `≤ options.max_bytes` (default 8 GiB, configurable).
3. `elf::parse_file` reads ≤ `ELF_HEADER_READ_BYTES` (1 MiB; hard-capped at
   16 MiB, `elf.rs:270`) → ELF magic, class (32/64), endianness, `e_machine`
   → architecture, AppImage magic `AI\x01`/`AI\x02` at ELF offset 8 →
   Type1/Type2. Program headers (≤128) and section headers (≤256) are walked
   to bound `payload_offset`; any section 1–4095 bytes whose content looks
   like `gh-releases-zsync|`, `zsync|`, or `bintray-zsync|` is captured as
   `.upd_info` (a heuristic — section names are never checked,
   `elf.rs:188-202`).
4. `magic_valid` requires known type **and** known arch — so i386/ARM
   AppImages fail inspection as invalid, not merely "unsupported"
   (`elf.rs:241-254`, `inspector.rs:97-110`).
5. SHA-256 streamed with cancel checks every MiB (`safe_fs.rs:293-316`).
6. Metadata extraction via external tools — never by executing the AppImage.

### 4.2 Extractor dispatch (`inspector.rs:275-286,359-396,398-481`)

| Type | Lister/extractor |
|---|---|
| Type 1 | `7zz l -ba <file>` / `7zz x -o<dest> <file> <members>` |
| Type 2 + squashfs magic (or no dwarfs magic) | `unsquashfs -l` / `unsquashfs -q -d <dest> -f <file> <members>` |
| Type 2 + DwarFS magic → relabeled `Dwarfs`; `AppImageType::Dwarfs` | **`dwarfsextract` selected as a name — but `list_archive` and `extract_members` have no arm for it** → "No safe lister for extractor dwarfsextract" error. **DwarFS metadata extraction can never succeed** even though `dwarfsextract`/`dwarfsck` are bundled in the Flatpak (`inspector.rs:284` vs `365-382,412-432`). |

Extracted member policy: `pick_desktop_member` prefers a top-level
`*.desktop`, then any `.desktop`; `.DirIcon` is requested alongside
unconditionally (`inspector.rs:290-295`). Members are validated by
`valid_archive_member` (rejects absolute, `..`, `-`-leading names that would
be read as tool switches, overlong, depth > `MAX_ARCHIVE_DEPTH*16`,
`safe_fs.rs:358-390`). Collected output is bounded: ≤12 files, ≤8 MiB total;
extracted symlinks are never followed (`inspector.rs:446-480`).

Icon: `<icon_name>.{png,svg,xpm}` member lookup, else `.DirIcon` with content
sniffing (PNG magic, `<svg`/`<?xml`, `/* XPM */`); staged into a private
`icon-*` dir under `$TMPDIR/gosh-appimage-manager`, ≤2 MiB
(`inspector.rs:307-352`). Stale `inspect-*`/`icon-*` dirs older than 1 h are
swept on each extraction (`inspector.rs:540-564`).

Metadata (`metadata_from_desktop`, `inspector.rs:485-505`): Name (≤256
chars), X-AppImage-Version, Comment, sanitized Icon name, Terminal, Url→
website, StartupWMClass, Categories/MimeType (item allowlist chars, ≤32),
Exec→`exec_arguments` (drops program + `%x` field codes), Desktop Actions
(id charset `[A-Za-z0-9-]`, ≤16; only their *arguments* are kept — action
Exec programs are rewritten to the managed path, `inspector.rs:640-668`).

`.upd_info` (`parse_upd_info`, `inspector.rs:752-812`): `|`-separated; hint
maps to github/gitlab/codeberg/forgejo/ftp/static; `gh-releases-zsync`
fills username/repo/release/filename; `zsync`/`bintray`/bare-URL →
`static{url}`; `ftp` → `{url}`; unknown hints → `fieldN` slots.

### 4.3 `--probe-inspect` guarantee

The probe always sets `allow_unsafe_extract=false`, so its
`INSPECT_NO_EXECUTION` marker is real for the path it exercises; the
`INSPECT_EXECUTED_UNSAFE` branch exists but is unreachable from the probe.

### 4.4 Unsafe `--appimage-extract` fallback — **implemented but unreachable**

`extract_via_appimage` (`inspector.rs:201-255`) genuinely runs
`<file> --appimage-extract` in a private 0700 work dir with a 30 s timeout,
then parses `squashfs-root/*.desktop` + icon. It requires
`allow_unsafe_extract && confirm_unsafe_extract` (`inspector.rs:161-162`).
`confirm_unsafe_extract` is **hardcoded `false`** at every production call
site: GUI worker (`gui.rs:454-459`), `IntegrationService::inspect_only`
(`integration.rs:61-66`), CLI `--replace` re-inspect (`cli.rs:430-435`),
`--probe-inspect` (`cli.rs:906-910`), `InspectOptions::default()`
(`types.rs:243-244`). No GUI dialog sets it per file. So:

- The Settings toggle persists a preference nothing can satisfy;
- the "each file still has to be confirmed individually" caption and the
  dialog text describe a flow that does not exist;
- the only executions are the gated tests in `tests/test_inspector.rs`.

This is the audit's fixed stub returning in a subtler form: the feature is
real code but has no reachable trigger. Effectively **dead feature**.

### 4.5 Known extraction edge cases

- `.DirIcon` is always requested with the desktop member; if a tool exits
  non-zero when one member is absent, extraction fails wholesale for
  AppImages lacking `.DirIcon` (behavior depends on the real tools — only
  fake-runner output is exercised in tests).
- `7zz` listing parse takes the last whitespace-separated token of each line
  (`inspector.rs:696-700`) — filenames containing spaces in Type-1 archives
  are mis-parsed.

---

## 5. Integration (`src/integration.rs`)

Flow: validate → inspect (`inspect_only`, unsafe fallback off) → arch check
→ `mkdir_0700` managed dir → conflict gate → stage sibling temp → verify
size+SHA-256 → build `InstalledApp` (reusing UUID and preserving
arguments/env/update-config/actions on replace, `integration.rs:218-264`) →
stage+install icon and desktop entry → backups when replacing → registry
snapshot → race-checked commit → registry upsert → drop backups →
best-effort `update-desktop-database` (`integration.rs:551-570`) →
move-mode trash of the source.

- **Naming**: `sanitize_file_base(source filename)` + `.AppImage` suffix
  enforced; conflicts on existing path or registry path.
- **KeepBoth**: `choose_keep_both` suffixes `-2`, `-3`, …
  (`integration.rs:165` + helper).
- **Replace**: requires an owned registry entry; destination = the old
  managed path; commit uses `rename_over` only when the canonical paths
  match, else `rename_no_replace` (hard-link-then-unlink, refuses if the
  destination appeared meanwhile, `safe_fs.rs:194-217`, `integration.rs:357-369`).
- **Rollback**: on commit/registry failure, restores AppImage/desktop/icon
  backups or removes newly created ones, restores the registry snapshot,
  sweeps `.gosh-*` temps (`integration.rs:384-429`). Injected fail points:
  AfterStage, DesktopWrite, DesktopInstall, BackupCreate, BeforeCommit,
  RegistrySave, SourceDelete (`types.rs:104-114`; exercised by
  `tests/test_rollback.rs`).
- **Ownership markers** written into generated desktop files:
  `X-Gosh-AppImage-Manager=true`, `X-Gosh-AppImage-Id=<uuid>`,
  `X-Gosh-Managed-Path=<path>` (`desktop.rs:280-286`, `limits.rs:42-44`).
- **Move mode** trashes the source only after successful commit; if source
  and destination canonicalize equal, it is skipped; trash failure →
  `partial` result, never a delete (`integration.rs:441-473`).
- `desktop::install_files` stages desktop (0644) and icon
  (`256x256/apps/gosh-appimage-<uuid>.<png|svg>`) via sibling-temp rename
  (`desktop.rs:346-396`). Icon extension is coerced to png/svg — an `xpm`
  staged icon is installed with a `.png` name.
- `InspectOptions` for integration never enables the unsafe fallback.

---

## 6. Launch (`src/launch.rs`)

- `launch()` requires a non-empty managed path; builds argv =
  `[managed_path] + app.arguments`; env = `app.environment` filtered to
  valid names **minus** `LD_*`, `DYLD_*`, `GCONV_PATH`, `LOCPATH`, with
  values free of NUL/control chars and ≤4096 bytes (`launch.rs:60-68,89-100`).
- `HostSpawn::ManagedAppImage` → `host_spawn_permitted` requires absolute
  path + existing regular file (`process.rs:77-96`); inside Flatpak the argv
  becomes `flatpak-spawn --host --env=K=V … <path> …` (`process.rs:130-145`).
- **Start-only detached**: `setsid()` in `pre_exec`, stdio to /dev/null, and
  a `goshaim-reap` thread `wait()`s the child so no zombie accumulates
  (`process.rs:182-204,345-368`). The manager never waits on or kills the app.
- NixOS: if `/etc/NIXOS` exists, probes `appimage-run --version`; on
  refused/timeout → error; otherwise launches via `appimage-run` as a Helper
  (`launch.rs:23-59`). A non-zero-but-spawned probe exit code is accepted.
- `is_running` uses canonical managed path vs `/proc/*/exe`, or
  `pgrep -x -f <path>` on the host in Flatpak (`launch.rs:27-34`,
  `proctable.rs:165-202`). In Flatpak a failed host probe falls back to the
  sandbox's own (blind) `/proc` scan — best effort, not strictly fail-closed;
  `running_among` batches via one `pgrep -a -f .AppImage` round trip
  (`proctable.rs:92-124,204-216`).
- Dead code: `let _ = &mut args;` at `launch.rs:69`.

**Asymmetry**: desktop-entry `Exec=` embeds env pairs unfiltered
(`desktop.rs:58-74` writes `NAME=value` prefixes), while direct launch drops
dangerous names — a `LD_*` pair saved on the detail page affects only the
.desktop-launched path.

---

## 7. Updates (`src/updates_sources.rs`, `src/updates_service.rs`)

### 7.1 Managers (`updates_sources.rs:87-108,921-965`)

Six sources behind `UpdateSource`; `UpdateSourceFactory::names()` =
`static, github, gitlab, codeberg, forgejo, ftp`.

| Manager | Config keys | Endpoint | Asset rules |
|---|---|---|---|
| static | `url` (+`version`) | `.zsync` → control file parse (≤256 KiB), download URL defaults to URL minus `.zsync`; else HEAD for size only | version required for `available`; no version → "No version information" (`updates_sources.rs:112-227`) |
| github | `username`,`repo`,`filename` (req), `release` | `api.github.com/repos/u/r/releases/latest` | asset name exact or `*`/`?` glob; download host must be `github.com`/`*.githubusercontent.com`; digest from `digest` field (`:251-368,429-435`) |
| gitlab | `project` (req), `host` (default gitlab.com), `filename`, `package`/`package_name` | `api/v4/projects/<p>/releases` → assets.links; fallback first `direct_asset_url` ending `.AppImage`; fallback generic packages API (`file_sha256` only) | asset host must equal/be-subdomain-of forge host unless `allow_any_asset_host=true` (`:465-664`) |
| codeberg | `owner`,`repo`,`filename`,`release`,`host`(default codeberg.org) | Forgejo `api/v1/repos/o/r/releases?limit=1` | same forge-host rule (`:768-806`) |
| forgejo | `host` (required by validate), `owner`,`repo`,`filename` | same shape; **default host is the literal `"forgejo"`** — a single-label name that resolves nowhere meaningful when `host` is empty in config but validation is bypassed via embedded fields (`:692-766,808-850`) |
| ftp | `url` (+`version`) | `SIZE`/`RETR` over plaintext FTP | needs `ftp://` URL; version required (`:854-915`) |

Empty `filename` on gitlab/codeberg/forgejo accepts the **first** asset
regardless of name; GitHub requires it. Forge `available` is set whenever an
asset matched — version comparison happens in `list_updates_detailed`
(`updates_service.rs:207-212`).

`config_from_embedded` whitelists keys per manager — `allow_local_network`
and `allow_any_asset_host` can never arrive from `.upd_info`.

### 7.2 Check vs apply

- `check()` resolves `(manager, config)` — stored config wins; empty stored
  config + embedded metadata derives one (`updates_service.rs:105-128`); sets
  `manager`, and marks `reduced_verification` when no parseable SHA-256 was
  advertised (`:141-151`).
- `list_updates_detailed` counts `skipped` (no source), `checked`,
  `failures`, `offers`, `cancelled` separately — "up to date" is never
  claimed for an unchecked app (`:173-234`).
- `apply()` (`:237-453`): owned-only → check → same-version refusal
  ("Already at the latest version") → running guard (`is_running`; `--force`
  overrides) → stream download to sibling `.gosh-upd-` temp at 0600 → chmod
  0755 → ELF/AppImage validation → `architecture_supported` (x86_64/aarch64)
  **and** equality with installed arch when known → SHA-256 verify when a
  usable digest exists; an *unparseable* advertised digest fails closed
  (`:362-376`) → hardlink/copy backup → `rename_over` → rewrite desktop entry
  → registry upsert → drop backup; on any failure after replace, the backup
  is renamed back and the registry snapshot restored. Injected fail points:
  AfterDownload, BackupCreate, AfterReplace, DesktopInstall, RegistrySave
  (`types.rs:116-124`).
- **No delta updates**: zsync control files are parsed for metadata, but the
  payload is always a full-file download (`network.rs:1-4`, README
  limitation confirmed by code).
- `parse_expected_sha256` accepts `sha256:<hex>` and bare 64-hex; MD5/base64/
  truncated values are rejected (`updates_service.rs:58-78`).
- `set_source`/`unset_source` persist with registry snapshot/restore
  (`:455-511`).

### 7.3 Background checks

`--fetch-updates` = `scan_updates` + stderr report + `notify_offers`
(`notify-rust`, count-only body, `notifier.rs:24-37`). Autostart entry
Exec = `<exe> --fetch-updates` or `flatpak run com.goshapps.AppImageManager
--fetch-updates` in Flatpak (`controller.rs:429-441`). Notify-only, as
advertised. The GUI performs no automatic periodic check — the only
"background" path is the autostart entry.

### 7.4 Dead fields

`UpdateCheckResult.{etag,last_modified,digest_algo}` are never assigned by
any source; `UpdateOffer.digest_algo` copies the always-empty field
(`updates_sources.rs:16-29`, `updates_service.rs:228`). Registry columns
`last_update_check`, `available_version`, `available_url`,
`available_size`, `update_available` are written only to be **cleared**
during apply/unset (`updates_service.rs:405-408,501-503`) — nothing ever
populates them, so `--list-installed --json` always emits empty
`available_version`/`0` for `download_size`. No conditional GET
(If-None-Match) exists; every check is a full fetch.

---

## 8. Removal (`src/removal.rs`)

- `resolve` accepts uuid or path **only if owned** (`removal.rs:54-70`).
- Pre-checks: desktop file must carry matching ownership markers; icon path
  must contain the app's uuid (`removal.rs:81-95`).
- **Trash mode**: `trash::delete` (freedesktop via `trash` crate) → fallback
  `gio trash <path>` on the host via flatpak-spawn path (`removal.rs:36-52`);
  any failure leaves everything intact ("Trash failed; leaving files
  intact"). Owned artifacts are removed **after** trash succeeds; registry
  row always removed on success; artifact/registry failure → `partial` with
  an explanatory error (`removal.rs:123-160`).
- **Permanent mode**: canonical target must survive
  `is_forbidden_permanent_target` — refuses `/`, `/home`, `/root`, `$HOME`,
  anything directly inside them, anything at `/` top level, and any path
  with ≤2 components (`removal.rs:188-213`) — plus the managed path itself
  must not be a symlink (`removal.rs:105-112`). Then `fs::remove_file`.
- A missing managed file skips trash entirely and still cleans artifacts +
  registry, reporting `error="already gone"` with `ok=true`
  (`removal.rs:96-121,157-159`) — a slightly odd "success with error text".
- `--remove-all` iterates owned apps per-item with `assume_yes:true`, counts
  removed/failed, summary to stderr, exit 1 if any failed (`cli.rs:584-648`).
- The artifact removal in `remove()`'s second `else if` (`removal.rs:135-140`)
  is unreachable in practice — it's the `has_icon && !has_desktop` case
  already covered above; harmless dead branch.

---

## 9. Persistence

### 9.1 Settings (`src/settings.rs`)

- Path: `<config_home>/gosh-appimage-manager/settings.json`, written
  atomically at mode **0600** (`settings.rs:98,303-334`).
- Keys: `ManagedFolder`, `MoveSource`, `ManageOutsideFolder`,
  `TerminalOmitSuffix`, `BackgroundUpdateChecks`, `UnsafeExtractionFallback`,
  `Appearance` (`system|light|dark`), `DebugLogging`, `MaxAppImageBytes`
  (clamped 1 MiB–32 GiB).
- Corrupt/unreadable file → `load_error` set, defaults used, **file left
  untouched** until a save succeeds; `loaded()` distinguishes absent from
  corrupt (`settings.rs:245-296`). All setters return save errors.
- `Dirs::from_env` honors **`GOSHAIM_HOME`, `HOME`,
  `GOSHAIM_XDG_DATA_HOME`, `GOSHAIM_XDG_CONFIG_HOME`,
  `GOSHAIM_XDG_CACHE_HOME`** — the *standard* `XDG_DATA_HOME`/
  `XDG_CONFIG_HOME`/`XDG_CACHE_HOME` are **never read** (`settings.rs:22-37`).
  The "XDG-aware" comment is inaccurate for real XDG overrides: a user with
  `XDG_DATA_HOME=/elsewhere` still gets `~/.local/share`. Fallback when
  neither GOSHAIM_HOME nor HOME is set: `/root`.
- Layout: data=`~/.local/share`, config=`~/.config`, cache=`~/.cache`;
  applications dir = `data_home/applications`; icons =
  `data_home/icons/hicolor`; registry =
  `data_home/gosh-appimage-manager/registry.sqlite`; autostart =
  `config_home/autostart/com.goshapps.AppImageManager-updates.desktop`.

### 9.2 Registry (`src/registry.rs`)

- SQLite `apps` + `meta` tables (`registry.rs:12-46`), `REGISTRY_SCHEMA_VERSION=1`.
- File + `-wal`/`-shm` sidecars forced to 0600 (`registry.rs:461` area).
- One-time import of adjacent legacy `registry.json` (`schema_version:1`):
  rows lacking uuid or managed_path skipped; legacy file preserved
  (`legacy_app_from_json`, `registry.rs:110-201`; import hook at open).
- Per-row `INSERT OR REPLACE` upsert and `DELETE` per uuid; in-memory
  canonical-path index for `by_path`; `snapshot()`/`restore()` for
  transactional callers (`registry.rs:203-240,514-627` summary area).
- `adopt_external` registers an external path with `owned=true`,
  `adopted=true`, `external_folder` set from whether the file lives outside
  the managed folder — writes no desktop/icon (`registry.rs:609-627`).
- Note: `apps()` ordering is unspecified (no ORDER BY); GUI sorts itself.

### 9.3 Discovery/adoption (`src/library.rs`)

- `scan()` covers the managed folder always; external desktop entries only
  when `manage_outside_folder` is on; owned entries skipped so own installs
  aren't re-offered; dedup via canonical-ish path set; results sorted by path
  (`library.rs:52-75,124-166`).
- `adopt()` validates via registry path check and registers
  (`library.rs:166-196`); CLI additionally inspects magic first
  (`cli.rs:352-367`).
- `external_folder` flag is set on adopted apps and the "external folder"
  badge exists. Adopted apps **are** `owned` (`registry.rs:623`), so update
  and remove apply to them normally — verified live: `--adopt` then
  `--remove` trashes the external file and deletes the registry row while
  leaving the foreign desktop entry untouched. Removal of an adopted app
  trashes the file at its original location, not a copy in the managed
  folder.

---

## 10. Network and SSRF (`src/network.rs`, `src/url_guard.rs`)

- `validate()`: non-empty, no control chars, schemes https (default),
  http (only when `allow_http` — used solely by FTP validation, which then
  requires `ftp://` anyway — and is otherwise never enabled for downloads),
  ftp (explicit), reject `file`/`data`/`javascript`/others; reject
  credentials; reject local-network literals unless `allow_private`
  (`url_guard.rs:26-70`).
- `is_local_ip`: loopback, link-local, private, unspecified, broadcast,
  documentation, CGNAT 100.64/10, 192.0.0.0/24, 198.18/15; IPv6 mapped
  (`::ffff:`) and NAT64 `64:ff9b::/96` unwrapped first; ULA/link-local/loopback
  (`url_guard.rs:77-120`). Hostnames: `localhost`, `*.localhost`, `*.local`,
  `*.internal` (`:122-130`).
- `check_redirect` per hop: no https→http, no credentials, scheme allowlist,
  no local literal host (`url_guard.rs:137-161`); the reqwest custom policy
  also re-resolves each hop's host through `pinned_ip` before following
  (`network.rs:161-183`), `MAX_REDIRECTS=3`.
- `pinned_ip` resolves `(host,port)` via `ToSocketAddrs`, takes the **first**
  answer, checks it, and pins the client via `.resolve()` — DNS rebinding
  between check and connect is closed (`network.rs:138-153,185-201`).
  Note: only the first returned address is checked/used.
- Clients cached per `(host,port,allow_local)` for 60 s, cache cleared at >64
  entries (`network.rs:249-301`).
- `get` bodies bounded to `MAX_JSON_BODY_BYTES` (2 MiB); `head_len` reads
  Content-Length; `download_bounded` caps at min(max_bytes,64 GiB) in memory;
  `download_to_file` streams 64 KiB chunks to a 0600 file with cancel checks
  and deletes partials (`network.rs:82-129,304-409`). FTP downloads still
  buffer fully in memory (`ftp_download` → `Vec`), then stream to file.
- `guard_final_url` re-checks requested vs final on every method
  (`network.rs:237-246`).
- **FTP** (`network.rs:411-627`): plaintext warning to stderr on every use;
  anonymous login tolerating 230-without-password; `TYPE I`; greeting
  consumed; RFC 959 multiline replies with 256-line bound; `SIZE` (5xx→
  unknown size); PASV parsed as six `u8` octets, port must be non-zero, and
  the data connection is **pinned to the control peer IP** (FTP bounce
  rejected). User-Agent: `gosh-appimage-manager/3.0.0`.
- `Local::from_config` honors `allow_local_network ∈ {true,yes,1}` — settable
  only via user-edited config (`network.rs:39-48`).
- `expect_https_marker()` mentioned in the audit: the doc comment on
  `validate` still describes an `expect_https` parameter that does not exist
  (`url_guard.rs:25`).

---

## 11. Desktop integration

- **App's own .desktop** (`data/com.goshapps.AppImageManager.desktop`):
  `Exec=gosh-appimage-manager %F`, MimeType covers
  `application/vnd.appimage`, `application/x-iso9660-appimage`,
  `application/x-appimage`; `SingleMainWindow=true`, `StartupWMClass`,
  `Terminal=false`. `%F` passes local paths/URIs; `file://` URIs are
  normalized to paths in `normalise_open_target` (`gui.rs:2653-2662`).
- **Generated entries** (`desktop.rs:205-315`): `Type=Application`, Name
  (escape + optional `.AppImage` strip for terminal apps), Comment, Exec
  (env prefixes + escaped program + bounded args, `%%`-escaped percent),
  TryExec, Icon (installed path or `application-x-executable`), Terminal,
  Categories (sanitized, fallback `Utility`), MimeType (≤32), StartupWMClass,
  `StartupNotify=true`, `X-AppImage-Version`, ownership markers, Desktop
  Actions (validated ids, Exec rebuilt against managed path).
- Filenames: `gosh-appimage-<uuid>.desktop`; icons
  `gosh-appimage-<uuid>.<ext>` under `icons/hicolor/256x256/apps`.
- `verify_ownership` parses the file and requires the marker trio
  (`desktop.rs:325-343`).
- `update-desktop-database <applications_dir>` best-effort post-integrate
  (`integration.rs:551-570` — a Helper host spawn in Flatpak).
- Bounded desktop parser: ≤64 KiB, ≤4096 lines, ≤512 keys, key ≤128, value
  ≤8192; `[section]` aware; localized `Key[xx]` variants ignored
  (`desktop.rs:127-163`).

---

## 12. Flatpak (`packaging/com.goshapps.AppImageManager.yml`)

- Runtime `org.freedesktop.Platform//23.08`, SDK + rust-stable extension
  declared but a **pinned Rust 1.90.0** tarball (per-arch SHA-256) is what
  actually builds, because the graph needs ≥1.89; `/rust` is cleaned and
  `no-debuginfo: true` works around dwz corrupting rustc's libLLVM.
- Finish-args: ipc, fallback-x11, wayland, dri, network, Flatpak + portal.Desktop
  + Notifications talk-names, own-name, filesystem creates for
  `~/AppImages`, `~/.local/share/{applications,icons,gosh-appimage-manager}`,
  `xdg-config/autostart`. **No `--filesystem=host:rw`** — but
  `--talk-name=org.freedesktop.Flatpak` + `flatpak-spawn --host` is
  effectively host command execution, narrowed only by the in-app
  `HOST_HELPERS`/managed-path policy (`process.rs:33-97`).
- Bundled tools, all SHA-256 pinned per arch: squashfs-tools 4.6.1 built
  from source (unsquashfs); 7-Zip 26.00 binaries + corresponding source +
  license; DwarFS 0.15.3 binaries (`dwarfsextract`, `dwarfsck`) + source +
  licenses. Licenses land in `/app/share/gosh-appimage-manager/licenses/`,
  corresponding sources in `corresponding-source/`.
  **`dwarfsck` is never invoked by the app at all; `dwarfsextract` is
  selected but has no lister/extractor arm (§4.2) — both bundled DwarFS
  binaries are dead weight.**
- App module builds offline (`CARGO_NET_OFFLINE=true`, vendored
  `cargo-sources.json`), installs binary/desktop/metainfo/icons/i18n,
  runs `--self-test` inside the build.
- `.github/workflows/flatpak.yml`: matrix x86_64 + aarch64 (qemu), host-mode
  flatpak-builder, bundles artifacts; fail-fast off.
- README caveat confirmed: managed folder other than `~/AppImages` needs
  portal/document access the manifest doesn't grant.

---

## 13. Safety limits (`src/limits.rs`)

| Limit | Value | Use |
|---|---|---|
| AppImage max bytes | 8 GiB default; 1 MiB–32 GiB clamp | inspect/integrate/download |
| ELF header read | 1 MiB (file capped 16 MiB) | `elf.rs:270` |
| Desktop file | 64 KiB | parse/build/install |
| Extraction | ≤12 files, ≤8 MiB total | `inspector.rs` |
| Icon | 2 MiB | stage/install |
| Process output | 1 MiB/stdout+stderr | `process.rs:264-332` |
| JSON body | 2 MiB | network get |
| Zsync control | 256 KiB | static source |
| Archive listing | 100 000 lines; entries kept ≤ 64×16 | `inspector.rs:686-725` |
| Archive member | path ≤ 1020, depth ≤ 48, no abs/`..`/`-` | `safe_fs.rs:358-390` |
| Redirects | 3 | `network.rs` |
| Timeouts | process/extract/network 30 s; connect 10 s; host pgrep 5 s | various |
| Hash cancel check | every 1 MiB | `safe_fs.rs` |
| Env pairs | 32; name ≤128 charset-checked; value ≤4096 no ctrl | desktop/launch |
| Arguments | 64 × ≤4096 | desktop/detail |
| Name | 256 chars | desktop values |
| Task history | 128, running entries never evicted | `tasks.rs:149-164` |
| Symlink hops | 8 | `safe_fs.rs:334` |
| Glob | pattern ≤256, text ≤512, linear matcher | `updates_sources.rs:370-427` |
| `.upd_info` | 4096 bytes | `elf.rs:191` |
| FTP multiline | 256 lines/reply | `network.rs:494-507` |
| Stale staging sweep | `inspect-*`/`icon-*` >1 h | `inspector.rs:540` |

---

## 14. Environment variables

| Var | Read where | Effect | Documented? |
|---|---|---|---|
| `GOSHAIM_HOME` | `settings.rs:23` | Home override (tests/verify use it) | scripts/verify.sh only |
| `HOME` | `settings.rs:24` | home fallback | implicit |
| `GOSHAIM_XDG_DATA_HOME` / `_CONFIG_` / `_CACHE_` | `settings.rs:27-30` | per-base overrides | **undocumented** |
| `XDG_DATA_HOME` etc. | — | **never read** (see §9.1) | — |
| `GOSHAIM_LOCALE_DIR` | `i18n.rs:194` | first catalog dir | i18n/README.md |
| `LC_ALL`,`LC_MESSAGES`,`LANG` | `i18n.rs:45-46` | locale selection; `LC_ALL=qps` → pseudolocale | i18n/README.md |
| `FLATPAK_ID` | `process.rs:118`, `cli.rs:998` | Flatpak detection (+`/.flatpak-info`) | — |
| `TMPDIR` | via `std::env::temp_dir()` `inspector.rs:202,264` | extraction work parent | — |
| `SKIP_GUI`,`SKIP_FLATPAK` | `scripts/verify.sh` | verify skips | script comment |
| `DISPLAY_NUM`,`OUT`,`RT`,`LIBGL_ALWAYS_SOFTWARE`,`WGPU_BACKEND` | `tools/gui-smoke.sh` | smoke harness | script header |
| `CARGO_BUILD_JOBS`,`CARGO_HOME`,`CARGO_NET_OFFLINE`,`RUSTC` | manifest | Flatpak build | manifest |
| `GOSHAIM_HOST_SPAWN` | — | **does not exist** (earlier suspicion disproven; no such var set or read) | — |

No telemetry/opt-out vars exist — there is no telemetry.

---

## 15. i18n (`src/i18n.rs`, `i18n/`)

- `Locale::from_env` POSIX order LC_ALL > LC_MESSAGES > LANG; `C`/`POSIX` →
  English; codeset/modifier stripped; region `xx-YY` tried before `xx`.
- Catalog = JSON `{id: text}`; dirs searched: `$GOSHAIM_LOCALE_DIR`,
  `/app/share/.../i18n`, `/usr/share/.../i18n`, `./i18n`.
- `tr()` falls back to the English at the call site; `qps` pseudolocale
  accents+pads via `pseudolocalize`.
- **Wired**: `t!` macro is `#[macro_export]`; gui.rs has **143 `t!(…)` call
  sites** covering nav, pages, dialogs, buttons, captions. The earlier
  grep-level suspicion of dead i18n is disproven.
- `is_rtl()` detects ar/he/fa/ur/ps/sd/ug/yi/dv/ckb but **no GUI code calls
  it** — RTL detection exists with no layout consumer.
- Ships only `i18n/qps.json` (3 bytes, `{}`) + README — **zero human
  translations**. English is the only real language.
- CLI strings, errors, and many detail-page labels are hardcoded English
  (machine output intentionally untranslated; some GUI strings like dialog
  titles/bodies and fact labels are also plain literals not routed through
  `t!` — e.g. `gui.rs:2461-2464`, fact labels at `1878-1913`).

---

## 16. Diagnostics

- `diagnostics::format_line`/`write_if`/`emit_if` (`src/diagnostics.rs`):
  `[category] detail` lines to an arbitrary writer (always stderr in
  practice), gated by `debug_logging` setting. `file_label` shortens home
  paths. Enabling the setting in the GUI immediately emits one line
  (`gui.rs:1240-1248`); workers emit on finish (`gui.rs:1620-1624`); CLI
  emits per-op lines.
- There is **no log file** — diagnostics go to stderr only; "diagnostics"
  is a verbosity flag, not a subsystem.
- `--self-test` `readiness()` checks registry path/file, data dir, settings
  load error, managed folder absolute+dir, and all six managers resolve
  (`controller.rs:449-482`) — real checks, not decoration.

---

## 17. Tests (`tests/` — 24 files)

`#[test]` count ≈153 across: `test_cli` (exit codes, JSON shape, confirms),
`test_desktop_tasks` (entry build/parse/ownership, task queue bounds),
`test_detail` (arg/env validation + GUI-side helpers incl. drop queue),
`test_diagnostics`, `test_elf` (magic/classes/archs/upd_info/payload sniff),
`test_ftp` (real RFC 959 server: greeting sync, multiline, PASV bounce,
oversized octets), `test_glob` (linear-time + semantics), `test_i18n`
(catalog load order, pseudo, candidates), `test_icon` (staging, .DirIcon
sniffing, installed bytes), `test_inspector` (invalid/dir/size/magic/no-exec,
fallback gating incl. confirm-path execution), `test_integration`
(conflicts, ownership, move-trash, rollback), `test_launch` (detached,
no-zombie, env filtering, NixOS helper), `test_library` (discovery gating,
adopt), `test_max_bytes` (setting clamp + enforcement), `test_network`
(bounds, redirect policy via fakes), `test_probes` (non-mutating autostart,
host, inspect markers), `test_registry` + `test_registry_perf` (round-trip,
migration, per-row perf), `test_removal` (trash-first, protected paths,
partial), `test_rollback` (injected fail points restore live+registry),
`test_settings` (corrupt/missing/save-fail), `test_ssrf` (hostname→loopback
refused pre-connect, conditional on resolver), `test_update` (github/gitlab
digests, arch refusal, oversize, cancel cleanup, running guard, source
roundtrip, JSON schema).

Harness (`tests/common.rs`): `Harness` = TempDir + shared fakes;
`write_fixture` writes the synthetic 128-byte ELF from
`inspector::make_test_elf` — **never a real AppImage, never executed**.

Not covered by automated tests: real extractor tool behavior (all runner
output is canned), real COSMIC/Wayland rendering (xvfb smoke + pixel
contrast only, `tools/gui-smoke.sh` + `tools/contrast.py` measure WCAG AA on
captured regions), real FTP/HTTP servers outside the in-test fake server,
DwarFS extraction (impossible — no code path), end-to-end `flatpak run`
session behavior.

`docs/verification.md` records: `cargo test` 134 passed/0 failed (count
predates some tests), clippy both feature sets, fmt, offline self-test,
isolated probes, GUI smoke, pseudolocale run; **no** flatpak-builder,
desktop-file-validate, or appstreamcli runs (tools absent) — these are
documented as skipped, not passed.

---

## 18. Documentation audit — source vs docs

| Doc claim | Reality |
|---|---|
| `docs/implementation-brief.md` stack: C++20/Qt6/KF6/Kirigami, `org.kde.Platform//6.10`, version 0.1.0 | **Superseded** — the file itself declares this at the top; no Qt/CMake/QML exists. Its behavioural contract still governs. |
| README "fully localizable" | True mechanism-wise (143 `t!` sites) but **no translations ship**; several GUI strings remain unrouted literals. |
| README/i18n-README "`LC_ALL=qps` accents every letter" | Works for `t!` strings only; hardcoded strings stay ASCII (by design for spotting them). |
| README "per-file confirmation" for unsafe fallback | **No such flow exists** — `confirm_unsafe_extract` is never set in production (§4.4). |
| Adopt dialog/CLI "can be updated and removed here" | Accurate — adopted rows are `owned=true` (`registry.rs:623`); remove/update work (verified live). Note removal trashes the file at its external location. |
| Settings "Move the original into the library" | Honored — but move uses **Trash**, not rename; a cross-filesystem "move" is copy+trash (`integration.rs:441-463`). |
| README "DwarFS extraction depends on bundled dwarfsextract" | The tool is bundled but **no code invokes it**; DwarFS inspection always warns "No safe lister". |
| Metainfo `<translation type="gettext">` | Was removed — metainfo now has no translation claim (good: none exists). |
| README "JSON uses installed/updates arrays" | Also `discovered`; schema accurate. |
| README "exit 8 when a check failed" | Accurate for list/fetch; note `--update` failures use 1/7, not 8. |
| `--help` text `[--remove-all [--yes]]` | Slightly off: `--delete` omitted there (present on `--remove`). |
| AGENTS/audit history claims | `AUDIT.md` documents the pre-fix state incl. stubs; all marked fixed items verified present **except** the fallback reachability caveat above. |
| `docs/verification.md` "134 tests" | Currently ~153 `#[test]`s — doc count is stale low. |
| Rust MSRV `1.75` vs `edition2021`/deps | `Cargo.toml:10` declares 1.75 but libcosmic-graph deps need ~1.89; the Flatpak ships Rust 1.90.0. `cargo build` without gui on 1.75 likely still works (core deps), but MSRV is untested for the gui feature. |
| Desktop file `%F` | Correct; earlier `%U` bug fixed (`AUDIT.md` C-9); file-chooser still normalizes `file://`. |

---

## 19. Dead, stubbed, ineffective, or surprising behavior

1. **Unsafe fallback unreachable** (§4.4) — full implementation, real
   execution path, zero production callers setting `confirm_unsafe_extract`.
   The Settings toggle + warning dialog + README text describe a two-step
   gate whose second step doesn't exist.
2. **DwarFS tools bundled but unusable** — `dwarfsextract`/`dwarfsck`
   shipped, pinned, licensed; code names `dwarfsextract` then fails with "No
   safe lister". `dwarfsck` is referenced nowhere. `AppImageType::Dwarfs`
   files therefore inspect with a metadata warning only (type still
   detected via payload magic, `elf.rs:308-313`).
3. **Adopted apps delete the original file** — adoption registers with
   `owned=true`, so "Move to Trash"/"Delete permanently" act on the file
   at its external location. Consistent with the adopt text, but worth
   documenting since the file never enters the managed folder.
4. **GUI single-line inputs vs multi-line data** — arguments, environment,
   and update-source config are described "one per line"/"key=value" but use
   `widget::text_input`; only paste can carry newlines. Multi-key manager
   configs (github/gitlab/codeberg/forgejo) are effectively unsettable in
   the GUI.
5. **Vestigial update-state columns** — `last_update_check`,
   `available_*`, `update_available`, `digest`/`reduced_verification` exist
   in schema+JSON but only `digest`/`reduced_verification` are ever written
   (on apply). ETag/Last-Modified fields exist end-to-end and are never
   populated; no conditional requests.
6. **Task progress never progresses** — `TaskQueue::progress` has no
   caller; progress bars only ever show 0→100 across finish. `Queued` state
   is unreachable (`begin` always sets Running).
7. **`ExitCode::NotFound` (3) dead**; `is_rtl` unused by the GUI;
   `let _ = &mut args` (`launch.rs:69`); unreachable icon-cleanup branch
   (`removal.rs:135-140`); stale `expect_https` doc comment
   (`url_guard.rs:25`); `make_test_elf(_, Dwarfs)` produces a Type-2 file
   (no DwarFS payload magic emitted — `inspector.rs:827-830`).
8. **`busy` single-slot** — a second worker started while one runs orphans
   the first's cancel flag (§3.2); correctness unaffected, cancel
   granularity lossy.
9. **`.DirIcon` unconditional extract** — extraction may fail entirely on
   tools that error for a missing member (§4.5).
10. **Fail-closed gaps (minor)**: Flatpak running-detection falls back to
    blind `/proc` when the host probe fails (§6); only the first DNS answer
    is pinned (§10); GitLab/forgejo `filename` empty → first asset wins (§7.1);
    forgejo default host `"forgejo"` is a meaningless single label; NixOS
    `appimage-run` probe accepts non-zero exit codes.
11. **Error-string-driven exit codes** — `contains("keep-both")`,
    `contains("running")` etc. couple user-facing wording to process
    semantics.
12. **Settings "XDG-aware" but ignores `XDG_*_HOME`** — only `GOSHAIM_XDG_*`
    overrides and `HOME`/`GOSHAIM_HOME` are honored (§9.1).
13. **No periodic in-app update check** — "background checks" means only the
    autostart `--fetch-updates` entry; the GUI never self-checks on a timer.
14. **Undocumented extras**: `-h`/`-V` short flags; `--probe-inspect` emits
    `INSPECT_NO_EXECUTION` provenance marker; `INSPECT_EXECUTED_UNSAFE`
    exists but unreachable; self-test does a real registry write+remove in
    the data dir; `Local` accepts `yes`/`1` spellings of
    `allow_local_network`; reveal opens the parent folder rather than
    selecting the file.

---

## 20. Final report — most surprising findings

- The headline safety feature "unsafe extraction fallback with per-file
  confirmation" is **two gates deep, and the second gate has no UI**: code
  exists, tests exercise it, the Settings dialog promises it — and no
  production path can ever run it.
- The Flatpak bundles and licenses two DwarFS binaries the code **cannot
  invoke**; DwarFS AppImages are detected, classified, and then fail
  metadata extraction every time.
- The Flatpak's real sandbox escape hatch is `flatpak-spawn --host` (needed
  for launching), narrowed only by an in-process allowlist — a deliberate,
  documented-in-code trade-off, not a manifest hardening.
- The headline safety feature "unsafe extraction fallback with per-file
  confirmation" is two gates deep, and the second gate has no UI.
- The registry faithfully stores `last_update_check`/`available_*`/
  `update_available` forever-empty, and the update pipeline plumbs
  ETag/Last-Modified/digest-algo fields no source ever fills.
- i18n is genuinely wired (143 `t!` call sites, runtime catalogs,
  pseudolocale) yet ships **zero translations** and several high-visibility
  strings (dialogs, fact labels) bypass it anyway.
- The app claims an XDG-aware layout but reads **none** of the standard
  `XDG_*_HOME` variables — only its own `GOSHAIM_XDG_*` set.
- The Flatpak's real sandbox escape hatch is `flatpak-spawn --host` (needed
  for launching), narrowed only by an in-process allowlist — a deliberate,
  documented-in-code trade-off, not a manifest hardening.
