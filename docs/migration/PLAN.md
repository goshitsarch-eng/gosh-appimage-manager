# Gosh AppImage Manager — Gear Lever parity + hardening plan

## Goal

Make the existing Rust + libcosmic app work as well as Gear Lever for the
workflows Gear Lever defines, without copying Gear Lever code, UI, assets, or
branding, then prove it with tests and a Flatpak verification pass.

## Success Criteria

- Every item in the parity checklist below is ticked with a stated
  verification method, verified against the running Flatpak where UI is
  involved.
- `scripts/verify.sh` passes from a clean checkout on this machine where its
  prerequisites exist, and states plainly what it skipped where they do not.
- No regressions in the safety contract: never execute to inspect by default,
  no shell-string execution, no permanent delete on Trash failure, no
  overwrite without ownership, transactional integration/updates with
  rollback.
- The app remains buildable and test-passing after every task.

## Context And Current Facts

- The tree is already Rust 2021 + libcosmic v0.12 (`gui` feature), not GTK4.
  Evidence: `Cargo.toml:20-39`, `src/gui.rs:1-12`, `README.md:1-9`,
  `docs/rewrite-3.0.0.md:1-32`. There is no GTK/GObject source to migrate.
- Gear Lever is a behavioral reference only (Python/GTK4/Libadwaita,
  commit `a2917f2`, v4.6.2). Evidence: `docs/upstream-audit.md:1-9`,
  `AGENTS.md`. Copying its source, templates, CSS, icons, screenshots, app
  ID, or branding is out of scope.
- The behavioral contract lives in `docs/implementation-brief.md` (stack
  sections superseded, behavior sections binding), `docs/upstream-audit.md:11-26`
  behavior inventory, and `AUDIT.md` end-to-end audit (2026-09-07, all
  findings fixed, 148 tests / 24 suites green, GUI builds, headless smoke
  passes).
- Current shell: libcosmic app with Library, Inspect, Updates, Tasks,
  Settings, About pages (`src/gui.rs`, 2612 lines; `src/controller.rs`;
  `tools/gui-smoke.sh` renders all six pages headless and checks contrast).
- Core services already exist and are tested through fakes: inspector,
  integration, registry (SQLite), desktop entries, launch, removal/trash,
  updates sources/service (static, GitHub, GitLab, Codeberg, Forgejo, FTP),
  tasks, notifier, CLI (`src/*.rs`, `tests/*.rs`, `tests/common.rs`).
- Known unverified remainder (not defects, stated in `docs/verification.md:8`
  and `AUDIT.md:362-376`): no pass on a real COSMIC/Wayland compositor, no
  Flatpak rebuild since the i18n manifest change, no `just validate` re-run
  where those tools exist, no human translations (pseudolocale `qps` only),
  no real-AppImage corpus run. Host Flatpak grant
  (`--talk-name=org.freedesktop.Flatpak`) remains because launching AppImages
  is the app purpose; its use is constrained to named helpers plus
  registry-resolved paths (`AUDIT.md:349-357`).
- Packaging is a single Flatpak manifest for both arches:
  `packaging/com.goshapps.AppImageManager.yml` (freedesktop 23.08, rust-stable
  extension + pinned Rust 1.90 toolchain, per-arch pinned 7zz/dwarfs).
  Commands live in `justfile` (`build`, `build-gui`, `test`, `lint`,
  `fmt-check`, `self-test`, `validate`, `vendor`, `flatpak-x86_64/aarch64`).
  There is no `scripts/verify.sh` yet.

## Constraints And Non-goals

- Constraints: original Rust + libcosmic implementation; GPL-3.0-or-later;
  Flatpak freedesktop 23.08 x86_64 + aarch64 single manifest; `cargo + just`;
  treat every AppImage/desktop/icon/URL/archive/process output as untrusted;
  unsafe extraction fallback stays opt-in, warned, default-off; program plus
  argv execution only; Trash failure never falls back to permanent delete;
  no overwrite without verified ownership and explicit replace semantics;
  validate downloads before atomic replacement, keep rollback until success.
