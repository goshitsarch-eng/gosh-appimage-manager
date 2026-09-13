# Documentation audit — Gosh AppImage Manager 3.0.0

Audit of every user- and developer-facing documentation claim in the
repository against the actual source and packaging. Source code is the
source of truth; documentation is treated as potentially wrong.

Method: every claim was checked by reading the cited code (all of `src/`,
`Cargo.toml`, `Cargo.lock`, `packaging/`, `data/`, `justfile`,
`scripts/`, `tests/`, `.github/workflows/`). No build or test could be
run in this environment (no cargo/rustc/flatpak-builder installed), so
dynamic claims are judged by code paths, not re-execution. No files were
modified; this report is the only artifact.

Verdicts: **VERIFIED** (claim matches the code), **OUTDATED** (was true
when written, is not now), **INCORRECT** (contradicts the code),
**MISLEADING** (technically defensible but invites a wrong conclusion),
**INCOMPLETE** (omits material facts), **UNVERIFIABLE** (cannot be checked
from the tree), **REDUNDANT** (duplicates another document at drift risk).

---

## 1. `README.md` (233 lines)

### Identity / top matter (lines 1–22)

| Claim | Verdict | Evidence |
|---|---|---|
| Version 3.0.0, Rust + libcosmic (iced-based) | VERIFIED | `Cargo.toml:3`, `Cargo.toml:39` (libcosmic tag `v0.12`) |
| Application ID `com.goshapps.AppImageManager`; executable `gosh-appimage-manager`; GPL-3.0-or-later; Gosh-Its-Arch | VERIFIED | `limits.rs:40-46`, `Cargo.toml:2,6,9`, `data/*.desktop:6-7`, manifest `id:` line |
| "Opening an AppImage never integrates or executes it" | VERIFIED | `inspector.rs:274-286` — extraction only ever runs `unsquashfs`/`7zz` on the file; `gui.rs:2026-2028` Inspect caption says the same |
| "Integration is transactional" | VERIFIED | `integration.rs` stage→commit→rollback; `tests/test_rollback.rs` (4 injected failure points) |
| Unsafe fallback opt-in + per-file confirm + default off | VERIFIED | `settings.rs:107` default `false`; `gui.rs:2523-2539` `PendingDialog::UnsafeExtract`; `inspector.rs` gated path |
| Gear Lever reference-only, not endorsed, nothing copied | VERIFIED | Consistent with `AGENTS.md`, `docs/upstream-audit.md`; no vendored Gear Lever content in tree |
| "Zero telemetry … network requests only to update metadata endpoints you configured" | MISLEADING (minor) | `updates_sources.rs` `config_from_embedded` adopts a source embedded in the AppImage's `.upd_info` — not one "you configured". Behaviour is as described otherwise; wording should say "configured or embedded" |

### Build / Test (lines 24–64)

| Claim | Verdict | Evidence |
|---|---|---|
| "Requires a Rust toolchain (see `rust-version` in `Cargo.toml`)" | MISLEADING | `Cargo.toml:10` says `rust-version = "1.75"`, but the committed `Cargo.lock` pins `notify-rust 4.18.0` (requires rustc 1.89), `trash 5.2.7` (1.85), `uuid 1.26.0` (1.85), `icu_* 2.3.x` (1.88), `zbus 5.19.0` (1.87). Even the CLI-only build cannot compile on 1.75. The real minimum is ≈1.89. |
| GUI build needs network once (libcosmic checkout) + Wayland/XKB dev files | VERIFIED (with gap) | `Cargo.toml:22-24,39` git dep + comment. `docs/verification.md:76` lists three Debian packages (`libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev`); README:125-127 lists only two — **INCOMPLETE** list there |
| No Qt/KDE runtime anywhere in tree | VERIFIED | No Qt/CMake deps in `Cargo.toml`; `AGENTS.md` forbids the stack |
| Tests use fake seams + synthetic fixtures, never touch real HOME | VERIFIED | `tests/common.rs`; `settings.rs:23-30` `GOSHAIM_*` overrides; 22 suites under `tests/` |
| `tools/gui-smoke.sh` needs xvfb, xdotool, ImageMagick | INCOMPLETE (minor) | Script also runs `tools/contrast.py` → needs `python3` too |
| `just` recipe list | VERIFIED | All named recipes exist in `justfile` (`release`, `fmt-check`, `self-test` also exist but README says "common flows", not exhaustive) |

### CLI (lines 66–105)

Every documented flag exists and semantics match `src/main.rs:11-26` and
`src/cli.rs`:

