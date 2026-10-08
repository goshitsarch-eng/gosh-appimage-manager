# Flutter front end: parity with the libcosmic GUI

Date of this review: 2026-10-08, aarch64 host (GNOME 50 on XWayland).

The reference is the libcosmic GUI in `src/gui.rs` (version 3.0.0). The
Flutter front end is `flutter/lib/`, on the bridge in `bridge/`. This file
records what was matched, how it was checked, and what still differs. It is the
input to the parity sign-off that gates the cut-over (`ARCHITECTURE.md`, section
13, phase 7).

## 1. Feature map

| Area | Original (`src/gui.rs`) | Flutter (`flutter/lib`) | Evidence |
|---|---|---|---|
| Pages | Library, Inspect, Updates, Tasks, Settings, About, in that order | `ui/shell.dart`, `ui/*_page.dart` | Golden per page (`flutter/test/goldens/`) |
| Window title | "Gosh AppImage Manager — <page>" | `platform/window_channel.dart` and the runner's `gosh/window` channel | Read back from the running release bundle: "Gosh AppImage Manager — Library" |
| Navigation rail | 280 px, floating 8 px in from the window edge, selected label in the accent | `ui/shell.dart` (`NavRail`) | Captures, Library (dark) |
| Condensed layout | Breakpoint at 648 px; rail becomes a drawer behind the header toggle; rows and sort buttons stack | `ui/widgets.dart` (`condensedBreakpoint`), `ui/shell.dart` | Golden `library-condensed.png`; header toggle wired (see section 4) |
| Library | Search by name, version or path; sort by Name, Version, Updates first; badges; empty, loading and no-match states; "Not managed yet" adoptable list | `ui/library_page.dart`, `state/app_model.dart` (`visibleLibrary`, sorts) | `app_model_test.dart` (sort and filter); golden `library.png`; capture comparison |
| Details | Facts, actions, command arguments and environment, update source | `ui/library_page.dart` (`DetailPage`) | Golden `details.png`; `app_model_test.dart` (environment validation) |
| Inspect | Path field, Browse, Inspect, queue note, error, summary rows, warnings, Integrate | `ui/inspect_page.dart` | Golden `inspect-result.png`; `app_model_test.dart`; integration test (inspect of a real file, invalid file) |
| Integrate | Conflict dialog with Keep both, Replace (only when one installation is implicated) and Cancel | `ui/dialogs.dart`, `state/app_model.dart`, bridge `OutcomeDto.conflict_uuid` | `app_model_test.dart`; integration test (second copy reports the candidate) |
| Remove | Trash and permanent confirmations with the original wording | `ui/dialogs.dart` | Golden `remove-dialog.png`; `app_model_test.dart`; integration test (permanent removal deletes the file) |
| Updates | Check now, Update all, Update one, running-app confirmation, failures listed before offers | `ui/updates_page.dart`, `state/app_model.dart` | `app_model_test.dart`; golden `updates.png`; integration test (check on an empty library) |
| Tasks | Newest first, state words, progress while running, Clear finished | `ui/tasks_page.dart` | Golden `tasks.png`; the model polls the core while an operation runs |
| Settings | Appearance, managed folder, max size, behaviour toggles, update checks, login check, unsafe fallback | `ui/settings_page.dart`, `state/app_model.dart` | Golden `settings.png`; `app_model_test.dart`; integration test (save and restore) |
| About | Version, byline, description, safety, attribution, licence, telemetry | `ui/about_page.dart` | Golden `about.png` |
| Status line | "Note", "Done", "Error" prefixes; "Working: <task>…" with Cancel while busy | `ui/shell.dart` (`_StatusLine`) | `shell_widget_test.dart` (busy line and Cancel) |
| Keyboard | Ctrl+O, Ctrl+R, Ctrl+F, F5, Escape | `ui/shell.dart` (`CallbackShortcuts`) | Escape covered by `shell_widget_test.dart`; the others are not exercised (section 3) |
| Drop onto the window | Queues behind current work; never discards an unconfirmed inspection | `ui/shell.dart` (`DropTarget`), `state/format.dart` (`planDrop`) | `format_test.dart` (planDrop cases); not exercised on screen |
| Files on the command line | First file inspected, the rest queued | `main.dart` passes the arguments to `AppModel.start` | `app_model_test.dart` (multi-file open) |
| Launch, reveal | Launch and Reveal in file manager; status texts | `state/app_model.dart`, bridge `launch_app`, `reveal_app` | `app_model_test.dart` (fakes); not exercised on screen |
| Colours | COSMIC dark and light palettes | `theme/cosmic_theme.dart` | Values derived by `cosmic-theme` itself (section 2) |
| Typography | Fira Sans; sizes 32/28/24/20/14/10 px | `theme/cosmic_theme.dart` (`CosmicType`) | Captures |

