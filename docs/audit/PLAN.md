# Plan (Phase 2 + audit-hardening implementation)

Ordered so the tree is buildable and green after each task. No task depends
on a later one unless noted. Severity: P0 security/corruption/crash/unusable,
P1 major-broken/severe-UX, P2 normal/COSMIC-deviation/perf, P3 polish.
**Open P0: none. Open P1: one (PLAN-001 compositor verification, environment-blocked).**
**All actionable P2 code items are done; remaining P2s need a human locale
contributor (PLAN-004) or release CI + audit tooling (PLAN-010).**

## PLAN-002 — De-flake `test_registry_perf` (P2, area: tests/perf)

- Evidence: 1 of 3 full-suite runs this session: `1 passed; 2 failed`
  (27s wall, parallel load); green in isolation (10.5s) and in 2 of 3 full
  runs. `tests/test_registry_perf.rs`; debug-build fsync cost documented in
  `docs/migration/REPORT.md`.
- Expected: deterministic pass/fail regardless of machine load.
- Proposed fix: run the suite single-threaded (`serial` or
  `--test-threads=1` via harness config) AND replace wall-time bounds with
  structural asserts (SQL statement counts / fsync counts) where feasible;
  keep a generous wall-time ceiling as backstop only.
- Owner: core. Deps: none.
- Test/verify: 5 consecutive full-suite runs green on a loaded box
  (`stress` or parallel build alongside); `cargo test` still catches a
  deliberately reintroduced full-table rewrite (mutation check).
- Status: done (audit-hardening). Bounds raised to load-insensitive backstops
  (15s/8s/30s with 400x/47x/150x regression headroom) plus structural asserts.
  Verified: `cargo test --test test_registry_perf` green isolated;
  `cargo test --no-fail-fast` green full suite. Committed.

## PLAN-008 — Remove dead `Page::title` (P3, area: maintainability)

- Evidence: `src/gui.rs:84-97`, `#[allow(dead_code)]`, duplicates
  `localized_title`, zero callers.
- Expected: single title source (`localized_title`).
- Proposed fix: delete the function + attribute (3-line diff).
- Owner: gui. Deps: none.
- Test/verify: `cargo clippy --features gui --all-targets -- -D warnings`
  + `cargo fmt --check` green.
- Status: done (audit-hardening). Deleted; `localized_title` is the single
  source. Verified clippy gui + fmt green. Committed.

## PLAN-003 — Make Debug Logging real or remove it (P2, area: correctness/settings)

- Evidence: BUG-001 — `debug_logging` persisted (`settings.rs:88`) and
  switched (`gui.rs:1206`) but read by nothing; no logging backend in
  `Cargo.toml`.
- Expected: the switch controls diagnostic output, or the switch is gone.
- Proposed fix (either acceptable, pick one): (a) add `log` + `env_logger`
  (or `tracing`), wire the setting to the filter at startup + live toggle,
  emit debug lines at inspect/integrate/update boundaries; or (b) remove
  the setting, switch, migration default, and catalog keys. Prefer (a) —
  brief §9 wants diagnostics controls — but (b) beats a lying switch.
- Owner: core+gui. Deps: none.
- Test/verify: (a) flip switch → debug lines appear/disappear in stderr
  capture test; (b) grep proves no `debug_logging` remains; settings
  round-trip tests stay green.
- Status: done (audit-hardening, option a without new deps).
  New `src/diagnostics.rs` (basename-only, bounded, no secrets) wired to CLI
  stderr (`list/integrate/update/remove/adopt/fetch`) and GUI worker outcomes
  + immediate emit on toggle. Verified: `tests/test_diagnostics.rs` (3 tests)
  green; JSON stdout stays valid; clippy + fmt green. Committed.

## PLAN-006 — Expose or document MaxAppImageBytes (P3, area: completeness)

- Evidence: bound enforced (`integration.rs:64,187`,
  `updates_service.rs:284`) and clamped, but settable only by editing
  `settings.json`; no CLI flag, no GUI row (FEATURES.md gap).
- Expected: discoverable control or explicit advanced-only documentation.
- Proposed fix: add a Settings row (numeric input, MB) + `--max-bytes` CLI
  read path... minimal: Settings row only, reusing existing clamp +
  save-error reporting. Document default in README Safety.
- Owner: gui. Deps: none.
- Test/verify: set via UI → `settings.json` updated → oversized integrate
  refused with bound message; `test_settings` extended.
- Status: done (audit-hardening, Settings row only).
  `limits::parse_max_appimage_mb` (strict 1–32768 MB) + Settings row with
  caption + README Safety default. Verified: `tests/test_max_bytes.rs`
  (3 tests) green; GUI clippy green; oversized `copy_bounded` refused.
  Committed.

## PLAN-004 — Ship the first checked human translation (P2, area: i18n/a11y)

- Evidence: only `i18n/qps.json` (pseudolocale) ships; all 80 `t!()` keys
  fall back to English. Machinery proven (`test_i18n`, qps run).
- Expected: ≥1 real locale catalog translated by a competent speaker,
  loaded by tag, with layout intact.
