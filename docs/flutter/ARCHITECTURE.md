# Architecture: Flutter GUI on the Rust core

Gosh AppImage Manager 3.0.0 has one front end, written in Flutter, and one
core, written in Rust. The GUI is the only part that is not Rust, and no toolkit other than Flutter is in the tree.

## The four parts

```
 flutter/               Flutter GUI (Dart) ── pages, shell, window chrome
   lib/src/rust/        generated Dart bindings (flutter_rust_bridge 2.13.0)
        │  flutter_rust_bridge calls (coarse operations, plain DTOs)
 bridge/                Rust crate "goshaim_bridge": the bridge API (src/api/*.rs)
        │  calls into the core's public functions
 src/                   Rust core, package gosh-appimage-manager (library goshaim_core)
 src/launcher.rs        the one executable the user runs
```

- **The core (`src/`)** owns every behaviour: inspection, integration, removal,
  updates, the task history, settings, and the safety rules in AGENTS.md. It
  is unchanged by the Flutter work.
- **The bridge (`bridge/`)** exposes the core as coarse operations (for
  example `check_updates`, `integrate_app`, `remove_app`) and plain data
  transfer objects (`AppDto`, `TaskDto`, `SettingsDto`). It holds no state of
  its own beyond what the core keeps. `bridge/Cargo.toml` depends on the core by
  path; `flutter/rust_builder` builds the bridge with cargokit, on every host.
- **The Flutter GUI (`flutter/`)** draws the screens and turns user actions
  into bridge calls. It never builds a shell command; every external action is
  a bridge call or a platform channel call.
- **The launcher (`src/launcher.rs`)**: `gosh-appimage-manager` with no CLI
  command replaces itself with
  `<install root>/libexec/gosh-appimage-manager/gosh-appimage-manager-gui`, or
  the path in `GOSH_APPIMAGE_GUI`. CLI commands are handled by the core
  directly. Paths given on the command line are forwarded to the GUI, which
  opens them in Inspect.

## Flutter GUI layout