| Claim | Verdict | Evidence |
|---|---|---|
| Command list (`--integrate`, `--update`, `--remove`, `--remove-all`, `--list-installed`, `--list-updates`, `--list-discovered`, `--adopt`, `--list-update-managers`, `--set-update-source`, `--fetch-updates`, `--self-test`, `--probe-host`, `--probe-inspect`, `--probe-autostart`) | VERIFIED | `main.rs:11-26` dispatch table; help text `main.rs:32-54` |
| `--version` prints `3.0.0` | VERIFIED | `main.rs:61-63` → `limits::VERSION` = `CARGO_PKG_VERSION` = `3.0.0` |
| `--json` → `schema_version: 1` + `installed`/`updates`/`discovered` arrays | VERIFIED | `cli.rs:145`, `limits.rs:31` (`JSON_SCHEMA_VERSION = 1`) |
| `--fetch-updates` non-mutating, stderr notice | VERIFIED | `cli.rs` fetch path calls `check_updates` + `notify_offers` only |
| Diagnostics to stderr, stdout stays valid JSON | VERIFIED | `cli.rs:1-4` contract, `confirm()` writes to stderr (`cli.rs:75-99`) |
| `--list-updates`/`--fetch-updates` exit 8 on failed checks | VERIFIED | `types.rs` `ExitCode::Network = 8`; `cli.rs:287-289` |
| `--list-discovered` gated on outside-folder setting; `--adopt` inert | VERIFIED | `library.rs:52-75,166-189` |
| Probes non-mutating | VERIFIED | `run_autostart_probe` renders/verifies without installing; `run_inspect_probe` inspects only; `cli.rs:962-965` markers |
| Undocumented: `-y` alias for `--yes` | INCOMPLETE (minor) | `cli.rs:30-32` accepts `-y`; README doesn't mention it |
| Destructive actions need TTY or `--yes`, fail closed | VERIFIED | `cli.rs:75-99` |

### GUI (lines 107–133)

| Claim | Verdict | Evidence |
|---|---|---|
| Six pages: Library/Inspect/Updates/Tasks/Settings/About | VERIFIED | `gui.rs` `Page` enum |
| Worker threads, window keeps repainting, Cancel works | VERIFIED | `Command::perform` workers; `tasks.rs:86` `mark_cancelling`; `gui.rs:1160-1166` Cancel wiring |
| Tasks page shows running/recent with progress + errors | VERIFIED | `gui.rs:2125+` `view_tasks` |
| Detail page: launch, reveal, check/update, refresh metadata, args/env edit, source set/reset, path/desktop id/hash/type/arch/size/manager/provenance | VERIFIED | `gui.rs:1878-1912` facts, `1925-2018` actions/sections |
| Library search + sorting | VERIFIED | `gui.rs:403-425` filter + 3 `SortOrder`s |
| Outside-folder files listed for explicit adoption | VERIFIED | `library.rs:63-72`; `PendingDialog::Adopt` `gui.rs:2554-2562` |
| Destructive choices go through modal dialogs naming the exact file | VERIFIED | `gui.rs:2484-2562` (Remove names `{name}`/`{path}`; UnsafeExtract; UpdateForce; Adopt; conflict dialog) |
| Appearance System/Light/Dark, live + startup | VERIFIED | `types.rs` `Appearance`; `gui.rs` `set_theme` apply at init + on select |
| Positional files → Inspect; several files one at a time; drop queues the same way | VERIFIED | `gui.rs:707+` initial-files queue; `gui.rs:763-766,847-855` `FileDropped` → `drop_queue::plan_drop`; `gui.rs:2652-2662` `%U`/`file://` normalisation |

### Flatpak (lines 135–168)

| Claim | Verdict | Evidence |
|---|---|---|
| Manifest path, freedesktop 23.08 + Rust | VERIFIED | `packaging/com.goshapps.AppImageManager.yml:2-3` |
| One manifest builds x86_64 + aarch64 | VERIFIED | Per-arch pinned sources, no `only-arches` restriction |
| Vendored deps via `packaging/cargo-sources.json` + `just vendor` | VERIFIED | `justfile:38-39`; manifest `CARGO_NET_OFFLINE` |
| No `--filesystem=host:rw` | VERIFIED | Manifest finish-args (see §15 below) |
| "Extraction tools (unsquashfs, 7zz, dwarfsextract) are pinned by SHA-256 with per-arch binaries" | MISLEADING (two ways) | (a) `unsquashfs` is compiled from a SHA-256-pinned **source** tarball, not shipped as a per-arch binary. (b) `dwarfsextract` (and `dwarfsck`) are bundled but **unreachable** — see Limitations row below. |
| "Corresponding source tarballs and license texts are installed beside the binaries" | MISLEADING (minor) | Installed under `/app/share/gosh-appimage-manager/{licenses,corresponding-source}` — present but not literally "beside" the binaries |
| Rust 1.90.0 pinned because the dep graph outgrew SDK's 1.81; `no-debuginfo` rationale | VERIFIED | Manifest comments + `rust-gosh` module; consistent with `Cargo.lock` requirements (needs ≤1.89, pins 1.90.0) |

### Upgrading from 2.x (lines 170–178)

| Claim | Verdict | Evidence |
|---|---|---|
| SQLite registry at `~/.local/share/gosh-appimage-manager/registry.sqlite`, mode 0600 | VERIFIED | `settings.rs:57-59,232-234`; `registry.rs:462-482` (`restrict_mode` also covers `-journal`/`-wal`/`-shm` at open and save; dir created `0700` at `registry.rs:273`) |
| Adjacent v2 `registry.json` imported once, rows without UUID/managed-path skipped, legacy untouched | VERIFIED | `registry.rs:277-336` — only when the SQLite file is fresh; `schema_version ≤ 1` accepted (missing = 0 OK); skips invalid/duplicate rows; never writes back |
| Settings → `~/.config/gosh-appimage-manager/settings.json` | VERIFIED | `settings.rs:98`, atomic `0o600` write at `settings.rs:333` |
| Managed files/desktop entries/icons reused in place | VERIFIED | Legacy import carries `managed_path`/`desktop_path`/`icon_path` fields (`registry.rs:110-195`) |
| Environment-variable overrides | INCOMPLETE | `GOSHAIM_HOME`, `GOSHAIM_XDG_DATA_HOME`, `GOSHAIM_XDG_CONFIG_HOME`, `GOSHAIM_XDG_CACHE_HOME` all relocate the documented paths (`settings.rs:23-30`); only `docs/verification.md:20` mentions them |

