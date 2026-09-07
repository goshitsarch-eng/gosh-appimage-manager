# Gosh AppImage Manager 3.0.0

Gosh AppImage Manager is a native application for inspecting, integrating,
launching, organizing, updating, and removing AppImages. Version 3.0.0 is
written in Rust with libcosmic (COSMIC Epoch, iced-based). Made by Gosh.

Application ID: `com.goshapps.AppImageManager`
Executable: `gosh-appimage-manager`
Licence: GPL-3.0-or-later
Public identity: Gosh-Its-Arch

Opening an AppImage never integrates or executes it. Integration is
transactional. Metadata extraction does not execute the AppImage unless the
user enables the unsafe fallback in Settings and confirms that file.

Gear Lever by Lorenzo Paderi is a GPLv3 behavioural reference only. This is
an independent original implementation and is not endorsed by Gear Lever's
authors. Gear Lever source, templates, CSS, icons, screenshots, application
ID, and branding were not copied.

Zero telemetry: the app makes network requests only to update metadata
endpoints you configured, and only when you check for or apply updates.

## Build

Requires a Rust toolchain (see `rust-version` in `Cargo.toml`).

```sh
cargo build
```

With the libcosmic GUI (needs network once for the pinned libcosmic
checkout; needs system Wayland/XKB dev files — present in the Flatpak SDK):

```sh
cargo build --features gui
cargo run --features gui
```

Without `--features gui`, `cargo run` serves the CLI (same binary, no GUI,
no Qt/KDE runtime anywhere in the tree).

## Test

```sh
cargo test
```

Tests use fake process/network/table/trash seams and synthetic ELF/AppImage
fixtures. They do not touch a real home directory, execute an AppImage,
launch a user app, or call live update APIs. Lint and format gates:

```sh
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`just` wraps the common flows: `just build`, `just build-gui`, `just test`,
`just lint`, `just validate`, `just vendor <fbtools>`, `just flatpak-x86_64`,
`just flatpak-aarch64`.

## CLI

The same executable provides GUI and CLI modes. CLI commands do not start the
GUI. `--version` prints `3.0.0`.

```
gosh-appimage-manager --integrate <path> [--keep-both|--replace] [--replace-uuid UUID|--target PATH] [--yes]
gosh-appimage-manager --update <path>|--all [--yes] [--force]
gosh-appimage-manager --remove <path> [--yes] [--delete]
gosh-appimage-manager --remove-all [--yes] [--delete]
gosh-appimage-manager --list-installed [--json]
gosh-appimage-manager --list-updates [--json]
gosh-appimage-manager --list-discovered [--json]
gosh-appimage-manager --adopt <path> [--yes]
gosh-appimage-manager --list-update-managers
gosh-appimage-manager --set-update-source <path> --manager <name> key=value...
gosh-appimage-manager --set-update-source <path> --unset
gosh-appimage-manager --fetch-updates
gosh-appimage-manager --self-test
gosh-appimage-manager --probe-host
gosh-appimage-manager --probe-inspect <path>
gosh-appimage-manager --probe-autostart
```

JSON list output uses `schema_version: 1` with an `installed`, `updates`, or
`discovered` array. `--fetch-updates` is non-mutating (check metadata only)
and prints a notice on stderr. Diagnostics go to stderr so stdout remains
valid JSON. `--list-updates` and `--fetch-updates` exit `8` when an app's
update check failed, so a script can tell "nothing to update" apart from
"nothing could be checked"; the JSON document on stdout is still complete.

`--list-discovered` reports AppImages in the managed folder and, when
"discover AppImages outside the managed folder" is on, ones referenced by
desktop entries elsewhere. `--adopt` registers such a file so it can be
updated and removed here; nothing on disk is changed and its existing desktop
entry is left alone.

All three probes are non-mutating: `--probe-autostart` renders and verifies
the autostart entry it *would* install without writing it or changing any
setting.

## GUI

`cargo run --features gui` (or the Flatpak) opens the libcosmic shell:
Library / Inspect / Updates / Tasks / Settings / About.

Every operation that touches disk, spawns a process, hashes, or uses the
network runs on a worker thread, so the window keeps repainting and Cancel
takes effect. The Tasks page shows running and recent work with progress and
errors. Selecting a library entry opens a detail page: launch, reveal in the
file manager, check and update, refresh metadata, edit the argument list and
environment pairs, set or reset the update source, and the file's path,
desktop id, hash, type, architecture, size, manager and provenance.

Library has search and sorting. AppImages found outside the managed folder
are listed for explicit adoption. Destructive choices — replace or keep both,
Trash versus permanent delete, enabling the unsafe extraction fallback,
updating a running app — go through a modal dialog that names the exact file.

Building the GUI needs the Wayland and XKB development headers
(`libwayland-dev`, `libxkbcommon-dev` on Debian/Ubuntu; present in the
Flatpak SDK).

Settings > Appearance offers System / Light / Dark, applied live with no
restart and restored at startup. System follows the COSMIC theme mode.
Positional file arguments open straight into the Inspect page; several files
are inspected and confirmed one at a time.

## Flatpak

Manifest: `packaging/com.goshapps.AppImageManager.yml`
(freedesktop 23.08 + Rust). One manifest builds **both** `x86_64` and
`aarch64` — no hardcoded architecture, no x86_64-only binaries. Cargo
dependencies are vendored in `packaging/cargo-sources.json`, generated with
the official flatpak-builder-tools generator:

```sh
just vendor /path/to/flatpak-builder-tools
flatpak-builder --force-clean build-dir packaging/com.goshapps.AppImageManager.yml --arch=x86_64
flatpak-builder --force-clean build-dir packaging/com.goshapps.AppImageManager.yml --arch=aarch64
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml \
  gosh-appimage-manager --self-test