- Non-goals: no Gear Lever source/asset/brand reuse; no human translations
  this round (keep strings localizable, ship English); no portal Trash/Open
  rewrite (no portal service here to verify against); no new update-manager
  protocols; no Qt/KDE/GNOME-runtime return.

## Key Decisions

- Parity means behavioral workflow parity, not widget parity. libcosmic
  equivalents stand in for Libadwaita patterns; where no equivalent exists,
  the closest faithful libcosmic alternative is acceptable if the workflow
  and safety properties match.
- Verify through messages and state plus the running Flatpak, not pixel
  matching. Existing seams (`tests/common.rs`) plus `tools/gui-smoke.sh`
  already set this pattern; extend it rather than adding screenshot-diff
  tests.
- One new entry point only: `scripts/verify.sh` chains the existing gates
  (`cargo build`, `cargo build --features gui` or `cargo check`, both clippy
  gates, `cargo test`, `cargo fmt --check`, `--self-test`, `just validate`,
  Flatpak build, Flatpak smoke). It must skip gracefully with a loud note
  where a prerequisite (flatpak-builder, Xvfb, validators) is absent, never
  fake a pass.
- Flatpak before polish: rebuild and smoke the packaged app before any UI
  refinement, so sandbox findings shape later fixes.
- Keep the two deliberate partials as-is unless new evidence reopens them:
  host-execution grant stays (constrained), localization stays English plus
  localizable machinery.

## Recommended Approach

Audit first, then package, then harden. Walk the checklist against core tests
plus the GUI state machine, rebuild the Flatpak and drive the running
packaged app, file every mismatch as a small fix with its own regression
test, and close with a full `verify.sh` pass plus a written report. Keep each
task buildable: core and GUI compile plus `cargo test` green at every step.

## Work Plan

1. Parity harness baseline. Map each checklist item to its current
   core/GUI/test evidence (`controller.rs`, `gui.rs`, relevant `tests/`).
   Output: checklist with pass/gap/unknown per item. No code changes.
2. `scripts/verify.sh`. Add the single chaining script (build, both clippy
   gates, test, fmt-check, self-test, validate, Flatpak build + smoke with
   loud skips). Validate by running it once and recording what ran vs
   skipped.
3. Flatpak rebuild + packaged smoke. `just flatpak-x86_64`
   (and aarch64 where qemu is available), in-builder `--self-test`,
   `--run` probes, `just validate`. File failures as new tasks.
4. Parity gap fixes, smallest first. Expected candidates from the audit
   remainder: outside-folder discovery/adoption affordance, detail-page
   actions (reveal, args/env editing, source configure/reset, metadata
   refresh, provenance display), update cancel/batch summary, running-app
   guard messaging, error/empty/offline states. Each fix carries its Message
   variants, `update()` unit coverage, and an integration flow test.
5. Sandbox and safety re-verification. Re-run SSRF/redirect/glob/archive/
   rollback/trash-failure/registry-mode cases against the packaged build
   surface; confirm no host:rw, no shell strings, Trash-fail safety, atomic
   replace with rollback.
6. Real-compositor and narrow-layout pass. Where a COSMIC/Wayland session
   exists, walk Library/Inspect/Updates/Tasks/Settings/About at 1024x768
   and 430px narrow, keyboard-only (Tab/Esc/arrows, shortcuts), with
   contrast/focus-ring notes. Where none exists, record it as a limitation
   and keep the headless smoke as the gate.
7. Report. Write `docs/migration/REPORT.md`: what was verified, deviations
   from Gear Lever behavior and why, known limitations, build/run/install
   instructions.

## Validation Plan

- `cargo build`, `cargo test` (full suite, currently 148 tests / 24 suites).
- `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy --features gui --all-targets -- -D warnings`.
- `cargo fmt --check`.
- `HOME=<isolated> ./target/debug/gosh-appimage-manager --self-test`
  expects `SELF_TEST_OK`.
