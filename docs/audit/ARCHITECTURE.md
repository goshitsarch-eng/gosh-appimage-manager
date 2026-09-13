# Architecture (Phase 2)

## Shape

```
            ┌─────────┐   ┌──────────┐
            │  main   │──▶│   CLI    │──┐
            │ (route) │   │ (cli.rs) │  │
            └─────────┘   └──────────┘  │   ┌──────────────────┐
                        ┌──────────┐   ├─▶▶│  AppController   │
                        │ GUI (gui │───┘   │ settings/registry│
                        │ .rs, opt)│       │ /tasks           │
                        └──────────┘       └────────┬─────────┘
                                                   │ services
        ┌──────┬──────┬──────┬──────┬──────┬─────────┴────┬──────┬───────┐
        │elf   │insp- │integ-│lib-  │desk- │updates     │net-  │pro-   │
        │      │ector │ration│rary  │top   │svc+sources │work  │cess   │
        └──┬───┴──┬───┴──┬───┴──┬───┴──┬───┴──┬──────┬────┴──┬───┴──┬────┘
           │      │      │      │      │      │      │       │      │
     ┌─────┴──────┴──────┴──────┴──────┴──────┴──────┴───────┴──────┴─────┐
     │ foundation: types / limits / safe_fs / url_guard / settings /  │
     │ registry(SQLite) / tasks / i18n / trash / notifier / proctable  │
     └────────────────────────────────────────────────────────────────┘
```

Single-process, synchronous-core + thin-shells design. GUI reversibly maps
service calls onto worker threads via `Command::perform`; CLI calls them
directly. No daemon, IPC service, or plugin system.

## Strengths (observed)

- Composition root (`AppController`) owns all shared state; services take
  what they need — no globals besides the i18n catalog.
- Seams everywhere it matters: process runner, network, proc table, trash
  are all faked in `tests/common.rs`; fail-point seams in integration and
  updates are now actually exercised (`test_rollback`).
- Error style is `Result<_, String>` with stderr diagnostics; CLI keeps
  stdout valid JSON.
- No unguarded panics in production paths; the remaining `unwrap`/`expect`
  calls are guarded invariants (`registry.rs` UUID presence checked
  beforehand) or test seams. Clippy `-D warnings` clean on rustc 1.98.
- Persistence split is sane: SQLite for the relational registry, JSON for
  flat settings, XDG paths with test overrides.

## Weaknesses / notes

### ARCH-01 — GUI keeps a view-model mirror beside `TaskQueue` (accepted)
`gui.rs:291` holds `tasks: Vec<TaskItem>` for rendering while also driving
`controller.tasks_mut()` (`begin/mark_cancelling/finish`). Two sources of
task truth can drift. Mitigation: GUI refreshes its mirror from queue
events; acceptable, but any future task feature should read the queue as
primary. No PLAN item (works, tested).

### ARCH-02 — `Result<_, String>` throughout (accepted)
String errors are easy to write and hard to match on.
Callers needing structured recovery (CLI exit codes, GUI retryability)
re-derive semantics from text. Migration to typed errors would touch every
module; not justified now. No PLAN item.

### ARCH-03 — Debug-logging has no backend (→ PLAN-003) — RESOLVED
`src/diagnostics.rs` is now the consumer: the setting gates stderr
diagnostic lines in the CLI and on GUI worker completion.

### ARCH-04 — Dead code residue (→ PLAN-008) — RESOLVED
`Page::title` was deleted; `localized_title` is the single source.

## Data flows

- Integrate: file → inspect → stage (managed dir) → desktop entry + icon →
  registry row → desktop-db refresh; any failure → rollback removes staged
  artifacts and restores backups.
- Update: check (per-manager metadata) → offer → staged download (0600,
  streamed, bounded) → AppImage+arch+digest validation → atomic rename →
  registry update; running-app guard gates apply unless `--force`.
- Launch: registry row → argv build (args + `--env=` under Flatpak) →
  detached spawn, start-only.
- Removal: ownership check → trash (or guarded delete) → desktop/icon
  cleanup → registry delete.

## Invariants (enforced, tested)

1. Open/inspect never integrates or executes.
2. Trash failure never becomes delete.
3. No shell strings; no unescaped `%` in Exec.
4. Redirects and resolved addresses re-checked per hop.
5. Rollback material retained until success.
