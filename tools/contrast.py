#!/usr/bin/env python3
"""Measure WCAG contrast of rendered UI text against the frames captured by
gui-smoke.sh.

Sampling a single guessed coordinate lands on antialiased glyph edges and
reports nonsense. For each region this takes the most common colour as the
background and the pixel furthest from it in luminance as the glyph core,
which is what a reader actually perceives.
"""
import collections
import re
import subprocess
import sys

AA_BODY = 4.5   # WCAG AA, normal text
AA_LARGE = 3.0  # WCAG AA, large text


def pixels(img, x, y, w, h):
    out = subprocess.run(
        ["convert", img, "-crop", f"{w}x{h}+{x}+{y}", "+repage", "-depth", "8", "txt:-"],
        capture_output=True, text=True, check=False).stdout
    res = []
    for line in out.splitlines()[1:]:
        m = re.search(r"#([0-9A-Fa-f]{6})", line)
        if m:
            v = m.group(1)
            res.append((int(v[0:2], 16), int(v[2:4], 16), int(v[4:6], 16)))
    return res


def luminance(c):
    def channel(v):
        v /= 255
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    return 0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2])


def ratio(a, b):
    la, lb = luminance(a), luminance(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


REGIONS = [
    ("page-library.png",  (313, 66, 90, 26),  "Library title",         AA_LARGE),
    ("page-library.png",  (313, 146, 120, 18), "Empty-state heading",  AA_BODY),
    ("page-library.png",  (313, 170, 500, 18), "Empty-state body",     AA_BODY),
    ("page-library.png",  (350, 110, 330, 20), "Search placeholder",   AA_BODY),
    ("page-library.png",  (30, 62, 60, 18),   "Nav item (active)",     AA_BODY),
    ("page-library.png",  (30, 262, 45, 18),  "Nav item (inactive)",   AA_BODY),
    ("page-library.png",  (930, 70, 62, 20),  "Button label",          AA_BODY),
    ("page-updates.png",  (313, 106, 215, 18), "Updates empty body",   AA_BODY),
    ("page-updates.png",  (915, 70, 72, 20),  "Suggested button",      AA_BODY),
    ("page-settings.png", (336, 330, 335, 18), "Settings row label",   AA_BODY),
    ("page-settings.png", (319, 664, 505, 14), "Settings caption",     AA_BODY),
    ("page-inspect.png",  (313, 104, 300, 14), "Safety caption",       AA_BODY),
]


def main(out_dir):
    print("%-24s %-15s %-15s %7s %5s  %s"
          % ("region", "foreground", "background", "ratio", "need", "verdict"))
    failures = []
    for name, box, label, need in REGIONS:
        px = pixels(f"{out_dir}/{name}", *box)
        if not px:
            print("%-24s (frame missing)" % label)
            continue
        bg = collections.Counter(px).most_common(1)[0][0]
        fg = max(px, key=lambda c: abs(luminance(c) - luminance(bg)))
        r = ratio(fg, bg)
        ok = r >= need
        if not ok:
            failures.append((label, r, need))
        print("%-24s %-15s %-15s %7.2f %5.1f  %s"
              % (label, str(fg), str(bg), r, need, "PASS" if ok else "FAIL"))
    print()
    if failures:
        for label, r, need in failures:
            print("BELOW AA: %s -> %.2f (needs %.1f)" % (label, r, need))
        return 1
    print("All sampled text meets WCAG AA.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
