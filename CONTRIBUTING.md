# Contributing

Issues and pull requests are welcome. Keep changes small and explain the
"why" in the commit message.

## Setup

- Rust 1.89 or newer (see `rust-version` in `Cargo.toml`).
- For the GUI: Wayland and XKB development headers
  (`libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev` on Debian/Ubuntu,
  `wayland-devel libxkbcommon-devel` on Fedora). The first
  `--features gui` build needs network for the pinned libcosmic checkout.
- Optional: `just`, `desktop-file-validate`, `appstreamcli`,
  `flatpak-builder` + the freedesktop 23.08 SDK, and `xvfb`/`xdotool`/
  ImageMagick/`python3` for the GUI smoke test.

## Everyday commands

```sh
cargo build                      # CLI only
cargo build --features gui       # GUI build
cargo test                       # full test suite
cargo clippy --all-targets -- -D warnings
cargo clippy --features gui --all-targets -- -D warnings
cargo fmt --check
```

`./scripts/verify.sh` chains all of the above plus an isolated-`HOME`
`--self-test`, desktop/AppStream validation, the GUI smoke test, and the
x86_64 Flatpak build. Whatever a prerequisite is missing for is skipped
loudly, never faked — read the tail of its output. `SKIP_GUI=1` and
`SKIP_FLATPAK=1` skip the heavy stages.

## Expectations

- Tests use fake process/network/process-table/trash seams and synthetic
  ELF fixtures. They must never touch a real home directory, execute an
  AppImage, or call a live update API.
- No shell command strings — spawn programs with argument arrays.
- Keep mutations transactional: stage, verify, atomically replace, keep
  rollback material until success.
- Keep the safety contract in `AGENTS.md`; it is the project's review bar.
- If `Cargo.lock` changes, regenerate `packaging/cargo-sources.json`
  (`just vendor /path/to/flatpak-builder-tools`) so the Flatpak build stays
  offline-capable.

## Translations

See `i18n/README.md` — catalogs are plain JSON keyed by message id, loaded
at runtime, no rebuild needed.

## Docs worth knowing

- `docs/documentation/APP-INVENTORY.md` — what the app actually does,
  feature by feature, with source references.
- `docs/documentation/PLAN.md` — how the documentation set is organized.
- `docs/rewrite-3.0.0.md` — module map and packaging decisions.
- `docs/implementation-brief.md` — the binding behavioural spec
  (superseded on stack, current on behaviour).
- `AUDIT.md`, `docs/audit/`, `docs/migration/` — dated records of past
  audits and hardening rounds. Treat their "current state" lines as
  snapshots, not status.