## 2. How the colours were checked

The palette was not hand-picked. A small program built against the
`cosmic-theme` crate at the revision in `Cargo.lock` printed the derived
`Theme::dark_default()` and `Theme::light_default()` colours. Every value in
`CosmicPalette` comes from that output, including the translucent button
states, which keep their alpha so Flutter blends them as iced does.

The dark Library captured from the original (`docs/flutter/parity/original-library-dark.png`)
and from the Flutter build (`docs/flutter/parity/flutter-library-dark.png`)
differ mainly in the header chrome and the search field's leading icon. The
rail, the sort row, row spacing, button shapes and colours line up to within a
few pixels at the same scale.

## 3. What was checked, and how

Commands, all run on this host:

- `just flutter-check`: `dart format` (clean), `flutter analyze` (no issues),
  `flutter test` (77 tests passed). The 8 bridge integration tests ran against a
  fresh `GOSHAIM_HOME` from `mktemp -d`, so they never touched a real home.
- `cargo test` in the repository root: all suites passed. `cargo clippy
  --all-targets -- -D warnings`: clean.
- `bridge`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` (the guard test turns a panic into an `Internal` error, which
  replaces the spike's panic probe).
- `flutter build linux --debug` and `flutter build linux --release`: both build.
- The release bundle launched on the scratch home and showed the page title.
  The run log had no error lines.

Not exercised on screen. From this session, XTEST pointer motion did not move
the pointer, and XTEST key presses did not navigate the Flutter window, so the
following were checked by tests only:

- Clicks on the navigation and on buttons (the widget tests press the same
  callbacks).
- The header toggle, Ctrl+O, Ctrl+R, Ctrl+F and F5.
- Drag and drop, and files given on the command line at launch.
- Launch, Reveal, Adopt, Update apply, Cancel of a long operation, and the
  autostart switch writing its desktop file.

## 4. Known differences

Intentional:

1. Multi-line fields for command arguments, environment and update-source
   configuration. The original's single-line fields could not hold more than
   one argument or `key=value` line, even though the caption asked for lines.
2. Update all reports running apps as "skipped (running)" in addition to the
   failure count. The original counted them only as failures.
3. Update errors appear under "Some updates did not apply". The original listed
   them under "Some apps could not be checked".
4. Library reads do not add Tasks entries or show the Working bar. The original
   recorded a "Loading library" task on every refresh.
5. The Tasks page refreshes every 400 ms while an operation runs, so progress
   moves. The original refreshed on events only.
6. The search field has a leading magnifier. The original's captured build shows
   a blank space there.
7. Focus outlines and Enter or Space on focused buttons.

Not matched yet:

8. Window chrome. Linux uses a GNOME header bar, with the bar colours set to the
   COSMIC background and text through CSS. The toggle icon is GNOME's
   `sidebar-show-symbolic`, and the window buttons are GNOME's.
9. Theme follows Flutter's platform brightness. The freedesktop colour-scheme
   portal is not queried yet.
10. The file chooser is GTK's (through `file_selector`), not the desktop portal.
11. The About page still says "Native COSMIC Epoch application", copied verbatim.
    The wording needs a decision at cut-over.
12. Translation catalogs. The original loads `i18n/*.json` at runtime
    (`GOSHAIM_LOCALE_DIR`, `LC_ALL`, and the `qps` pseudolocale). The Flutter UI
    shows English only. This is a gap, not a decision.
13. Dialog titles wrap where the font metrics require it. The golden for the
    removal dialog shows the title on two lines.

## 5. Open before the cut-over

- Phase 7 of `ARCHITECTURE.md`: the AGENTS.md update, the application ID and
  binary name (the runner still says `com.goshapps.gosh_appimage_flutter` and
  `gosh_appimage_flutter`), a Flatpak manifest for the Flutter build (flatpak is
  not installed here, so it cannot be built), and removal of the libcosmic GUI.
- The x86-64 build and the CI run (`.github/workflows/flutter.yml`, run
  37710389558) are blocked by the account billing lock.
- The core lock redesign (D-03) and the core fixes D-01, D-02 and D-11 are not
  started. The bridge and the Dart side keep the front end safe in the meantime.
