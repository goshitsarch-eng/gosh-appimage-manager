# COSMIC UX review (Phase 2)

Standard order: libcosmic component guidance → freedesktop specs →
WAI-ARIA APG for semantics libcosmic leaves unspecified. No compositor was
available in this session, so layout/render claims below are code-derived
and flagged for the real-compositor pass (PLAN-001).

Per-area: current | pattern | problems | planned change | verify.

## Navigation (6 pages)

- Current: `Page` enum (Library/Inspect/Updates/Tasks/Settings/About),
  `nav_bar` with localised titles (`gui.rs:61-99`), condensed layout keeps
  a nav toggle (AUDIT follow-up verified by clicking).
- Pattern: libcosmic `nav_bar` pages.
- Problems: none structural. Narrow-width behaviour unverified on a
  compositor (AppStream claims 360px usability).
- Planned change: none; cover in PLAN-001 compositor pass.
- Verify: PLAN-001 (tile to 360px, condensed-nav click-through).

## Dialogs (destructive confirmations)

- Current: `PendingDialog` enum (conflict/replace-UUID resolved from
  inspection, remove, unsafe-extract, update-force, adopt) rendered via
  `view_dialog` as a modal layer (`gui.rs:2358`).
- Pattern: cosmic dialog surface with scrim/focus-trap/Esc.
- Problems: none known; G-3 (inline non-modal dialog) fixed in prior round.
- Planned change: none; Esc/focus-trap behaviour re-confirm in PLAN-001.
- Verify: PLAN-001 keyboard walkthrough.

## Library + detail

- Current: search field, 3 sort orders, rows with running badges, empty
  state; selecting a row opens detail (launch/reveal/check/update/refresh,
  args+env edit, source set/reset, provenance).
- Pattern: settings-section list + detail page.
- Problems: rows-per-frame now bounded (commit `e13e851`); no
  virtualization at very large libraries — acceptable at tens of apps.
- Planned change: none.
- Verify: `test_detail`, GUI manual in PLAN-001.

## Inspect (multi-file)

- Current: portal picker + file args queued (`gui.rs:713`), confirmed one
  at a time; safety caption present and translated.
- Pattern: single-confirmation sequential flow (brief §2).
- Problems: no drag-and-drop (upstream lists it) → PLAN-007 (P3).
- Planned change: add dnd target if libcosmic/iced supports it on the
  pinned stack; else keep picker+args and document.
- Verify: drop 3 files → 3 sequential confirmations.

## Updates

- Current: per-app offers with reduced-verification display, per-row
  failure indicators (C-6/G-2 fixed), Update-all with working cancel,
  loading states (G-1 fixed).
- Pattern: status list with error/empty/offline states.
- Problems: none known.
- Verify: offline run shows failure state, not "up to date" (regression
  test `failed_checks_are_reported_not_silently_dropped`).

## Tasks

- Current: `view_tasks` shows running + recent with progress/errors and
  "Clear finished" (`gui.rs:2125`); backed by `TaskQueue`.
- Pattern: activity history.
- Problems: none known.
- Verify: run update → entry appears, progresses, completes.

## Settings

- Current: appearance, background checks, autostart (file-existence read),
  managed folder, terminal-suffix, unsafe fallback, debug logging.
- Pattern: `settings::section` + labelled togglers (G-4 fixed: no bare
  `toggler(None)`).
- Problems: (1) debug-logging switch does nothing (BUG-001);
  (2) autostart-vs-background-checks relationship worth one caption line.
- Planned change: BUG-001 fix; caption copy in the same pass.
- Verify: PLAN-003 acceptance.

## Status/errors

- Current: differentiated status sink with severity styling (G-5 fixed);
  errors cleared on next action.
- Pattern: inline status + toast-equivalent.
- Problems: none known.
- Verify: PLAN-001 spot check.

## Keyboard

- Current: shortcuts/accelerators added in prior round (G-7).
- Pattern: standard accelerators (quit, open, refresh, delete).
- Problems: full focus-order walk unverified without compositor.
- Verify: PLAN-001 keyboard-only run.

## Appearance/theme

- Current: System/Light/Dark via `set_theme`, applied live and at startup
  (G-8 fixed); startup applies saved preference.
- Pattern: COSMIC theme mode follow.
- Problems: none known.
- Verify: restart on each mode in PLAN-001.

## Localization/RTL

- Current: 80 `t!()` lookups, JSON catalogs, `qps` pseudolocale proof;
  English fallback; product name + data stay untranslated by design.
- Pattern: catalog lookup with source-English default.
- Problems: no human-language catalog ships (PLAN-004); no explicit RTL
  pass (libcosmic/iced handles base direction; unverified visually).
- Verify: PLAN-004 ships ≥1 checked translation; RTL screenshot in PLAN-001.

## Motion/text scaling

- Current: label columns scale with text size (commit `e13e851`); fixed
  spacing constants remain elsewhere (G-16 partial).
- Pattern: spacing scale + text-size-relative layout.
- Problems: residual fixed `spacing/padding` constants; no reduced-motion
  consultation (low impact — little animation).
- Planned change: none scheduled (P3 backlog, PLAN-009).
- Verify: 1.5× text scale screenshot in PLAN-001.
