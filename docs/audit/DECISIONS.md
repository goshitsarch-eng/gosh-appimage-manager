# Decisions (Phase 2)

Seeded triage order for all future calls:
**security/data-loss > correctness > completeness > Flatpak > a11y >
COSMIC > perf > maintainability > cosmetic.**

## Standing decisions (carried, with rationale)

1. **Rust + libcosmic is the stack; `docs/implementation-brief.md` stack
   claims are superseded, its behavioural requirements still bind.**
   The tree contains no Qt/CMake; `rewrite-3.0.0.md` records the pivot.
2. **Gear Lever is a behavioural reference only.** No upstream source,
   templates, CSS, icons, screenshots, app-id, or branding reused
   (project rule, `AGENTS.md`, `upstream-audit.md`).
3. **Open/inspect never integrates or executes.** Non-negotiable invariant;
   unsafe fallback stays opt-in + per-file confirmed + default off.
4. **Host-execution Flatpak grant stays, constrained.** Removing it removes
   AppImage launching. Constrained to named helpers + checked paths;
   portal migration only where verifiable (SEC-01 / PLAN-005).
5. **No unverifiable translations ship.** Pseudolocale-proven machinery +
   English until a competent speaker contributes (PLAN-004). Same rule
   applies to any future locale.
6. **Zsync = full-file download.** Documented limitation; delta support is
   not on the plan (cost/benefit at current scale).
7. **FTP stays as explicit legacy with warning.** No removal, no promotion.
8. **Wall-time perf asserts are backstops, not gates.** Load-sensitive
   bounds get structural asserts (PLAN-002); a bound that flakes is a test
   bug, not a product bug, until proven otherwise.
9. **`Result<_, String>` stays.** Typed errors would touch every module for
   little user gain (ARCH-02).
10. **Uppercase app-id pedantic note accepted.** The id is shipped; the
    AppStream info-level note cannot be fixed by us.
11. **L-2 (offline cold-cache build) is not a defect.** Withdrawn with
    evidence in `AUDIT.md`; do not re-raise without new data.
12. **No synthetic-corpus expansion into real-AppImage execution in tests.**
    Safety rule: fixtures stay synthetic; extraction tools driven through
    seams.

## Decisions taken this round

13. **Debug-logging switch must become real or go (PLAN-003, P2).** A
    settings control with no effect violates correctness-over-completeness
    ordering; either end state is acceptable.
14. **Real-compositor pass is the sole P1 (PLAN-001).** Everything else is
    P2 or lower: no known crash, corruption, or security hole remains open.
15. **aarch64 trust = CI, not local rebuild.** No qemu here; the release
    gate is green CI on both arches (PLAN-010), not a local aarch64 build.

## Decisions taken in audit-hardening

16. **Diagnostics without new dependencies.** std-only `diagnostics.rs`
    (basename-only, bounded) beats `log`/`env_logger`: no vendoring churn,
    no Flatpak source regen, testable via writer injection. Security >
    maintainability.
17. **Wall-time perf asserts are backstops with headroom math.** 15s/8s/30s
    ceilings document the rewrite ratio they still catch (400x/47x/150x);
    structural asserts are the real gates. A bound that flakes is a test bug.
18. **Drops never preempt.** `drop_queue::plan_drop` is pure and tested:
    busy or unconfirmed work queues drops; idle starts the first. Empirical
    queue rule over ad-hoc handler logic.
19. **No machine translation ships.** Decision 5 holds against AI-generated
    catalogs: PLAN-004 needs a human speaker + compositor layout proof.
    Correctness > completeness.
20. **No dependency changes without a security/correctness reason.** All fixes
    used std + existing crates; `Cargo.lock` untouched. Maintenance >
    novelty.
