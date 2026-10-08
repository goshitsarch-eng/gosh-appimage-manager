# Flutter front end on the Rust core — architecture proposal

Status: **proposal, not adopted.** The shipping application is still the
Rust + libcosmic 3.0.0 binary described in `docs/audit/ARCHITECTURE.md` and
`AGENTS.md`. This document records the direction chosen on 2026-10-07 (Flutter
UI on the Rust core, Linux first) and the constraints that shape it. Nothing
here changes the released application.

The bridge spike (`bridge/` and `flutter/`) builds, generates bindings and passes
its tests on aarch64 (2026-10-08, section 9). The x86-64 build runs only in CI,
through `.github/workflows/flutter.yml`, which is not yet pushed.

## 1. Scope and constraints

- **Direction.** Flutter (Dart) owns the user interface. The existing Rust crate
  `goshaim_core` owns the domain behaviour. The libcosmic GUI stays in the tree
  until the Flutter front end reaches parity; the two never ship as competing
  implementations of the same behaviour. The cut-over, and the matching change
  to `AGENTS.md`, are separate decisions taken at that point.
- **Platform scope.** Linux first, through the freedesktop 23.08 Flatpak and a
  standalone tarball. Windows and macOS are out of scope until the owner defines
  what those platforms should offer (section 12). AppImage execution and the
  desktop-entry integration are Linux concepts (`MIGRATION_AUDIT.md`, section 18.4).
- **Build hosts (verified).** The owner asked for both architectures.
  - **aarch64 (this host, and the native `ubuntu-24.04-arm` CI runner).** The
    Flutter SDK runs. The tool bootstraps from the git repository at the pinned
    tag `3.47.6`, which fetches the arm64 Dart SDK. `flutter precache --linux`
    fetches the `linux-arm64` engine artifacts (debug, profile and release GTK
    embedders), and `flutter doctor` reports the Linux toolchain as available.
    `flutter build linux` produces an AArch64 bundle. The official release
    archives under `releases_linux.json` are x86-64 only, so they are not used.
  - **x86-64 (the `ubuntu-24.04` CI runner).** Not run from this host. x86-64
    binaries cannot execute here without root: the binfmt handler hands them to
    `/usr/bin/binfmt-dispatcher`, which tries to install FEX and blocks. The x86-64
    build is therefore verified only when the CI workflow runs.
  - **Codegen.** `flutter_rust_bridge_codegen` 2.13.0 installs and runs on
    aarch64, and generates the bindings there. The bindings are architecture
    independent.
- **Repository rules still in force.** `cargo` and `just` remain the entry points
  for the Rust side. The Flutter side adds its own recipes. AGENTS.md's "build and
  test with cargo + just" is kept; its description of the project as libcosmic-only
  is what changes at cut-over.

## 2. Layers

```text
┌────────────────────────────────────────────┐
│ Flutter widgets (Dart)                     │
│ pages, dialogs, forms, shortcuts, theme,   │
│ focus, accessibility labels, drag and drop │
└───────────────────┬────────────────────────┘
                    │ Dart services (ChangeNotifier + plain classes)
┌───────────────────▼────────────────────────┐
│ Dart application layer                     │
│ presentation state, command dispatch,      │
│ mapping of core DTOs to view models,       │
│ platform plugins (file picker, drop, window)│
└───────────────────┬────────────────────────┘
                    │ generated bridge (flutter_rust_bridge), coarse calls
┌───────────────────▼────────────────────────┐
│ bridge crate (Rust, cdylib + staticlib)    │
│ #[frb] API functions, DTOs, error mapping, │
│ panic containment, progress streams        │
└───────────────────┬────────────────────────┘
                    │ direct Rust calls
┌───────────────────▼────────────────────────┐
│ goshaim_core (Rust, unchanged where possible)│
│ inspection, integration transaction,       │
│ registry, settings, removal, launch,       │
│ updates, networking, URL policy            │
└───────────────────┬────────────────────────┘
                    │
┌───────────────────▼────────────────────────┐
│ Linux: filesystem, SQLite, /proc, Trash,   │
│ flatpak-spawn, D-Bus notifications         │
└────────────────────────────────────────────┘
```

