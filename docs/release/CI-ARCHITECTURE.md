# CI architecture

Three workflows, one reusable:

| File | Trigger | Jobs |
|---|---|---|
| `ci.yml` | push to main, PRs, dispatch | fmt · clippy (both feature sets) · build · test · self-test |
| `flatpak.yml` | push to main, PRs, dispatch, `workflow_call` | per-arch Flatpak build → bundle → artifact |
| `release.yml` | tags `v*`, dispatch | validate → tarball + flatpak → release |

`release.yml` calls `flatpak.yml` as a reusable workflow with a `version`
input, so the Flatpak build exists once and both CI artifacts and release
bundles come from the same code path. With no `version` the bundle is
named `gosh-appimage-manager-<arch>.flatpak` (CI); with a version it is
`gosh-appimage-manager-<ver>-linux-<arch>.flatpak` (release).

## Runners

| Arch | Runner | Why |
|---|---|---|
| x86_64 | `ubuntu-24.04` | stable hosted image |
| aarch64 | `ubuntu-24.04-arm` | native arm64; no qemu, no cross |

Both are `runs-on: ${{ matrix.arch == 'aarch64' && 'ubuntu-24.04-arm' || 'ubuntu-24.04' }}`.
`fail-fast: false` everywhere so one arch's failure stays diagnosable.

## Release flow

```
tag vX.Y.Z ──► validate (tag == Cargo.toml == metainfo)
                 │
        ┌────────┴────────┐
        ▼                 ▼
   tarball (matrix)   flatpak (reusable workflow, matrix)
   x86_64 + aarch64   x86_64 + aarch64
        │                 │
        └────────┬────────┘
                 ▼
   release: download-artifact → verify-release.sh →
   gh release create/upload --clobber → re-verify 5 assets
```

The release job `needs:` all three, so any arch failure aborts
publication. Artifacts travel between jobs as GitHub Actions artifacts
named after their final filenames; `download-artifact` merges them into
`dist/` by pattern.

## Verification inside the pipeline

- `check-version.sh` — tag vs `Cargo.toml` vs newest metainfo `<release>`.
- `file` on the built binary in every build job (tarball job via
  `package-release.sh`, Flatpak job as its own step).
- `flatpak-builder --run … --self-test` on the packaged app per arch.
- `verify-release.sh` before publication: names, sizes, `tar -tzf`
  integrity, embedded ELF arch per tarball, `flatpak` magic + declared
  arch per bundle, then writes `SHA256SUMS`.
- Post-publish: `gh release view` asserts exactly the 5 expected assets
  and no zero-byte files.

## Permissions

Top-level `contents: read` everywhere. Only the `release` job gets
`contents: write` (for `gh release`). Third-party actions:
`actions/checkout@v4`, `actions/cache@v4`, `actions/upload-artifact@v4`,
`actions/download-artifact@v4`, `jlumbroso/free-disk-space@main`
(pre-existing repo choice; needed for the ~15 GB Flatpak build).

## Caching

`actions/cache` holds `~/.cargo/registry` + `~/.cargo/git` only, keyed by
OS + runner arch + `Cargo.lock`. `target/` is never cached — release
binaries are always built clean, and nothing can leak between
architectures.