| Path | What it holds |
|---|---|
| `lib/main.dart` | Starts the bridge, creates the `AppModel`, shows the window, then loads (command-line files open in Inspect). |
| `lib/app.dart` | `GoshApp`: the Material theme, the light and dark palettes, and the system or saved appearance. |
| `lib/state/app_model.dart` | Every workflow and its status text. Pages render it and call into it. Mutations run one at a time through a queue. |
| `lib/state/format.dart` | Pure helpers: sizes (`84.2 MB`), `~` paths, clock and task times, sort and filter rules, environment parsing. |
| `lib/core/core_api.dart` | The `CoreApi` interface the model calls, and `BridgeCore`, its implementation over the generated bindings. Tests use a fake. |
| `lib/theme/app_theme.dart` | The mockup's tokens: `AppPalette` (light and dark, SPEC section 3), `AppType` (Onest and IBM Plex Mono roles, section 4), radii, `appThemeData`. |
| `lib/ui/shell.dart` | The window: custom title bar, sidebar (or the narrow menu drawer), page area, status line and bar, keyboard shortcuts, file drop, dialog overlay. |
| `lib/ui/widgets.dart` | The controls: buttons, icon buttons, text fields, segmented control, toggles, count pills, key hints, dots, letter tiles, cards, progress, dashed border, SVG icons. |
| `lib/ui/library_page.dart`, `detail_page.dart` | Library (desktop table, narrow list, empty state, adopt list) and the app's Detail page. |
| `lib/ui/inspect_page.dart`, `updates_page.dart`, `tasks_page.dart`, `settings_page.dart`, `about_page.dart` | The other pages. |
| `lib/ui/dialogs.dart` | The confirmation card (`AppDialogCard`, 460 px) and the dialogs: remove, name conflict, running app, adopt. |
| `lib/ui/page_frame.dart` | The page heading and gutters shared by the pages. |
| `lib/platform/window_channel.dart` | The Dart side of the window channel (`gosh/window`). |
| `lib/platform/file_picker.dart` | The file and folder choosers, behind `FilePickers` so tests can answer them. |
| `assets/fonts/` | Onest (400, 500, 600, 700) and IBM Plex Mono (400, 500, 600) as full TTF files from the google/fonts repository, with `OFL.txt`. |
| `assets/icons/` | The 17 SVG icons copied from the mockup's own markup, drawn with `flutter_svg`. |
| `test/` | Unit tests (`format_test`, `app_model_test`), the bridge test, `support/fakes.dart` (the mockup's sample library), `support/harness.dart` (pumps the app), `pages_golden_test.dart` (one golden per frame), `features_test.dart` and the `parity_*_test.dart` files (the FEATURES rows; docs/flutter/PARITY.md names each test). |

### Layout rules

- The desktop frame is the content box of the mockup window: 1280 x 800. The
  mockup's 1 px border is the OS window edge in the app, so the layout origin
  is the mockup's (1,1). Title bar 46 px plus a 1 px border; sidebar 224 px
  plus 10 px padding each side and a 1 px border; status bar 30 px plus a 1 px
  border.
- The **narrow layout** starts below **800 px** of window width
  (`narrowBreakpoint` in `ui/widgets.dart`). The sidebar becomes a drawer behind
  the menu button, rows collapse to a name, version and one status line, and
  the row actions move into the row menu. The mockup's 360 px frame is inside
  this range.
- Bordered controls are their declared height plus 2 px (CSS content-box
  sizing), so a 32 px secondary button is 34 px tall. Cards take their 1 px
  border inside their box.

### Window chrome

The Linux runner (`flutter/linux/runner/my_application.cc`) creates an
undecorated GTK window of 1280 x 800 with a 360 x 480 minimum. Flutter draws
the title bar (logo tile, title, minimize, maximize, close). The channel
`gosh/window` carries these calls from Dart to the runner: `setTitle`,
`minimize`, `toggleMaximize`, `close`, `startDrag`, and `startResize` with an
edge name. The runner sends `maximizedChanged` back so the maximize control
shows the right state. Invisible 6 px grips on the edges start native resizes.

The runner's window has square corners. The mockup's 12 px corner radius and
drop shadow belong to the presentation board; a native window is rectangular.

## Data and safety

- Every mutation goes through the core, and the core enforces the AGENTS.md
  rules (Trash before deletion, verified ownership before replacement,
  rollback material kept until success). The GUI adds confirmations where the
  mockup asks for them, and never deletes after a failed Trash: the model
  keeps the row and reports the error (widget test `trash that fails deletes
  nothing`).
- The unsafe extraction fallback is shown disabled and marked **Unavailable**,
  as the mockup shows it. The core refuses to enable it
  (`UNSAFE_EXTRACTION_AVAILABLE` in `src/settings.rs` is false), so the GUI and
  the core agree.
- Task history carries what the Tasks page shows: the start and end times
  (Unix seconds), the versions an update moves between, and the update stage
  (1 Download, 2 Verify, 3 Swap in) with the bytes moved in it. An update
  reports these through `ApplyEvent` (`src/types.rs`) while it runs, for single
  and batch updates alike. The percent is the download fraction.
- An installed app carries `integrated_at`, the Unix time it was first
  integrated or adopted. The registry adds that column to an existing database
  with an idempotent migration (`ADDED_COLUMNS` in `src/registry.rs`). A row
  from before the column keeps its data and has no date; none is invented.
  The same migration adds `integrated_folder`, the folder the AppImage was
  integrated from, which the integration writes in the row's own statement and
  `AppDto` carries to the Detail page.
- An app's icon is read from its AppImage by the core, never by running it: the
  icon the desktop entry names, then `.DirIcon` (a symlink in nearly every real
  AppImage, resolved by name inside the archive's own listing), then the theme
  folders by size. Each candidate is judged by its content (PNG, SVG, XPM). The
  core installs it: beside the menu entry in the icon theme for an integrated
  app, and in `~/.local/share/gosh-appimage-manager/icons/` for an adopted one,
  because adoption writes nothing to the user's menu or icon theme. The file name
  carries the app's id, which is how removal proves it is ours. The GUI draws the
  file with the image decoder, or with `flutter_svg` for an SVG, and shows the
  letter tile for anything it cannot draw. After start, `heal_library_icons`
  gives apps with no icon file another look, once per app per run, and the model
  reloads the Library if any changed (`AppModel.healIcons`).
- Adoption reads the file as Inspect does, so an adopted app has the name,
  version, architecture, checksum and update string the AppImage carries. An
  embedded `gh-releases-zsync` string selects the GitHub source; its pattern names
  the `.zsync` control file, and the AppImage beside it is what is downloaded.
  Whether a release is an update is one predicate, `offers_update`, used by the
  list, the single check, the apply and the bridge.
- "Check for update" on the Detail page calls `check_one_update`. It reports
  what the source offers and never downloads or applies. Applying is the
  Update action, which the core refuses while the app runs unless forced.
- Background checks are off by default (owner decision). The login entry runs
  `--fetch-updates --background`. The core refuses to add that entry while
  background checks are off, the CLI stops before contacting a source when they
  are off, and startup reconciles an existing entry with the setting.

## Build, test and package

- Bridge bindings are generated with `flutter_rust_bridge_codegen` 2.13.0
  (stable) from `flutter/flutter_rust_bridge.yaml`. Regenerate after a change
  to `bridge/src/api`.
- `just flutter-check` (run from the repository root) formats, analyzes and
  tests the Dart code with a temporary `GOSHAIM_HOME`.
- `flutter build linux --release` builds the bundle. Its executable is
  `gosh-appimage-manager-gui`, with application ID `com.goshapps.AppImageManager`.
  The Flatpak manifest installs the bundle under `libexec/gosh-appimage-manager/`.
- `tools/gui-smoke.sh` runs the release bundle against a temporary home, checks
  that its window is open, saves a screenshot, and stops it.

## Verification record

Mockup parity, frame by frame, with the evidence paths, is in
[PARITY.md](PARITY.md). That file also maps each FEATURES row to its test.
