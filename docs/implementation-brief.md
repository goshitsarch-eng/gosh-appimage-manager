# Gosh AppImage Manager implementation brief

> **Status: superseded on stack, current on behaviour.**
>
> This is the original mission statement, written for the 2.x
> C++20/Qt 6/KF6/Kirigami implementation. That stack was retired: 3.0.0 is
> Rust + libcosmic, and there is no Qt, KDE Frameworks, CMake, QML or ECM
> anywhere in the tree. Section 1's stack and runtime lines, section 3's Qt
> type names, section 9's KConfig, section 10's KDE Platform runtime, and
> section 13's Qt Test are all obsolete — see `docs/rewrite-3.0.0.md` for the
> module-by-module map, and `docs/verification.md` for what has actually been
> verified.
>
> The version line below reads `0.1.0`; the shipping version is `3.0.0`.
>
> Everything else — the required user experience, the safety contract, the
> validation rules, the update semantics, the CLI surface, and the acceptance
> criteria — still governs, and `AUDIT.md` tracks the project against it.

## 1. Mission and identity

Build a production-quality standalone application named **Gosh AppImage Manager**: a native KDE application for safely inspecting, integrating, launching, organizing, updating and removing AppImages.

- Application ID: `com.goshapps.AppImageManager`
- Executable: `gosh-appimage-manager`
- Repository: `gosh-appimage-manager`
- Version: `0.1.0`
- Public identity: **Gosh-Its-Arch**
- License: `GPL-3.0-or-later`
- Stack: C++20, Qt 6, KDE Frameworks 6, Kirigami, CMake/ECM
- Runtime/SDK: `org.kde.Platform//6.10`, `org.kde.Sdk//6.10`
- Product appearance: genuinely native Plasma/KDE, adaptive Kirigami, System/Light/Dark modes

Gear Lever at pinned commit `a2917f2adafc78e0478e47d5843de9ede6c1aa3f` is a GPLv3 behavioral reference. This is an original implementation. Do not copy its Python/GTK code, templates, CSS, icons, screenshots, app ID or branding. Credit Gear Lever and Lorenzo Paderi in README, AUTHORS and About as inspiration/behavioral reference.

## 2. Required user experience

### Application shell

Use `Kirigami.ApplicationWindow` with responsive navigation suitable for desktop and narrow windows. Required destinations:

1. **Library**: integrated AppImages with search, sorting, update/running/external-folder badges and useful empty/error/loading states.
2. **Updates**: available updates, selection, update-all, per-item progress, cancel and a completion summary.
3. **Tasks**: active/recent integration, update, metadata and removal operations with progress/errors/retry where safe.
4. **Settings**: integration folder, copy/move behavior, manage outside folder, terminal executable naming, background checks, unsafe extraction fallback, appearance and diagnostics/log controls.
5. **About**: version, authorship, Gear Lever attribution, GPL notice/warranty and viewable license text.

Provide a prominent **Open AppImage** action, file chooser, open-with support and drag/drop for one or multiple local AppImages. Opening an AppImage shows an inspection/confirmation flow; it never immediately integrates or executes it.

### Inspection/import flow

For each candidate show:

- original path and size
- detected AppImage type and ELF architecture
- name, version, comment/summary, terminal mode and icon if safely extracted
- embedded update source status
- whether it appears already managed
- architecture incompatibility or unsupported-format warnings
- copy versus move outcome and target folder
- conflict choice: keep both or replace a specifically identified managed installation

Multi-file import must inspect independently and require a single explicit confirmation listing each mutation. Failures are per-item and do not corrupt successful items.

### Library details

An integrated item detail page must support:

- launch using a start-only detached process path
- reveal in file manager
- check update / update now
- edit command arguments as an argument list, not a shell fragment
- edit validated environment variable name/value pairs
- configure/reset an update source
- refresh safely extracted metadata
- show file path, desktop ID, file hash, AppImage type, architecture, version, size, update manager and source provenance
- remove to Trash by default
- permanently delete only through an explicit destructive secondary flow

Do not show raw untrusted HTML. Do not load arbitrary remote icons.

## 3. Core architecture

Keep UI and side effects separated and testable. A suitable design includes:

