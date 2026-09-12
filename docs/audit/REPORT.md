# Audit-hardening report — Gosh AppImage Manager 3.0.0

Branch: `audit-hardening` (from `main` at `d11deaf`).
Scope: full audit per brief — bugs/reliability/completeness, COSMIC UX,
performance, security/robustness, architecture, packaging/QA, red-team.

## Executive summary

The tree entered audit-hardening in strong shape: prior rounds had closed all
2026-09-07 findings, gates were green, and the remaining PLAN held 10 items
with zero P0. Audit-hardening fixed all actionable code items (5), documented
the rest as environment-blocked with exact justification, added 12 regression
tests, and left the suite at 160 passed / 0 failed with both clippy gates,
fmt, self-test, and desktop/AppStream validation green.

## Original condition

- Rust 2021 + libcosmic, `AppController` composition root, CLI + GUI thin
  shells over shared services, SQLite registry + JSON settings, no daemon.
- 150 tests green in isolation; `test_registry_perf` flaky under parallel load
  (1 of 3 full runs: 1 passed, 2 failed).
- Three residual defects: debug-logging no-op switch, flaky perf bounds, dead
  `Page::title`; two gaps: MaxAppImageBytes unsettable, drag/drop absent.
- Metadata green except expected uppercase-id pedantic info; x86_64 Flatpak
  built in-tree; aarch64 via CI.

## Bugs found (residual, 2026-09-12)

- BUG-001 debug-logging no-op (P2) — fixed.
- BUG-002 flaky perf tests (P2) — fixed structurally (write-statement
  counter + serialization; wall time is backstop-only after 20x fsync-storm
  variance was measured: 1.7s → 33s).
- BUG-003 dead `Page::title` (P3) — fixed.

## Broken / unwired features found

- Debug-logging switch persisted but controlled nothing.
- MaxAppImageBytes enforced but only editable via `settings.json`.
- Drag/drop advertised by upstream behaviour, absent (picker + file args only).

No other orphaned widgets, messages, settings, shortcuts, or backend
capabilities were found; every visible action traces UI → Message → service →
state → feedback.

## Functionality completed

- Debug logging is real: `src/diagnostics.rs` (basename-only, bounded, no
  secrets) drives CLI stderr lines for list/integrate/update/remove/adopt/fetch
  and GUI worker-outcome lines plus immediate emit on toggle.
- MaxAppImageBytes is a Settings row (MB, strict 1–32768 parse, caption with
  default) reusing clamp + save-error reporting; README Safety documents it.
- Drag/drop works: `window::Event::FileDropped` → `Message::FileDropped` →
  existing inspect queue via pure `drop_queue::plan_drop` (never preempts a
  busy worker or unconfirmed result); README documents the queue.
- Perf bounds are load-insensitive backstops with structural asserts primary.

## Security issues found and fixed

No new vulnerabilities. Red-team re-verified: no `unwrap/expect/panic` in
production paths (remaining unwraps are test fakes or a guarded `is_none`
check), no shell strings (argv arrays only), no TODO/stub markers in `src/`,
diagnostics log only basenames/counts/bools, corrupt settings fail open
without overwrite or crash, missing files refuse gracefully, Flatpak grants
unchanged and minimal. Residual SEC-01 host-spawn grant stays constrained
(PORTAL migration blocked on environment, PLAN-005).

## Performance problems found and fixed

- Flaky wall-time gates → STRUCTURAL SQL-write counters as the deterministic
  gate (100 upserts ≤120 writes vs ~40,000 on rewrite; 300 removals ≤330 vs
  ~45,000; lookups 0 writes) plus std-`Mutex` serialization of the fsync-heavy
  tests. Wall time kept only as a generous backstop (240s/60s/300s) after
  measuring ~20x parallel-fsync inflation (1.7s → 33s). Mutation probe proved
  the counter fires on rewrite-like saves. Serialized perf suite ~75s on a
  slow debug disk; full suite green.

## Architecture improvements

- Added `diagnostics` and `drop_queue` modules (pure, tested, no globals);
  `limits` gained strict MB parse + MB formatter. No rewrites of working code;
  `Result<_, String>` and `AppController` seams retained per standing decisions.

## COSMIC / libcosmic UX changes

- Removed dead title helper (single localized source).
- Added max-size row + caption to the Integration-folder settings section using
  existing `settings::section`, text-input + Apply, and caption patterns.
- Added window drop handling via pinned-iced `FileDropped` + existing queue;
  Inspect page activates on drop; busy/unconfirmed work is never discarded.
- Compositor verification (360px tiling, scaling, focus order, Esc/trap,
  theme restart, contrast) remains PLAN-001: no compositor in this environment.

## Accessibility changes

- No new barriers: settings row uses labelled inputs + text captions (not
  colour-alone); toggles already carry labels. Reduced-motion/spacing scale
  stays P3 backlog (PLAN-009, low impact, needs toolkit-scale pass).

## Flatpak / packaging changes

