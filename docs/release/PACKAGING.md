# Packaging

Two artifact kinds per architecture, produced by
`scripts/package-release.sh` (tar.gz) and `flatpak.yml` (Flatpak).

## tar.gz

`gosh-appimage-manager-<ver>-linux-<arch>.tar.gz` contains one
top-level directory `gosh-appimage-manager-<ver>-linux-<arch>/`:

```
bin/gosh-appimage-manager                      # release build (CLI and launcher)
share/applications/com.goshapps.AppImageManager.desktop
share/metainfo/com.goshapps.AppImageManager.metainfo.xml
share/icons/hicolor/scalable/apps/com.goshapps.AppImageManager.svg
share/icons/hicolor/symbolic/apps/com.goshapps.AppImageManager-symbolic.svg
share/gosh-appimage-manager/i18n/*.json + README.md
share/gosh-appimage-manager/COPYING
share/gosh-appimage-manager/licenses/          # bundled-tool licenses
README.md
install.sh                                     # PREFIX-aware installer
```

The `bin/` and `share/` layout matches what the Flatpak manifest installs under `/app`, so
`install.sh` can copy it straight into a prefix (`/usr/local` default,
`PREFIX=~/.local` supported). i18n catalogs resolve at `/usr/share/…`
or via `GOSHAIM_LOCALE_DIR` (see `src/i18n.rs:catalog_dirs`).

The script refuses to package a binary whose `file` output does not
match the requested arch. Tarballs are written with sorted names and
zeroed owners for reproducibility.

Not included: the Flutter GUI bundle (the desktop app ships in the Flatpak),
build intermediates, cargo caches, tests, CI files.

## Flatpak

`gosh-appimage-manager-<ver>-linux-<arch>.flatpak` — an ostree
single-file bundle of `com.goshapps.AppImageManager`, built by
`flatpak-builder` from `packaging/com.goshapps.AppImageManager.yml`
against freedesktop 26.08. The bundle has two parts: the Rust binary at
`/app/bin/gosh-appimage-manager` (the command), and the Flutter GUI at
`/app/libexec/gosh-appimage-manager/`. The Rust core's cargo sources are
vendored (`packaging/cargo-sources.json`); the Flutter module fetches over
the network during its build. Per-arch pinned toolchain/tools live in the
manifest (Rust 1.90.0, 7zz, DwarFS binaries; `unsquashfs` from source).

Each bundle job self-verifies before upload:

1. `file` on `build-dir/files/bin/gosh-appimage-manager` and on the GUI
   under `build-dir/files/libexec/` must match the matrix arch.
2. The launcher's GUI path exists inside the sandbox
   (`/app/libexec/gosh-appimage-manager/gosh-appimage-manager-gui`).
3. `flatpak-builder --run … --self-test` must print `SELF_TEST_OK`.

## Checksums

`scripts/verify-release.sh` writes a single `SHA256SUMS` over all four
artifacts after the full-set checks pass. It is uploaded with the rest.