- immutable/value types for candidate, installed app, metadata, update source, task and result
- `AppImageInspector` for bounded validation and metadata extraction
- `ManagedRegistry` for configuration/ownership state
- `DesktopIntegration` for `.desktop` and icon lifecycle
- `AppImageLibrary` for discovery/adoption and models
- `UpdateService` plus update-source implementations
- `TaskQueue` for serialized conflicting mutations and cancellable non-conflicting reads
- argument-array `ProcessRunner` abstraction with fake implementation for tests
- bounded `NetworkClient` abstraction with fake implementation for tests
- `StoreController`/application controller exposing Qt models and commands to QML
- separate CLI frontend using the same production services, not duplicate logic

Long-running disk, extraction, process, hash and network work must not block the GUI thread. Thread ownership, cancellation and shutdown must be deterministic. Never destroy a live QProcess/QNetworkReply/QThread. Never use `QThread::terminate()`.

## 4. Candidate and AppImage validation

Treat all candidates as hostile.

- Require a local regular file, reject directories/devices/FIFOs and unsafe symlink resolution.
- Apply configurable but bounded file-size policy; do not read the entire file merely to inspect it.
- Validate ELF magic/class/endian/machine where applicable and AppImage magic at the defined offset; MIME/extension alone is insufficient.
- Detect Type 1 and Type 2, x86_64 and aarch64 at minimum. Clearly report unknown/unsupported architecture.
- Hash using streaming SHA-256 with cancellation.
- Do not execute an untrusted AppImage for metadata by default.
- Extract only required root metadata (`*.desktop`, `.DirIcon`, referenced PNG/SVG) into a private temporary directory.
- Support safe extractor paths for SquashFS/Type 2 and available Type 1/DwarFS tooling. Package pinned tools or sources with hashes/licenses where required.
- Bound archive listing/extraction count, output bytes, path length, nesting, decompressed bytes and timeout. Reject absolute paths, `..`, device nodes and escaping symlinks.
- Parse desktop metadata with strict limits. Sanitize `Name`, `Comment`, `Icon`, `Exec`, actions and custom keys. Ignore dangerous/unknown action execution.
- Unsafe direct `--appimage-extract` fallback is disabled by default. If enabled in Settings, show an explicit warning that it executes untrusted code and require per-file confirmation; never use it in tests or automatic/background flows.
- Temporary directories are mode 0700 and removed safely without following symlinks.

## 5. Integration and ownership

Default managed folder: `~/AppImages`, configurable to a user-selected directory. Persist settings with `KConfig`/`KConfigXT` or an equally native KDE mechanism.

Integration must be transactional:

1. validate and inspect source
2. select a collision-free final name
3. create private staging file in the destination filesystem
4. copy with byte limits/cancellation, chmod only the staged copy, flush and verify size/hash
5. generate icon and desktop entry in staging
6. atomically place the AppImage and owned integration artifacts
7. update registry only after filesystem success
8. refresh the desktop database/model
9. delete/move the source only after final verification and only when the setting and user confirmation allow it
10. roll back all newly created artifacts on failure without touching pre-existing files

Desktop entries must use a stable Gosh-owned ID and ownership markers, for example `X-Gosh-AppImage-Manager=true`, a stable UUID and canonical managed path. Use correct Desktop Entry escaping and field-code handling. Build `Exec` from validated program, environment and argument tokens; no shell evaluation. Set `TryExec` to the managed path. Sanitize names and icon paths. Desktop actions from the source may only be retained after safe argument-token rewriting.

Never overwrite or remove an arbitrary desktop file/icon. Mutate only artifacts proven owned in the registry and markers. Existing Gear Lever or manually integrated AppImages may be discovered as external/unmanaged and offered for explicit adoption. Adoption must not silently rewrite or delete anything.

Name conflicts require explicit keep-both or replace semantics. Replace may target only a specific managed item and must preserve a rollback copy until the transaction commits.

## 6. Removal and launch safety

