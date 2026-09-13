#!/usr/bin/env bash
# Check that a release tag agrees with the version recorded in the source.
#
#   scripts/check-version.sh v3.0.0
#   scripts/check-version.sh v3.0.0-rc.1
#
# The tag must be `vX.Y.Z` optionally followed by a pre-release suffix such
# as `-rc.1`. The base X.Y.Z must match `version` in Cargo.toml and the
# newest `<release version=...>` in the AppStream metainfo. Exits non-zero
# on any mismatch; CI treats that as "do not release".
set -euo pipefail

cd "$(dirname "$0")/.."

tag="${1:?usage: check-version.sh <tag>}"
# Strict shape: vX.Y.Z with optional -rc.N. Besides keeping odd tags out of
# filenames, this keeps the tag safe to interpolate into CI run scripts
# (quotes and semicolons are legal git refnames).
if ! [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?$ ]]; then
  echo "MISMATCH: tag '$tag' is not vX.Y.Z[-rc.N]" >&2
  exit 1
fi
version="${tag#v}"
base="${version%%-*}"

cargo_version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
meta_version="$(sed -n 's/.*<release version="\([^"]*\)".*/\1/p' \
  data/com.goshapps.AppImageManager.metainfo.xml | head -1)"

fail=0
check() {
  if [ "$2" = "$base" ]; then
    echo "ok: $1 = $2"
  else
    echo "MISMATCH: $1 is $2, tag $tag implies $base" >&2
    fail=1
  fi
}

check "Cargo.toml version" "$cargo_version"
check "metainfo release" "$meta_version"

[ "$fail" -eq 0 ] || exit 1
echo "version check passed: $tag -> $base"