Rules that follow from the layering:

- Flutter never opens `registry.sqlite` or `settings.json` directly. Every fact
  it shows comes from a core response.
- The core never refers to Flutter types, widget trees, or UI wording that is
  only meaningful in one front end.
- User-facing strings for core errors are typed (section 6). Widgets choose the
  wording.

## 3. Responsibility split

| Responsibility | Owner | Reason |
|---|---|---|
| Pages, navigation, dialogs, forms, focus order | Flutter | Presentation. Built-in Flutter widgets cover it |
| Keyboard shortcuts and command dispatch | Flutter (`Shortcuts` and `Actions`) | Platform conventions live in the UI layer. Each command calls one core operation |
| Theme (System, Light, Dark), text scaling | Flutter (`ThemeMode`) | Flutter reads the platform setting. Only the user's choice is stored |
| Window size and position persistence | Flutter, through a plugin (to evaluate) | Window management is a UI concern |
| File pickers (Browse) | Flutter, through a plugin (to evaluate for the Flatpak portal) | Dialogs should be native where possible |
| Drag and drop of files | Flutter, through a plugin (to evaluate) | Drop events are UI events; paths are converted in the core |
| Inspection, validation, ELF and AppImage parsing | Rust core | CPU work, parsing of untrusted input, already tested (`elf.rs`, `inspector.rs`) |
| Integration transaction, rollback, ownership checks | Rust core | Safety-critical; must not depend on a front end |
| Registry (SQLite), legacy import, settings store | Rust core | Single source of truth; schema compatibility is a core concern |
| Removal (Trash first), protected paths, symlink refusal | Rust core | Data-safety rules |
| Confirmation policy for destructive actions | Rust core (request types carry an explicit confirmation) | Today this lives in the frontends (defect D-11). Moving it into the core makes it impossible to skip |
| Launch, running-process detection, host spawn | Rust core | Process and `/proc` handling is Linux-specific and needs the allowlist |
| Update check, download, verification, atomic replacement | Rust core | Networking and integrity checks |
| Desktop notifications | Rust core (`notify-rust`, shared with the CLI) | Keeps one implementation for the CLI and the app |
| Autostart entry | Rust core | Shared with `--probe-autostart` |
| Logging | Rust core (`tracing` to be decided), Dart forwards its own logs | One log format across the boundary (section 7) |

## 4. The bridge

### 4.1 Candidates

| Criterion | `flutter_rust_bridge` (FRB) | Direct `dart:ffi` with a C ABI |
|---|---|---|
| Installed and run here | Yes: codegen 2.13.0 installs and runs on aarch64; the spike is generated, built and tested there | Not applicable |
| Linux desktop build | Verified on aarch64. The CMake integration (cargokit) builds the Rust crate and bundles `libgoshaim_bridge.so` next to the executable | Same CMake requirements |
| Typed DTOs and enums | Generated from Rust types | Handwritten; every struct is mirrored by hand |
| Error propagation | `Result<T, CoreError>` arrives as a Dart `CoreError` that implements `FrbException`, with the typed `kind` (verified) | Handwritten status codes and out-parameters |
| Async | Rust functions return Dart futures; `StreamSink` carries progress (verified) | Handwritten threads and callbacks |
| Memory ownership | Managed by the generated code | Each allocation and free must be paired by hand. The brief warns against this |
| Generated code | Must not be edited by hand; regenerate and diff in CI | Not applicable |
| Maintenance risk | Third-party project; version must be pinned in both Cargo and pub | Low dependency risk, high maintenance cost |
| Build complexity | Codegen step plus pinned versions | `cbindgen` step plus hand-written wrappers |

### 4.2 Decision (provisional)