### Safety (lines 180–210)

| Claim | Verdict | Evidence |
|---|---|---|
| Regular-file + ELF + AppImage magic required | VERIFIED | `elf.rs:67-118`, `inspector.rs` |
| 8 GiB default bound, adjustable 1 MB–32 GB in Settings | VERIFIED | `limits.rs:4-6,48-73`; `gui.rs:2295-2314` Settings row + caption |
| Archive `..`/absolute/symlink-escape rejected | VERIFIED | `safe_fs.rs` `valid_archive_member`, `mkdir_0700` squat checks |
| Exec built from tokens, no shell strings; `%` escaped | VERIFIED | `desktop.rs:12-74` (`escape_exec_arg` emits `%%`; `build_exec_line` argv-only); `process.rs` argv arrays |
| Trash failure never becomes delete; permanent delete extra-confirmed + protected paths refused | VERIFIED | `removal.rs` trash-first ordering; `trash.rs:1-2` contract; `gui.rs:2484-2506` dialog |
| Resolved-address pinning + per-hop redirect checks; loopback/private needs `allow_local_network` on user-created source; embedded can never set it | VERIFIED | `network.rs` pin + `check_redirect`; `url_guard.rs` address classes; `updates_sources.rs` embedded-config guard |
| SHA-256 `sha256:`/bare-hex digest verify, uninterpretable refused, foreign arch refused, reduced-verification display | VERIFIED | `updates_service.rs` (digest parse, arch check, `reduced_verification` flag); `gui.rs:2165-2166` label |
| **"A probe that fails means 'cannot tell', never 'not running'"** | **MISLEADING** | `proctable.rs:195-201`: when the `flatpak-spawn` host probe fails, `pids_for_executable` **falls through to the local `/proc` scan** — which inside the sandbox always returns empty, i.e. *is* treated as "not running". The `Option` ("cannot tell") signal is discarded before callers see it. The doc-comment intent exists (`proctable.rs:156-158`) but the API (`Vec<u32>`) cannot express it. In the shipped Flatpak config a failed probe degrades the guard to "not running" — exactly what the README denies. |
| Launch start-only detached, never wait-and-kill | VERIFIED | `launch.rs`, `process.rs` |
| Updates stage → validate → atomic replace → rollback kept until success | VERIFIED | `updates_service.rs` |
| Running apps block updates unless `--force` | VERIFIED (with caveat above) | `updates_service.rs:216,272` `is_running` gate — effective when the probe works |
| Unsafe `--appimage-extract` fallback default-off, gated, never in tests/background | VERIFIED | `inspector.rs` fallback path; `settings.rs:107`; dialog `gui.rs:2523-2539` |
| Failed integration removes created artifacts and restores replaced ones | VERIFIED | `integration.rs` rollback; `test_rollback.rs` |

### Limitations (lines 212–225)

| Claim | Verdict | Evidence |
|---|---|---|
| Zsync metadata understood; full-file download | VERIFIED | `network.rs` zsync bound (`MAX_ZSYNC_BYTES`); `updates_service.rs` full download |
| GUI never used on a real compositor | VERIFIED (honest) | Consistent with `docs/audit/PLAN.md` PLAN-001 |
| Only English shipped | VERIFIED | `i18n/` contains only `qps.json` + README |
| Static/FTP without version info reports "no version information" | VERIFIED | `updates_sources.rs:183,207,349,898` |
| FTP legacy + warning; credentials rejected | VERIFIED | `url_guard.rs`, `updates_sources.rs` FTP manager |
| Managed-folder change may need portal access under Flatpak | VERIFIED | Manifest grants only `~/AppImages:create` |
| Background checks notify only | VERIFIED | `notifier.rs:1-2,23-37`; `--fetch-updates` path |
| **"Type 1 ISO and DwarFS extraction depend on the bundled 7zz and dwarfsextract tools"** | **INCORRECT for DwarFS** | `inspector.rs:275-286` selects `"dwarfsextract"` for DwarFS payloads, but `list_archive` (`inspector.rs:359-396`) and `extract_members` (`inspector.rs:398-432`) have **no `dwarfsextract` arm** — both hit the catch-all `Err("No safe lister/extractor …")`. Safe DwarFS extraction therefore always fails; the bundled `dwarfsextract`/`dwarfsck` binaries are dead payload (`dwarfsck` is not referenced anywhere in `src/`). Type 1 via `7zz` does work (`inspector.rs:369-376,423-430`). |

### Attribution (lines 227–233)

| Claim | Verdict | Evidence |
|---|---|---|
| Gear Lever commit `a2917f2…` + GPL-3.0-or-later credit | VERIFIED | Matches `docs/upstream-audit.md:4` and `.grok/task-prompt.md:10` |

**README verdict: keep, with fixes.** Three material fixes needed:
(1) Rust requirement pointer/`rust-version` mismatch; (2) the "cannot
tell" running-probe claim; (3) the DwarFS/`dwarfsextract` limitation is
untrue as written. Plus minor: incomplete Debian package list, `-y`
alias, embedded-source wording, `python3` smoke dep.

---

## 2. `AGENTS.md`

