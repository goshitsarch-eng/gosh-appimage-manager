# Gosh AppImage Manager

Inspect, integrate, launch, update, and remove AppImages. A Rust core does
the work, a Flutter GUI is the desktop app, and one `gosh-appimage-manager`
binary provides a scriptable CLI and starts that GUI.

- Application ID: `com.goshapps.AppImageManager`
- Version 3.0.0 · GPL-3.0-or-later · by Gosh Apps / Gosh-Its-Arch

## AI-assisted development

AI tools are used during development to speed up implementation and assist
with coding. The maintainer works architecture-first: the design is defined
by a human, and AI output is reviewed, tested, and refactored rather than
trusted. AI is treated as a junior developer — useful for implementation
and exploration, never the authority. The maintainer accepts responsibility
for the architecture, technical decisions, and code quality.

This notice is here so you can make an informed choice about whether
AI-assisted software is something you're comfortable using.

## Features

- **Inspect without side effects.** Opening an AppImage shows its type,
  architecture, SHA-256, embedded desktop metadata, icon, and update
  source. Nothing is executed or installed.
- **Integrate.** Copies the file into the managed folder (`~/AppImages` by
  default), writes a menu entry, and installs the extracted icon. Name
  conflicts offer keep-both or replace.
- **Manage.** Launch, reveal in the file manager, refresh metadata, edit
  launch arguments and environment variables, trash or permanently delete.
- **Update.** Per-app or batch updates from GitHub, GitLab, Codeberg,
  Forgejo, static URLs (including zsync metadata), or FTP. Embedded
  `.upd_info` in the AppImage is picked up automatically. Downloads are
  staged, verified, and applied atomically with rollback.
- **Discover and adopt.** AppImages dropped into the managed folder — or
  referenced by desktop entries elsewhere when discovery is enabled — are
  listed for adoption.
- **CLI** with JSON output, non-mutating probes, and an offline self-test.
- **Localizable.** The Rust core loads JSON message catalogs (`i18n/`), but
  no interface uses them yet; only English ships.

## Install

### Flatpak

Releases attach `gosh-appimage-manager-<ver>-linux-<arch>.flatpak` for
x86_64 and aarch64 — install the one matching your machine:

```sh
flatpak install gosh-appimage-manager-*-linux-x86_64.flatpak
```