- Default removal moves the managed AppImage to Trash using a host/portal-safe operation and then removes only owned desktop/icon/config artifacts after Trash succeeds.
- If Trash fails, report failure and leave everything intact. Never fall back automatically to permanent deletion.
- Permanent deletion requires explicit destructive confirmation naming the exact file, verifies canonical path/ownership, refuses roots/home/top-level directories and does not follow symlinks.
- Remove-all is CLI-only or deeply confirmed; it affects only owned managed items.
- Launch via detached, start-only program+argument array. Report success only when process start succeeds. Never wait five seconds and kill the launched application.
- Before update/replacement, detect the exact executable path in running processes. Default is to block; force override must be explicit.
- NixOS may use `appimage-run` when available and clearly report when missing.

## 7. Update system

Support embedded `.upd_info` and configurable managers with a common interface:

- Static HTTPS file
- GitHub releases
- GitLab releases/packages
- Codeberg
- Forgejo
- FTP only as an explicit legacy option with an insecure-transport warning; never accept credentials in URLs or persist secrets

At minimum understand embedded `gh-releases-zsync|...` and `zsync|https://...` metadata. Reading `.upd_info` uses bounded argument-array tooling or an internal ELF parser.

Network rules:

- HTTPS by default; bounded redirects, timeout, JSON/body bytes and download size
- reject `file:`, `data:`, `javascript:`, embedded credentials, NUL/control characters and HTTPS-to-HTTP downgrade
- guard loopback, link-local and private-network destinations unless an explicit user-created source opts in with warning
- validate API host per manager and safely encode path components
- bounded concurrency and cancellation
- stream downloads to mode-0600 temp files; never buffer an AppImage wholly in memory
- use advertised SHA-256/digest or zsync SHA-1 when available; otherwise compare ETag/size/version and show reduced-verification status

Applying an update must:

1. refuse a running app unless force was explicitly chosen
2. download to private staging
3. validate regular file, AppImage magic/type and compatible architecture
4. hash and inspect metadata safely
5. preserve custom arguments/environment and update configuration
6. atomically exchange/replace the managed file and integration metadata
7. retain rollback material until all validation and desktop integration steps pass
8. restore the prior version on any failure
9. never delete the working installation merely because a network or metadata step fails

Update-all is serialized, cancellable between items, reports per-item results and does not let one failure corrupt others. Background checks are opt-in, non-mutating, notify only, and must not download/apply updates.

## 8. CLI parity

The executable must provide GUI mode plus these non-GUI options using the same production services:

- `--integrate <path>` with `--keep-both`, `--replace`, `--yes`
- `--update <path>` or `--update --all`, with `--yes` and `--force`
- `--remove <path>` with `--yes` and explicit `--delete`
- `--remove-all` with `--yes`
- `--list-installed [--json]`
- `--list-updates [--json]`
- `--list-update-managers`
- `--set-update-source <path> --manager <name> key=value...` and `--unset`
- `--fetch-updates` for non-mutating background checking/notification
- `--self-test`
- non-mutating diagnostic probes useful for package verification

Interactive destructive CLI actions require a TTY confirmation unless `--yes`; fail closed without a TTY. `--json` documents use `schema_version: 1` and stable fields for name, path, desktop ID, current/available versions, download size, manager, embedded source and running state. Diagnostics go to stderr so stdout remains valid JSON.

Return meaningful exit codes. No GUI startup for CLI commands. Paths and values are literal arguments, never shell-parsed.

## 9. Settings and configuration

Required settings:

- managed AppImages folder (default `~/AppImages`)
- copy or move source after successful integration
- manage/discover outside-folder entries
- terminal apps may omit `.AppImage` suffix
- background update checks (off by default)
- unsafe extraction fallback (off by default with warning)
- appearance: System, Light, Dark
- debug logging (off by default)

Per-app settings: stable ownership UUID, canonical path, desktop/icon artifacts, default and customized argument tokens, validated environment pairs, website, update manager/config and last check state. Use atomic durable writes and schema/versioning. Do not log secrets, complete query strings or private tokens.

## 10. Flatpak packaging

Provide a buildable manifest at `packaging/com.goshapps.AppImageManager.yml`.