Engineering rules, not prose claims. Every checkable statement verified:
3.0.0 / Rust + libcosmic / app ID / GPL-3.0-or-later (`Cargo.toml`),
freedesktop 23.08 single-manifest both arches (manifest), safety
invariants all enforced in code (trash-first `removal.rs`, argv-only
`process.rs`/`desktop.rs`, ownership markers `desktop.rs`/`removal.rs`,
transactional updates `updates_service.rs`, unsafe-fallback default-off
`settings.rs:107`).

**VERIFIED — keep.**

---

## 3. `AUDIT.md` (root, 393 lines)

Dated, branch-scoped record: "Audit date: 2026-09-07. Branch:
`claude/repo-end-to-end-audit-mplc6u`" (line 3). It is a *completed*
audit record: findings table all `fixed`/`withdrawn`, "Fixes applied"
lists 19 commits, "Final gate state" records 148 tests/24 suites.

- The findings/measurements are internally consistent and were
  spot-verified against current code (e.g. S-14 registry mode now
  enforced at `registry.rs:357,462-482`; C-9 desktop `%U`→`%F` fix
  shipped in `data/*.desktop:6`).
- **OUTDATED details:** file inventory "25 source files, 10,188 lines"
  (line 34) is now 29 files / ~12,845 lines; "79 tests (12 suites)"
  baseline vs 22 suites today; Phase-3 "Partially wired"/"Stub, dead"
  tables describe the *pre-fix* state with no per-row status — a reader
  can misread "Tasks page does not exist", "no drag-and-drop exists",
  "unsafe fallback is a warnings.push() stub" as current facts;
  "No Flatpak build" scope limit (lines 370-372) was later closed by
  `docs/migration/REPORT.md` and CI.
- Its own meta-claim (line 24-30) that `implementation-brief.md` "still
  specifies C++20/Qt6" remains true — the brief now carries a
  superseded-banner instead of a rewrite.

**Verdict: keep, marked historical.** Add/keep a header noting the
snapshot date and that Phase-3 tables describe the pre-fix state.
Do not treat line references as current.

---

## 4. `docs/implementation-brief.md`

Now carries an explicit, accurate banner: "**Status: superseded on
stack, current on behaviour**" (lines 3-18) — it names exactly which
sections are obsolete (§1 stack, §3 Qt types, §9 KConfig, §10 KDE
runtime, §13 Qt Test) and which still bind. Checked against code:

- Behavioural requirements all verified implemented (shell pages,
  inspection flow, detail page, transactional integration, trash-first
  removal, update semantics, CLI surface, settings list, unsafe-fallback
  gating).
- Deviations under the disclaimer: §11 asks for `Exec=… %U`
  (`data/*.desktop:6` ships `%F` — deliberate fix C-9, better); §11 asks
  for developer identity "Gosh-Its-Arch" while metainfo ships
  `<name>Gosh</name>` (`metainfo.xml:6-8`); §15 acceptance broadly met.

**VERIFIED (as superseded-but-binding spec) — keep.**

---

## 5. `docs/rewrite-3.0.0.md`

Rewrite record. Module map matches `src/` (all named modules exist;
`diagnostics.rs`, `drop_queue.rs`, `i18n.rs` are later additions not in
the map — the map documents the *rewrite*, so this is expected).
Deliberate-change claims verified: cosmic-text `=0.13.2` `[patch]`
(`Cargo.toml:51-55`), freedesktop 23.08 + rust-stable extension, pinned
Rust 1.90.0 with the 1.89-graph rationale, `no-debuginfo` reason — all
match the manifest. Verification-status section is a snapshot consistent
with `docs/migration/REPORT.md`.

**VERIFIED — keep (historical record).**

---

## 6. `docs/upstream-audit.md`

Gear Lever inventory is consistent (`a2917f2`, v4.6.2, GPL-3+, Python/GTK4).
Safety-lessons list matches what the code implements.

- **OUTDATED:** line 9 — "Gosh AppImage Manager is a new native
  C++20/Qt 6/Kirigami implementation." The tree is Rust + libcosmic and
  this file lacks the superseded disclaimer `implementation-brief.md`
  now carries.

**Keep — fix line 9** (or add the same superseded banner).

---

## 7. `docs/verification.md`

Dated-style verification record (toolchain rustc 1.94.1). Spot-checks:

- "134 passed (20 suites)" — historical count; current tree has
  `common.rs` + 22 suites. Record-style, acceptable.
- Probe transcripts (§5) verified against `cli.rs`/`main.rs` output
  formats; `--probe-autostart` now non-mutating as claimed.
- §7 pseudolocale claim — VERIFIED mechanism (`i18n.rs:169-185,250-315`;
  see §10 below for the `qps.json` wrinkle).
- **INCORRECT:** lines 7-8 — "`cargo clippy` enforces that MSRV and
  passes." Cargo/clippy do not enforce `rust-version` against the
  toolchain, and the declared 1.75 is itself wrong for the locked deps.
  The sentence implies 1.75 is a real, enforced floor — it is neither.
- **OUTDATED:** §8 "No Flatpak build … neither architecture was built"
  — superseded by `docs/migration/REPORT.md` (x86_64 build + packaged
  probes passed) and CI. Also `%U`→`%F` desktop change did ship.
- `GOSHAIM_HOME` documented here — the only doc that does.

**Keep, marked historical.** Fix or annotate the MSRV sentence.

