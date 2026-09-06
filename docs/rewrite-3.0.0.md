# Gosh AppImage Manager 3.0.0 — rewrite notes

Full rewrite of the C++20/Qt 6/KF6/Kirigami implementation (2.x) to
Rust + libcosmic (COSMIC Epoch, iced-based). Behavioural parity with the
Gear Lever workflow reference is preserved; the safety contract is unchanged.

## Module map (C++ → Rust)

| Before (`src/`) | After (`src/`) |
|---|---|
| `core/ElfParser` | `elf.rs` |
| `core/UrlGuard` | `url_guard.rs` |
| `core/SafeFs` | `safe_fs.rs` |
| `core/DesktopParser` + `core/DesktopIntegration` | `desktop.rs` |
| `core/SettingsStore` | `settings.rs` (JSON instead of KConfig) |
| `core/ManagedRegistry` | `registry.rs` (SQLite instead of JSON; one-time v2 import) |
| `core/ProcessRunner` | `process.rs` |
| `core/ProcessTable` | `proctable.rs` |
| `core/NetworkClient` | `network.rs` (reqwest + rustls, DNS pinning) |
| `core/AppImageInspector` | `inspector.rs` |
| `core/IntegrationService` | `integration.rs` |
| `core/RemovalLaunch` (removal) | `removal.rs` + `trash.rs` |
| `core/RemovalLaunch` (launch) | `launch.rs` |
| `core/UpdateSources` | `updates_sources.rs` |
| `core/UpdateService` | `updates_service.rs` |
| `core/CheckStateStore` + `core/UpdateNotifier` | `notifier.rs` |
| `core/TaskQueue` | `tasks.rs` |
| `core/AppImageLibrary` | `library.rs` |
| `models/Models` + `AppController` + `ThemeController` | `controller.rs` |
| `Cli` | `cli.rs` |
| `main` + `qml/*` | `main.rs` + `gui.rs` (libcosmic, `--features gui`) |
| `tests/*.cpp` | `tests/*.rs` (shared `tests/common.rs` seams) |

## Deliberate changes

- Registry: SQLite (`registry.sqlite`, mode 0600) with a one-time,
  fail-soft import of an adjacent v2 `registry.json`. Legacy file untouched.
- Settings: JSON file instead of KConfig. Same keys and clamps.
- GUI: libcosmic `Application` + `nav_bar` pages; System/Light/Dark via
  `cosmic::app::command::set_theme`; cosmic dialogs for destructive choices;
  portal file picker for Inspect.
- Dependency pins required by libcosmic v0.12: cosmic-text is redirected to
  crates.io `=0.13.2` via `[patch]` (the git branch moved API); see
  `Cargo.toml`. Regenerate `packaging/cargo-sources.json` after any lock
  change (`just vendor …`).
- Flatpak runtime moved to `org.freedesktop.Platform//23.08` with the
  rust-stable SDK extension; sandbox grants unchanged (no `host:rw`).
- The 23.08 rust-stable extension ships Rust 1.81, but the locked graph
  needs up to Rust 1.89, so the manifest installs a pinned Rust 1.90.0
  toolchain per arch (`rust-gosh` module, SHA-256 pinned, removed from the
  final app by top-level `/rust` cleanup). Top-level `no-debuginfo: true`
  is required: the 23.08 dwz/debugedit corrupt rustc 1.90's libLLVM
  (zeroed program headers) when splitting debuginfo, and the split pass
  of later modules also covers earlier modules' files.

## Verification status

- `cargo test`: full suite green (fake seams + synthetic fixtures only).
- `cargo clippy --all-targets -- -D warnings`: clean. `cargo fmt --check`: clean.
- `cargo check --features gui`: clean against libcosmic v0.12 tag
  (needs system Wayland/XKB dev files; present in the Flatpak SDK).
- `desktop-file-validate` + `appstreamcli validate --pedantic --no-net`: pass.
- `flatpak-builder --arch=x86_64` (org.flatpak.Builder 1.4.9): full build
  passes, release binary reports 3.0.0, in-builder `--self-test` prints
  SELF_TEST_OK, and `flatpak-builder --run` re-verifies SELF_TEST_OK.
  `--arch=aarch64` (qemu-user): in progress; same manifest, per-arch
  pinned toolchains for 7zz/dwarfs/Rust.
