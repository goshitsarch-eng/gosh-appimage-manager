# Security (Phase 2)

Threat model (per `AGENTS.md`): every AppImage, desktop file, icon, update
descriptor, URL, archive entry, and process output is untrusted. Prior-round
fixes S-1…S-14 verified present in this tree by inspection + green suites
(`test_ssrf`, `test_network`, `test_ftp`, `test_update`, `test_inspector`).

## Controls in place

| Area | Control | Evidence |
|---|---|---|
| SSRF: resolved address | connect pinned to resolved IP **and** IP class checked | `network.rs`, `test_ssrf` real-socket test green |
| SSRF: redirects | every hop re-checked before follow; pin per hop | `network.rs:check_redirect` on all 3 methods |
| SSRF: private opt-in | `allow_local_network` only on user-created sources; embedded metadata can never set it | `updates_sources.rs`, `test_ssrf` |
| SSRF: literal coverage | mapped/NAT64/CGNAT/trailing-dot localhost handled | `url_guard.rs:is_local_ip/hostname`, matrix test |
| Transport | https→http downgrade refused on payload path | `network.rs` redirect policy |
| Credentials in URL | rejected before any packet | `url_guard.rs:50-53`, `credentials_in_url_fail_without_network` |
| Archive extraction | `..`/absolute/symlink-escape rejected; `-`-leading members refused (argv injection) | `safe_fs.rs:valid_archive_member`, `argv_safe_path` |
| Temp dirs | pre-existing path verified (dir, not symlink, safe mode) | `safe_fs.rs:mkdir_0700` + squatting tests |
| Exec construction | program + argv tokens only; `%` escaped in desktop Exec | `desktop.rs`, `process.rs`, percent test |
| No shell strings | `process.rs` spawns argv arrays only (searched: no `sh -c`) | `process.rs` |
| Trash safety | trash failure returns before cleanup; never becomes delete | `removal.rs:117-120`, test |
| Delete guards | protected paths + symlinks refused; extra confirmation | `removal.rs:100-116`, test |
| Ownership | mutations gated on ownership markers / explicit adopt | `desktop.rs:271-289`, `removal.rs` |
| Update authenticity | sha256:/bare-hex parsed, mismatch refused; uninterpretable refused | `updates_service.rs`, `test_update` |
| Update arch compat | foreign-arch payload refused | `updates_service.rs`, dedicated test |
| Streaming download | to 0600 temp, bounded, cancellable; oversized refused without buffering | `network.rs:stream_to_file`, tests |
| Atomic replace + rollback | validate-then-rename; rollback material kept till success | `updates_service.rs`, `test_rollback` |
| Running-app guard | Flatpak-safe via host-spawn probe; probe failure = "cannot tell", never "not running" | `proctable.rs` + host lookup, fail-safe test |
| Unsafe fallback | off by default; per-file confirm; never in tests/background | `inspector.rs`, `settings.rs` |
| Registry perms | `registry.sqlite` 0600 enforced incl. pre-existing files | `registry.rs`, S-14 test |
| Bounded everything | size/extraction/output/JSON/download/redirect/timeout caps | `limits.rs`, `safe_fs.rs`, `network.rs` |
| Forge pinning | forge assets must come from the serving forge | `updates_sources.rs`, `forge_assets_must_come_from_the_forge…` |
| Glob DoS | linear matcher | `updates_sources.rs`, `test_glob` |
| Zombie reaping | detached children reaped; no `mem::forget` leak | `process.rs`, zombie-count test |
| Zero telemetry | network only to configured update endpoints on check/apply | `cli.rs`, `updates_service.rs` |

## Residual risks

### SEC-01 — `--talk-name=org.freedesktop.Flatpak` host-execution grant (accepted, P3 future work → PLAN-005)
- The sandbox is not a containment boundary for a compromised manager:
  the grant permits `flatpak-spawn --host`. It cannot be removed without
  removing AppImage launching (the app's purpose).
- Mitigation in place: host execution constrained to six named helpers
  plus registry-resolved paths re-checked as regular files (commit
  `2accedd`); env forwarding uses `--env=` (no shell).
- Future reduction: portal Trash/OpenDirectory would remove two helpers;
  deferred for lack of a portal service to verify against. Tracked as
  PLAN-005 (P3): re-attempt only where portals are exercisable.

### SEC-02 — No human review of translations pipeline risk (low)
- Only `qps.json` ships; catalog loader parses JSON with size bounds
  (`i18n.rs:128`). Malicious catalogs can at most alter displayed strings
  locale-side. No action.

### SEC-03 — FTP plaintext (documented, accepted)
- FTP is an explicit legacy option with an insecure-transport warning;
  credentials rejected. Documented in README. No action.

## Out of scope / not assessed

- Real-compositor GUI input handling (no compositor here) — PLAN-001.
- Fuzzing of ELF/inspector parsers beyond unit fixtures — none; bounds +
  no-exec make this low priority.
- `cargo audit`: prior round reported 0 vulnerabilities; not re-run here
  (network-light session) — re-run in CI/release.
