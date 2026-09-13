#!/usr/bin/env bash
# Package a built release binary into the distributable tarball.
#
#   scripts/package-release.sh <version> <arch> [binary]
#
# version  e.g. 3.0.0 or 3.0.0-rc.1 (no leading v)
# arch     x86_64 | aarch64 — must match the binary's actual ELF arch
# binary   path to the release binary (default: target/release/gosh-appimage-manager)
#
# Produces gosh-appimage-manager-<version>-linux-<arch>.tar.gz in the
# current directory.
set -euo pipefail

out_dir="$(pwd)"
cd "$(dirname "$0")/.."

version="${1:?usage: package-release.sh <version> <arch> [binary]}"
arch="${2:?usage: package-release.sh <version> <arch> [binary]}"
binary="${3:-target/release/gosh-appimage-manager}"
case "$binary" in /*) ;; *) binary="$out_dir/$binary";; esac

case "$arch" in
  x86_64)  want="x86-64" ;;
  aarch64) want="ARM aarch64" ;;
  *) echo "unknown arch: $arch" >&2; exit 2 ;;
esac

[ -f "$binary" ] || { echo "binary not found: $binary" >&2; exit 1; }
file "$binary" | grep -q "$want" \
  || { echo "arch mismatch: $binary is not $want" >&2; file "$binary" >&2; exit 1; }

name="gosh-appimage-manager-$version-linux-$arch"
stage="$(mktemp -d)/$name"
trap 'rm -rf "${stage%/*}"' EXIT

install -Dm755 "$binary" "$stage/bin/gosh-appimage-manager"
install -Dm644 data/com.goshapps.AppImageManager.desktop \
  "$stage/share/applications/com.goshapps.AppImageManager.desktop"
install -Dm644 data/com.goshapps.AppImageManager.metainfo.xml \
  "$stage/share/metainfo/com.goshapps.AppImageManager.metainfo.xml"
install -Dm644 data/icons/hicolor/scalable/apps/com.goshapps.AppImageManager.svg \
  "$stage/share/icons/hicolor/scalable/apps/com.goshapps.AppImageManager.svg"
install -Dm644 data/icons/hicolor/symbolic/apps/com.goshapps.AppImageManager-symbolic.svg \
  "$stage/share/icons/hicolor/symbolic/apps/com.goshapps.AppImageManager-symbolic.svg"
install -Dd "$stage/share/gosh-appimage-manager/i18n"
install -m644 -t "$stage/share/gosh-appimage-manager/i18n" i18n/*.json
install -m644 i18n/README.md "$stage/share/gosh-appimage-manager/i18n/README.md"
install -Dm644 COPYING "$stage/share/gosh-appimage-manager/COPYING"
cp -a third_party/licenses "$stage/share/gosh-appimage-manager/licenses"
install -m644 README.md "$stage/README.md"
install -Dm755 packaging/install.sh "$stage/install.sh"

tar --sort=name --owner=0 --group=0 --numeric-owner \
  -czf "$out_dir/$name.tar.gz" -C "${stage%/*}" "$name"

echo "wrote $out_dir/$name.tar.gz"