Use `flutter_rust_bridge` 2.13.0 (the stable line; 2.14.0-beta.2 is a
pre-release and is not used). The aarch64 spike has passed steps 1–5 and 7 of
section 9. The decision becomes firm after the x86-64 build passes in CI and the
progress-cancellation test (step 6) is added. If the spike fails for a reason the
generator cannot work around, the fallback is direct FFI, limited to the handful
of entry points, with ownership rules written down before any code.

### 4.3 Layout

- `bridge/` — a Rust crate, `cdylib` and `staticlib`, depending on
  `goshaim_core` by path (`package = "gosh-appimage-manager", path = ".."`). It
  holds only the `#[frb]` API, the DTOs and the error mapping. It contains no
  business logic. It has its own `Cargo.lock`, because it is not a workspace
  member of the root package.
- `flutter/` — the Flutter application (`pubspec.yaml`, `lib/`, `linux/`,
  `rust_builder/` with cargokit). The generated Dart code lives in
  `flutter/lib/src/rust/`. The configuration is in `flutter/flutter_rust_bridge.yaml`:
  `rust_input: crate::api`, `rust_root: ../bridge/`, `dart_output: lib/src/rust`.
- The repository root remains the Rust package. Nothing in `src/` moves in the
  first phase.

### 4.4 Rules for the bridge

- Coarse operations only: one call per user action. The examples in section 5
  are the target surface. No getter-per-field calls.
- Requests and responses are immutable DTOs. No handles to internal objects are
  exposed. The one exception is a single `CoreHandle` created at start-up. It
  owns the registry path and the settings path, not live locks.
- Long operations return an operation identifier. Progress is a Dart `Stream`
  of events. Cancellation takes the identifier.
- A panic in Rust never crosses the boundary as a crash. Each exported function
  catches unwinds and returns `CoreError` with kind `internal`. The spike's
  `panic_for_contract_test` proves this on aarch64 (section 9, step 5).
- No `unsafe` in the bridge crate beyond what the generator emits. Any `unsafe`
  elsewhere stays in the core where it is today.

## 5. The coarse API (target surface)

Each function maps to existing `AppController` methods (`controller.rs`), so
the core work is mostly reuse. The spike implements the first row, the stream
and the error and panic rules; the rest is later work.

| Bridge function | Core method today | Returns |
|---|---|---|
| `bridge_version()` (sync) | — | version string (spike) |
| `probe_host()`, `readiness()` | `controller.rs` readiness and `--probe-host` | host report |
| `inspect_path(path)` (spike) | `AppImageInspector::inspect` | `InspectSummary` (path, size, SHA-256, type, architecture, warnings) |
| `integrate(request)` | `integrate` | `IntegrateOutcome` (ok, conflict, or error) |
| `resolve_conflict(request, choice)` | `integrate` with keep-both or replace | `IntegrateOutcome` |
| `library()` | `registry` read and `discover` | `Vec<AppRecord>` (one call for the page) |
| `app_details(uuid)` | registry row plus provenance | `AppRecord` with provenance |
| `launch(uuid)` | `launch_service().launch` | `LaunchOutcome` |
| `reveal(uuid)` | `reveal_in_file_manager` | `Result<()>` |
| `remove(request)` | `remove_app` | `RemoveOutcome`; the request carries an explicit `ConfirmedPermanent` or `ConfirmedTrash` token |
| `set_arguments_and_environment(uuid, args, env)` | same | `Result<()>` |
| `set_update_source(uuid, config)` / `unset_update_source(uuid)` | same | `Result<()>` |
| `check_updates(scope, operation_id)` | `check_updates` / `scan_updates` | `Stream<CheckEvent>` ending in `UpdateReport` |
| `apply_update(uuid, force, operation_id)` | `apply_update` | `Stream<ApplyEvent>` ending in `ApplyOutcome` |
| `cancel(operation_id)` | cancel flag | `Result<()>` |
| `settings()` / `update_settings(patch)` | `settings()` and setters | `Settings` (one batch write) |
| `set_autostart(enabled)` | `sync_autostart` | `Result<()>` |

