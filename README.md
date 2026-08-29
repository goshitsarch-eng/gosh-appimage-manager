# Gosh AppImage Manager

Gosh AppImage Manager is a native KDE application for inspecting, integrating,
launching, organizing, updating, and removing AppImages. It is written in
C++20 with Qt 6.10, KDE Frameworks 6, and Kirigami.

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

## Build

Requires the KDE 6.10 SDK or an equivalent KF6/Qt 6.10 toolchain.

```sh
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build build
```

Inside the Flatpak SDK:

```sh
flatpak run --filesystem="$(pwd)" --share=network --devel \
  --command=bash org.kde.Sdk//6.10 -lc \
  'cmake -S /the/source -B /the/source/build -G Ninja && cmake --build /the/source/build'
```

`scripts/sdk-build.sh` configures, builds, and runs CTest.

## Test

```sh
QT_QPA_PLATFORM=offscreen ctest --test-dir build --output-on-failure
```

Tests use fake process/network seams and synthetic ELF/AppImage fixtures. They
do not touch a real home directory, execute an AppImage, launch a user app, or
call live update APIs.

QML static checks:

```sh
find src/qml -name '*.qml' -print0 | xargs -0 qmllint
```

Desktop and AppStream:

```sh
desktop-file-validate data/com.goshapps.AppImageManager.desktop
appstreamcli validate --pedantic --no-net --explain data/com.goshapps.AppImageManager.metainfo.xml
```

## CLI

The same executable provides GUI and CLI modes. CLI commands do not start the
GUI.

```
gosh-appimage-manager --integrate <path> [--keep-both|--replace] [--yes]
gosh-appimage-manager --update <path>|--all [--yes] [--force]
gosh-appimage-manager --remove <path> [--yes] [--delete]
gosh-appimage-manager --remove-all [--yes]
gosh-appimage-manager --list-installed [--json]
gosh-appimage-manager --list-updates [--json]
gosh-appimage-manager --list-update-managers
gosh-appimage-manager --set-update-source <path> --manager <name> key=value...
gosh-appimage-manager --set-update-source <path> --unset
gosh-appimage-manager --fetch-updates
gosh-appimage-manager --self-test
gosh-appimage-manager --probe-host
gosh-appimage-manager --probe-inspect <path>
```

JSON list output uses `schema_version: 1`. Diagnostics go to stderr.

## Flatpak

Manifest: `packaging/com.goshapps.AppImageManager.yml`

The manifest does not use `--filesystem=host:rw`. It grants the managed
folder, user applications, and icon directories, plus portals and
argument-safe `flatpak-spawn --host`. Extraction tools (unsquashfs, 7zz,
dwarfsextract) are pinned by commit or SHA-256.

```sh
flatpak-builder --force-clean --user --install-deps-from=flathub \
  --repo=repo build-dir packaging/com.goshapps.AppImageManager.yml
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml \
  env QT_QPA_PLATFORM=offscreen gosh-appimage-manager --self-test
```

## Safety

- Candidates must be regular files with ELF and AppImage magic. MIME/extension is not enough.
- Size, extraction, process output, JSON, and download bodies are bounded.
- Archive paths with `..`, absolute names, or escaping symlinks are rejected.
- Desktop `Exec` is built from program plus argument tokens. No shell strings.
- Trash failure never becomes delete. Permanent delete requires an extra confirmation and refuses protected paths.
- Launch is start-only detached. The manager never waits five seconds and kills the app.
- Updates download to staging, validate as an AppImage, then atomically replace with rollback material retained until success.
- Running apps block updates unless `--force` is explicit.
- The unsafe `--appimage-extract` fallback is off by default, warned, and never used in tests or background checks.

## Limitations

- Zsync metadata is understood, but updates download the full file rather than applying a binary delta.
- FTP is a legacy explicit option with an insecure-transport warning. Credentials in URLs are rejected.
- Changing the managed folder away from `~/AppImages` in the Flatpak may require portal/document access for that path.
- Background update checks notify only; they never download or apply updates.
- Type 1 ISO and DwarFS extraction depend on the bundled 7zz and dwarfsextract tools.

## Attribution

Copyright Gosh Apps / Gosh-Its-Arch.

Inspired by the workflows of Gear Lever (https://github.com/mijorus/gearlever)
at commit a2917f2adafc78e0478e47d5843de9ede6c1aa3f. Gear Lever is copyright
Lorenzo Paderi and licensed under GPL-3.0-or-later.