---

## 8. `docs/audit/` (11 files — the 2026-09-12 Phase-2 set + hardening)

### `ARCHITECTURE.md`
- Diagram + data flows + invariants: VERIFIED against `lib.rs`,
  `controller.rs`, `gui.rs` Command::perform workers.
- **INCORRECT:** "Zero `unwrap/expect/panic` in `src/`" (line 39) —
  `registry.rs:368` `expect`, `tasks.rs:146` `expect`, `removal.rs:99`
  `unwrap`, plus fake-seam unwraps (`trash.rs:53-59`,
  `proctable.rs:231,240`, `process.rs:423`, `network.rs:645+`). The
  claims are true *in spirit* (no unguarded production panics) but
  literally false as written. Same absolute claim in `BASELINE.md:127`
  and `BUGS.md:31`.
- **OUTDATED:** ARCH-03 "debug-logging has no backend" — `diagnostics.rs`
  now exists and is wired (BUG-001 fixed). ARCH-04 `Page::title` — deleted
  (BUG-003/PLAN-008 done).

### `BASELINE.md`
Dated 2026-09-12 snapshot (commit `d11deaf`). Structure/module list
mostly accurate ("26 modules" vs actual 25 core + cli + gui — trivial
miscount). But its "Advertised vs visible" table went stale **within the
same round it describes**:
- "Drag/drop open — **absent**" (line 115): INCORRECT —
  `gui.rs:763-766,847-855` + `src/drop_queue.rs` implement it
  (PLAN-007 done).
- "Debug-logging … **nothing consumes it**" (line 117): INCORRECT —
  `src/diagnostics.rs`, wired into CLI/GUI (PLAN-003 done).
- "MaxAppImageBytes … **no UI/CLI control**" (line 118): INCORRECT —
  Settings row at `gui.rs:2295-2314` (PLAN-006 done).
- "No `expect/unwrap/panic/todo` in `src/` (searched)" (line 127):
  INCORRECT — see ARCHITECTURE entry.
- `rust-version = "1.75"` restated uncritically (line 10).
- "150 tests" / 22 suites: historical, plausible for that commit.

### `BUGS.md`
- BUG-001/002/003 fixed — verified (`diagnostics.rs`,
  `write_statements` counter at `registry.rs:485-496`, `Page::title`
  gone).
- **INCORRECT:** "No `expect/unwrap/panic/todo` in `src/` — zero hits"
  (line 31) — same counterexamples as above.
- **OUTDATED:** "MaxAppImageBytes … user-unsettable" (lines 36-38) —
  PLAN-006 done; GUI row exists.

### `COSMIC-UX.md`
The stalest file in the set — several "Current/Problems" entries are
pre-hardening state:
- "no drag-and-drop … → PLAN-007; planned change: add dnd target
  **if** libcosmic/iced supports it" (lines 47-49): OUTDATED —
  implemented via `window::Event::FileDropped` (PLAN-007 done).
- "debug-logging switch does nothing (BUG-001)" (line 76): OUTDATED.
- "80 `t!()` lookups" (line 106): OUTDATED — ~143 `t!` calls in
  `gui.rs` now.
- Verified still-true: 6 pages + nav_bar; modal `PendingDialog` via
  `view_dialog`; 3 sort orders (`gui.rs:220-236`); labelled togglers;
  System/Light/Dark live + startup; compositor gaps correctly deferred
  to PLAN-001.

### `DECISIONS.md`
All 20 standing decisions verified consistent with code and other docs
(incl. decision 18 drop-queue — matches `drop_queue.rs`; decision 16
std-only diagnostics — matches `diagnostics.rs`). **VERIFIED.**

### `FEATURES.md`
Feature matrix, newest state — rows verified against code including the
late additions (drag/drop done row 45, debug-logging done row 47,
MaxAppImageBytes row 48, JSON schema row 49, exit-8 row 50).
- **MISLEADING:** row 25 "fail-safe 'cannot tell'" — same overstatement
  as README (probe failure inside Flatpak degrades to "not running").
- Minor: "80 calls" (row 43) stale count.

### `PACKAGING.md`
Verified against the manifest end-to-end (finish-args, pins, `/rust`
cleanup, `no-debuginfo`, i18n `install -Dd` split, in-builder
`--self-test`, verify.sh chain, CI shape, `repo-x86_64/` removal).
**VERIFIED.**

### `PERFORMANCE.md`
Prior-round fix table verified (worker commands, cancel wiring, client
reuse, targeted SQL, batch proc scan, bounded join, linear glob).
- **INCONSISTENT:** PERF-01 listed "Open" while `BUGS.md` BUG-002 and
  `PLAN.md` PLAN-002 record it done (structural SQL-write gates +
  serialization implemented — `registry.rs:485-496`).

### `PLAN.md`
Live plan; status mix is accurate: done = PLAN-002/003/006/007/008;
open = PLAN-001 (compositor, env-blocked), PLAN-004 (translator),
PLAN-005 (portal env), PLAN-009 (backlog), PLAN-010 (release env).
Counts table consistent. **VERIFIED — keep.**

### `REPORT.md`
Audit-hardening report, `audit-hardening` branch. Internally coherent
before/after structure; fix claims verified (`diagnostics.rs`,
`drop_queue.rs`, `parse_max_appimage_mb`, `write_statements`). Its
unwrap claim (line 60) is the *accurate* phrasing: "remaining unwraps
are test fakes or a guarded `is_none` check" — true (`registry.rs:368`
is guarded by the preceding `is_none()` check). "160 passed" is a
historical count. "Work is uncommitted for review" (line 91 area) is a
session note, not a durable fact.