Every destructive function takes an explicit confirmation token (section 3). A
frontend cannot call the operation without naming the confirmation it has
obtained.

The spike's `count_ticks(total, sink)` is the progress pattern for these
stream-returning operations. Its values arrive in order on the Dart side, and
the stream completes.

## 6. Errors

Rust returns a typed `CoreError` with:

- `kind`: `user_input`, `validation`, `not_found`, `conflict`, `permission`,
  `corrupt_data`, `network`, `process`, `internal`. The spike implements
  `user_input`, `validation`, `not_found` and `internal`.
- `message`: a short, user-safe English text. Localisation happens in Dart from
  the kind, so the core stays free of UI wording.
- `details`: technical text for logs only. It may contain paths, so it is never
  shown directly to the user.

Dart maps `kind` to the generated `ErrorKind` enum and decides the presentation.
`internal` shows a generic message and writes the details to the log. The
current string-matching on error text (defect D-20) is replaced by `kind`.

## 7. Logging

- The core emits through one facade. `tracing` is the candidate; the current
  stderr switch (`diagnostics.rs`) keeps working for the CLI.
- Dart uses `package:logging` and forwards nothing that contains user paths,
  except where the path is the subject of the event.
- Both sides write to stderr. The Flatpak writes to the sandbox's state directory.
  Logs never contain file contents or credentials.

## 8. State and concurrency

- **Dart.** `ChangeNotifier` and plain service classes, one per feature area.
  No state-management package is added yet. Revisit only if the number of
  screens grows beyond what this structure handles.
- **Rust.** The controller's single mutex is the cause of defect D-03 (the
  whole inspection hashes a file while holding the lock). The bridge must not
  inherit that. Requirements for the core refactor:
  - Hashing, download and archive reads run without the registry lock.
  - The registry lock is held only for reads and writes of rows.
  - Each long operation has its own cancel flag, found through `operation_id`.
- **Threads.** Long operations run on a Rust worker thread pool. The Dart side
  awaits them without blocking the UI isolate, as the FRB examples do. The spike's
  inspection runs on a bridge worker and the Flutter page stays responsive.

## 9. Validation plan and status

The spike proves the design before any page is built. Results are for aarch64
(this host) unless marked.

