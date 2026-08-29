# Verification record

All commands below were executed on 2026-08-29 UTC in this worktree. Output is
summarised from the real tool transcripts; nothing here is fabricated.

Host: Ubuntu, `/root/projects/gosh-appimage-manager-grok`, branch
`grok/appimage-manager`.
SDK: `org.kde.Sdk//6.10` and `org.kde.Platform//6.10` from Flathub.

No command mutated the operator's real `~/AppImages`, desktop entries, icons,
or application configuration. Mutation tests used `QStandardPaths` test mode
and isolated temporary directories. Inspection never executed an AppImage.
Automated tests inject a recording notifier and never send a desktop
notification. Remove-all tests inject a Trash seam and never touch the real
Trash.

## 1. CMake configure and Ninja compile (KDE 6.10 SDK)

```
flatpak run --filesystem=/root/projects/gosh-appimage-manager-grok --share=network \
  --devel --command=bash org.kde.Sdk//6.10 \
  -lc 'cmake --build /root/projects/gosh-appimage-manager-grok/build'
```

Incremental rebuild after the quality-review remediations succeeded.
Configure remains ECM 6.10, Qt 6.10.3, KF6 6.27.0 including Kirigami, I18n,
Config, DBusAddons, Notifications. Ninja linked
`build/bin/gosh-appimage-manager` and the test binaries.

## 2. CTest

```
QT_QPA_PLATFORM=offscreen ctest --test-dir build --output-on-failure
```

(run inside the SDK)