**VERIFIED — keep (historical).**

### `SECURITY.md`
Controls table verified against code (SSRF pinning + per-hop redirects,
`allow_local_network` opt-in path, archive-member argv safety, Exec `%`
escaping, trash-first, digest/arch refusal, 0600 registry incl.
sidecars, bounded everything). SEC-01 residual correctly describes the
Flatpak host-spawn grant as a non-boundary.
- **MISLEADING:** row "probe failure = 'cannot tell', never 'not
  running'" (line 29) — same overstatement as README/FEATURES.

---

## 9. `docs/migration/` (3 files — 2026-09-10 record)

### `PARITY_BASELINE.md`
Dated Task-1 snapshot. Per-item PASS mapping verified plausible against
current code. "Drag/drop … remaining unknown" and the
`bulk_removal_is_linear` ~4.4s flake were true at that date and were
later resolved (PLAN-007/PLAN-002). **Keep — historical.**

### `PLAN.md`
Work-plan for the completed parity round. Accurate for its date, but:
- **OUTDATED presentation:** "There is no `scripts/verify.sh` yet"
  (line 56) — exists now; the entire feature-parity checklist
  (lines 174-209) is left unchecked `[ ]` although
  `docs/migration/REPORT.md` records every item done. The plan still
  *reads* as pending work.
- Context facts verified: Rust+libcosmic stack, Gear Lever ref-only,
  freedesktop 23.08 single manifest, `just` recipe list.

**Keep — mark historical; tick or annotate the checklist.**

### `REPORT.md`
Completion report. Claims verified: `verify.sh` stage list matches the
script; `cargo-sources.json` regeneration story consistent with
PACKAGING.md; i18n `install -Dd` fix consistent with the manifest;
perf-bound raise rationale consistent with `test_registry_perf.rs` and
PLAN-002; "work is uncommitted for review" was a session note.
**Keep — historical.**

---

## 10. `i18n/README.md` vs `src/i18n.rs` + `i18n/qps.json`

| Claim | Verdict | Evidence |
|---|---|---|
| Catalog = JSON id→text map; `de-AT.json` tried before `de.json` | VERIFIED | `i18n.rs:96-103` candidates, `126-146` loader |
| Fallback to call-site English for missing keys | VERIFIED | `i18n.rs:169-180` |
| Lookup dirs in order: `GOSHAIM_LOCALE_DIR`, `/app/share/…`, `/usr/share/…`, `./i18n` | VERIFIED | `i18n.rs:192-201` `catalog_dirs` |
| Locale from `LC_ALL` → `LC_MESSAGES` → `LANG`; `C`/`POSIX` = no localization | VERIFIED | `i18n.rs:44-53,64-73` |
| `LC_ALL=qps` run accents every letter, pads ~⅓ | VERIFIED — but the mechanism is misdescribed | `i18n.rs:183-185` `pseudo()` keys off the locale **name**; `pseudolocalize` (`i18n.rs:250-315`) does the accent/pad in code. `i18n/qps.json` is literally **`{}` (3 bytes, empty)** — the file provides no translations; the README's phrasing "`qps.json` is a pseudolocale where every ASCII letter is replaced" attributes the transform to the file. Functionally the documented run still works (the empty file even selects the `qps` locale, though `active()` at `i18n.rs:222-225` would pseudolocalize anyway). **MISLEADING.** |
| CLI/JSON output stays English | VERIFIED | `cli.rs` emits literals, not `t!()` |
| `is_rtl()` reported so layout can respond | VERIFIED (carefully worded) | `i18n.rs:152-154`; note: **nothing in `gui.rs` consumes `is_rtl()`** — "can respond" is honest, no claim that it does |

**Keep — clarify that the pseudolocale transform lives in code and that
`qps.json` is an empty marker file.**

---

## 11. `.grok/task-prompt.md`

The original Grok mission prompt for the retired **2.x C++20/Qt6/KF6/
Kirigami** implementation: `/root/projects/…` paths, branch
`grok/appimage-manager`, `org.kde.Sdk//6.10`, Qt tests, CTest. None of it
describes the current tree; it carries no historical/historical banner.
It is an artifact of the 2.x process, not documentation of 3.0.0.

**OUTDATED / historical artifact — remove or move under an archive with
a note; at minimum it should not sit unmarked next to live docs.**

---

## 12. `packaging/com.goshapps.AppImageManager.yml` (comments)

Every user-facing/manifest claim verified: runtime
`org.freedesktop.Platform//23.08` + `org.freedesktop.Sdk`; finish-args
exactly `--share=ipc`, `--socket=fallback-x11`, `--socket=wayland`,
`--device=dri`, `--share=network`,
`--talk-name=org.freedesktop.Flatpak`, portal + notifications +
own-name talk-names, `~/AppImages:create`,
`~/.local/share/{applications,icons,gosh-appimage-manager}:create`,
`xdg-config/autostart:create`; **no `host:rw`**; `no-debuginfo: true`
with the rustc-1.90/23.08-splitter rationale; Rust 1.90.0 pinned per
arch (SDK's 1.81 too old for the ≤1.89 dep graph — verified against
`Cargo.lock`); bundled `unsquashfs` (compiled from pinned source),
`7zz` + `dwarfsextract`/`dwarfsck` (per-arch pinned binaries);
`CARGO_NET_OFFLINE` vendored build; in-builder `--self-test`; `/rust`
cleaned from final image. One caveat that belongs in docs, not the
manifest: the bundled `dwarfsextract`/`dwarfsck` are unreachable from
`src/` (see README limitations row).

