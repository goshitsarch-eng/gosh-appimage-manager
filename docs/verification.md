# Verification record

All commands below were executed on 2026-08-28 in this worktree. Output is
summarised from the real tool transcripts; nothing here is fabricated.

Host: Ubuntu, `/root/projects/gosh-appimage-manager-grok`, branch
`grok/appimage-manager`.
SDK: `org.kde.Sdk//6.10` and `org.kde.Platform//6.10` from Flathub.

No command mutated the operator's real `~/AppImages`, desktop entries, icons,
or application configuration. Mutation tests used `QStandardPaths` test mode
and isolated temporary directories. Inspection never executed an AppImage.

## 1. CMake configure and Ninja compile (KDE 6.10 SDK)

```
flatpak run --filesystem=/root/projects/gosh-appimage-manager-grok --share=network \
  --devel --command=bash org.kde.Sdk//6.10 \
  -lc 'cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo && cmake --build build'
```

Configure succeeded (ECM 6.10, Qt 6.10.3, KF6 6.27.0 including Kirigami,
I18n, Config, DBusAddons). Ninja linked `build/bin/gosh-appimage-manager`
and the test binaries.

## 2. CTest

```
QT_QPA_PLATFORM=offscreen ctest --test-dir build --output-on-failure
```

(run inside the SDK)

```
100% tests passed, 0 tests failed out of 15
Total Test time (real) =   6.08 sec
```

Tests: `appstreamtest`, `test_elf`, `test_hash`, `test_desktop`,
`test_archive`, `test_inspector`, `test_integration`, `test_removal`,
`test_launch`, `test_update`, `test_cli`, `test_tasks`, `test_models`,
`test_network`, `test_controller`.

ECM `appstreamtest` reported PASS because the helper is not installed. That
skip is **not** treated as metainfo proof. Metainfo proof is the explicit
`appstreamcli validate --pedantic --no-net --explain` command below.

Production-path coverage that these tests actually exercise:

- ELF/AppImage magic, type 1/2, x86_64/aarch64/i386, truncated and missing magic
- streaming SHA-256, cancellation, and size bounds
- desktop parser, Exec token rewriting, environment name/value validation
- archive absolute/`..`/symlink-escape/count/size bounds, 7z and DwarFS listing parsers
- inspector extractors list then filter; unsafe paths never reach extract args;
  candidate path is never the executed program without setting+consent
- transactional integrate copy/replace; desktop-install and registry-save
  failures restore prior AppImage/desktop/registry bytes; move-source failure
  is reported; no `.gosh-*` orphans
- missing executable reconciles registry; permanent-delete containment for `/` and `$HOME`
- launch is start-only detached; running-process guard uses a real temp file
- URL guards reject `file:`, credentials, and private/loopback hosts
- GitHub API host validation; advertised digest mismatch leaves the install;
  matching digest replaces and keeps arguments; FTP `allowFtp`; static same
  ETag is not an update; update desktop-install failure restores bytes
- QtNetworkClient local HTTP: size abort, timeout, cancellation; resolver
  rejects private/rebind addresses
- CLI `--list-installed --json` stdout parsed for `schema_version` and
  `installed` (not `items`); `--integrate` without TTY needs confirmation;
  `--replace --replace-uuid` succeeds for owned and refuses missing UUID
- task mutation serialisation, cancel, join-safe shutdown, no live-thread
  destruction warnings
- inspectPaths returns before completion; checkAll does not target empty UUID

## 3. QML lint

```
find src/qml -name '*.qml' -print0 | xargs -0 qmllint
```

(run inside the SDK; recursive)

qmllint reported unqualified-access warnings for `i18n` and context-property
`Store` (expected without the runtime KLocalizedContext) and no errors.
Exit status 0.

## 4. Desktop and AppStream

```
desktop-file-validate data/com.goshapps.AppImageManager.desktop
```

Exit 0. Hint only: `Categories` contains more than one main category
(`System` and `Utility`).

```
appstreamcli validate --pedantic --no-net --explain data/com.goshapps.AppImageManager.metainfo.xml
```

```
P: com.goshapps.AppImageManager:3: cid-contains-uppercase-letter com.goshapps.AppImageManager
   The component ID should only contain lowercase characters.

✔ Validation was successful: pedantic: 1
```

The uppercase `AppImageManager` segment is the binding application ID and is
kept. Screenshot metadata is present; the original PNG is
`data/screenshots/library.png`.

## 5. Native/SDK offscreen `--self-test`

```
QT_QPA_PLATFORM=offscreen HOME=/tmp/gosh-aim-selftest-home3 \
  build/bin/gosh-appimage-manager --self-test
```

Exit 0. Models and services probed, synthetic catalog row loaded, QML `Main`
loaded with `objectName: mainWindow`. Isolated HOME.

## 6. Non-mutating CLI probes (SDK binary)

```
build/bin/gosh-appimage-manager --list-update-managers
```

Printed: `static`, `github`, `gitlab`, `codeberg`, `forgejo`, `ftp`.

CTest `test_cli` captured `--list-installed --json` stdout and asserted
`schema_version` 1 and the `installed` array.

## 7. Flatpak builder

```
flatpak-builder --force-clean --user --install-deps-from=flathub \
  --repo=repo build-dir packaging/com.goshapps.AppImageManager.yml
```

Exit 0. Manifest has no `--filesystem=host:rw`. Bundled
`/app/bin/unsquashfs`, `/app/bin/7zz`, `/app/bin/dwarfsextract`.
Corresponding source and licenses installed under
`/app/share/gosh-appimage-manager/`.

Exported commit: `c2950bf49f59ec2d5928f87a62c8428df1b6a1f802aa8febd9bac25412f9d484`
(app), debug `1c411b00a7da695593e8a7cdedb69aad5c2f821e5fae8f1e4bcc9c5273d8fc89`.

## 8. Packaged offscreen `--self-test`

```
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml \
  env QT_QPA_PLATFORM=offscreen HOME=/tmp/gosh-aim-flatpak-home \
  gosh-appimage-manager --self-test
```

Exit 0.

## 9. Packaged non-mutating host probe

```
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml \
  env QT_QPA_PLATFORM=offscreen HOME=/tmp/gosh-aim-flatpak-home \
  gosh-appimage-manager --probe-host
```

```
host_spawn_program=flatpak-spawn
host_spawn_exit=0
in_flatpak=true
managed_folder=/tmp/gosh-aim-flatpak-home/AppImages
HOST_PROBE_OK
```

## 10. Packaged synthetic inspect probe

The sandbox cannot see host `/tmp` from outside (intentional; no `host:rw`).
The fixture was written inside the sandbox `/tmp` and inspected there:

```
magic_valid=true type=type-2 unsafe_fallback=false
INSPECT_NO_EXECUTION
```

Architecture of the 256-byte stub was `unknown` (headers too small for a
machine type). Magic and no-execution still hold.

## 11. `git diff --check`

No whitespace errors.

## 12. Worktree

Intended project files committed after this record. Build products (`build/`,
`build-dir/`, `repo/`, `.flatpak-builder/`) remain gitignored.