```
100% tests passed, 0 tests failed out of 15
Total Test time (real) =   7.50 sec
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
  candidate path is never the executed program without setting+consent; after
  consented unsafe extract, extracted-tree verification still fails closed
- **large-tree extraction:** 200+ unrelated members plus `usr/bin/foo -> ../lib/foo`
  leave root `demo.desktop` extractable; production unsquashfs `-e` args contain
  only that desktop and ingest `Name=Demo App`; wanted-path unsafe symlink still
  fails closed; 80 root `.desktop` files still fail the extracted-file cap
- transactional integrate copy/replace; new-integration registry-save and
  partial desktop/icon install failures delete dest/desktop/icon and leave no
  `.gosh-*` leftovers; backup-creation failure is fail-closed before overwrite;
  replace desktop-install and registry-save failures restore prior bytes
- **no-replace commit:** KeepBoth with an unowned dest planted after naming
  leaves those bytes unchanged, fails the integration, and cleans staging
- missing executable reconciles registry for both Trash and Permanent; permanent-delete containment for `/` and `$HOME`
- launch is start-only detached; running-process guard uses a real temp file
- URL guards reject `file:`, credentials, and private/loopback hosts
- GitHub API host validation; advertised digest mismatch leaves the install;
  matching digest replaces and keeps arguments; FTP `allowFtp` and invalid FTP
  config fail closed; static same size as installed is not an update; zsync
  compares a bounded SHA-1 of the installed AppImage (same SHA-1 unavailable,
  changed available); apply refuses genuine `!available`
- **check then apply:** `listUpdates`/`check()` for static and zsync with a
  changed remote, then `apply()`; dest bytes replace once; the next check is
  unavailable; applying the same remote as the installed file refuses and
  leaves bytes; CLI `--update --all --yes` after `--list-updates` replaces
  once and a second `--update --all` does not change bytes
- GitHub `sha256:<hex>` matches installed digest+tag+size => `!available`;
  matching digest with a different tag is still unavailable; changed digest
  or size => available; empty digest does not itself force available;
  `updateAll` does not enqueue that uuid when `!available`
- **forge identity:** GitHub and GitLab with `app.version=1.0.0`, tag
  `v1.0.0`, matching digest/size => unavailable; changed tag/digest =>
  available; check then apply once, second check/apply refuses and bytes
  stay put; `_applied_version` is the remote tag, desktop Version is not
  overwritten with the tag
- **HEAD/metadata-only probes:** local HTTP body >4096 with Content-Length;
  QtNetworkClient HEAD + StaticFileSource::check return ok/availability from
  headers with transferred body 0; FTP `metadataOnly+allowFtp` is recorded
  on FakeNetworkClient and QtNetworkClient uses GET (not HEAD) for FTP
- **short-write:** FileSink seam that short-writes makes fetch fail and
  removes staging; apply with a failing download leaves original bytes
- **adoption:** foreign desktop bytes unchanged after adopt, update, and
  remove; Gosh `gosh-appimage-<uuid>.desktop` has exact ownership markers
  and is the only launcher removed
- **remove-all:** `--remove-all --yes` uses Trash via a test seam; permanent
  only with `--delete --yes`; two owned files never hit real Trash
- **CLI `--integrate --yes file`:** path is the AppImage, not `--yes`
- Library `updateAvailable` role is true after `checkAll` from in-memory
  offers (no registry write) and false after a successful apply
- Update download reports progress in (0, 100) while a fake download is held
  in flight; TaskQueue publishes that progress into history under mutex
- Update-all summary counts only the task IDs created by that `updateAll`;
  an in-flight inspect finish does not emit “1 succeeded”; a failed enqueue
  does not leave remaining work and does not rewrite the summary when the
  blocking mutation later succeeds
- Autostart desktop is written to a session-visible `…/autostart/` path;
  Exec is `flatpak run com.goshapps.AppImageManager --fetch-updates` inside
  Flatpak/SDK and the native executable otherwise; `--fetch-updates` does
  not modify `registry.json`
- dest-exists unowned conflicts need KeepBoth and do not offer Replace
- `refreshMetadata` save failure restores registry state and prior desktop bytes
- QtNetworkClient local HTTP: size abort, timeout, cancellation; resolver
  rejects private/rebind addresses; fetch pins the validated public address so
  a later private resolve cannot write a body
- CLI `--list-installed --json` and `--list-updates --json` stdout parsed for
  `schema_version` 1 and `installed`/`updates` (not `items`); `--integrate`
  without TTY needs confirmation; `--yes` without `--keep-both` does not
  suffix `*-2`; `--replace --replace-uuid` succeeds for owned and refuses
  missing UUID; `--fetch-updates` calls the notifier once for offers, zero for
  none, and does not save `registry.json`
- task mutation serialisation; cancelling queued B leaves running A successful;
  join-safe shutdown; no live-thread destruction warnings
- **concurrency:** GUI argument edits while an update is held in
  download/replace; registry JSON still parses; final arguments are coherent;
  no live QObject destruction warnings
- Inspect keep-both/replace: selecting Replace on row 0 sets that row only;
  single-candidate Replace is in range; QML required-index seam; Integrate
  waits for explicit choices; argument token save round-trips `["a b","c"]`
  after editing the second token; cancel of update X does not cancel inspect

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
QT_QPA_PLATFORM=offscreen HOME=/tmp/gosh-aim-selftest-home-qfix \
  build/bin/gosh-appimage-manager --self-test
```

(run inside the SDK; the host cannot load the SDK-linked Qt libraries)

Exit 0. Models and services probed, synthetic catalog row loaded, QML `Main`
loaded with `objectName: mainWindow`. Isolated HOME.

## 6. Non-mutating CLI probes (SDK binary)

```
build/bin/gosh-appimage-manager --list-update-managers
```

Printed: `static`, `github`, `gitlab`, `codeberg`, `forgejo`, `ftp`.

```
HOME=/tmp/gosh-aim-autostart-probe-qfix QT_QPA_PLATFORM=offscreen \
  build/bin/gosh-appimage-manager --probe-autostart
```

```
autostart_path=/tmp/gosh-aim-autostart-probe-qfix/.config/autostart/com.goshapps.AppImageManager-updates.desktop
[Desktop Entry]
Type=Application
Name=Gosh AppImage Manager update checks
Exec=flatpak run com.goshapps.AppImageManager --fetch-updates
X-GNOME-Autostart-enabled=true
OnlyShowIn=KDE;
AUTOSTART_OK
```

The SDK has `/.flatpak-info`, so Exec is the Flatpak launch line.