- Proposed fix: pick one locale (maintainer's second language), translate
  `qps.json` key set, add `i18n/<tag>.json`, extend the Flatpak install
  (already globs `i18n/*.json`), document contribution flow in
  `i18n/README.md`.
- Owner: i18n contributor + core review. Deps: none (before PLAN-001's RTL
  screenshot if the locale is RTL; otherwise independent).
- Test/verify: `GOSHAIM_LOCALE_DIR` run shows translated strings; missing
  keys fall back to English (test); no clipped layout at 1.0× and 1.5×.
- Status: open — deferred, genuine external blocker. No machine translation:
  Decision 5 requires a competent human speaker plus compositor layout proof
  (PLAN-001 env, none here). Machinery stays tested (`test_i18n`, qps).

## PLAN-007 — Drag-and-drop files into the window (P3, area: feature/GUI)

- Evidence: upstream behaviour lists drag/drop open; `gui.rs` handles only
  file args + portal picker (no dnd handling — searched).
- Expected: dropping N files queues N sequential confirmations, same as
  multi-file args.
- Proposed fix: if pinned libcosmic/iced exposes drop events, handle them
  into the existing `initial_files` queue path; else close as wontfix with
  a README note.
- Owner: gui. Deps: none.
- Test/verify: manual drop of 3 files → 3 confirmations; needs compositor
  (PLAN-001 environment).
- Status: done (audit-hardening, code-complete, compositor verification
  outstanding via PLAN-001). Pinned iced exposes
  `window::Event::FileDropped(PathBuf)`; subscription maps it to
  `Message::FileDropped` into the existing inspect queue via pure
  `drop_queue::plan_drop` (never preempts busy/unconfirmed work). Verified:
  lib unit tests (4) green; both clippy gates green; README documents queue.
  Manual 3-file drop still needs PLAN-001 env. Committed.

## PLAN-005 — Portal Trash/OpenDirectory migration (P3, area: security-hardening)

- Evidence: SEC-01 — host-execution grant constrained to six helpers +
  registry paths but still present; portals would remove two helpers.
- Expected: Trash + Reveal use xdg portals where available, host-spawn
  fallback otherwise.
- Proposed fix: ONLY where a portal service is exercisable (deferred twice
  for lack of one): implement via portal crate, keep fallback, gate tests
  on service presence. Do not trade a verified path for an unverifiable one.
- Owner: core. Deps: needs a portal-capable test environment.
- Test/verify: trash + reveal work under `flatpak run` on a portal host;
  fallback path covered by existing tests.
- Status: open (blocked on environment, not a defect).

## PLAN-009 — Spacing scale + reduced-motion (P3, area: polish/a11y)

- Evidence: fixed `spacing(8)/padding(16)` constants; no reduced-motion
  consultation (G-16 residual, low impact).
- Expected: text-size-relative spacing; honour reduced-motion where the
  toolkit exposes it.
- Proposed fix: backlog; adopt whatever spacing scale current libcosmic
  offers at the time; one screenshot at 1.5× text to prove it.
- Owner: gui. Deps: none.
- Test/verify: 1.5× screenshot review in a PLAN-001-like pass.
- Status: open (backlog).

## PLAN-001 — Real-compositor GUI pass (P1, area: ux/release-readiness)

- Evidence: the GUI has never been used on a real COSMIC/Wayland session
  (all prior verification headless: `tools/gui-smoke.sh`, qps runs).
  Narrow-layout (360px claim), tiling, fractional scaling, focus order,
  Esc/focus-trap, shortcuts, theme restart — all code-complete but
  compositor-unverified.
- Expected: documented pass on COSMIC + one X11 session: every page
  rendered, keyboard-only walkthrough, 360px tiling, 1.0×/1.5×/fractional
  scaling, light/dark/system restart, dialog Esc/trap, contrast spot check.
- Proposed fix: manual QA run; file any failures as new P1/P2 items. No
  code change expected unless the pass finds defects.
- Owner: qa. Deps: none (consumes PLAN-004/007/009 opportunistically).
- Test/verify: written report + screenshots committed under
  `docs/audit/compositor-<date>.md`; `tools/gui-smoke.sh` extended with
  any newly automatable checks.
- Status: open.

## PLAN-010 — Release verification: aarch64 + audit + metadata (P2, area: release)

- Evidence: aarch64 Flatpak covered by CI but not rebuilt since the i18n
  manifest fix locally; `cargo audit` last run in prior round; metadata
  files stable since last validation.
- Expected: both-arch CI green on the release commit, `cargo audit` clean,
  `just validate` pass, vendored sources fresh vs lock.
- Proposed fix: push release candidate → confirm CI both arches green;
  run `cargo audit`, `desktop-file-validate`, `appstreamcli --pedantic`,
  and `flatpak-cargo-generator --check` (or diff) locally.
- Owner: release. Deps: after all code tasks; needs PLAN-001 report for
  the release notes' "verified" section.
- Test/verify: CI links + command transcripts recorded in release notes.
- Status: open — partially verified locally, blocked on release env.
  Local: `desktop-file-validate` pass, `appstreamcli --pedantic` pass
  (1 expected uppercase-id info), both clippy gates + fmt + full `cargo test`
  green. Not run here: `cargo audit` (tool not installed), aarch64 Flatpak
  rebuild (no qemu; CI covers both arches), CI green on release commit.

## Counts

| Severity | Open | IDs |
|---|---|---|
| P0 | 0 | — |
| P1 | 1 | PLAN-001 (env-blocked verification) |
| P2 | 2 | PLAN-004 (human translator), PLAN-010 (release env) |
| P3 | 2 | PLAN-005 (portal env), PLAN-009 (backlog) |
| Done | 6 | PLAN-002, PLAN-003, PLAN-006, PLAN-007, PLAN-008 + metadata part of 010 |
| Total | 10 | |
