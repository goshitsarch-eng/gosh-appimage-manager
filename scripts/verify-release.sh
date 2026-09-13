#!/usr/bin/env bash
# Verify a release-artifact directory and write SHA256SUMS into it.
#
#   scripts/verify-release.sh <dir> <version>
#
# Expects the four required assets for <version> (e.g. 3.0.0-rc.1):
#   gosh-appimage-manager-<ver>-linux-x86_64.tar.gz
#   gosh-appimage-manager-<ver>-linux-aarch64.tar.gz
#   gosh-appimage-manager-<ver>-linux-x86_64.flatpak
#   gosh-appimage-manager-<ver>-linux-aarch64.flatpak
#
# Checks: presence, plausible size, gzip/flatpak magic, per-archive ELF
# architecture, then regenerates SHA256SUMS over all four files.
set -euo pipefail

dir="${1:?usage: verify-release.sh <dir> <version>}"
version="${2:?usage: verify-release.sh <dir> <version>}"
name="gosh-appimage-manager-$version"
fail=0

err() { echo "MISSING/BAD: $*" >&2; fail=1; }

check_file() { # path min_bytes
  [ -f "$1" ] || { err "$1 not found"; return 1; }
  size=$(stat -c%s "$1")
  [ "$size" -ge "$2" ] || { err "$1 only $size bytes"; return 1; }
  echo "ok: $(basename "$1") ($size bytes)"
}

for arch in x86_64 aarch64; do
  tgz="$dir/$name-linux-$arch.tar.gz"
  fpk="$dir/$name-linux-$arch.flatpak"

  if check_file "$tgz" 1000000; then
    tar -tzf "$tgz" >/dev/null || err "$tgz is not a valid gzip tar"
    bin="$(tar -tzf "$tgz" | grep '/bin/gosh-appimage-manager$' || true)"
    [ -n "$bin" ] || err "$tgz has no bin/gosh-appimage-manager"
    if [ -n "$bin" ]; then
      tmp="$(mktemp -d)"
      tar -xzf "$tgz" -C "$tmp" "$bin"
      desc="$(file "$tmp/$bin")"
      rm -rf "$tmp"
      case "$arch" in
        x86_64)  want="x86-64" ;;
        aarch64) want="ARM aarch64" ;;
      esac
      echo "$desc" | grep -q "$want" \
        && echo "ok: $tgz binary is $want" \
        || err "$tgz binary is not $want: $desc"
    fi
  fi

  if check_file "$fpk" 10000000; then
    magic="$(head -c7 "$fpk")"
    [ "$magic" = "flatpak" ] || err "$fpk lacks flatpak bundle magic"
    # Bundle headers record the ref and runtime with their arch; both live
    # in the first few KB. (head -c before strings avoids a SIGPIPE trip
    # under pipefail.)
    head -c 8192 "$fpk" | strings | grep -q "/$arch/" \
      && echo "ok: $fpk declares $arch" \
      || err "$fpk does not declare arch $arch"
  fi
done

[ "$fail" -eq 0 ] || { echo "release verification FAILED" >&2; exit 1; }

(cd "$dir" && sha256sum "$name"-linux-*.tar.gz "$name"-linux-*.flatpak > SHA256SUMS)
echo "release verification passed; SHA256SUMS written"
cat "$dir/SHA256SUMS"