CTest `test_cli` captured `--list-installed --json` and `--list-updates --json`
stdout and asserted `schema_version` 1 plus the `installed`/`updates` arrays.

## 7. Flatpak builder

```
flatpak-builder --force-clean --user --install-deps-from=flathub \
  --repo=repo build-dir packaging/com.goshapps.AppImageManager.yml
```

Exit 0. Manifest has no `--filesystem=host:rw`. Bundled
`/app/bin/unsquashfs`, `/app/bin/7zz`, `/app/bin/dwarfsextract`.
Corresponding source and licenses installed under
`/app/share/gosh-appimage-manager/`. KF6 Notifications is linked; notifyrc is
installed at `/app/share/knotifications6/gosh-appimage-manager.notifyrc`.
`--talk-name=org.freedesktop.Notifications` is granted.
`--filesystem=xdg-config/autostart:create` is granted so login autostart can
reach the session.

Exported commit: `f1e831484c265c1128d855992acaa20203837f1fed2d70979f444ed58af5c052`
(app), debug `c97ce2ae1aa8d07e3c367506457e4ec77a010e83a5f060c4bbf876ea7996c590`.

## 8. Packaged offscreen `--self-test`

```
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml \
  env QT_QPA_PLATFORM=offscreen HOME=/tmp/gosh-aim-flatpak-home-qfix \
  gosh-appimage-manager --self-test
```

Exit 0.

## 9. Packaged non-mutating host probe

```
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml \
  env QT_QPA_PLATFORM=offscreen HOME=/tmp/gosh-aim-flatpak-home-qfix \
  gosh-appimage-manager --probe-host
```

Inside this `--run` sandbox, `HOME` is `/tmp/gosh-aim-flatpak-home-qfix` while
`XDG_CONFIG_HOME` / `XDG_DATA_HOME` remain
`/root/.var/app/com.goshapps.AppImageManager/{config,data}`. KConfig therefore
still holds the managed folder from an earlier isolated probe:

```
host_spawn_program=flatpak-spawn
host_spawn_exit=0
in_flatpak=true
managed_folder=/tmp/gosh-aim-flatpak-home/AppImages
HOST_PROBE_OK
```

That path is leftover isolated-probe configuration under the app's sandbox
config, not the operator's real `~/AppImages`.

## 10. Packaged synthetic inspect probe

The sandbox cannot see host `/tmp` from outside (intentional; no `host:rw`).
The fixture was written inside the same `--run` sandbox `/tmp` and inspected
there:

```
{"architecture":"unknown","error":"","extractor":"","magic_valid":true,"name":"synth","path":"/tmp/synth.AppImage","schema_version":1,"sha256":"b09afa18211d28a0db0d8995aa90dce1e2604e2f65ed7f5494586a15a3d48091","size":256,"type":"type-2","unsafe_fallback":false}
INSPECT_NO_EXECUTION
```

Architecture of the 256-byte stub was `unknown` (headers too small for a
machine type). Magic and no-execution still hold. Exit 0.

## 11. Packaged autostart / `--fetch-updates` probe

```
flatpak-builder --run build-dir packaging/com.goshapps.AppImageManager.yml \
  env QT_QPA_PLATFORM=offscreen HOME=/tmp/gosh-aim-flatpak-home-qfix \
  gosh-appimage-manager --probe-autostart
```

```
autostart_path=/tmp/gosh-aim-flatpak-home-qfix/.config/autostart/com.goshapps.AppImageManager-updates.desktop
[Desktop Entry]
Type=Application
Name=Gosh AppImage Manager update checks
Exec=flatpak run com.goshapps.AppImageManager --fetch-updates
X-GNOME-Autostart-enabled=true
OnlyShowIn=KDE;
AUTOSTART_OK
```

`--fetch-updates` in the same sandbox printed `0 update(s) available`.

## 12. `git diff --check`

No whitespace errors.

## 13. Worktree

Intended project files committed after this record. Build products (`build/`,
`build-dir/`, `repo/`, `.flatpak-builder/`) remain gitignored.