```

The manifest does not use `--filesystem=host:rw`. It grants the managed
folder, user applications, and icon directories, session autostart
(`xdg-config/autostart:create`), plus portals and
argument-safe `flatpak-spawn --host`. Extraction tools (unsquashfs, 7zz,
dwarfsextract) are pinned by SHA-256 with per-arch binaries. Corresponding
source tarballs and license texts are installed beside the binaries.
Because the 23.08 Rust SDK extension (1.81) predates this dependency graph,
the manifest installs a pinned Rust 1.90.0 toolchain per architecture
(SHA-256 pinned, cleaned from the final app) with top-level
`no-debuginfo: true` (the 23.08 debuginfo splitter corrupts rustc 1.90's
libLLVM).

Validate the metadata:

```sh
desktop-file-validate data/com.goshapps.AppImageManager.desktop
appstreamcli validate --pedantic --no-net data/com.goshapps.AppImageManager.metainfo.xml
```

## Upgrading from 2.x

Version 3.0.0 stores the installed registry in SQLite
(`~/.local/share/gosh-appimage-manager/registry.sqlite`, mode 0600) instead
of `registry.json`. On first run, an adjacent v2 `registry.json`
(`schema_version: 1`) is imported once — rows without a UUID or managed path
are skipped, and the legacy file is left untouched. Settings move from
KConfig to `~/.config/gosh-appimage-manager/settings.json`; managed files,
desktop entries, and icons are reused in place.

## Safety

- Candidates must be regular files with ELF and AppImage magic. MIME/extension is not enough.
- Size, extraction, process output, JSON, and download bodies are bounded.
- Archive paths with `..`, absolute names, or escaping symlinks are rejected.
- Desktop `Exec` is built from program plus argument tokens. No shell strings.
- Trash failure never becomes delete. Permanent delete requires an extra confirmation and refuses protected paths.
- Update sources are checked against the address DNS actually returns, not
  just the hostname, and every redirect hop is checked before it is followed.
  Reaching a loopback or private-network endpoint needs an explicit
  `allow_local_network=true` on a source the user created; embedded metadata
  in an AppImage can never set it.
- Updates verify an advertised SHA-256 (GitHub's `sha256:` form and GitLab's
  bare hex), refuse a digest they cannot interpret, and refuse a payload whose
  architecture differs from the installed one. An update with no published
  checksum is shown as "reduced verification".
- Running-app detection works inside the Flatpak sandbox, where `/proc` shows
  only the sandbox itself, by asking the host through `flatpak-spawn`. A probe
  that fails means "cannot tell", never "not running".
- Launch is start-only detached. The manager never waits five seconds and kills the app.
- Updates download to staging, validate as an AppImage, then atomically replace with rollback material retained until success.
- Running apps block updates unless `--force` is explicit.
- The unsafe `--appimage-extract` fallback is off by default. It runs only when
  safe extraction found nothing, the setting is on, *and* that exact file was
  confirmed; it is never reached by tests or background checks.
- A failed integration removes everything it created and restores everything
  it replaced. Nothing is left in the managed folder or the applications
  directory.

## Limitations

- Zsync metadata is understood, but updates download the full file rather than applying a binary delta.
- The GUI has not been visually verified on a running compositor. It compiles
  and type-checks against libcosmic v0.12, and every operation behind it is
  covered by tests, but no screenshots have been taken. See
  `docs/verification.md`.
- On X11/Xwayland the GUI panics before mapping a window (softbuffer 0.4.1
  rejects the 32-bit visual libcosmic requests). Wayland, the primary target,
  is unaffected.
- Strings are English only. The AppStream metadata declares a gettext domain,
  but no translation infrastructure exists yet.
- A static/ftp source without version information reports "no version information" instead of guessing.
- FTP is a legacy explicit option with an insecure-transport warning. Credentials in URLs are rejected.
- Changing the managed folder away from `~/AppImages` in the Flatpak may require portal/document access for that path.
- Background update checks notify only; they never download or apply updates.
- Type 1 ISO and DwarFS extraction depend on the bundled 7zz and dwarfsextract tools.

## Attribution

Copyright Gosh Apps / Gosh-Its-Arch.

Inspired by the workflows of Gear Lever (https://github.com/mijorus/gearlever)
at commit a2917f2adafc78e0478e47d5843de9ede6c1aa3f. Gear Lever is copyright
Lorenzo Paderi and licensed under GPL-3.0-or-later.
