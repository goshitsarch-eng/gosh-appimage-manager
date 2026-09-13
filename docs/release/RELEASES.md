# GitHub Releases

## Trigger

`git tag vX.Y.Z && git push origin vX.Y.Z`. Pre-release tags
(`vX.Y.Z-rc.N`) produce a GitHub *pre-release*. Manual re-runs: Actions →
Release → Run workflow with an existing tag.

`scripts/check-version.sh` gates the whole pipeline: the tag's base
version must equal `version` in `Cargo.toml` and the newest
`<release version>` in `data/com.goshapps.AppImageManager.metainfo.xml`.
Mismatch → fail before anything builds.

## Publication

The `release` job (only job with `contents: write`) runs `gh` with the
job's `GITHUB_TOKEN`:

- release absent → `gh release create <tag> dist/* --title "Gosh AppImage
  Manager X.Y.Z" --generate-notes` (`--prerelease` for `-` versions)
- release exists → `gh release upload <tag> dist/* --clobber`

That keeps re-runs idempotent: no duplicate releases, assets overwritten
in place.

## Required assets (enforced)

| Asset | Check |
|---|---|
| `…-linux-x86_64.tar.gz` | tar integrity, ELF `x86-64` inside |
| `…-linux-aarch64.tar.gz` | tar integrity, ELF `ARM aarch64` inside |
| `…-linux-x86_64.flatpak` | `flatpak` magic, declares `x86_64` |
| `…-linux-aarch64.flatpak` | `flatpak` magic, declares `aarch64` |
| `SHA256SUMS` | generated over the four files |

Missing, wrong-arch, or zero-byte assets fail the job. After upload the
job lists the release's assets and asserts exactly these five exist —
a partial or duplicated release is a failure, not a warning.

## Failure behavior

`needs:` chains mean a broken aarch64 or x86_64 job prevents any release
from being created. Nothing is published until all four artifacts are
built, verified, and collected.