| Step | Check | Status |
|---|---|---|
| 1 | `bridge` builds with `cargo build` | Done (aarch64). `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass |
| 2 | `flutter_rust_bridge_codegen generate` produces the Dart files with no hand edits; a second run changes nothing | Done (aarch64). No drift |
| 3 | `flutter build linux` builds the spike, with the bridge library bundled next to the executable | Done (aarch64, debug and release; `file` reports AArch64 for the executable and `libgoshaim_bridge.so`) |
| 4 | `flutter test` calls `inspect_path` on a synthetic AppImage and receives an `InspectSummary` | Done (aarch64). The test writes a 128-byte AArch64 type-2 fixture |
| 5 | Negative tests: a missing file maps to `not_found`, an empty path to `user_input`, a text file to `validation` with the core message; a forced panic maps to `internal` and the bridge keeps working | Done (aarch64) |
| 6 | Progress: `count_ticks` streams values in order and completes; a cancel test stops a long operation | Stream done (aarch64). Cancel waits for `check_updates` in a later phase |
| 7 | The spike application runs under XWayland and shows the `inspect` result | Done (aarch64). A screenshot shows the core version, the inspection summary, the stream and the panic mapping |
| 8 | CI: regenerate the bindings and fail on any diff; build and test on both architectures | Drift check done locally. `.github/workflows/flutter.yml` defines the check and both architectures; it has not run yet (needs a push) |
| x86-64 | The same build and tests on the `ubuntu-24.04` runner | Pending: runs in CI only |

Passing steps 1–8 on both architectures is required before the first real page
is built.

## 10. Packaging (Linux)

- Flatpak: freedesktop 23.08 runtime, GTK 3 (provided by the runtime). The
  Flutter SDK, the pub packages and the cargo crates must be available to the
  build without network access. The Flutter SDK is available on both
  architectures, but the offline arrangement is not yet designed (section 11).
- Offline Dart packages: the Flatpak build cannot fetch pub packages. A vendored
  pub cache, generated the same way as `packaging/cargo-sources.json`, is
  required. This is a risk until it is proven (section 11).
- The Rust core keeps the existing `packaging/cargo-sources.json` workflow. A
  `rustls` bump updates it, as done in this audit. The bridge crate needs its own
  sources file when it moves into the Flatpak.
- The tarball gains a `lib/` directory with the bridge library. Its layout must
  match the Flatpak's.

## 11. Risks and blockers

| Item | Status | Effect |
|---|---|---|
| Official release archives are x86-64 only; arm64 uses the git bootstrap and the `linux-arm64` engine artifacts | Verified (aarch64). The bootstrap works; the tag pins the version | Pin the tag (`3.47.6`) in CI and locally; revisit when the stable channel moves |
| x86-64 builds cannot run on this host without root | Verified | The x86-64 build is verified only in CI |
| Pub packages and the Flutter SDK must be available offline to the Flatpak | Not yet designed | Could block the Flatpak build |
| FRB version drift between Cargo and pub | Mitigated by pinning (`=2.13.0` and `2.13.0`) | Must be re-checked at every upgrade |
| Windows and macOS scope undefined | Open decision | Affects whether the bridge must avoid Linux-only types now |
| Drag and drop and file picker plugins on Linux and the Flatpak portal | Not evaluated | May need a custom portal call |
| `cargo-expand` not installed for codegen | Codegen falls back to parsing the source and succeeds (verified) | Install `cargo-expand` only if a feature needs it |
| Defects D-01 to D-50 in the audit | Open | Several must be fixed in the core before it is exposed through a bridge (D-01, D-02, D-03, D-11) |
| Spike API includes a deliberate panic (`panic_for_contract_test`) | Intentional, spike-only | Remove before real pages use the bridge |

## 12. Decisions

1. **Build hosts (decided by the owner, 2026-10-08).** x86-64 is built and
   tested on the `ubuntu-24.04` CI runner. aarch64 is built and tested natively
   on this host and on the `ubuntu-24.04-arm` runner. The workflow is written but
   not pushed. Pushing a branch runs CI and needs the owner's explicit approval.
2. **Windows and macOS scope:** inspect-only, or not offered. Still open.
3. **AGENTS.md at cut-over:** the amended wording is written when the cut-over is
   decided, not before.
4. **Logging:** adopt `tracing` in the core, or keep the stderr switch. Still open.
5. **Core refactor (D-03):** approve the lock redesign in section 8 before the
   bridge is built on top of the controller. Still open.

## 13. Phases (for tracking)

| Phase | Content | Feature IDs (`MIGRATION_AUDIT.md`) | Entry condition | Status |
|---|---|---|---|---|
| 0 | This proposal, reviewed | — | — | Draft |
| 1 | Spike (section 9) | — | Build-host decision (done) | Done on aarch64; x86-64 in CI pending |
| 2 | Core fixes needed for the bridge: D-03, D-01, D-02, D-11 | F-08, F-24, F-63 | Owner approval | Not started |
| 3 | Shell and read-only pages: Library, Inspect, Settings | F-01 to F-06, F-14, F-16 | Phase 1 passed on both architectures | Not started |
| 4 | Integrate, remove, launch, details | F-07 to F-10, F-41 to F-50 | Phase 3 passed | Not started |
| 5 | Updates and update sources | F-12, F-34, F-58 to F-67 | Phase 4 passed | Not started |
| 6 | Autostart, notifications, diagnostics | F-20 to F-22, F-37, F-52 | Phase 5 passed | Not started |
| 7 | Packaging, Flatpak, release, cut-over, AGENTS.md update, removal of the libcosmic GUI | F-69 to F-74 | Parity inventory complete | Not started |

Each phase ends with the parity status of every inventory row it touches
(`MIGRATION_AUDIT.md` section 12) and with a screenshot on each architecture.
