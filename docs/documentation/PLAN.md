# Documentation plan — Gosh AppImage Manager 3.0.0

Produced after the full application review (`APP-INVENTORY.md`) and the
claim-by-claim fact check (`AUDIT.md` in this directory). Sources of truth:
the source code, the Flatpak manifest, live runs of the CLI against an
isolated `HOME`, and a real `cargo`/`flatpak-builder` toolchain.

Legend: KEEP (no change) / FIX (small factual corrections) / REWRITE /
ANNOTATE (add a dated-snapshot banner; content is a point-in-time record) /
REMOVE / CREATE.

## Root files

| File | Action | Reason |
|---|---|---|
| `README.md` | REWRITE | Mostly accurate today, but: no installation section for users, no usage walkthrough, no contributing link, no AI-transparency notice, no keyboard-shortcut list; two safety claims are wrong (running-probe "cannot tell", DwarFS extraction), the `rust-version` pointer is misleading, the unsafe-fallback description promises a per-file confirmation that does not exist. |
| `AGENTS.md` | KEEP | Engineering rules; every checkable statement verified. |
| `AUDIT.md` | ANNOTATE | 2026-09-07 point-in-time audit. Internally consistent, but its Phase-3 "partially wired / dead" tables describe the pre-fix state with no per-row marker. Add a banner stating that; do not rewrite history. |
| `CONTRIBUTING.md` | CREATE | None exists. Cover: toolchain floor, build, `scripts/verify.sh`, tests, clippy/fmt, Flatpak dev build, where the docs live. |
| `Cargo.toml` | FIX | `rust-version = "1.75"` is below what the locked dependency graph compiles on (≥1.89 per manifest comment + dep metadata). Set the real floor so the field users are pointed at is truthful. |
| `AUTHORS`, `COPYING`, `third_party/` | KEEP | Legal/attribution files; accurate. |

## docs/

| File | Action | Reason |
|---|---|---|
| `docs/implementation-brief.md` | KEEP | Carries an accurate "superseded on stack, current on behaviour" banner. |
| `docs/rewrite-3.0.0.md` | KEEP | Rewrite record; claims verified. |
| `docs/upstream-audit.md` | FIX | Line 9 still calls the app "a new native C++20/Qt 6/Kirigami implementation" with no banner. One-line correction. |
| `docs/verification.md` | REWRITE | Stale: MSRV-enforcement claim is wrong, test count (134) is now 160, "no Flatpak build" superseded, self-test transcript shows retired `models-ready` check. Re-verify and update numbers. |
| `docs/audit/ARCHITECTURE.md` | FIX | "Zero unwrap/expect/panic in src/" is literally false; ARCH-03/04 items since fixed. |
| `docs/audit/BASELINE.md` | ANNOTATE + FIX | Dated 2026-09-12 snapshot whose "Advertised vs visible" table went stale mid-round (drag/drop, debug logging, max-size all now exist). Banner it as point-in-time and correct the three rows. |
| `docs/audit/BUGS.md` | FIX | "No unwrap/expect/panic in src/" false; MaxAppImageBytes row stale. |
| `docs/audit/COSMIC-UX.md` | ANNOTATE + FIX | Several "Current/Problems" entries describe the pre-hardening state. Banner + correct stale rows. |
| `docs/audit/DECISIONS.md` | KEEP | Verified consistent. |
| `docs/audit/FEATURES.md` | FIX | One row repeats the "cannot tell" overstatement; `t!` count stale. |
| `docs/audit/PACKAGING.md` | KEEP | Verified end-to-end against the manifest. |
| `docs/audit/PERFORMANCE.md` | FIX | PERF-01 listed open but recorded done in PLAN/BUGS. |
| `docs/audit/PLAN.md` | KEEP | Live plan; statuses accurate. |
| `docs/audit/REPORT.md` | KEEP | Historical report; internally consistent. |
| `docs/audit/SECURITY.md` | FIX | One row repeats the "cannot tell" overstatement. |
| `docs/migration/PLAN.md` | ANNOTATE | Reads as pending work; REPORT.md records it complete. Banner + tick the checklist. |
| `docs/migration/PARITY_BASELINE.md` | KEEP | Dated snapshot, clearly labelled. |
| `docs/migration/REPORT.md` | KEEP | Historical completion report; verified. |
| `docs/documentation/` | CREATE | This directory: `APP-INVENTORY.md`, `AUDIT.md`, `PLAN.md`. |

## i18n/

| File | Action | Reason |
|---|---|---|
| `i18n/README.md` | FIX | Instructions verified correct; clarify that the `qps` pseudolocale transform lives in `i18n.rs` (the `qps.json` file is an empty marker). |
| `i18n/qps.json` | KEEP | Empty marker file; works as intended. |

## Other

| File | Action | Reason |
|---|---|---|
| `.grok/task-prompt.md` | REMOVE | Unmarked 2.x C++/Qt mission prompt for a stack that no longer exists; only audit docs reference it (as having been read). Git history preserves it. |
| `justfile`, `scripts/verify.sh` | KEEP | Verified against behaviour; `just` recipes referenced from README/CONTRIBUTING. |
| `packaging/com.goshapps.AppImageManager.yml` | KEEP | Manifest comments all verified. |
| `data/*.desktop`, `data/*.metainfo.xml` | KEEP | Verified; `%F`, MIME types, licences consistent. |
| `.github/workflows/flatpak.yml` | KEEP | Comments match behaviour. |
| `tools/gui-smoke.sh`, `tools/contrast.py` | KEEP | Script comments accurate (add python3 to its dep note if mentioned). |

## README rewrite outline

Order: name + one-line description → AI-assisted development notice →
features → install (Flatpak bundle from CI / build from source) → use
(GUI walkthrough + CLI reference) → settings & file locations → keyboard
shortcuts → build/test/dev → Flatpak packaging → upgrading from 2.x →
safety → limitations → attribution → licence.

Corrections to carry in (all verified against code or live runs):

- Rust floor is ~1.89, not the declared 1.75; the Flatpak pins 1.90.0.
- Running-app detection inside Flatpak asks the host via `flatpak-spawn`;
  if that fails it falls back to the sandbox's own `/proc` view, which
  sees nothing — a failed probe effectively reads as "not running".
- DwarFS AppImages are detected and can be integrated, but embedded
  metadata extraction always fails: `dwarfsextract` is selected by name
  yet has no lister/extractor arm; the bundled `dwarfsextract`/`dwarfsck`
  are never invoked.
- Unsafe `--appimage-extract` fallback: implemented and off by default,
  but the per-file confirmation step has no UI, so it cannot run today.
- Update sources configured from the GUI accept a single `key=value`
  line; multi-key managers (github/gitlab/codeberg/forgejo) need the CLI.
- `-h`/`-V`/`-y` short flags exist.
- `XDG_*_HOME` variables are not read; only `HOME`, `GOSHAIM_HOME`, and
  `GOSHAIM_XDG_*_HOME` relocate app paths.
- "Move source" trashes the original after verified integration, never
  hard-deletes it.
- `ExitCode::NotFound` (3) is defined but never returned.
- `--probe-inspect` prints an `INSPECT_NO_EXECUTION` marker.
- Reveal opens the containing folder rather than selecting the file.
- `mimeinfo.cache` gets written beside desktop entries by
  `update-desktop-database` (best-effort).