- `just validate` where `desktop-file-validate` and `appstreamcli` exist.
- `tools/gui-smoke.sh` after `cargo build --features gui` where
  Xvfb/xdotool/ImageMagick exist: all pages render, contrast meets WCAG AA.
- Flatpak: `just flatpak-x86_64` (plus aarch64 where supported),
  in-builder `--self-test`, `flatpak-builder --run` probes.
- `scripts/verify.sh` from a clean checkout as the closing gate.
- Highest-risk validation: the packaged-app walk of the parity checklist,
  because sandbox, portal, and compositor behavior cannot be proven by unit
  tests alone.

## Risks / Rollback

- Real-compositor access may be unavailable: record as a limitation, do not
  claim the pass. Rollback: keep headless smoke as the enforced gate.
- `flatpak-builder` or GUI system deps may be absent: `verify.sh` skips
  loudly rather than failing silently; no code change depends on them.
- libcosmic v0.12 pin plus cosmic-text `[patch]` is fragile: any lock change
  requires `just vendor` regeneration of `packaging/cargo-sources.json` and
  a GUI check. Rollback: revert the lock change.
- Each fix stays behind its own commit and test; a fix that regresses the
  suite is reverted individually, never by rolling up the plan.

## Open Questions

None. Translation scope defaults to English-plus-localizable; portal Trash
rewrite stays deferred; any new Gear Lever behavior discovered mid-audit
enters the checklist as a gap task rather than a scope debate.

## Feature parity checklist

Source: `docs/upstream-audit.md:11-26` plus shell/detail requirements from
`docs/implementation-brief.md:2`.

- [ ] Shell pages: Library, Updates, Tasks, Settings, About render and
  navigate (verify: packaged app walk + `tools/gui-smoke.sh`).
- [ ] Open AppImage action: file chooser, open-with, drag/drop, single and
  multi-file (verify: packaged walk + integration tests).
- [ ] Inspection flow per candidate: path/size, type/arch, name/version/
  summary/terminal/icon, update-source status, already-managed state,
  incompatibility warnings, copy-vs-move outcome, keep-both vs replace
  identified install; single confirmation for multi-file; per-item failures
  isolated (verify: `test_inspector`/`test_integration` + packaged walk).
- [ ] Integration: configurable folder, menu entries, copy/move per
  settings, conflict handling, transactional with rollback (verify:
  `test_integration`/`test_rollback`).
- [ ] Library detail: launch (detached), reveal, check/update now, args list
  editing, env name/value editing, source configure/reset, metadata refresh,
  path/ID/hash/type/arch/version/size/manager/provenance display, Trash by
  default, destructive permanent delete only via explicit secondary flow
  (verify: `test_detail` + packaged walk).
- [ ] Discovery/adoption including outside-folder opt-in (verify:
  `test_library` + packaged walk).
- [ ] Updates: check/download/cancel/apply per-item and batch, progress,
  completion summary, running-app guard with explicit force override,
  embedded `.upd_info` (verify: `test_update` + packaged walk).
- [ ] Update managers: static, GitHub, GitLab, Codeberg, Forgejo, FTP
  (verify: `test_update`/`test_ftp`/`test_glob`).
- [ ] CLI with JSON schema v1 list output: integrate, update, remove,
  remove-all, list-installed, list-updates, list-update-managers,
  set-update-source, background fetch (verify: `test_cli` + isolated-HOME
  probes).
- [ ] Background checks + notifications, autostart probe non-mutating
  (verify: `test_probes` + probes).
- [ ] Extraction safety: Type 1/2, SquashFS/DwarFS tooling, bounded archive
  handling, unsafe fallback default-off with per-file warning (verify:
  `test_inspector`/`test_icon` + settings walk).
- [ ] Sandbox posture: minimal finish-args, portals, no host:rw, constrained
  host execution, validators pass (verify: manifest review + `just validate`
  + packaged probes).
