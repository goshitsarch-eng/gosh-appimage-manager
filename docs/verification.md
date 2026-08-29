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
  /root/projects/gosh-appimage-manager-grok/scripts/sdk-build.sh
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
100% tests passed, 0 tests failed out of 13
Total Test time (real) =   0.52 sec
```

Tests: `appstreamtest`, `test_elf`, `test_hash`, `test_desktop`,
`test_archive`, `test_inspector`, `test_integration`, `test_removal`,
`test_launch`, `test_update`, `test_cli`, `test_tasks`, `test_models`.

ECM `appstreamtest` reported PASS because the helper is not installed. That
skip is **not** treated as metainfo proof. Metainfo proof is the explicit
`appstreamcli validate --pedantic --no-net --explain` command below.

Production-path coverage includes:

- ELF/AppImage magic, type 1/2, x86_64/aarch64/i386, truncated and missing magic
- streaming SHA-256, cancellation, and size bounds
- desktop parser, Exec token rewriting, environment name/value validation
- archive absolute/`..`/symlink-escape/count bounds
- inspector never uses the candidate path as the executed program; unsafe
  fallback stays off without settings+consent
- transactional integrate copy, ownership markers, replace requires an owned UUID
- Trash failure does not delete; permanent-delete containment for `/` and `$HOME`
- launch is start-only detached and reports start failure as failure; shells refused
- URL guards reject `file:`, credentials, and private/loopback hosts
- GitHub API host validation; running-app update guard preserves the install
- CLI `--list-update-managers`, JSON list schema, `--integrate` without TTY
  returns needs-confirmation
- task mutation serialisation, cancel, and join-safe shutdown

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
kept.

## 5. Native/SDK offscreen `--self-test`

```
QT_QPA_PLATFORM=offscreen HOME=/tmp/gosh-aim-selftest-home2 \
  build/bin/gosh-appimage-manager --self-test
```

Exit 0. QML `Main` loaded with `objectName: mainWindow`. Isolated HOME.

## 6. Non-mutating CLI probes (SDK binary)

```
build/bin/gosh-appimage-manager --list-update-managers
```

Printed: `static`, `github`, `gitlab`, `codeberg`, `forgejo`, `ftp`.

```
build/bin/gosh-appimage-manager --list-installed --json
```

```
{"items":[],"schema_version":1}
```

```
build/bin/gosh-appimage-manager --probe-inspect /tmp/gosh-aim-synthetic.AppImage
```

Synthetic 256-byte ELF64 Type 2 x86_64 fixture. Result included
`magic_valid: true`, `type: type-2`, `unsafe_fallback: false`, and
`INSPECT_NO_EXECUTION`.

## 7. Flatpak builder

```
flatpak-builder --force-clean --user --install-deps-from=flathub \
  --repo=repo build-dir packaging/com.goshapps.AppImageManager.yml
```

Exit 0. Manifest has no `--filesystem=host:rw`. Bundled
`/app/bin/unsquashfs`, `/app/bin/7zz`, `/app/bin/dwarfsextract`.

Exported commit: `15190f30b5b80adbcc15e272a90a4b0913ac9196d240287a69ded67d293e9024`
(app), debug `d4c19cdd43637e06b7f37f5b7e50751425c94135a2ffc052ec24205b3b546d72`.

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

The sandbox cannot see host `/tmp` (intentional; no `host:rw`). The fixture
was written inside the sandbox `/tmp` and inspected there:

```
magic_valid=true type=type-2 architecture=x86_64 unsafe_fallback=false
INSPECT_NO_EXECUTION
```

## 11. `git diff --check`

Run after the documentation update. No whitespace errors.

## 12. Worktree

Intended project files committed. Build products (`build/`, `build-dir/`,
`repo/`, `.flatpak-builder/`) remain gitignored.
