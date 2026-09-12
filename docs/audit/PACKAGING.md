# Packaging (Phase 2)

## Flatpak

- Manifest: `packaging/com.goshapps.AppImageManager.yml` —
  `org.freedesktop.Platform//23.08`, single manifest for x86_64 + aarch64.
- Sandbox grants: IPC, X11-fallback + Wayland, DRI, network, Flatpak +
    portal + notification talk-names, own-name, and narrow `create`
  filesystems (`~/AppImages`, applications, icons, app data,
  `xdg-config/autostart`). **No `--filesystem=host:rw`.**
- Toolchain: pinned Rust 1.90.0 per arch (SHA-256), removed from final
  `/app` by top-level `/rust` cleanup; `no-debuginfo: true` (23.08
  splitter corrupts rustc 1.90 libLLVM).
- Helpers built/pinned per arch with sources + licences installed:
  unsquashfs (compiled), 7zz 26.00, dwarfs 0.15.3.
- Vendoring: `packaging/cargo-sources.json` regenerated with the official
  generator against committed `Cargo.lock` (migration round fixed the
  missing `iced_sctk` entry); module builds with `CARGO_NET_OFFLINE=true`.
- Installs: binary, desktop, metainfo, both icons, i18n catalogs
  (`install -Dd` + `-t` split — the old glob form silently failed),
  COPYING/AUTHORS, third-party licences. In-builder `--self-test` gate.

### Status (observed)

- x86_64: **built in this tree** — `build-dir/files/bin/` contains
  `gosh-appimage-manager`, `7zz`, `dwarfsck`, `dwarfsextract`,
  `unsquashfs`. Sandboxed `--self-test` → `SELF_TEST_OK` per
  `docs/migration/REPORT.md`.
- aarch64: manifest unchanged in arch coverage; built in CI (qemu-user,
  `CARGO_BUILD_JOBS=4`); **not rebuilt locally** (no qemu here) → verify
  via CI (PLAN-001/PLAN-010 context).
- `verify.sh` chains the x86_64 build; needs `--disable-rofiles-fuse`
  where fusermount is denied (already in script).

## Desktop / AppStream (observed this session)

- `desktop-file-validate` → pass. `Exec=%F` (file paths; C-9 fixed).
- `appstreamcli validate --pedantic --no-net` → **successful** with one
  pedantic info: `cid-contains-uppercase-letter` — inherent to the shipped
  id `com.goshapps.AppImageManager`; accepted, no action.

## Release hygiene

- `scripts/verify.sh`: build → test (`--no-fail-fast`) → both clippy gates
  → fmt → isolated-HOME self-test → desktop/AppStream validation → GUI
  smoke → Flatpak x86_64. Missing prerequisites skip loudly, never fake.
- CI (`.github/workflows/flatpak.yml`): host (not container) build for
  disk space, both arches, fail-fast off, `--repo=repo` for bundling.
- `.gitignore` covers build outputs; the old committed OSTree repo
  (`repo-x86_64/`) was untracked (H-13 fixed) and is absent.
- Pinned upstream reference for behaviour: Gear Lever `a2917f2`
  (reference only; nothing copied — see `docs/upstream-audit.md`).

## Gaps

- Vendored sources must be regenerated after any lock change
  (`just vendor …`) — process risk, documented in justfile + rewrite notes.
- `cargo audit` cadence: last reported clean (0 vulns) in prior round;
  re-run at release (PLAN-010).