- KDE Platform/SDK 6.10
- Wayland, fallback X11, IPC, DRI and network as needed
- exact D-Bus permissions only
- `--talk-name=org.freedesktop.Flatpak` is allowed for argument-safe `flatpak-spawn --host`
- avoid `--filesystem=host:rw`
- grant only managed folder/config/data paths necessary for normal operation; use portals/document paths and safe host operations for user-selected external files
- package required extraction tools from pinned sources/artifacts with SHA-256 and license compliance
- no secrets or mutable unpinned downloads
- export desktop file, icons, metainfo and supported AppImage MIME/open-with declarations

Also provide native build/install instructions. The package must work from a clean checkout.

## 11. Branding, metadata and legal

Create original professional Gosh AppImage Manager artwork, scalable and symbolic, visually related to Gosh Apps but not copied from Gear Lever. Do not use generated text inside the icon.

Provide:

- desktop file with `Exec=gosh-appimage-manager %U`, AppImage MIME types and sensible KDE categories
- AppStream metainfo with name, summary, long description, screenshots or a deterministic original mock screenshot, releases, content rating, URLs and developer identity Gosh-Its-Arch
- `COPYING`, `AUTHORS`, README and About legal notice
- Gear Lever attribution that makes clear this is independent and not endorsed by its authors
- GPL appropriate legal notices and viewable license text in the UI

## 12. Accessibility and polish

- keyboard reachable actions and predictable focus
- accessible names/descriptions for icon-only controls
- tooltips for destructive or ambiguous controls
- no color-only state communication
- scalable layout/text and no hard-coded pixel assumptions that break at 200% scaling
- honor reduced animations where available
- confirmation dialogs state exact effects and paths
- light/dark/system behavior must work under Plasma

## 13. Testing requirements

Use Qt Test and fake process/network/filesystem seams. Tests must not mutate the real home directory, execute an AppImage, launch a user app, or call live update APIs.

Required coverage:

- ELF/AppImage magic, type and architecture parsing, truncated/malformed files
- bounded streaming hash and cancellation
- desktop parser/sanitizer/Exec token rewriting and environment validation
- archive entry traversal, absolute path, symlink escape, count/size/decompression limits
- extraction fallback remains disabled without explicit consent
- collision naming, ownership markers, managed/unmanaged/adoption distinction
- transactional copy/move, rollback, replace rollback and source preservation on failure
- Trash failure never becomes delete; permanent delete containment/ownership rules
- launch uses start-only detached path and never kills a launched app
- running-process update guard
- embedded update-info parsing and each manager's URL/config validation
- redirects/downgrade/private-host/credentials/size/timeouts/cancellation
- update download validation, atomic replacement, rollback and preservation of settings
- task serialization, cancel, shutdown and no late callbacks/live QObjects
- CLI argument parsing, confirmations, exit codes and exact JSON schema
- model roles/readiness, search/filter and meaningful `--self-test`
- Flatpak host-path visibility and non-mutating package probes

Include malformed/adversarial fixtures. Capture Qt warnings in lifecycle tests and fail on live QProcess/QNetworkReply/QThread destruction.

## 14. Required verification evidence

Before completion, run and document real output in `docs/verification.md`:

1. clean Qt 6.10/KF6 CMake configure and Ninja build in the KDE SDK
2. full CTest, 100% passing
3. recursive `qmllint` over every shipped QML file
4. `desktop-file-validate`
5. explicit `appstreamcli validate --pedantic --no-net --explain`
6. clean `flatpak-builder` build from the manifest
7. native/SDK offscreen `--self-test`
8. packaged offscreen `--self-test`
9. packaged non-mutating host integration probe
10. packaged non-mutating synthetic AppImage inspection probe proving no execution/mutation
11. `git diff --check`
12. clean worktree and cohesive commit

Do not claim real integration/update/removal unless safely proven in a disposable temporary HOME with synthetic fixtures. Never touch the user's real AppImages, desktop entries or configuration during verification.

## 15. Acceptance criteria

The task is complete only when the repository contains a coherent working native app—not a mock, plan or shell around Gear Lever—with production services, adaptive UI, CLI, tests, packaging and verification. Every critical path must either work or be explicitly and honestly documented as a non-blocking limitation. No stubbed success, fabricated output, TODO-only feature, unsafe shell execution, broad unreviewed permissions, or release-blocking lifecycle flaw is acceptable.