- No manifest changes. Verified: `desktop-file-validate` pass,
  `appstreamcli validate --pedantic --no-net` pass (1 expected uppercase-id
  info), both clippy gates + fmt + 160-test suite + `--self-test`
  (`SELF_TEST_OK`) green. Not run here: `cargo audit` (tool absent),
  aarch64 rebuild (no qemu; CI covers both arches), release-commit CI.

## Tests added

- `src/diagnostics.rs` unit tests (2): disabled writes nothing, labels hide
  directories and truncate.
- `tests/test_diagnostics.rs` (3): silent-when-off, emits-when-on, JSON
  stdout stays valid.
- `tests/test_max_bytes.rs` (3): strict MB parse, persist + reload + clamp,
  oversized `copy_bounded` refused with bound message and no destination.
- `src/drop_queue.rs` unit tests (4): idle starts first, busy never preempts,
  unconfirmed never discarded, empty drop is a no-op.
- Total: 160 passed / 0 failed (`cargo test --no-fail-fast`).

## Dependencies changed

None. Deliberately no `log`/`env_logger`/`tracing`/`serial` additions:
diagnostics uses std only (no vendoring churn), perf fix uses bounds (no new
harness). `Cargo.lock` and `packaging/cargo-sources.json` untouched.

## Known limitations

- Zsync = full-file download (documented, accepted).
- FTP legacy plaintext with warning; credentials rejected (accepted).
- English-only UI; i18n machinery + qps proven (accepted until a human
  translator contributes).
- GUI compositor behaviour unverified here (PLAN-001).

## Intentionally deferred (exact justification)

- PLAN-004 translation (P2): needs a competent human speaker + 1.0×/1.5×
  layout proof; machine translation would violate Decision 5. External
  blocker (contributor).
- PLAN-001 compositor pass (P1): needs a real COSMIC/Wayland session; none in
  this environment. Code-complete; headless gates green. External blocker.
- PLAN-010 release tail (P2): needs `cargo audit` tool + aarch64 qemu/CI on
  the release commit. Local metadata + both clippy + tests green. External
  blocker.
- PLAN-005 portals (P3): needs a portal-capable host to verify against;
  verified host-spawn path retained. External blocker.
- PLAN-009 spacing scale (P3): toolkit-scale polish, low impact; backlog.

## Build instructions

```sh
cargo build
cargo build --features gui   # libcosmic GUI (needs Wayland/XKB headers)
```

## Run instructions

```sh
./target/debug/gosh-appimage-manager --help
./target/debug/gosh-appimage-manager --self-test
GOSHAIM_HOME=$(mktemp -d) ./target/debug/gosh-appimage-manager --list-installed
cargo run --features gui -- file1.AppImage file2.AppImage
```

## Flatpak installation instructions

```sh
flatpak-builder --force-clean build-dir packaging/com.goshapps.AppImageManager.yml --arch=x86_64
flatpak-builder --force-clean build-dir packaging/com.goshapps.AppImageManager.yml --arch=aarch64
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml \
  gosh-appimage-manager --self-test
```

Manifest is freedesktop 23.08, single file, both arches, no
`--filesystem=host:rw`. `SKIP_FLATPAK=1` / `SKIP_GUI=1` skip those
`scripts/verify.sh` stages loudly when tooling is absent.

## Verification results (observed)

- `cargo fmt --check`: pass.
- `cargo build`: pass.
- `cargo clippy --all-targets -- -D warnings`: pass.
- `cargo clippy --features gui --all-targets -- -D warnings`: pass.
- `cargo test --no-fail-fast`: 160 passed, 0 failed.
- `--self-test` (isolated HOME): `SELF_TEST_OK`, exit 0.
- `desktop-file-validate`: pass.
- `appstreamcli validate --pedantic --no-net`: pass (1 info:
  uppercase app-id, accepted).
- GUI smoke: skipped loudly (no Xvfb/xdotool here); GUI compile covered by
  gui clippy.
- `cargo audit`: not run (tool not installed here).
- `scripts/verify.sh`: **9 passed, 1 skipped** (skip = GUI smoke, no
  Xvfb/xdotool). Includes a fresh Flatpak x86_64 build: PASS.
  aarch64 via CI.
- Red-team: corrupt settings / missing files refuse gracefully without crash
  or overwrite; no production unwraps; no shell strings; no secret logging;
  no stray TODO/stub markers in `src/`.

## Category table

| Category | Found | Fixed | Remaining |
|---|---|---|---|
| Security (P0) | 0 | 0 | 0 |
| Major broken / severe UX (P1) | 0 code defects (1 verification gap PLAN-001) | 0 | 1 env-blocked verification |
| Normal bugs / completeness / perf (P2) | 4 (002, 003, 004, 010) | 2 (002, 003) | 2 external-blocker (004 translator, 010 release env) |
| Polish / hardening (P3) | 5 (005–009) | 3 (006, 007, 008) | 2 (005 portal env, 009 backlog) |
| Docs accuracy | 3 gaps (max-size, dnd, plan status) | 3 | 0 |
| Tests | 0 missing gates for fixed behavior | 12 added | 0 |
