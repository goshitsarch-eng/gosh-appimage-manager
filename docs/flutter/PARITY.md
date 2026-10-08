# Mockup parity record

The reference is the design file `Gosh AppImage Manager Mockups.html` (read
only). Its exact values are in the specification the design was decoded into
(`SPEC.md`, outside the repository). This record lists each frame, how close
the Flutter app comes to it, which test asserts each FEATURES row, and the
decisions the owner still has to make.

Comparisons are at 1x. The mockup renders carry a 1 px frame; they are cropped
so both images share an origin. Goldens are the Flutter widget renders in
`flutter/test/goldens/`. The pixel figures below are ImageMagick's AE count
(differing pixels) as a share of the frame, measured in this pass; anti-aliased
text accounts for most of them.

## Frames

| Frame | Mockup render | Golden (flutter/test/goldens) | Differing pixels | Status |
|---|---|---|---|---|
| 01 Library | `renders/01-library.png` | `01-library.png` | 1.3% | Matches. Row order differs (deviation 1). |
| 02 Empty library | `renders/02-empty-library.png` | `02-empty-library.png` | 0.4% | Matches. |
| 03 Detail | `renders/03-detail.png` | `03-detail.png` | 1.5% | Matches, Provenance included (deviation 7, closed). The sample data of deviation 5 is the other difference. |
| 04 Inspect | `renders/04-inspect.png` | `04-inspect.png` | 0.6% | Matches. |
| 05 Updates | `renders/05-updates.png` | `05-updates.png` | 0.7% | Matches. Arrow glyph (deviation 6). |
| 06 Tasks | `renders/06-tasks.png` | `06-tasks.png` | 0.8% | Matches. Every value comes from the core's task fields. |
| 07 Settings | `renders/07-settings.png` | `07-settings-background-forced-on.png` | 1.0% | Matches. The golden forces "Check in the background" on, the mockup's state; the real default is off (deviation 11). |
| 08 Library, dark | `renders/08-library-dark.png` | `08-library-dark.png` | 1.2% | Matches in layout and palette. |
| 09 Detail, dark | `renders/09-detail-dark.png` | `09-detail-dark.png` | 1.4% | Matches with the frame 03 differences (deviation 5). |
| 10 Library, narrow (360 px) | `renders/10-library-narrow.png` | `10-library-narrow.png` | 2.4% | Matches the structure. Deviations 2 and 3. |
| 11 Remove dialog | `renders/11-dialog-remove.png` | `11-dialog-remove.png` | 0.8% over 183 rows | Matches. The app's card is 183 px tall; the mockup's is 184 (its spec height is 183.6). |
| 12 Name conflict dialog | `renders/12-dialog-conflict.png` | `12-dialog-conflict.png` | 1.4% | Matches, including the 208 px height. |
| 13 Running app dialog | `renders/13-dialog-running.png` | `13-dialog-running.png` | 0.9% over 183 rows | Matches; 183 px against the mockup's 184 (as frame 11). |
| About (no mockup frame) | none | `about.png` | none | Regression only. Settings row grammar; HEAD's About wording, and the Application ID, Homepage and footer lines (deviation 15). |

The mockup's 12 px window corners and drop shadow are not drawn (the native
window is rectangular; see ARCHITECTURE.md).

## Deviations and decisions for the owner

1. **Row order.** The Sort control reads "Name" and sorts A to Z. The mockup
   lists its sample rows in sample order (Quill Notes first), which is not
   alphabetical. The app follows the label. Say if the mockup order should win.
2. **Narrow window controls.** The mockup's narrow title bar has no window
   controls. The app adds minimize, maximize and close after Browse…, because
   the runner draws no native frame and the window would otherwise have no way
   to close. (Decided; kept.)
3. **Narrow list length.** The mockup shows six rows (Ledgerline is left out)
   beside "All 7". The app shows every app, so the seventh row appears.
   (Decided; kept.)
4. **Narrow search icon.** The mockup's narrow field has no icon. FEATURES row
   20 lists one, so the app keeps it. (Decided; kept.)
