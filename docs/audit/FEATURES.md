# Feature matrix (Phase 1)

Columns: feature | exposed (GUI/CLI) | expected behaviour | implementation path |
status | coverage | verify.

Status values: `done` (wired + tested), `partial`, `missing`, `gap`
(implemented but unreachable/ineffective).

| Feature | Exposed | Expected | Impl path | Status | Coverage | Verify |
|---|---|---|---|---|---|---|
| Open/inspect AppImage, never execute | GUI Inspect, `--probe-inspect` | metadata without exec/integrate | `inspector.rs`, `elf.rs`, `gui.rs:view_inspect` | done | `test_inspector` (10), `test_elf` (10) | `cargo test`, `--probe-inspect` |
| ELF+AppImage magic validation | both (via inspect/integrate) | reject non-AppImages | `elf.rs`, `inspector.rs:88-110` | done | `test_elf` | `cargo test` |
| Integrate (copy/move) | GUI, `--integrate` | stage→commit, ownership markers | `integration.rs`, `desktop.rs` | done | `test_integration` (5) | `cargo test` |
| Conflict keep-both/replace | GUI dialog, CLI flags | explicit choice, UUID-resolved replace | `integration.rs:135-148`, `gui.rs:PendingDialog` | done | `test_integration`, `test_rollback` | `cargo test` |
| Transactional rollback | both | failure removes all new artifacts, restores replaced | `integration.rs` commit/rollback | done | `test_rollback` (4 fail points) | `cargo test` |
| Multi-file open, confirm each | GUI file args | queue, one confirmation at a time | `gui.rs:713` queued initial files | done | code review | GUI manual |
| Launch (start-only detached) | GUI row/detail, core API | spawn, never wait-and-kill | `launch.rs`, `process.rs` | done | `test_launch` (9) | `cargo test` |
| Reveal in file manager | GUI detail | open containing dir | `gui.rs:view_detail` | done | `test_detail` (5) | GUI manual |
| Detail page (hash/type/arch/size/source) | GUI | full provenance display | `gui.rs:view_detail` | done | `test_detail` | GUI manual |
| Edit args/env per app | GUI detail | persisted, applied on launch incl. Flatpak env forward | `launch.rs`, `gui.rs` | done | `test_launch` | `cargo test` + GUI manual |
| Refresh metadata | GUI detail | re-inspect in place | `inspector.rs` via GUI | done | `test_inspector` | GUI manual |
| Set/reset update source | GUI detail, `--set-update-source` | per-app manager config | `updates_sources.rs`, `cli.rs` | done | `test_update` roundtrip | `cargo test` |
| Per-item update | GUI, `--update <path>` | check→download→verify→replace | `updates_service.rs` | done | `test_update` (18) | `cargo test` |
| Batch update + cancel | GUI Updates, `--update --all` | serial apply, working cancel | `gui.rs`, `tasks.rs`, `cli.rs` | done | `test_update`, `test_desktop_tasks` | `cargo test` |
| Running-app guard + `--force` | both | block unless explicit force; fail-safe "cannot tell" | `proctable.rs`, host-spawn probe | done | `test_update` running-block | `cargo test` |
| Digest verification (sha256:/bare) | both | refuse uninterpretable/mismatch | `updates_service.rs` | done | `test_update` digest cases | `cargo test` |
| Arch-compat check on update | both | refuse foreign-arch payload | `updates_service.rs` | done | `foreign_architecture_update_is_refused` | `cargo test` |
| Reduced-verification flag | GUI/CLI offer display | shown when no published checksum | `types.rs`, sources | done | `test_update` | `cargo test` |
| 6 update managers | core + `--list-update-managers` | static/github/gitlab/codeberg/forgejo/ftp | `updates_sources.rs` | done | `test_update`, `test_ftp` (5) | `cargo test` |
| `.upd_info` embedded source | via inspect/check | parsed, adopted as source | `updates_sources.rs:config_from_embedded` | done | `test_update` | `cargo test` |
| Local-network opt-in (`allow_local_network`) | user-created sources only | embedded metadata can never set it | `network.rs`, `url_guard.rs` | done | `test_ssrf`, `test_network` | `cargo test` |
| Trash-first removal | GUI, `--remove` | trash; trash failure never deletes | `removal.rs:117-120` | done | `trash_failure_leaves_everything_intact` | `cargo test` |
| Permanent delete (guarded) | GUI dialog, `--delete` | extra confirm, refuse protected/symlink | `removal.rs:100-116` | done | `permanent_delete_refuses_protected_paths` | `cargo test` |
| `--remove-all` per-item results | CLI | continue past single failure, summary | `cli.rs:464-483` | done | `test_cli` | `cargo test` |
| Discovery + adoption | GUI list, `--list-discovered`, `--adopt` | opt-in outside-folder scan, inert adopt | `library.rs`, `cli.rs` | done | `test_library` (5) | `cargo test` + isolated-HOME probe |
| Library search + sort | GUI | filter + name/version/updates-first | `gui.rs:411-425` | done | code review | GUI manual |
| Background checks (notify only) | setting + autostart | never download/apply | `cli.rs:fetch`, `notifier.rs` | done | `test_cli` | code review |
| Autostart entry (non-mutating probe) | setting, `--probe-autostart` | probe renders without writing | `cli.rs`, `controller.rs` | done | `test_probes` (5) | `--probe-autostart` exit 0 |
| Tasks page (running + history) | GUI | progress, errors, clear-finished | `gui.rs:view_tasks`, `tasks.rs` | done | `test_desktop_tasks` (16) | GUI manual |
| Settings (theme/checks/autostart/suffix) | GUI | live apply, restore at startup | `gui.rs:view_settings`, `settings.rs` | done | `test_settings` | GUI manual |
| Appearance System/Light/Dark | GUI | live, restored at startup | `gui.rs` + `set_theme` | done | code review | GUI manual |
| Unsafe-extract fallback (opt-in) | GUI toggle + per-file confirm | off default; runs only when safe found nothing | `inspector.rs`, `settings.rs` | done | gated-execution test | `cargo test` |
| i18n catalog + pseudolocale | all GUI strings via `t!` (80 calls) | localizable, qps-proven | `i18n.rs`, `i18n/qps.json` | done (machinery) | `test_i18n` (9) | `LC_ALL=qps` run |
| Human translations | — | non-English catalogs | — | missing | — | PLAN-004 |
| Drag/drop files into window | — | upstream lists drag/drop open | — | missing | — | PLAN-007 |
| Zsync delta update | — | binary delta instead of full file | `updates_service.rs` (full only) | missing (documented) | — | accepted limitation |
| Debug-logging toggle effect | GUI switch persists | setting changes log output | `settings.rs:88`, `gui.rs:1206` | gap: nothing reads it, no log backend | none | PLAN-003 |
| MaxAppImageBytes control | — | user-adjustable bound | `settings.rs` (persisted+clamped) | gap: enforced, no UI/CLI | none | PLAN-006 |
| JSON list output schema v1 | CLI `--json` | installed/updates/discovered arrays | `cli.rs:137-155` | done | schema tests | `cargo test` |
| Exit 8 on failed update checks | CLI | distinguish "none" from "unchecked" | `cli.rs` | done | `test_cli` | `cargo test` |
| `--self-test` readiness | CLI | real checks incl. readiness | `cli.rs:run_self_test` | done | binary run → SELF_TEST_OK | observed this session |
| Icon extraction + install | via integrate | real icon, not generic fallback | `inspector.rs`, `desktop.rs:316-340` | done | `test_icon` (4) | `cargo test` |
| Desktop metadata carry-over | via integrate | categories/mime/terminal honored | `desktop.rs` | done | `test_icon` metadata | `cargo test` |
| Desktop-db refresh after integrate | via integrate | u-d-d / update-desktop-database | `integration.rs:465-480` | done | `test_integration` | `cargo test` |
