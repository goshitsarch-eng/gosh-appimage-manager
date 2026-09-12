# Bugs (Phase 2 — current tree)

All 2026-09-07 audit findings (S-1…S-14, C-1…C-13, P-1…P-9, G-1…G-16, L-1,
H-13) are fixed in this tree; L-2 withdrawn as not-a-defect. What follows
are the **residual defects found against the current tree** (2026-09-12).
Each maps to a PLAN item.

## Open

### BUG-001 — Debug-logging toggle is a no-op (→ PLAN-003, P2)
- Evidence: `debug_logging` appears only in `src/settings.rs` (persist,
  getter) and `src/gui.rs:1206,2263,2484-2496` (switch + snapshot). No
  logging backend exists (`Cargo.toml` has no `log`/`tracing`/`env_logger`;
  no call site reads the flag). Flipping the switch changes a JSON key and
  nothing else.
- Expected: the toggle controls diagnostic output (brief §9 wants
  diagnostics/log controls), or is absent.
- Impact: low functional, medium trust — a settings switch that does nothing.

### BUG-002 — Timing-flaky perf tests (→ PLAN-002, P2)
- Evidence: `tests/test_registry_perf.rs` failed 1 of 3 full-suite runs here
  (`1 passed; 2 failed`, 27s wall under parallel load); passes in isolation
  (10.5s) and in the other two full runs. `docs/migration/REPORT.md`
  already raised the `bulk_removal` bound to 10000ms noting debug-build
  fsync cost.
- Expected: deterministic pass/fail independent of machine load.
- Impact: CI reliability; flakes train reviewers to ignore red.

### BUG-003 — Dead `Page::title` helper kept with `allow(dead_code)` (→ PLAN-008, P3)
- Evidence: `src/gui.rs:84-97` — `const fn title` duplicates
  `localized_title` and is never called.
- Expected: removed (localised title is the single source).
- Impact: cosmetic; confuses the next reader about which title is canonical.

## Closed / verified absent (spot-checked this session)

- No `expect/unwrap/panic/todo` in `src/` (searched — zero hits).
- `models-ready` placeholder closed: `--self-test` prints
  `[self-test] readiness: ok` and exits 0 (observed).
- `TaskQueue` live: GUI drives `begin/mark_cancelling/finish`
  (`gui.rs:328,1128,1135,1551`); Tasks page renders from it.
- `MaxAppImageBytes` enforced (`integration.rs:64,187`,
  `updates_service.rs:284`) — but user-unsettable; tracked as gap
  (→ PLAN-006) rather than bug.
- `debug_logging`/`MaxAppImageBytes` persistence round-trips
  (`test_settings` green); the defect is effect/exposure, not storage.