CI also builds unversioned bundles on every push if you want a
development build. The app is not published to Flathub; the bundle is the
distribution format. To build it yourself, see the [Flatpak](#flatpak-1)
section below.

### Tarball

Each release also carries `…-linux-<arch>.tar.gz` containing the binary,
desktop integration files, icons, licenses, and an `install.sh` that
installs under `/usr/local` (or `PREFIX=~/.local`). The tarball has the CLI
and launcher only; the desktop app comes with the Flatpak.

### Build from source

Native build needs Rust 1.89 or newer — that is what the locked dependency
graph requires (`rust-version` in `Cargo.toml`). The GUI is a separate
Flutter app in `flutter/`, built on the Rust core through the bridge in
`bridge/`; see [Development](#development).

```sh
cargo build                 # the Rust core, CLI, and launcher
cargo test
```

With a command, the binary runs the CLI. With no command, it starts the GUI
from `../libexec/gosh-appimage-manager/gosh-appimage-manager-gui`, relative
to its own directory. Set `GOSH_APPIMAGE_GUI` to an absolute path to run a
different GUI build. If the GUI is missing, the binary prints one line and a
hint to stderr and exits with status 1.

## Using the app

Six pages in the sidebar: **Library, Inspect, Updates, Tasks, Settings,
About.**

**Add an AppImage.** Open it from your file manager (the app registers the
AppImage MIME types), drop it onto the window, or use Inspect → Browse.
The Inspect page shows what the file is before you commit. Press
**Integrate** to add it.

**Work with the library.** Search by name, version, or path; sort by name,
version, or updates-first. A row offers Launch / Details / Trash. Details
shows the full record — path, desktop ID, SHA-256, type, architecture,
size, update manager, provenance — plus argument and environment editors,
the update-source editor, and permanent delete.

**Update.** Updates → Check now lists offers; Update per row or Update all.
An app needs an update source: either one embedded in the AppImage or one
set on its Details page. A source with no published checksum is marked
"reduced verification". A running app blocks its update unless you confirm
the override.

**Keyboard shortcuts.** Ctrl+O browse a file · Ctrl+R or F5 refresh the
library · Ctrl+F check for updates · Esc dismiss a dialog.

**Settings** (all optional, all off unless noted):

- Appearance — System / Light / Dark, applied live and restored at startup.
- Integration folder — where integrated AppImages live, plus a max file
  size (1–32768 MB, default 8192).
- Move the original into the library instead of copying — after a verified
  integration the source goes to the Trash, never a hard delete.
- Discover AppImages outside the managed folder — lists desktop-entry
  AppImages from elsewhere for adoption.
- Terminal apps: drop the .AppImage suffix from the name.
- Verbose diagnostics — per-operation lines on stderr.
- Check for updates in the background (notify only), and Run those checks
  at login via an autostart entry.
- Unsafe extraction fallback — opt-in in Settings, off by default. See Safety
  below.

## CLI

The same executable doubles as a CLI; commands never start the GUI.
`--version` prints `3.0.0`. `--help` (or `-h`) prints usage.

```
gosh-appimage-manager --integrate <path> [--keep-both|--replace] [--replace-uuid UUID|--target PATH] [--yes] [--allow-unsafe]
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
gosh-appimage-manager --probe-inspect <path> [--allow-unsafe]
gosh-appimage-manager --probe-autostart
```

`--allow-unsafe` lets the unsafe extraction fallback run for that one file,
when Settings' Unsafe extraction fallback is also on. Without it, an AppImage
that needs the fallback is left pending: `--integrate` exits with code 5 and
installs nothing, and the message names the flag to add.

`-y` is a short alias for `--yes`. Destructive commands ask for
confirmation on a terminal; without one they refuse (exit 5) unless `--yes`
is passed.

`--list-update-managers` prints the six source types: `static`, `github`,
`gitlab`, `codeberg`, `forgejo`, `ftp`. Source config is `key=value` pairs
after `--manager <name>` — for example:

```sh
gosh-appimage-manager --set-update-source ~/AppImages/foo.AppImage \
  --manager github username=me repo=app filename=app-x86_64.AppImage
```

Required keys: `github` needs `username`, `repo`, `filename`; `gitlab`
needs `project` (`host` defaults to gitlab.com); `codeberg` needs `owner`,
`repo`; `forgejo` needs `host`, `owner`, `repo`; `static` and `ftp` need
`url` (a `version` key is recommended — without one the source reports
"no version information"). `allow_local_network=true` opts a source into
private/loopback endpoints; embedded metadata can never set it. The GUI's
update-source field accepts a single `key=value` line. For GitHub that line is
`repo=owner/name`. The AppImage's `filename` is still needed unless its embedded
update information names the same repo; the GUI then shows the exact
`--set-update-source` command. Other multi-key managers are configured from the
CLI.

`--list-discovered` reports AppImages in the managed folder and, when
discovery is enabled, ones referenced by desktop entries elsewhere.
`--adopt` registers such a file so it can be updated and removed here;
nothing on disk changes, and its existing desktop entry is left alone —
but removing an adopted app later trashes the file at its original
location.

Machine output goes to stdout; prompts and diagnostics to stderr, so
`--json` output stays parseable. List documents use `schema_version: 1`
with an `installed`, `updates`, or `discovered` array. `--list-updates` and
`--fetch-updates` exit 8 when any app's check failed — the JSON is still
complete, so a script can tell "nothing to update" from "nothing could be
checked". Other exit codes: 0 ok · 1 failure · 2 usage · 4 not integrated ·
5 confirmation declined · 6 validation · 7 app is running. (3 is reserved.)

`--fetch-updates` only checks and notifies — it never downloads or applies
anything; the login autostart entry runs exactly this command.

`--probe-inspect <path>` prints a JSON description of a file followed by
`INSPECT_NO_EXECUTION` — proof it was never executed. `--probe-host` and
`--probe-autostart` verify host spawning and render the autostart entry
without writing it. `--self-test` exercises the stack against synthetic
fixtures and prints `SELF_TEST_OK`.

## Where things live

| What | Path |
|---|---|
| Managed AppImages | `~/AppImages` (configurable in Settings) |
| Registry | `~/.local/share/gosh-appimage-manager/registry.sqlite` (mode 0600) |
| Settings | `~/.config/gosh-appimage-manager/settings.json` (mode 0600) |
| Generated desktop entries | `~/.local/share/applications/gosh-appimage-<uuid>.desktop` |
| Installed icons | `~/.local/share/icons/hicolor/256x256/apps/` |
| Login checks entry | `~/.config/autostart/com.goshapps.AppImageManager-updates.desktop` |

`HOME`, `GOSHAIM_HOME`, and `GOSHAIM_XDG_{DATA,CONFIG,CACHE}_HOME` relocate
these roots (the last are mostly for tests). The standard `XDG_*_HOME`
variables are not read — inside the Flatpak these are the sandbox's
`~/.var/app/com.goshapps.AppImageManager/` tree anyway.

## Development

```sh
cargo test                        # full suite: fake seams + synthetic fixtures, no network
cargo clippy --all-targets -- -D warnings
cargo fmt --check
./scripts/verify.sh               # everything above + self-test + validators + optional Flatpak
```

`just` wraps the common flows (`just build`, `test`, `lint`, `fmt-check`,
`self-test`, `flutter-check`, `validate`, `vendor`, `flatpak-x86_64`,
`flatpak-aarch64`). See [CONTRIBUTING.md](CONTRIBUTING.md) for the full
workflow and `docs/RELEASING.md` for how releases are cut.

## Flatpak

Manifest: `packaging/com.goshapps.AppImageManager.yml` (freedesktop 26.08).
The Rust core uses a pinned Rust 1.90.0 toolchain (the SDK extension's 1.81
predates the dependency graph); its Cargo deps are vendored in
`packaging/cargo-sources.json` (`just vendor /path/to/flatpak-builder-tools`
regenerates it). The Flutter GUI is a second module that builds the bundle
the way `.github/workflows/flutter.yml` does and installs it to
`/app/libexec/gosh-appimage-manager/`. That module uses the network during
the build. One manifest builds both x86_64 and aarch64.

```sh
flatpak-builder --user --force-clean build-dir packaging/com.goshapps.AppImageManager.yml --arch=x86_64
flatpak-builder --user --install build-dir packaging/com.goshapps.AppImageManager.yml   # install the result
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml gosh-appimage-manager --self-test
```

Sandbox grants: Wayland/fallback-X11, IPC, dri, network, notifications,
portals, the managed folder, `~/.local/share/{applications,icons}`,
`xdg-config/autostart:create`, and the app data dir. Two worth knowing:

- `--talk-name=org.freedesktop.Flatpak` grants `flatpak-spawn --host`,
  which is how the app launches AppImages and checks running processes
  outside the sandbox. It is restricted to named helpers and managed
  AppImage paths in code — but it means the sandbox is not a hard security
  boundary for this app.
- The manifest does **not** grant `--filesystem=host:rw`. Pointing the
  managed folder outside `~/AppImages` may need an extra filesystem grant
  or a portal-granted path.

Bundled extraction tools are pinned by SHA-256 per architecture
(`unsquashfs` built from source; 7-Zip 26.00 and DwarFS 0.15.3 binaries)
with license texts and corresponding source installed under
`/app/share/gosh-appimage-manager/`. Validate the metadata with
`just validate` (`desktop-file-validate` + `appstreamcli`).

## Upgrading from 2.x

The registry moved from `registry.json` to SQLite. On first run an adjacent
v2 `registry.json` is imported once — rows without a UUID or managed path
are skipped, and the old file is left in place. Settings moved from KConfig
to `settings.json`; managed files, desktop entries, and icons are reused
where they are.

## Safety

- A file is only a candidate if it is a regular file with ELF and AppImage
  magic — MIME type and extension are never enough.
- Everything untrusted is bounded: file size, extraction output, archive
  listings, process output, JSON bodies, downloads, redirects, timeouts.
- Archive members with `..`, absolute paths, or option-like names are
  rejected; extraction lands in private mode-0700 temp dirs.
- Desktop `Exec` lines are built from argument tokens — no shell strings.
- Removal is Trash-first; a failed Trash never becomes a delete. Permanent
  delete asks again and refuses protected paths.
- Update URLs are checked against the address DNS actually returns, and
  every redirect hop is re-checked. Private/loopback endpoints need an
  explicit `allow_local_network=true` on a source you created.
- Downloads are staged, validated as AppImages, checked against an
  advertised SHA-256 when one exists, refused on architecture mismatch,
  then swapped in atomically. Rollback material is kept until success.
- Running apps block updates unless `--force` / "Update anyway" is
  confirmed. Inside the Flatpak, running-detection asks the host through
  `flatpak-spawn`; if that probe fails it falls back to the sandbox's own
  (empty) process view — a failed probe effectively reads as "not running".
- Launch is start-only and detached; the manager never waits then kills.
- A failed integration removes what it created and restores what it
  replaced.
- The unsafe `--appimage-extract` fallback is opt-in. It is off by default, and
  Settings' Unsafe extraction fallback turns it on after a warning. It runs only
  after safe extraction fails, and only for a file you confirm, one file at a
  time (`--allow-unsafe` on the command line, or the confirmation in the app).
  It runs the AppImage itself: in a private staging copy, with a timeout, a
  minimal environment, and a check of the extracted tree. Turn it on only for
  AppImages you trust.
- No telemetry. Network requests go only to update endpoints you configured
  or that an AppImage's embedded metadata named, and only when checking or
  applying updates.

## Limitations

- **DwarFS AppImages are detected but their metadata cannot be read.** The
  extractor dispatch selects `dwarfsextract` but no lister/extractor arm is
  implemented, so safe extraction always fails for them — the bundled
  `dwarfsextract`/`dwarfsck` binaries are currently unused. Such files
  still integrate, named after the file.
- zsync metadata is parsed, but updates always download the full file —
  no binary deltas.
- The GUI is written in Flutter. Its feature parity and verification status
  are recorded in `docs/flutter/PARITY.md` and `docs/verification.md`.
- Only English ships. The interface is localizable — see `i18n/README.md`.
- The GUI's update-source field takes a single `key=value` pair. A GitHub
  source that needs a `filename` is set with the command the GUI shows; other
  multi-key managers are configured via `--set-update-source`.
- FTP is a legacy explicit option with an insecure-transport warning; URLs
  with credentials are rejected.
- Static and FTP sources without a `version` key report "no version
  information" rather than guessing.

## Attribution

Gear Lever (https://github.com/mijorus/gearlever) by Lorenzo Paderi at
commit `a2917f2adafc78e0478e47d5843de9ede6c1aa3f` was the GPL-3.0-or-later
behavioural reference. This is an independent implementation — no Gear
Lever code, UI, assets, or branding — and is not endorsed by its authors.

Copyright Gosh Apps / Gosh-Its-Arch. Licensed GPL-3.0-or-later; see
`COPYING`. Bundled tool licenses are in `third_party/licenses/`.
