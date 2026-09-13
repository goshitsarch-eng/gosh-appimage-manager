#!/bin/sh
# Install Gosh AppImage Manager from this tarball.
#
#   ./install.sh              # installs under /usr/local (needs sudo/root)
#   PREFIX=~/.local ./install.sh
#
# To remove, delete the same paths under $PREFIX. i18n catalogs are only
# found at /usr/share or via GOSHAIM_LOCALE_DIR; only English ships today,
# so a ~/.local install loses nothing.
set -eu

PREFIX="${PREFIX:-/usr/local}"
src="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

install -Dm755 "$src/bin/gosh-appimage-manager" "$PREFIX/bin/gosh-appimage-manager"
install -Dm644 "$src/share/applications/com.goshapps.AppImageManager.desktop" \
  "$PREFIX/share/applications/com.goshapps.AppImageManager.desktop"
install -Dm644 "$src/share/metainfo/com.goshapps.AppImageManager.metainfo.xml" \
  "$PREFIX/share/metainfo/com.goshapps.AppImageManager.metainfo.xml"
install -Dm644 "$src/share/icons/hicolor/scalable/apps/com.goshapps.AppImageManager.svg" \
  "$PREFIX/share/icons/hicolor/scalable/apps/com.goshapps.AppImageManager.svg"
install -Dm644 "$src/share/icons/hicolor/symbolic/apps/com.goshapps.AppImageManager-symbolic.svg" \
  "$PREFIX/share/icons/hicolor/symbolic/apps/com.goshapps.AppImageManager-symbolic.svg"
mkdir -p "$PREFIX/share/gosh-appimage-manager/i18n"
cp -a "$src/share/gosh-appimage-manager/i18n/." "$PREFIX/share/gosh-appimage-manager/i18n/"
install -Dm644 "$src/share/gosh-appimage-manager/COPYING" \
  "$PREFIX/share/gosh-appimage-manager/COPYING"
cp -a "$src/share/gosh-appimage-manager/licenses" "$PREFIX/share/gosh-appimage-manager/"

echo "Installed to $PREFIX"
