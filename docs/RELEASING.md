# Releasing Gosh AppImage Manager

Releases are tag-driven. Everything after the tag push is automated.

## Cut a release

```sh
# 1. bump the version
$EDITOR Cargo.toml                                   # version = "X.Y.Z"
$EDITOR data/com.goshapps.AppImageManager.metainfo.xml  # add <release version="X.Y.Z">
cargo check   # refresh Cargo.lock
git commit -am "release X.Y.Z" && git push

# 2. tag and push the tag
git tag vX.Y.Z
git push origin vX.Y.Z
```

The tag must be `vX.Y.Z` (or `vX.Y.Z-rc.N` for a pre-release). The
`release` workflow first runs `scripts/check-version.sh`, which refuses
to release when the tag disagrees with `Cargo.toml` or the newest
`<release>` in the metainfo.

## What CI does

1. **validate** — tag ↔ Cargo.toml ↔ metainfo version agreement.
2. **tarball** — native `cargo build --release --features gui` on
   `ubuntu-24.04` (x86_64) and `ubuntu-24.04-arm` (aarch64), packaged by
   `scripts/package-release.sh`.
3. **flatpak** — the shared `flatpak.yml` workflow, same two runners,
   producing versioned `.flatpak` bundles.
4. **release** — downloads every artifact, runs
   `scripts/verify-release.sh` (presence, sizes, tar/ELF arch, bundle
   arch, SHA256SUMS), then creates the GitHub Release and re-verifies all
   five assets are attached.

A failed architecture job blocks the release — you never get a partial
"x86-only" release.

## Release assets

- `gosh-appimage-manager-X.Y.Z-linux-x86_64.tar.gz` / `-aarch64.tar.gz`
- `gosh-appimage-manager-X.Y.Z-linux-x86_64.flatpak` / `-aarch64.flatpak`
- `SHA256SUMS`

## Re-running

Push the same tag again after deleting it, or use *Actions → Release →
Run workflow* with an existing tag. If the release already exists the
workflow re-uploads assets over it (`--clobber`) instead of creating a
duplicate.

## Local dry-run

```sh
cargo build --release --features gui
scripts/package-release.sh X.Y.Z x86_64      # writes the tarball here
scripts/verify-release.sh <dir> X.Y.Z        # checks a full artifact dir
```