**VERIFIED — keep.**

---

## 13. `justfile` comments

Verified: recipe names match README; "GUI needs network once" matches
`Cargo.toml:22-23,39`; `vendor` uses the official generator;
`flatpak-*` recipes run the single manifest per-arch; `self-test`
builds then runs the debug binary; `validate` runs both validators.

**VERIFIED — keep.**

---

## 14. `scripts/verify.sh` comments

Verified against the script body: stage order (build →
`test --no-fail-fast` → clippy default → clippy-gui gated on
`pkg-config wayland-client xkbcommon` → fmt → isolated-HOME
`--self-test` → `desktop-file-validate` → `appstreamcli` → GUI
build+smoke gated on Xvfb/xdotool → flatpak-builder x86_64);
`SKIP_GUI`/`SKIP_FLATPAK`; loud-skip semantics; `--disable-rofiles-fuse`
rationale comment matches the Flatpak environment constraint documented
in `docs/migration/REPORT.md`. Consistent with PACKAGING.md's
description.

**VERIFIED — keep.**

---

## 15. `data/*.desktop` + `data/*.metainfo.xml`

Desktop file: `Exec=gosh-appimage-manager %F`, three AppImage
MIME types, `SingleMainWindow`, `StartupWMClass=com.goshapps.AppImageManager`
— all consistent (`%F` is the deliberate C-9 fix; `gui.rs:2652` comment
still says `%U` — stale comment, harmless). **VERIFIED.**

Metainfo: id/name/licenses/launchable/mediatypes/URLs consistent with
the product; no `<translation>` tag (false gettext claim was removed per
AUDIT.md). Caveats:
- `<display_length compare="ge">360` — `narrow()`/`is_condensed()`
  exists (`gui.rs:399-401`) and smoke tests drive 430px, but real 360px
  compositor behaviour is unverified (PLAN-001) — **partially
  UNVERIFIABLE**.
- `<control>touch</control>` — UNVERIFIABLE from code.
- Screenshot URL `https://goshapps.com/screenshots/…png` — external,
  UNVERIFIABLE.
- Developer name `Gosh` vs brief's "Gosh-Its-Arch" identity — minor
  deviation (implementation-brief §11).

**Keep — verified with minor unverifiable external claims.**

---

## 16. `.github/workflows/flatpak.yml`

Comments match the workflow exactly: builds both arches on
push(main)/PR/dispatch; host-not-container build for disk space
(jlumbroso/free-disk-space + `--user` flatpak); qemu-user-static for
aarch64; `fail-fast: false`; `--repo=repo` needed for `build-bundle`;
per-arch bundle artifacts uploaded. Consistent with
`docs/audit/PACKAGING.md` and `BASELINE.md` descriptions. **VERIFIED.**

---

## Summary of the most important discrepancies

