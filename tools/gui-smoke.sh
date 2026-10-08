#!/usr/bin/env bash
# Smoke check for the Flutter GUI. It starts the release bundle against a
# private GOSHAIM_HOME, checks that the process stays up and that its window
# is named, takes a screenshot when a tool for it is installed, and stops it.
#
# Build the bundle first:
#   (cd flutter && flutter build linux --release)
#
# Needs a display (DISPLAY or WAYLAND_DISPLAY); exits 2 without one. The window
# check runs the GUI on X11 (GDK_BACKEND=x11), so it needs an X server or
# XWayland. The window is found through _NET_CLIENT_LIST (xprop) or xdotool.
# Screenshots use ImageMagick's import, or grim on Wayland.
#
# Output goes to a new temporary folder, which is printed at the end.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECONDS_UP="${SMOKE_SECONDS:-8}"

if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "no display: set DISPLAY or WAYLAND_DISPLAY to run the GUI smoke check" >&2
  exit 2
fi

BUNDLE="$(ls -d "$ROOT"/flutter/build/linux/*/release/bundle 2>/dev/null | head -1 || true)"
APP="$BUNDLE/gosh-appimage-manager-gui"
if [ -z "$BUNDLE" ] || [ ! -x "$APP" ]; then
  echo "FAIL: no release bundle. Build it with: (cd flutter && flutter build linux --release)" >&2
  exit 1
fi

OUT="${SMOKE_OUT:-$(mktemp -d)}"
mkdir -p "$OUT"
mkdir -p "$OUT/home"
export GOSHAIM_HOME="$OUT/home"
if [ -n "${DISPLAY:-}" ]; then
  export GDK_BACKEND=x11
fi

"$APP" >"$OUT/gui.log" 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null || true' EXIT
sleep "$SECONDS_UP"

if ! kill -0 "$PID" 2>/dev/null; then
  echo "FAIL: the GUI exited within ${SECONDS_UP}s. Log:" >&2
  head -20 "$OUT/gui.log" >&2
  exit 1
fi

# The window is looked up in the window manager's client list (EWMH), which
# xprop reads, or with xdotool when that is installed.
WIN=""
if [ -n "${DISPLAY:-}" ] && command -v xprop >/dev/null 2>&1; then
  for id in $(xprop -root _NET_CLIENT_LIST 2>/dev/null | grep -o '0x[0-9a-f]*' || true); do
    if xprop -id "$id" WM_NAME 2>/dev/null | grep -q 'Gosh AppImage Manager'; then
      WIN="$id"
      break
    fi
  done
elif [ -n "${DISPLAY:-}" ] && command -v xdotool >/dev/null 2>&1; then
  WIN="$(xdotool search --name 'Gosh AppImage Manager' 2>/dev/null | head -1 || true)"
fi
if [ -z "$WIN" ]; then
  if [ -n "${DISPLAY:-}" ]; then
    echo "FAIL: no window named 'Gosh AppImage Manager' is open" >&2
    exit 1
  fi
  echo "skip: the window check needs an X display" >&2
else
  echo "window: $WIN"
fi

if [ -n "$WIN" ] && command -v import >/dev/null 2>&1; then
  import -window "$WIN" "$OUT/window.png"
  echo "screenshot: $OUT/window.png"
elif command -v grim >/dev/null 2>&1 && [ -n "${WAYLAND_DISPLAY:-}" ]; then
  grim "$OUT/window.png"
  echo "screenshot: $OUT/window.png"
else
  echo "skip: no screenshot taken (needs a window id and ImageMagick import, or grim on Wayland)" >&2
fi

kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
trap - EXIT

if grep -q -E "EXCEPTION|Unhandled|FATAL|Segmentation" "$OUT/gui.log"; then
  echo "FAIL: the GUI logged an error. Log: $OUT/gui.log" >&2
  exit 1
fi

echo "PASS: the GUI started, stayed up for ${SECONDS_UP}s and stopped cleanly"
echo "output: $OUT"
