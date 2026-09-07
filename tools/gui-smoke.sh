#!/usr/bin/env bash
# Headless GUI smoke check: start the app on a private X server, drive it,
# capture each page, and measure text contrast against the rendered pixels.
#
# The project had never rendered a frame before this existed -- the GUI
# crashed on X11 (softbuffer could not use the 32-bit visual libcosmic asks
# for) and no compositor was available, so every GUI claim was reasoned about
# rather than observed. Building with the wgpu renderer fixes the crash and
# makes this possible.
#
# Requires: Xvfb, xdotool, ImageMagick, python3. On Debian/Ubuntu:
#   apt-get install xvfb xdotool imagemagick libgl1-mesa-dri
# Build first:  cargo build --features gui
set -euo pipefail

DISPLAY_NUM="${DISPLAY_NUM:-:99}"
OUT="${OUT:-$(mktemp -d)}"
HOME_DIR="$OUT/home"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/debug/gosh-appimage-manager"

[ -x "$BIN" ] || { echo "build first: cargo build --features gui" >&2; exit 1; }

mkdir -p "$HOME_DIR" "$OUT"
# XDG_RUNTIME_DIR must be short: the wayland socket path has a 108-byte cap.
RT="${RT:-/run/goshaim-smoke}"
mkdir -p "$RT" && chmod 700 "$RT"

pkill -f "Xvfb $DISPLAY_NUM" 2>/dev/null || true
sleep 1
Xvfb "$DISPLAY_NUM" -screen 0 1280x900x24 -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
XVFB=$!
trap 'kill $XVFB 2>/dev/null || true' EXIT
sleep 3

export DISPLAY="$DISPLAY_NUM" XDG_RUNTIME_DIR="$RT" GOSHAIM_HOME="$HOME_DIR"
# No GPU in CI; wgpu falls back to the software GL backend.
export LIBGL_ALWAYS_SOFTWARE=1 WGPU_BACKEND=gl

"$BIN" >"$OUT/gui.log" 2>&1 &
APP=$!
sleep 14

if ! kill -0 $APP 2>/dev/null; then
  echo "FAIL: the GUI exited during startup" >&2
  head -20 "$OUT/gui.log" >&2
  exit 1
fi

# There is no window manager, so the toplevel gets no X input focus and
# synthetic keystrokes would go nowhere.
WIN=$(xdotool search --name "Gosh AppImage Manager" | head -1)
xdotool windowfocus "$WIN" || true
sleep 1

y=71
for page in library inspect updates tasks settings about; do
  xdotool mousemove 147 $y click 1
  sleep 2
  import -window root "$OUT/page-$page.png"
  y=$((y + 40))
done

# Condensed layout: rows must stack rather than overflow.
xdotool windowsize "$WIN" 430 800; sleep 3
import -window root "$OUT/narrow.png"
xdotool windowsize "$WIN" 1024 768; sleep 2

kill $APP 2>/dev/null || true
wait $APP 2>/dev/null || true

echo "screenshots in $OUT"
python3 "$ROOT/tools/contrast.py" "$OUT"
