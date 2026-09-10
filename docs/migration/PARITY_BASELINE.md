# Parity baseline (Task 1) — 2026-09-10

Method: mapped each `docs/migration/PLAN.md` checklist item to current
core/GUI/test evidence by reading `src/controller.rs`, `src/gui.rs`,
`src/cli.rs`, and the relevant `tests/`. No code changes.

## Per-item status

- Shell pages (Library/Inspect/Updates/Tasks/Settings/About): PASS.
  `gui.rs:74-91` nav, `view_library/inspect/updates/tasks/settings/about`,
  `tools/gui-smoke.sh` renders all six.
- Open AppImage (chooser, open-with, multi-file): PASS with gap note.
  `gui.rs:715` queues all `initial_files` (prior single-file-drop bug fixed
  per AUDIT); portal file picker for Inspect. Drag/drop acceptance into the
  window is the remaining unknown — needs packaged-compositor confirmation.
- Inspection flow (path/size, type/arch, name/version/summary/terminal/icon,
  source status, already-managed, incompatibility, copy-vs-move, keep-both vs
  replace, single confirmation, per-item isolation): PASS at core level.
  `controller.inspect_file/inspect_with/integrate`, `test_inspector`,
  `test_integration`.
- Integration (configurable folder, menu entries, copy/move, conflicts,
  transactional + rollback): PASS. `test_integration`, `test_rollback`
  (4 injected failure points).
- Library detail (launch detached, reveal, check/update now, args-list edit,
  env NAME=value edit, source configure/reset, metadata refresh,
  path/ID/hash/type/arch/version/size/manager/provenance, Trash default,
  explicit destructive delete): PASS at core+GUI level.
  `controller.remove_app/reveal_in_file_manager/refresh_metadata/
  set_arguments_and_environment/apply_update/set_update_source/
  unset_update_source`, `gui.rs` detail view (~1793-1910), Adopt flow,
  `test_detail`.
- Discovery/adoption incl. outside-folder opt-in: PASS.
  `controller.discover/adopt_external`, `test_library`, GUI adoptable section.
- Updates (check/download/cancel/apply per-item + batch, progress, summary,
  running guard + force, `.upd_info`): PASS.
  `controller.check_updates/scan_updates/apply_update`,
  `gui.rs start_update(force)`, `Message::Cancel`, `test_update` (18 tests).
- Update managers (static, GitHub, GitLab, Codeberg, Forgejo, FTP): PASS.
  `updates_sources.rs`, `test_update`, `test_ftp`, `test_glob`.
- CLI JSON v1 (integrate, update, remove, remove-all, list-installed,
  list-updates, list-update-managers, set-update-source, --fetch-updates,
  --adopt, --probe-*): PASS. `cli.rs`, `test_cli`, `test_probes`.
- Background checks + notifications, non-mutating autostart probe: PASS.
  `notifier.rs`, `cli.rs run_autostart_probe`, `test_probes`.
- Extraction safety (Type 1/2, SquashFS/DwarFS, bounded archives, unsafe
  fallback default-off with per-file warning): PASS. `inspector.rs`,
  `test_inspector`, `test_icon`.
- Sandbox posture (minimal finish-args, portals, no host:rw, constrained host
  execution, validators): PASS on review, UNVERIFIED packaged.
  Manifest reviewed; `flatpak-builder` rebuild + `just validate` still owed
  (Task 3).

## Test baseline (this box)

- `cargo test --no-fail-fast`: 147 passed, 1 failed.
- Sole failure: `test_registry_perf::bulk_removal_is_linear` —
  ~4.38s vs 4000ms bound (300 SQLite removals, debug build). Single-test
  re-run varies around the bound (marginal/environment-sensitive, not a
  behavioral regression). All functional suites green.
- `cargo test --offline` fails at fetch (iced submodule needs network);
  online build required once. Not a defect.

## Gaps carried into Task 4

1. Packaged verification owed (Flatpak rebuild, --self-test, --run probes,
   validate, smoke).
2. Compositor-confirmed checks owed: drag/drop into window, narrow layout,
   keyboard-only walk, focus/contrast on real session.
3. Perf-bound flakiness (`bulk_removal_is_linear` ~4.4s vs 4s on this box):
   decide in Task 4 whether to raise bound, batch removals in a transaction,
   or record as environment-sensitive.