| # | Doc claim | Reality | Files |
|---|---|---|---|
| 1 | `rust-version = "1.75"` is the toolchain floor | Locked deps need rustc ≥1.89 (notify-rust 4.18.0, icu 2.3, zbus 5.19, trash 5.2.7, uuid 1.26.0); Flatpak pins 1.90.0 for exactly this reason | `Cargo.toml:10`; `README.md:26`; `docs/verification.md:7-8`; `BASELINE.md:10` |
| 2 | Failed running-app probe means "cannot tell", never "not running" | `pids_for_executable` falls back to a sandbox-local `/proc` scan that returns empty inside Flatpak → treated as "not running"; the `Option` signal is dropped (`proctable.rs:195-201`) | `README.md:199-201`; `SECURITY.md:29`; `FEATURES.md:25` |
| 3 | DwarFS extraction "depends on the bundled dwarfsextract" | `list_archive`/`extract_members` have no `dwarfsextract` arm — safe DwarFS extraction always errors; `dwarfsck` is never referenced in `src/` | `README.md:225`; `inspector.rs:275-284,377-431` |
| 4 | "Zero/No unwrap/expect/panic in src/" (multiple files) | `expect` at `registry.rs:368`, `tasks.rs:146`; `unwrap` at `removal.rs:99` and in shipped fake seams (`trash.rs`, `proctable.rs`, `network.rs`, `process.rs`) | `ARCHITECTURE.md:39`; `BASELINE.md:127`; `BUGS.md:31` |
| 5 | Drag/drop "absent" / debug switch "does nothing" / max-size "unsettable" | All three implemented: `gui.rs:763+` + `drop_queue.rs`; `diagnostics.rs`; `gui.rs:2295-2314` | `BASELINE.md:115-118`; `COSMIC-UX.md:47-49,76`; `BUGS.md:36-38` |
| 6 | `cargo clippy` "enforces" the MSRV | No such enforcement; MSRV claim itself wrong (see #1) | `docs/verification.md:7-8` |
| 7 | "No Flatpak build" / "no scripts/verify.sh" | Both exist since the 2026-09-10 migration round | `docs/verification.md:173-176`; `docs/migration/PLAN.md:56` |
| 8 | "new native C++20/Qt 6/Kirigami implementation" | Tree is Rust + libcosmic; disclaimer missing here | `docs/upstream-audit.md:9` |
| 9 | `qps.json` "is a pseudolocale" | File is empty `{}`; the transform is hard-coded in `i18n.rs:250-315` keyed on the `qps` locale name | `i18n/README.md:29-30`; `i18n/qps.json` |
| 10 | PERF-01 "Open" | Fixed per BUG-002/PLAN-002 (structural gates) | `PERFORMANCE.md:25-33` vs `PLAN.md:25-33` |
| 11 | Debian GUI deps = `libwayland-dev libxkbcommon-dev` | Project's own record also requires `libxkbcommon-x11-dev` | `README.md:125-127` vs `verification.md:76` |

## Keep / rewrite / remove

- **Keep as current:** `README.md` (after fixes), `AGENTS.md`,
  `docs/audit/PLAN.md`, `docs/audit/PACKAGING.md`,
  `docs/audit/DECISIONS.md`, `docs/audit/FEATURES.md`,
  `docs/audit/SECURITY.md`, `docs/rewrite-3.0.0.md`,
  `i18n/README.md`, `justfile`, `scripts/verify.sh`, packaging
  manifest comments, `data/*`, `.github/workflows/flatpak.yml`.
- **Keep but mark/fix:** `README.md` items above;
  `docs/upstream-audit.md` line 9; `docs/verification.md` MSRV + stale
  "not verified" section; `docs/audit/ARCHITECTURE.md` (unwrap claim,
  ARCH-03/04); `docs/audit/BASELINE.md`, `BUGS.md`, `COSMIC-UX.md`,
  `PERFORMANCE.md` (stale "current state" rows — either fix or banner
  as point-in-time); `docs/migration/PLAN.md` (tick checklist or
  banner); `i18n/README.md` qps mechanism wording.
- **Keep, explicitly historical (add/retain banners):** root
  `AUDIT.md` (2026-09-07 record), `docs/migration/REPORT.md`,
  `docs/migration/PARITY_BASELINE.md`, `docs/audit/REPORT.md`,
  `docs/implementation-brief.md` (already bannered).
- **Remove or archive:** `.grok/task-prompt.md` (retired 2.x mission
  prompt, no disclaimer, describes a stack that no longer exists).

## Worst offenders

1. **`docs/audit/COSMIC-UX.md`** — multiple "Current"/"Problems"
   entries describe the pre-hardening state (no drag/drop, dead debug
   switch, 80 `t!()` calls) even though the same doc set records them
   fixed. Reads as live status; is actually stale.
2. **`docs/audit/BASELINE.md`** — the "Advertised vs visible" table and
   the "no unwrap/expect/panic" sweep claim are wrong for the tree they
   describe; a baseline that reports the wrong baseline.
3. **`docs/verification.md`** — the "clippy enforces MSRV" sentence is
   flatly wrong and props up the false 1.75 floor; the "no Flatpak
   build" section was superseded but not annotated.
4. **`README.md`** — mostly accurate, but two safety-relevant claims are
   wrong: the running-probe "cannot tell" guarantee and the
   DwarFS/`dwarfsextract` limitation; plus the `rust-version` pointer
   directs users to a toolchain that cannot build the lockfile.
5. **`.grok/task-prompt.md`** — an unmarked C++/Qt 2.x mission prompt
   sitting in the tree as if current.

## Caveats

- No cargo/rustc/git/flatpak-builder in this environment: no tests or
  builds were re-run; dynamic claims were verified by tracing code
  paths. Line references were taken from the current tree and will
  drift.
- Historical docs are not "wrong" for recording before/after states;
  the defect is presenting pre-fix state as current without a banner.
- `Cargo.lock` itself does not record `rust-version`; the ≥1.89
  requirement is established from the pinned crates' published
  metadata (notify-rust 4.18.0 → 1.89; trash 5.2.7/uuid 1.26.0 → 1.85;
  icu_* 2.3.x → 1.88; zbus 5.19.0 → 1.87) and is corroborated by the
  manifest comment "the dependency graph needs up to 1.89".

## Resolution (2026-09-13)

Every finding above is resolved in this tree:

- `README.md` rewritten end-to-end (accurate features, limits, safety,
  paths, CLI contract; running-probe degradation and DwarFS limitation
  stated correctly; AI-development notice added).
- `Cargo.toml` `rust-version` raised 1.75 → 1.89 to match the locked
  dependency graph.
- `CONTRIBUTING.md` created; `docs/verification.md` rewritten with the
  re-verified results (160 tests, x86_64 Flatpak build green).
- `docs/audit/{ARCHITECTURE,BASELINE,BUGS,COSMIC-UX,FEATURES,PERFORMANCE,SECURITY}.md`
  corrected inline (guarded-panic wording, drag/drop, debug diagnostics,
  max-size row, probe fallback, unsafe-fallback reachability, PERF-01).
- Root `AUDIT.md` and `docs/migration/PLAN.md` bannered as
  point-in-time records; `docs/upstream-audit.md` stack line fixed;
  `i18n/README.md` qps mechanism corrected.
- `.grok/task-prompt.md` removed (retired 2.x C++/Qt mission prompt).
- The auditor's only contested claim — that adopted apps are unowned and
  cannot be updated/removed — was disproven by live test (`adopt`
  registers `owned=true`); `APP-INVENTORY.md` §9.3 was corrected.
