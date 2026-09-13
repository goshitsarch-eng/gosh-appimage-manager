# Release pipeline report

Date: 2026-09-13. Repo: `goshitsarch-eng/gosh-appimage-manager` (private).

## Configuration

- **Trigger:** tag push `v*` (`release.yml`), or Actions → Release → Run
  workflow with an existing tag. `ci.yml` and `flatpak.yml` run on
  push-to-main/PRs; `flatpak.yml` is also the reusable build for releases.
- **Version source:** `Cargo.toml` `version`, cross-checked by
  `scripts/check-version.sh` against the newest metainfo `<release>`.
- **Runners:** `ubuntu-24.04` (x86_64), `ubuntu-24.04-arm` (aarch64).
  aarch64 builds are fully native — qemu was removed.
- **Permissions:** `contents: read` everywhere except the `release` job,
  which holds `contents: write` for `gh release`.
- **Artifacts:**
  `gosh-appimage-manager-<ver>-linux-<arch>.{tar.gz,flatpak}` + `SHA256SUMS`.
- **Tarball:** `cargo build --release --features gui` →
  `scripts/package-release.sh` (intentional share/bin layout + install.sh).
- **Flatpak:** `flatpak-builder` on freedesktop 23.08 with vendored cargo
  sources; in-job `file` arch check + packaged `--self-test`, then
  `flatpak build-bundle`.
- **Checksums:** `scripts/verify-release.sh` regenerates `SHA256SUMS`
  after presence/size/integrity/arch checks pass.

## Runs

| Run | Event | Result | Notes |
|---|---|---|---|
| 34781621931 | push main | success 19m37s | First native aarch64 flatpak on `ubuntu-24.04-arm` (was ~4h under qemu); arch check + packaged self-test green both arches |
| 34781621958 | push main | success 9m53s | `ci.yml` fmt/clippy/test/self-test |
| 34783086836 | tag `v3.0.0-rc.1` | success 19m45s | Full release pipeline; created pre-release with 5 assets |
| 34784367432 | dispatch `v3.0.0-rc.1` | success | Idempotent re-run: existing release → `upload --clobber`, no duplicate |

## Verified on the produced release (`v3.0.0-rc.1`)

- Release `Gosh AppImage Manager 3.0.0-rc.1`, prerelease=true, draft=false,
  exactly 5 assets, none zero-byte.
- Downloaded all assets; `verify-release.sh` passed:
  both tarballs contain matching-arch ELF binaries, both bundles declare
  the right arch; `sha256sum -c SHA256SUMS` all OK.
- x86_64 tarball: extracted, `--version` → 3.0.0, `--self-test` →
  `SELF_TEST_OK`; `install.sh` installed into a scratch prefix and the
  installed binary runs.
- x86_64 Flatpak: `flatpak --user install` from the bundle, `flatpak run`
  `--self-test` → `SELF_TEST_OK` inside the sandbox.
- aarch64 Flatpak: ostree checkout of the bundle — `gosh-appimage-manager`,
  `7zz`, `unsquashfs` all `ARM aarch64` ELF. (Not executed: no aarch64
  host; the arm runner already ran the packaged self-test natively.)

## Failures encountered and fixed

1. **Committed a 27 MB test tarball** to main (packaging script wrote into
   the repo root before `out_dir` was captured). Removed in c3d2136;
   script now writes to the caller's directory.
2. **Reusable `flatpak.yml` lacked a `ref` input** — a dispatched release
   would have built flatpaks from `main` HEAD while tarballs came from the
   tag. Added `ref` input, passed by `release.yml`.
3. **Tag→shell injection surface:** `";` are legal in refnames; the
   version string was interpolated into `run:` scripts.
   `check-version.sh` now enforces `vX.Y.Z[-rc.N]` and the version flows
   through `env:` in run steps.
4. **`upload-artifact` v4 immutability** → re-runs would 409; added
   `overwrite: true` to both upload steps.
5. **Update path dropped release metadata** — `--clobber` upload left
   title/prerelease stale; `gh release edit` now refreshes them.
6. **`install.sh` non-idempotent** `licenses` copy (nested dir on
   reinstall) → `mkdir -p` + `cp -a licenses/.`.
7. **`verify-release.sh` SIGPIPE** under `pipefail`: `strings | head`
   failed even on match → `head -c 8192` before `strings`.

## Known limits

- `jlumbroso/free-disk-space@main` is unpinned (pre-existing repo choice;
  required for the ~15 GB flatpak build).
- aarch64 install/run is verified by the arm runner's packaged self-test
  and bundle inspection, not by an aarch64 login session.
- No Flathub publication — bundles attach to the GitHub Release.
