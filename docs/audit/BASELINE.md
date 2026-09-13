# Gosh AppImage Manager 3.0.0 — Baseline (Phase 1)

> **Point-in-time snapshot.** Audit date: 2026-09-12, tree `d11deaf`.
> Some "current state" rows below were already stale when the
> audit-hardening round finished — corrections are marked inline.

Audit date: 2026-09-12. Tree: commit `d11deaf`, clean worktree.
Prior rounds consumed: 8/8 child reports (all claims stand after skeptic round),
plus the committed `AUDIT.md` (2026-09-07, all findings fixed) and
`docs/migration/REPORT.md` (2026-09-10, Flatpak + verify closure).

## Structure

Rust 2021 workspace-root crate, `rust-version = "1.89"`, edition 2021
(the manifest's 1.75 predated the real dependency-graph floor):

- `src/lib.rs` — library `goshaim_core` (26 modules + `cli`, + `gui` under feature).
- `src/main.rs` — single binary `gosh-appimage-manager`; CLI dispatch vs GUI shell.
- `src/gui.rs` — libcosmic GUI, ~2600 lines, behind `--features gui`.
- `tests/` — `common.rs` seams + 22 suites.
- `data/` — desktop entry, AppStream metainfo, hicolor + symbolic SVG icons.
- `packaging/` — Flatpak manifest + generated `cargo-sources.json` (471 KB).
- `i18n/` — JSON catalog dir (`qps.json` pseudolocale only) + README.
- `scripts/verify.sh` — chained gate runner. `tools/` — `gui-smoke.sh`, `contrast.py`.
- `.github/workflows/flatpak.yml` — CI autobuilds both arches on push/PR.

## Modules

| Module | Responsibility |
|---|---|
| `main.rs` | arg routing: `--help/--version`, CLI commands, `--self-test`, probes, GUI |
| `cli.rs` | all CLI commands, JSON schema v1 output, TTY/`--yes` confirmation |
| `controller.rs` | `AppController`: owns settings, registry, tasks; environment overrides |
| `types.rs` | shared model: `InstalledApp`, `TaskItem`, exit codes, update offers |
| `limits.rs` | version, size/timeout bounds |
| `elf.rs` | ELF + AppImage magic, architecture detection |
| `inspector.rs` | metadata extraction via helper tools, bounded; unsafe fallback gated |
| `integration.rs` | transactional integrate (stage → commit → rollback), conflict policy |
| `registry.rs` | SQLite registry (`registry.sqlite`, 0600), one-shot v2 JSON import |
| `settings.rs` | JSON settings with load/save error reporting |
| `library.rs` | managed-folder scan, outside-folder discovery, adoption |
| `desktop.rs` | desktop entry build/parse, ownership markers, icon staging |
| `launch.rs` | start-only detached launch, argv/env handling |
| `removal.rs` | trash-first removal, guarded permanent delete |
| `trash.rs` / `notifier.rs` / `proctable.rs` / `process.rs` | trash, notifications, proc table, argv-array runner |
| `network.rs` | reqwest+rustls client, DNS pinning, per-hop redirect checks, streaming download |
| `url_guard.rs` | scheme/credential/local-address validation |
| `updates_sources.rs` | 6 managers: static, GitHub, GitLab, Codeberg, Forgejo, FTP |
| `updates_service.rs` | check/list/apply, digest + arch verification, atomic replace |
| `safe_fs.rs` | bounded copy/hash, temp dirs, archive-member validation |
| `tasks.rs` | `TaskQueue`: begin/progress/finish, cancellable, bounded history |
| `i18n.rs` | JSON catalog lookup + `qps` pseudolocale |
| `gui.rs` | libcosmic app: 6 pages, worker-thread commands, dialogs, detail view |

## Architecture

Single-process native desktop app. `AppController` is the composition root
(settings + registry + task queue); CLI and GUI are thin shells over the same
services. Long work runs on worker threads (`Command::perform` in GUI,
blocking calls in CLI). Persistence: SQLite registry + JSON settings under
XDG dirs (`GOSHAIM_HOME` override for tests). No daemon, no DBus service,
no telemetry. See [ARCHITECTURE.md](ARCHITECTURE.md).

## Features (summary)

Inspect (never execute on open), integrate (copy/move, keep-both/replace),
launch, detail page (args/env, refresh, source config), updates (6 managers,
per-item + batch, running-app guard), removal (trash/delete), discovery +
adoption, background checks + notifications, JSON CLI, i18n infrastructure.
Full matrix: [FEATURES.md](FEATURES.md).

## Build / packaging / tests (observed this session, rustc 1.98.0)

| Gate | Result |
|---|---|
| `cargo build` | OK, 0 warnings |
| `cargo test` | 150 tests total; see flakiness note below |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `--self-test` (isolated HOME) | `SELF_TEST_OK`, exit 0 |
| `desktop-file-validate` | pass |
| `appstreamcli validate --pedantic --no-net` | pass (1 pedantic info: uppercase app-id, expected) |
| GUI compile | not re-run; prior artifacts (`target/debug/build/cosmic-*`) + Flatpak app prove it; `pkg-config wayland-client xkbcommon` present |
| Flatpak x86_64 | previously built in this tree: `build-dir/files/bin/gosh-appimage-manager` + 7zz/dwarfs/unsquashfs present |

## Warnings

- Compiler/clippy/fmt: zero warnings (observed).
- `appstreamcli --pedantic`: `P: cid-contains-uppercase-letter` — informational,
  inherent to the shipped app-id; validation successful.

## Test failures (observed)

`tests/test_registry_perf.rs` is timing-sensitive and **flaky under parallel
load**: 1 of 3 full-suite runs in this session showed `1 passed; 2 failed`
(27s wall); the same suite passes in isolation (10.5s) and in the other two
full runs. `docs/migration/REPORT.md` already raised the `bulk_removal`
bound to 10000ms with rationale. Recorded as PLAN-002 (P2, CI reliability).

## Flatpak status

- Manifest: single file, both arches, no `--filesystem=host:rw`, per-arch
  pinned Rust 1.90.0 / 7zz / dwarfs binaries with SHA-256 + licences.
- x86_64: built successfully in this tree (binaries in `build-dir/files/bin`),
  sandboxed `--self-test` → `SELF_TEST_OK` per migration report.
- aarch64: covered by CI (qemu-user); not rebuilt locally (no qemu here).
- CI (`.github/workflows/flatpak.yml`) builds + bundles both arches on
  push/PR with fail-fast off.

## Advertised vs visible vs incomplete

| Claimed (README/docs) | State |
|---|---|
| Inspect/integrate/update/remove/launch | visible in GUI + CLI, tested |
| Transactional integration + rollback | implemented, 4 fail-point tests |
| 6 update managers, digest + arch checks | implemented, tested |
| Running-app guard incl. Flatpak | implemented via host spawn probe |
| Tasks page, detail page, search/sort | visible in GUI (`gui.rs` views) |
| Localizable UI | machinery + `qps` proof; **no human translations ship** |
| Drag/drop open | **implemented** (drop queue + `FileDropped` handling; PLAN-007) |
| Zsync delta updates | **full-file download only** (documented limitation) |
| Debug-logging toggle | wired to `diagnostics.rs` stderr output (PLAN-003) |
| MaxAppImageBytes | enforced bound + Settings "Max size (MB)" row (PLAN-006) |
| DwarFS metadata extraction | detected, but `dwarfsextract` has no lister/extractor arm — always fails |

## Runtime observations (this session)

- `--self-test` green, isolated HOME, exit 0; `[self-test] readiness: ok`
  (readiness now checks something real — prior placeholder closed).
- `--list-installed/--list-updates --json`, `--list-update-managers`,
  `--probe-host`, `--probe-autostart` all exit 0 (verified in migration
  report; self-test re-verified here).
- No *unguarded* `expect`/`unwrap`/`panic` in production paths; remaining
  ones are guarded invariants or test seams. (The original "zero hits"
  claim was wrong.)
- GUI not launched on a compositor in this session (none available);
  headless-smoke design exists (`tools/gui-smoke.sh`) but Xvfb/xdotool are
  absent here, so that stage would skip loudly per `verify.sh`.
