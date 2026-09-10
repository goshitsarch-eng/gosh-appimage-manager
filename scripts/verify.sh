#!/usr/bin/env bash
# Gosh AppImage Manager — single verification entry point.
# Chains the existing gates; prerequisites that are absent are skipped LOUDLY,
# never faked as passes. Set SKIP_FLATPAK=1 / SKIP_GUI=1 to skip those stages.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

pass=0; skip=0
note() { echo "[verify] $*"; }
pass_step() { pass=$((pass+1)); note "PASS: $*"; }
skip_step() { skip=$((skip+1)); note "SKIP: $*"; }

note "stage: cargo build"
cargo build
pass_step "cargo build"

note "stage: cargo test (--no-fail-fast so one failure cannot hide the rest)"
cargo test --no-fail-fast
pass_step "cargo test"

note "stage: clippy (default features)"
cargo clippy --all-targets -- -D warnings
pass_step "clippy default"

if pkg-config --exists wayland-client xkbcommon 2>/dev/null; then
  note "stage: clippy (gui features)"
  cargo clippy --features gui --all-targets -- -D warnings
  pass_step "clippy gui"
else
  skip_step "clippy gui (missing wayland/xkb system headers)"
fi

note "stage: fmt"
cargo fmt --check
pass_step "fmt"

note "stage: self-test (isolated HOME)"
QA_HOME="$(mktemp -d)"
export HOME="$QA_HOME"
export GOSHAIM_HOME="$QA_HOME"
./target/debug/gosh-appimage-manager --self-test
rm -rf "$QA_HOME"
pass_step "self-test SELF_TEST_OK"

if command -v desktop-file-validate >/dev/null 2>&1; then
  note "stage: desktop-file-validate"
  desktop-file-validate data/com.goshapps.AppImageManager.desktop
  pass_step "desktop-file-validate"
else
  skip_step "desktop-file-validate (not installed)"
fi

if command -v appstreamcli >/dev/null 2>&1; then
  note "stage: appstreamcli"
  appstreamcli validate --pedantic --no-net data/com.goshapps.AppImageManager.metainfo.xml
  pass_step "appstreamcli"
else
  skip_step "appstreamcli (not installed)"
fi

if [ "${SKIP_GUI:-0}" = "1" ]; then
  skip_step "gui smoke (SKIP_GUI=1)"
elif command -v Xvfb >/dev/null 2>&1 && command -v xdotool >/dev/null 2>&1; then
  note "stage: gui build + smoke"
  cargo build --features gui
  bash tools/gui-smoke.sh
  pass_step "gui smoke"
else
  skip_step "gui smoke (needs Xvfb + xdotool; GUI compile checked by clippy-gui where possible)"
fi

if [ "${SKIP_FLATPAK:-0}" = "1" ]; then
  skip_step "flatpak build (SKIP_FLATPAK=1)"
elif command -v flatpak-builder >/dev/null 2>&1; then
  note "stage: flatpak x86_64"
  # --disable-rofiles-fuse: containers often deny fusermount; the flag only
  # changes how the builder accesses files, not the produced app.
  flatpak-builder --disable-rofiles-fuse --force-clean build-dir packaging/com.goshapps.AppImageManager.yml --arch=x86_64
  pass_step "flatpak x86_64"
else
  skip_step "flatpak build (flatpak-builder not installed)"
fi

note "done: $pass passed, $skip skipped"
