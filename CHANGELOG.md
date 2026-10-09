# Changelog

All notable changes to Gosh AppImage Manager are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project follows [Semantic Versioning](https://semver.org/).

Each release's version must agree with `version` in `Cargo.toml` and the newest
`<release>` in `data/com.goshapps.AppImageManager.metainfo.xml`; see
`docs/RELEASING.md`.

## [Unreleased]

## [3.0.1] - 2026-10-09

A fix release for icons and updates that did not work on real AppImages. The
causes, how each was reproduced, and what was verified are in
`docs/qa/FIX-2026-10-09-icons-and-updates.md`.

### Fixed

- **App icons.** Icons are now found in the AppImages people actually have: ones
  whose `.DirIcon` is a symlink (nearly all of them), whose `Icon=` carries a file
  extension, or whose icon exists only under `usr/share/icons`. The best size is
  chosen, and a file is accepted as an icon by its content, not its name.
- **SVG icons** are drawn. The GUI could not decode them, so every app with an SVG
  icon showed a letter. Non-square icons are no longer stretched to a square.
- **Adopted apps** had no icon, version or update information, because adoption
  stored only the file name. Adoption now reads the file the same safe way
  Inspect does. Refresh metadata no longer discards the icon it reads.
- **Updates for AppImages with built-in update information.** The usual
  `gh-releases-zsync` string was handed to the wrong source and failed with
  "Static source needs a url". It now selects GitHub, downloads the AppImage
  rather than its `.zsync` control file, honours a named release such as
  `continuous`, and prefers the build for the installed CPU architecture (a
  build labelled for another one is refused, before anything is downloaded).
- **"No update method was found"** for adopted apps, which now carry the update
  information in their file.
- **Version comparison.** A release tag `v1.2.3` and an AppImage that says `1.2.3`
  are the same version, numbers compare as numbers (`1.9` is older than `1.10`),
  and an older release is never offered as an update. A rebuild under the same
  tag counts as an update only when the source publishes a SHA-256 and the
  installed file's differs.
- **Network failures that were not the app's fault.** The client now trusts the
  system certificate bundle (`SSL_CERT_FILE`, or the distribution's own), so
  networks that re-sign TLS no longer fail with "UnknownIssuer"; it tries every
  address a name resolves to instead of only the first; a server that refuses
  `HEAD` is probed with a one-byte ranged `GET`; and a zsync control file's
  relative `URL:` is resolved.
- **`allow_local_network=true`** could not be saved on a static or GitLab update
  source, so self-hosted update servers could not be used. Embedded update
  information still cannot set it.
- An XPM icon is no longer installed under a `.png` name.

### Added

- Apps that already have no icon file (adopted by an older build, or whose icon
  was lost) get it back after the next start, once per app per run, without a
  task or a prompt. It never rewrites a menu entry that is no longer ours.
- The Inspect page shows the icon extracted from the AppImage in its header.
- Update failures say what happened: GitHub 404, 403 and rate-limit answers, a
  timeout, the real cause of a connection or certificate error, and what to do
  for an app with no update source.

### Changed

- Adopted apps keep their icon in `~/.local/share/gosh-appimage-manager/icons/`.
  Adoption still writes nothing to your menu or icon theme.
- After an update the app's version is recorded as the AppImage names it
  (`2.1.0`, not the tag `v2.1.0`).

### Known limits

- DwarFS AppImages are detected, but their metadata is still not read, so they
  keep a letter tile and have no embedded update information.
- XPM icons are kept for the desktop, but the GUI cannot draw them.

## [3.0.0] - 2026-09-06

The native Rust rewrite: a Rust core with a Flutter GUI, a scriptable CLI, and
multi-architecture Flatpak builds. See the README for what it does, and
`docs/release/REPORT.md` for how the release pipeline was verified.
