# Contributing

Issues and pull requests are welcome. Keep changes small and explain the
"why" in the commit message.

## Setup

- Rust 1.89 or newer (see `rust-version` in `Cargo.toml`).
- For the GUI: Flutter 3.47.6 on Linux, plus clang, CMake, Ninja, pkg-config,
  and the GTK 3 development headers. The package list for Debian/Ubuntu is in
  `.github/workflows/flutter.yml`. Building the GUI also builds the Rust
  bridge in `bridge/`.
- Optional: `just`, `desktop-file-validate`, `appstreamcli`, and
  `flatpak-builder` with the freedesktop 26.08 SDK.

## Everyday commands

```sh
cargo build                      # the Rust core, CLI, and launcher
cargo test                       # full test suite
cargo clippy --all-targets -- -D warnings
cargo fmt --check
just flutter-check               # Flutter format, analysis, and tests
```

To run the GUI from a checkout, build it and point the launcher at the
executable with `GOSH_APPIMAGE_GUI` (an absolute path):

```sh
(cd flutter && flutter build linux --release)
GOSH_APPIMAGE_GUI=/abs/path/to/flutter/build/linux/<arch>/release/bundle/gosh-appimage-manager-gui cargo run
```

`./scripts/verify.sh` chains the Rust gates above plus an isolated-`HOME`
`--self-test`, desktop/AppStream validation, and the x86_64 Flatpak build.
Whatever a prerequisite is missing for is skipped loudly, never faked — read
the tail of its output. `SKIP_FLATPAK=1` skips the Flatpak stage.

## Expectations

- Tests use fake process/network/process-table/trash seams and synthetic
  ELF fixtures. They must never touch a real home directory, execute an
  AppImage, or call a live update API.
- No shell command strings — spawn programs with argument arrays.
- Keep mutations transactional: stage, verify, atomically replace, keep
  rollback material until success.
- Keep the safety contract in `AGENTS.md`; it is the project's review bar.
- Record a change a user would notice under `## [Unreleased]` in `CHANGELOG.md`,
  in plain words: what was wrong or new, not which function changed.
- If `Cargo.lock` changes, regenerate `packaging/cargo-sources.json`
  (`just vendor /path/to/flatpak-builder-tools`) so the Flatpak build stays
  offline-capable.

## Translations

The Rust core loads JSON message catalogs (see `i18n/README.md`), but no
interface uses them yet.

## Docs worth knowing

- `README.md` — what the app does, the CLI reference, and build steps.
- `CHANGELOG.md` — what changed in each release.
- `docs/RELEASING.md` — how releases are cut.
- `docs/verification.md` — the evidence record for the current build.
- `docs/flutter/` — the Flutter front end: architecture and status.
- `AGENTS.md` — the safety contract and engineering rules.