5. **Update source sample data.** The mockup shows "GitHub · example-org/quill-notes"
   in the Record and "repo=quill-notes" in the key=value field. One config value
   cannot produce both, so the fixture uses `example-org/quill-notes` in both.
   Owner decision: keep as is.
6. **Arrow glyph.** IBM Plex Mono has an arrow (U+2192). The mockup's subset
   lacked it and fell back to another font. The app uses Plex's own arrow.
7. **Integration date and source folder (closed).** The core stores the date an
   app was integrated (`integrated_at`) and the folder its AppImage was in when
   it was integrated (`integrated_folder`: the parent of the source path, made
   absolute). Detail's Provenance line reads `Integrated 2 Sep 2026 from ~/Downloads`,
   with the home folder as `~`, the way the Library rows write paths. An app
   integrated before this build has no stored date or folder and reads
   `Integrated`; none is invented. An adoption records no folder. A replace records
   the replacing integration's date and folder (owner decision: the installed file
   is the new one). `shortenHome` now shortens only the home folder and
   paths inside it: `/home/gosher` is no longer written `~er`.
8. **Unsafe extraction fallback (owner decision: opt-in).** The mockup shows the
   row disabled and marked Unavailable. AGENTS.md requires the fallback to be
   opt-in, clearly warned, and off by default, so the row is an enabled toggle,
   off by default. Turning it on opens a warning dialog: Cancel keeps it off, and
   only Turn on saves it. The core runs the fallback only when the stored setting
   is on, only after safe extraction fails, and only for a file the user confirms:
   the Inspect and Integrate dialog ("Run this AppImage to read it?", Cancel by
   default), or `--allow-unsafe` on the command line. Without a confirmation the
   read is reported as pending and nothing runs. It runs the AppImage itself, in a
   private staging copy with a timeout, a minimal environment, and a check of the
   extracted tree.
9. **Dialog button styles.** The mockup has no rule for red buttons (SPEC open
   question 11). "Move to Trash" is the accent primary, as drawn. Permanent
   delete uses the red button, because it cannot be undone.
10. **Folder picker.** "Change…" opens the system folder chooser. The old GUI had
    a text field and Apply. The chooser feeds the same validation (absolute,
    non-empty path) and the same save. Owner decision: the typed entry is restored.
    An editable absolute-path field sits beside Change…. A typed path saves on Enter
    or when the field loses focus, with the same checks, and an invalid path is shown
    inline and not saved. The mockup shows right-aligned text here instead of a field.
11. **Background checks default off (owner decision, row 88).** The mockup shows
    "Check in the background" on. The core default is off (`src/settings.rs`)
    and stays off. The toggle switches it on and off. The frame 07 golden forces
    it on, as the mockup shows it, and is named `07-settings-background-forced-on`.
    The login check ("Also check at login") is disabled until background checks
    are on; the core refuses it too.
12. **Dialog button box.** A bordered secondary button is 30 px high plus its
    1 px border on each side, so Cancel renders 32 px; the borderless primary
    renders 30 px. The mockup's measured Cancel is 32 px too.
13. **Dialog title line height.** SPEC 10 says "normal". The title takes a line
    height of 1.22, the value that puts its glyphs on the mockup's rows (measured
    against frame 11; the body then lands on the mockup's row as well).
14. **Login entry.** The entry runs `--fetch-updates --background`, so a login
    check does nothing when background checks are off. Startup removes an entry
    an earlier build wrote without the gate, or rewrites it with the flag.
15. **About wording.** The About page restores the sentences HEAD commits in
    `flutter/lib/ui/about_page.dart`. These are the same sentences as the
    text of the earlier GUI in `src/gui.rs` at HEAD~1, with its platform reference removed
    ("Native application for …"). Labels that HEAD never had keep the wording
    the working tree had: Purpose, Safety, Telemetry and the card titles. The
    Application ID and Homepage rows and the footer line `Licensed under
    GPL-3.0-or-later.` are back, in the Settings row grammar, because the user
    asked that About keep the content the existing About page shows. HEAD does
    not have them. Each is checked against its source: the application ID and
    the homepage in `data/com.goshapps.AppImageManager.metainfo.xml` and
    `Cargo.toml`, and the license in both. The footer sits under the cards, in the
    Settings help role.

