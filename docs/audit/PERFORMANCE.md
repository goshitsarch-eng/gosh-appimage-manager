# Performance (Phase 2)

Format: issue | evidence | impact | fix | before→after.
Prior-round fixes (P-1…P-9) verified present; figures from `docs/verification.md`
+ `docs/migration/REPORT.md` where noted, else observed this session.

## Fixed in prior rounds (regression-guarded)

| Issue | Evidence | Impact | Fix | Before → after |
|---|---|---|---|---|
| Sync work on GUI thread (P-1) | `gui.rs` now routes work via `Command::perform` worker threads | UI froze up to 30s/app | worker commands + progress messages | freeze → responsive + cancel |
| Dead cancel (P-2) | `mark_cancelling` wired to `TaskQueue` (`gui.rs:1128`) | Cancel never took effect | real cancellation tokens | dead → working |
| Per-request HTTP client (P-3) | client reuse per host in `network.rs` | root-store parse + handshake per app | cached clients | N parses → 1 per host |
| Full-table registry rewrite (P-4) | single-row SQL in `registry.rs` | O(N) write per mutation | targeted UPDATE/DELETE | 100 updates/400 rows: ~320ms → ~150ms |
| Syscalls per lookup (P-5) | canonicalize-once in `by_path` | O(files×apps) lstat per scan | normalize once, compare in memory | 300 lookups/300 rows: ~47ms → ~0.24ms |
| stat() in render path (P-6) | autostart existence cached out of `view_settings` | stat every frame | snapshot at message time | per-frame I/O → none |
| Redundant copy+rehash (P-7) | backup via rename-hardlink strategy | extra full read+write per GB app | link, hash in-memory buffer | 2 extra GB passes → 0 |
| /proc scan per app (P-8) | single batch enumeration | O(apps×procs) readlinks | one pass, join in memory | batch == per-app answers (test) |
| Reader-thread join hang (P-9) | bounded join after timeout kill | 30s timeout defeatable | timeout on join, drop handles | unbounded → bounded |
| Glob DoS (S-4) | linear matcher in `updates_sources.rs` | 15-char pattern ≈ 3.15s; 25-char ≈ hours | linear algorithm | 49-char case: >3min → <1s |
| Rows per frame | bound added (commit `e13e851`) | jank on huge libraries | cap materialised rows | unbounded → bounded |

## Open

### PERF-01 — Registry perf tests are load-sensitive (→ PLAN-002, P2)
- Evidence: `test_registry_perf` 2 failures in 1 of 3 full-suite runs here
  (27s wall, parallel); green in isolation (10.5s). Migration report notes
  300 individually-fsyncing DELETEs in debug builds (~4.4s vs old 4s bound).
- Impact: flakes erode CI signal; could mask a P-4 regression.
- Fix: run the suite `serial`/`--test-threads=1`, raise bounds with
  machine-normalized rationale, or assert operation counts (fsync/SQL
  statements) instead of wall time.
- Before→after: flaky wall-time assert → deterministic structural assert.

### PERF-02 — Debug-build fsync cost dominates bulk removal (accepted)
- Evidence: 300 DELETEs ≈ 4.4s debug (migration report).
- Impact: test-only; release + WAL unaffected at real library sizes.
- Fix: none required; optionally batch DELETEs in one transaction.
- Before→after: n/a (accepted; batching is P3 if ever user-visible).

## Not applicable (confirmed)

- No ORM/query-builder/user SQL: registry loads into memory; N+1/`SELECT *`
  classes do not arise. Pagination unnecessary at tens-of-apps scale.
- No assets pipeline, no server, no bundle to split.
