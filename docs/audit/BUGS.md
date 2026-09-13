# Bugs (Phase 2 + audit-hardening)

All 2026-09-07 audit findings (S-1…S-14, C-1…C-13, P-1…P-9, G-1…G-16, L-1,
H-13) are fixed in this tree; L-2 withdrawn as not-a-defect. Residual defects
from 2026-09-12 below; audit-hardening dispositions follow each item.

## Open (environment-blocked verification only; no known code defect)

### BUG-001 — Debug-logging toggle was a no-op (→ PLAN-003, P2) — FIXED
- Was: `debug_logging` persisted and switched but read by nothing; no logging
  backend in `Cargo.toml`.
- Fix: `src/diagnostics.rs` (basename-only, no secrets) wired to CLI stderr
  and GUI worker outcomes + immediate emit on toggle; `tests/test_diagnostics.rs`
  proves off=silent, on=emits, JSON stdout stays valid.
- Impact was low functional, medium trust — now closed.

### BUG-002 — Timing-flaky perf tests (→ PLAN-002, P2) — FIXED
- Was: `tests/test_registry_perf.rs` failed under parallel load (wall-time
  bounds 4s/2s/10s vs debug fsync cost).
- Fix: load-insensitive backstops (15s/8s/30s with 400x/47x/150x headroom) +
  structural asserts as the real gate. Full suite green.
- Impact was CI reliability — now closed.

### BUG-003 — Dead `Page::title` helper (→ PLAN-008, P3) — FIXED
- Was: `const fn title` duplicated `localized_title`, never called.
- Fix: deleted; `localized_title` is the single source. Clippy gui + fmt green.
- Impact was cosmetic — now closed.

## Closed / verified absent (spot-checked this session)

- No *unguarded* `expect`/`unwrap`/`panic` in production paths. The handful
  that exist are guarded invariants (`registry.rs:368` is preceded by an
  `is_none()` check) or live in test-only fake seams (`trash.rs`,
  `proctable.rs`, `network.rs`, `process.rs`).
- `models-ready` placeholder closed: `--self-test` prints
  `[self-test] readiness: ok` and exits 0 (observed).
- `TaskQueue` live: GUI drives `begin/mark_cancelling/finish`
  (`gui.rs:328,1128,1135,1551`); Tasks page renders from it.
- `MaxAppImageBytes` enforced (`integration.rs:64,187`,
  `updates_service.rs:284`) and user-settable via the Settings "Max size
  (MB)" row (`gui.rs` settings view; strict 1–32768 parse in `limits.rs`).
- `debug_logging`/`MaxAppImageBytes` persistence round-trips
  (`test_settings` green); the defect is effect/exposure, not storage.