16. **App icons (owner decision).** Library rows and the Detail header show the
    AppImage's own icon when it has one, and the letter tile otherwise. The Updates
    and Tasks pages keep letter tiles. The mockup shows letter tiles for its sample
    apps, which have no icons.
17. **GitHub update source (owner decision).** The field keeps one `key=value` pair:
    `repo=owner/name`. The AppImage's `filename` is still needed unless its embedded
    update information names the same repo. When it is missing, the refusal shows the
    exact `--set-update-source` command, with the app's path and the values typed.

## Known gaps

- **Restore after maximize.** The restored window can come back a few percent
  smaller (3090×1738 instead of 3200×1800). The window manager sets that size, and
  the app never requests it (QA2-006, D-19).
- **Library refresh.** Keyboard only: Ctrl+R or F5 (`flutter/lib/ui/shell.dart`).
  No refresh button exists in the mockup.
- Task times, byte counts and the versions of a running update now come from
  the core. They are no longer GUI guesses.

## FEATURES rows (SPEC section 12)

Status: **Done** (implemented; a test asserts the row's state or action),
**Done (golden)** (static text or decoration, covered by the frame golden),
**Deviation n** (see the deviations above), **Owner decision** (deviation 11).

Test prefixes: `features:` is `flutter/test/features_test.dart`; `lib:` is
`flutter/test/parity_library_detail_test.dart`; `upd:` is
`flutter/test/parity_updates_tasks_test.dart`; `format:` is
`flutter/test/format_test.dart`; `bridge:` is
`flutter/test/bridge_integration_test.dart` (the real core in a scratch home);
`Rust:` is a test in `tests/`; `crate:` is a unit test in
`bridge/src/api/updates.rs`. Every name below was checked against the files.

| Row | Feature | Frames | Status | Test (prefix: name) |
|---|---|---|---|---|
| 1 | Window minimize, maximize, close | 01-09 | Done | features: F1 window minimize, maximize and close act on the window |
| 2 | App title | 01-09 | Done (golden) | features: F2 and F3 the title bar names the app and its logo tile |
| 3 | Logo tile | 01-09 | Done | features: F2 and F3 the title bar names the app and its logo tile; lib: F3 the title-bar logo tile is a 22 by 22 accent tile on every page |
| 4 | Nav Library, count | 01-09 | Done | features: F4 the Library nav item shows its count and is the current page |
| 5 | Nav Inspect | 01-09 | Done | features: F5 the Inspect nav item opens Inspect |
| 6 | Nav Updates, pill | 01-09 | Done | features: F6 the Updates nav item opens Updates and shows its pill |
| 7 | Nav Tasks, count | 01-09 | Done | features: F7 the Tasks nav item opens Tasks and shows the running count |
| 8 | Nav Settings | 01-09 | Done | features: F8 the Settings nav item opens Settings |
| 9 | Nav About | 01-09 | Done | features: F9 the About nav item opens About with its identity and license; F9b the About rows use the Settings label and help roles and keep the original wording |
| 10 | Managed folder box | 01-09 | Done | lib: F10 the managed folder box names the folder and the apps it holds; lib: F10 the box shows the managed folder the core saved; features: F10 the managed folder box shows the folder and its note |
| 11 | Status counts | 01, 03-10 | Done | features: F11 the status bar counts installed apps, updates and failed checks; lib: F11 the status bar counts apps, updates and failed checks, and a new check moves the counts |
| 12 | Last checked | 01-09 | Done | features: F12 the status bar shows when updates were last checked; lib: F12 the status bar reads the time of the last check, right aligned in mono; lib: F12 before any check the status bar reads Not checked yet |
| 13 | 0 installed | 02 | Done | features: F13 the status bar reads 0 installed on an empty library; lib: F13 the status bar reads 0 installed on an empty library, then counts the install |
| 14 | Library heading | 01, 08 | Done | features: F14 the Library heading summarises the folder and adopted apps; lib: F14 the Library heading counts the apps and the adopted ones, and follows the library |
| 15 | Browse… | 01, 02, 08, 10 | Done | features: F15 Browse… opens the file chooser and inspects what is picked; F59d Browse… on Inspect picks a file to inspect |
| 16 | Ctrl O hint | 01, 02, 08 | Done | features: F16 Ctrl O is the Browse shortcut and shows its key hint |
| 17 | Filter segments | 01, 08 | Done | features: F17 the filter control narrows the table to updates or attention |
| 18 | Narrow filter tabs | 10 | Done | features: F18 the narrow filter tabs read All, Updates and Attention |
| 19 | Search field | 01, 08 | Done | features: F19 the search field filters by name, version or path |
| 20 | Narrow search field | 10 | Deviation 4 (keeps its icon) | features: F20 the narrow search field takes name, version or path |
| 21 | Sort dropdown | 01, 08 | Done (sorts A to Z, deviation 1) | features: F21 the sort dropdown reads Sort: Name and changes the order |
| 22 | Table column headers | 01, 05, 08 | Done | lib: F22 the table headers read APP, VERSION and STATUS over their columns; lib: F22 the table headers are shown only with a table to head |
| 23 | App row | 01, 03, 05, 08, 09, 10 | Done (row order, deviation 1) | features: F23 an app row shows its tile, name and path; lib: F23 an app row shows its tile, name and path, and a tap on the name opens its Detail; lib: F23 the row path follows the managed path the core reports |
| 24 | Running badge | 01, 03, 06, 08, 09 | Done | lib: F24 a running app shows a green Running badge, and the badge follows the running flag; features: F24 a running app shows the Running badge |
| 25 | Version cell with update arrow | 01, 08 | Done | features: F25 the version cell shows the update arrow to the new version; lib: F25 the version cell shows an arrow to the new version only for an app with an offer; lib: F25 an applied update removes its arrow and its update status |
| 26 | Updates page version change | 05 | Done | features: F26 the Updates page shows the version change as an arrow; lib: F26 the Updates rows show the version change as an arrow, and a check that drops an offer drops its row |
| 27 | Status: Update available | 01, 08 | Done (the 'Updating · 62%' state is the second half of this row) | features: F27 an available update reads Update available; lib: F27 an available update reads Update available in accent, with the running note; features: F27b a running update reads Updating with its percent; lib: F27b a running update reads Updating with its percent, and the percent follows the core task |
| 28 | Status: Up to date | 01, 05, 08, 10 | Done | features: F28 an up-to-date app reads Up to date with its check time; lib: F28 an up-to-date app reads Up to date in green, with the time of the last check |
| 29 | Status: Reduced verification | 01, 08, 10 | Done | features: F29 a source without a checksum reads Reduced verification; lib: F29 a source without a checksum reads Reduced verification in amber, and reads Up to date once it is not marked so |
| 30 | Status: Check failed | 01, 05, 08, 10 | Done | features: F30 a failed check reads Check failed, never up to date; lib: F30 a failed check reads Check failed in red, never Up to date |
| 31 | Status: Adopted | 01, 08 | Done | features: F31 an adopted app outside the folder reads Adopted; lib: F31 an adopted app outside the managed folder reads Adopted in grey, with its place |
| 32 | Launch (row) | 01, 08 | Done | features: F32 Launch on a row starts the app |
| 33 | Row menu | 01, 08, 10 | Done | features: F33 the row menu button opens the app actions |
| 34 | Narrow menu button | 10 | Done | features: F34 the narrow menu button opens the six pages |
| 35 | Narrow rows collapse | 10 | Deviation 3 (all seven rows) | features: F35 narrow rows collapse to name, version and one status line |
| 36 | Narrow status bar | 10 | Done | features: F36 the narrow status bar counts apps, updates and failed checks; lib: F36 the narrow status bar counts apps, updates and failed checks, with the failure in red |
| 37 | Empty-state card, dashed border | 02 | Done | features: F37 the empty library card asks for an AppImage; lib: F37 the empty library shows a dashed drop card, and the card goes once an app is installed |
| 38 | Empty-state text | 02 | Done | features: F38 the empty card says nothing is installed until Integrate; lib: F38 nothing is installed until Integrate: an inspect reads the file and installs nothing |
| 39 | No apps yet | 02 | Done | features: F39 the empty library subtitle reads No apps yet; lib: F39 the empty library subtitle reads No apps yet until an app is installed |
| 40 | Open Inspect | 02 | Done | features: F40 Open Inspect on the empty card opens Inspect |
| 41 | Breadcrumb | 03, 09 | Done | features: F41 the breadcrumb returns from Quill Notes to Library |
| 42 | Reveal in folder | 03, 09 | Done | features: F42 Reveal in folder reveals the app |
| 43 | Check for update | 03, 09 | Done | features: F43 Check for update asks the source and applies nothing; F43b Check for update on an app that is current clears its offer; F43c a check that times out reads Check failed, not up to date; Rust: checking_one_app_offers_the_release_and_changes_nothing_installed (tests/test_update_progress.rs); bridge: an integration records its date and source folder, and checking an app changes nothing installed |
| 44 | Launch (Detail) | 03, 09 | Done | features: F44 Launch on the Detail header starts the app |
| 45 | Status line: Running, update, integration | 03, 09 | Done | features: F45 the Detail status line shows running, the update and the integration; lib: F45 the Detail status line shows Running, the waiting update and the integration date; lib: F45 an app with no recorded install date reads Integrated alone; format: a Detail status line reads the integration date the core stored; Rust: a_registry_from_before_the_integration_date_keeps_its_rows, an_integration_date_is_stored_and_read_back (tests/test_registry.rs) |
| 46 | Record card | 03, 09 | Done | features: F46 the Record card lists the app facts; lib: F46 the Record card lists the facts of the app in the library; lib: F46b the Provenance line names the source folder, and reads Integrated alone for an older app; format: provenanceLabel names the folder an app was integrated from; format: shortenHome writes only the home folder and paths inside it as ~; Rust: integration_stores_the_source_folder_and_reads_it_back (tests/test_integration.rs); Rust: a_registry_from_before_the_integration_folder_keeps_its_rows (tests/test_registry.rs) |
| 47 | Record card Refresh | 03, 09 | Done | features: F47 Refresh on the Record card refreshes the metadata |
| 48 | Launch options: Arguments | 03, 09 | Done | features: F48 Launch options shows the arguments and saves them |
| 49 | Launch options: Environment and Add | 03, 09 | Done | features: F49 Launch options shows the environment and Add saves it |
| 50 | Update source selector | 03, 09 | Done | features: F50 the update source selector offers the managers and saves the choice |
| 51 | Update source key=value | 03, 09 | Done | features: F51 the source key=value field shows the pair and saves on Enter |
| 52 | Update source help | 03, 09 | Done (the sentence is constant text; its placement and the one-pair save are asserted) | features: F52 the source help says one key=value pair and reduced verification; lib: F52 the source help sits under the source field, which saves one key=value pair |
| 53 | Move to Trash | 03, 09, 11 | Done | features: F53 Move to Trash asks, then trashes the app; trash that fails deletes nothing (AGENTS: never delete when Trash fails) |
| 54 | Delete… asks again | 03, 09 | Done | features: F54 Delete… asks again before it deletes permanently |
| 55 | Remove help | 03, 09 | Done | upd: F55 Delete asks again, and no permanent removal reaches the core until that dialog is confirmed; features: F55 the remove card says it moves to the Trash and asks again |
| 56 | Inspect subtitle | 04 | Done (wording constant) | features: F56 the Inspect subtitle explains what it reads; upd: F56 the Inspect subtitle heads the Inspect page and leaves with it |
| 57 | Banner | 04 | Done | features: F57 the banner says nothing is executed or installed; upd: F57 inspecting a file only asks the core to read it, so the banner holds |
| 58 | Metadata card | 04 | Done | features: F58 the metadata card lists the file facts; upd: F58 the metadata card shows the categories and checksum the core inspected |
| 59 | Architecture · matches | 04 | Done | features: F59 a matching architecture reads · matches; upd: F59 the architecture reads · matches in green only when the core reports it supported |
| 60 | Integrate steps | 04 | Done (step 2 constant) | features: F60 the Integrate card lists the three steps; upd: F60 the Integrate card appears after an inspection and lists steps 1 to 3 in order |
| 61 | Step 1 | 04 | Done | features: F61 step one names the managed folder; upd: F61 step 1 names the managed folder the settings hold, and follows a folder change |
| 62 | Move the original | 04 | Done | features: F62 Move the original toggles the setting and says the source is never hard-deleted |
| 63 | Integrate | 04 | Done | features: F63 Integrate copies the inspected file into the library |
| 64 | Cancel (cross-page) | 04, 05, 06, 11-13 | Done | features: F64 Cancel on Inspect forgets the inspected file; F64 Cancel on a running task stops that task; Cancel on a running update cancels that task; F64b Cancel closes the name-conflict dialog without integrating; F64c Cancel closes the running-app dialog without updating; upd: F64 Cancel on an update in progress asks the core to cancel that update; F64 Cancel on a running task on the Tasks page asks the core to cancel it; F64 Cancel on an inspected file forgets it and asks the core for nothing |
| 65 | Updates subtitle | 05 | Done | features: F65 the Updates subtitle gives the check time; upd: F65 the Updates subtitle reads Not checked yet until a check runs, then the check time |
| 66 | Check now | 05 | Done | features: F66 Check now checks every source again |
| 67 | Update all | 05 | Done | features: F67 Update all applies every update |
| 68 | Summary cards | 05 | Done | features: F68 the summary cards count available, up to date and unknown |
| 69 | Update… | 05 | Done | features: F69 Update… on a running app asks before it updates; F69b a stale running flag still asks before the update |
| 70 | App is running | 05 | Done | features: F70 a running app reads App is running and asks to confirm; upd: F70 a running app with an update reads App is running, and the label follows the library |
| 71 | Progress bar 62% | 05, 06 | Done | features: F71 an update in progress shows its percent on a bar; upd: F71 the progress bar on a running update shows the core percent and follows it; crate: a_download_moves_the_task_bar_before_the_update_finishes; Rust: an_update_reports_its_versions_then_download_bytes_then_verify_then_swap_in (tests/test_update_progress.rs) |
| 72 | Reduced verification notice | 05 | Done | features: F72 reduced verification is noted in amber; upd: F72 the reduced verification notice follows the offer, in the warning colour |
| 73 | Retry | 05 | Done | features: F73 Retry checks the failed app again |
| 74 | Timed-out status | 05 | Done | features: F74 a timed-out check reads unknown, not up to date; upd: F74 a timed-out check reads the mockup sentence and a refusal reads the core text; upd: F74 an update that timed out and did not apply reads the mockup sentence in the list; format: a timed-out check reads the mockup sentence; any other failure reads the core text; crate: a_timeout_is_recognised_so_the_page_says_unknown |
| 75 | Tasks subtitle | 06 | Done | features: F75 the Tasks subtitle counts running and finished work; upd: F75 the Tasks subtitle counts running and finished tasks from the core |
| 76 | Clear finished | 06 | Done | features: F76 Clear finished removes completed entries only |
| 77 | Running task: title and versions | 06 | Done | features: F77 a running update shows its title, target and versions; upd: F77 the running card names the update and its versions from the core; Rust: a_task_records_when_it_started_and_ended_and_the_versions_it_moves (tests/test_update_progress.rs) |
| 78 | Running task phases | 06 | Done | features: F78 the update phase highlight follows the core phase index; upd: F78 the phase highlight follows the core phase index |
| 79 | Byte progress | 06 | Done | features: F79 the byte line reads the core byte counts; upd: F79 the byte line follows the core byte counts, and hides without a total; format: the byte line of a running update reads the core counts in MB; Rust: a_task_shows_the_bytes_of_its_download_stage |
| 80 | Finished list with times | 06 | Done | features: F80 finished work lists what happened with the time it ended; F80b the finish time is the one the core recorded; upd: F80 the finished list labels each task and times it from the core, newest first; upd: F80 the finish time is the one the core recorded, and a task without one shows its state word; upd: F80 a cancelled task reads cancelled, not failed |
| 81 | Settings subtitle | 07 | Done | features: F81 the Settings subtitle says options are off unless noted; upd: F81 with only background checks on, every other option reads off |
| 82 | Theme segmented control | 07 | Done | features: F82 the theme control offers System, Light and Dark and saves the choice |
| 83 | Managed folder Change… | 07 | Done | features: F83 Change… picks the managed folder and saves it |
| 84 | Maximum file size | 07 | Done | features: F84 the maximum file size takes whole megabytes and says MB |
| 85 | Move originals toggle | 07 | Done | features: F85 Move originals instead of copying toggles on and off |
| 86 | Discover AppImages toggle | 07 | Done | features: F86 Discover AppImages elsewhere toggles |
| 87 | Drop .AppImage toggle | 07 | Done | features: F87 Drop .AppImage from terminal app names toggles |
| 88 | Check in the background | 07 | Owner decision: default off; the toggle switches it on | features: F88 Check in the background starts off and switches on; F88b the background setting shows its stored value after a restart; F88c turning background checks off removes the login entry first; bridge: background checks start off in a fresh home (the owner default); turning background checks on stores it, and the login check can then be added; turning background checks off removes the login entry and stores off; Rust: a_fresh_home_has_background_checks_off, background_checks_stored_on_survive_a_restart_and_off_is_stored_too (tests/test_autostart.rs) |
| 89 | Also check at login | 07 | Done (gated on row 88) | features: F89 Also check at login is disabled until background checks are on; F89b the app does not ask the core for a login check while background checks are off; bridge: the login check is refused while background checks are off; Rust (tests/test_autostart.rs): a_login_check_cannot_be_added_while_background_checks_are_off, a_login_check_is_written_with_the_background_flag_when_background_checks_are_on, the_login_flag_stops_a_check_before_any_source_is_contacted_when_background_is_off, the_login_flag_still_scans_when_background_checks_are_on, startup_removes_an_earlier_login_entry_when_background_checks_are_off, startup_rewrites_an_earlier_login_entry_to_carry_the_background_flag, startup_leaves_a_login_file_that_is_not_ours_alone |
| 90 | Verbose diagnostics toggle | 07 | Done | features: F90 Verbose diagnostics toggles |
| 91 | Unsafe extraction fallback | 07 | Done (deviation 8: the core refuses it) | features: F91 the unsafe extraction fallback is disabled and marked Unavailable; F91b the unsafe row fades its text to 0.6 and its toggle to 0.5, once; bridge: the unsafe extraction fallback cannot be enabled through the core; Rust: the_unsafe_extraction_fallback_cannot_be_enabled (tests/test_settings.rs) |
| 92 | Settings card headings | 07 | Done | features: F92 the Settings cards are Appearance, Integration, Updates and Advanced; upd: F92 each Settings heading holds the controls it names, and a change there saves to the core |
| 93 | Dialog: Remove | 11 | Done | features: F203 the Remove dialog names the app and says it moves to the Trash; S204 the Remove dialog body and Cancel leave the app alone; D30 the dialog buttons are 30 px high, as section 10 specifies; upd: F93 Move to Trash opens the trash confirmation for that app, and Cancel closes it with no core call; upd: F93 confirming the trash dialog sends a trash removal for that app to the core |
| 94 | Dialog: Name conflict | 12 | Done | features: S205 the name conflict title names the installed app; F206 the name conflict body gives both versions and what each choice does; S207 Keep both on the conflict dialog is a secondary choice; S208 Replace on the conflict dialog is the primary choice; upd: F94 a name conflict names the installed app, and Cancel closes it with no core call; upd: F94 Keep both integrates the new copy beside the installed one; upd: F94 Replace integrates over the installed copy the core named |
| 95 | Dialog: Running app | 13 | Done | features: S209 the running-app dialog names the running app; F210 the running-app dialog body asks before replacing an open app; S211 Update anyway applies the update over the running app; F64c Cancel closes the running-app dialog without updating; F69b a stale running flag still asks before the update; D30 the dialog buttons are 30 px high, as section 10 specifies; upd: F95 updating a running app asks first, and Cancel closes it with no core call; upd: F95 Update anyway applies the running app's update with force; upd: F95 a stopped app with an offer updates without asking |

Section 10's dialog inventory (tpl 203, 205, 207 to 210) is covered by rows 93
to 95: 203 is the title and 205 the button row (F203, S204); 207 is Move to Trash
(F53 and F93); 208 to 210 are the notes that the presentation board shows and the
app does not draw (SPEC section 10: "shown in the presentation board only").


## Verification

Run from the repository root, with `export PATH=$HOME/.cargo/bin:$HOME/flutter/bin:$PATH`:

- `just flutter-check`: exit 0. `dart format`: "Formatted 39 files (0 changed)".
  `flutter analyze`: "No issues found!". `flutter test`: "+272: All tests passed!"
  (272 passed, 0 failed, 0 skipped). The goldens are in this run.
- `just test`: exit 0. `cargo test`: 199 passed, 0 failed.
- `just lint` (`cargo clippy --all-targets -- -D warnings`): exit 0.
- `just fmt-check`: exit 0.
- `just self-test`: exit 0, prints `SELF_TEST_OK`.
- `cargo build --release`: finished.
- `(cd flutter && flutter build linux --release)`: exit 0. The bundle
  `flutter/build/linux/arm64/release/bundle/gosh-appimage-manager-gui` is an
  aarch64 ELF.
- `SMOKE_OUT=<scratch> bash tools/gui-smoke.sh`: `PASS: the GUI started, stayed
  up for 8s and stopped cleanly`.
- `cd bridge && cargo test`: 3 passed, 0 failed.
- Mutation checks, each restored and compared byte for byte afterwards: removing
  the login-check gate fails `a_login_check_cannot_be_added_while_background_checks_are_off`;
  removing the unsafe-fallback refusal fails `the_unsafe_extraction_fallback_cannot_be_enabled`.
- Two bugs found by the new tests and fixed: a successful update kept its offer
  (`lib/state/app_model.dart`, `_startUpdate` and `updateAll`), and a cancelled
  task read "failed" (`lib/ui/tasks_page.dart`). Their tests were skipped and are
  now unskipped and pass (`lib:` F25 and `upd:` F80 a cancelled task).

Goldens: `flutter test --update-goldens` regenerated frames 01 to 13 and
`about.png`. The source-folder change (deviation 7) regenerated frames 03 and 09
and `about.png` in a second pass, selected by test name so no other golden moved.
The superseded `07-settings.png` was deleted. The dialog frames were
regenerated after the title line height changed (deviation 13).

Not verified:

- The x86_64 bundle and the Flatpak build (no x86_64 host here; the manifest was
  not built).
- Screen readers, and interactive input in the release bundle. The smoke run
  opens the window, checks its name and screenshot, and stops it.
- An exact pixel match. The frames differ by 0.4% to 2.4% of pixels, which is
  anti-aliasing plus the two known text differences in frame 03.
- Failure on the old code for every new test. Two gates were checked by mutation
  (above); the two bug fixes were checked by lifting their skips against the old
  code, which failed.

Decisions for the owner:

1. Background checks default off (`src/settings.rs`, `background_update_checks:
   false`). This pass did not change it, as instructed.
2. The unsafe fallback: removed from the core's reach (deviation 8). Confirm, or
   name the reviewed implementation that should replace it.
3. Row order: A to Z (deviation 1), or the mockup's sample order.
4. The source folder in Provenance (deviation 7): stored in this pass. Confirm the
   replace rule: a replace keeps the date and folder of the integration it
   replaces.
5. The dialog title line height of 1.22 (deviation 13): accept the measured value.
