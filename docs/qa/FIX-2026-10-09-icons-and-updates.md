# Icons and updates: fix record (2026-10-09)

Released as 3.0.1; see `CHANGELOG.md`.

Reported: the app does not show the icons of the actual AppImages, and updating
AppImages never works ("can't find" them, or random errors).

This record names each cause, how it was reproduced before the fix, what changed,
and what was run afterwards. Everything below was run for real on an x86_64 Linux
host (rustc/cargo 1.97.0, Flutter 3.47.6 / Dart 3.13.5, `squashfs-tools` 4.6.1,
the version the Flatpak builds). Where something was not verified, it says so.

## How the causes were found

Reading the icon and update paths end to end, then reproducing each suspected
cause **before** changing code:

- Update causes were reproduced with tests that drive the real controller on the
  fake network, using what real projects publish (an `.AppImage.zsync` pattern, a
  `v`-prefixed tag, several architectures in one release). Nine of the ten first
  tests failed on the original code; all failed for the reason named below.
- Icon causes were reproduced with **real AppImages**: an ELF header carrying the
  Type-2 magic and an `.upd_info` section, then a payload made by `mksquashfs`,
  read by the real `unsquashfs`. Seven of eleven icon tests failed on the original
  inspector (the four that passed are the one layout the old code handled).
- The fixes were then run through the real binary, the real Flutter GUI under a
  virtual display, and a real TLS server (see "Verification").

The earlier icon tests used a fake extractor that planted a PNG at exactly the name
the desktop entry gave. That is the one shape of AppImage the old code handled,
which is why the tests were green while real AppImages showed no icon.

## Causes and fixes

### Updates

| # | Cause | Fix |
|---|---|---|
| U1 | `detect_embedded` asked the **static** source first, and it claimed any hint containing "zsync", which includes `gh-releases-zsync`, the form nearly every AppImage uses. The check died with **"Static source needs a url"**. | Forge sources are asked first; the static source claims only `static`, `zsync` and `bintray*`; the substring fallback no longer maps "zsync" to static. `gh-releases-direct` is recognised as GitHub. |
| U2 | The embedded pattern names the `.zsync` control file (`App-*x86_64.AppImage.zsync`) and was matched against asset names, so the control file (or nothing) was chosen. | The `.zsync` suffix is dropped before matching (GitHub and Forgejo). |
| U3 | Versions were compared as raw text in three places. `v1.1.0` vs `1.1.0` made current apps look outdated; an older release was offered as an update. | New `versions` module: a leading `v` is decoration, dotted numbers compare as numbers, pre-releases come first, and labels that cannot be ordered count as different. One predicate, `updates_service::offers_update`, is used by the list, the single-app check, the apply and the bridge DTO. The version is recorded without the tag's `v`. |
| U4 | The `release` field was ignored, so a project's moving `continuous` release could not be followed. | A named release is fetched by tag (`/releases/tags/<tag>`). A rebuild under the same tag is an update only when the source publishes a SHA-256 and the installed file's differs. |
| U5 | A pattern that does not name an architecture matches every build in a release, so an x86_64 install could be sent the aarch64 file. | Assets built for another architecture are dropped; with none left the error says so before anything is downloaded. |
| U6 | **Adoption wrote "a registry row and nothing else"**, so an adopted app had no embedded update string and every check said "No update method was found". | Adoption reads the file (name, version, architecture, checksum, update string, icon). See I4. |
| U7 | A zsync control file names its target relative to itself (`URL: App.AppImage`); it was used as written and failed. | Resolved against the control file's own address. |
| U8 | Many servers and object stores refuse `HEAD`, which failed the size probe. | A refused `HEAD` (400/403/405/501) is retried as a one-byte ranged `GET`; the size comes from `Content-Range`. |
| U9 | The request was pinned to the **first** address the resolver returned, so a dead or unreachable first address (or an IPv6 address on a host without IPv6) failed the whole request. | All public addresses are pinned and the connector tries them in turn. The legacy FTP connection does the same. |
| U10 | The HTTP client trusted only its bundled public roots, so any network that re-signs TLS with a certificate the machine trusts failed with `invalid peer certificate: UnknownIssuer`. | The bundle named by `SSL_CERT_FILE`, or else the distribution's, is added, read once and bounded. A bad bundle falls back to the bundled roots. No new crate: the offline Flatpak vendoring is unchanged. |
| U11 | Transport errors read "error sending request for url (...)" and hid the cause (DNS, refused, bad certificate); timeouts were not recognisable as timeouts; GitHub 404/403/rate-limit were bare status codes; "No update method was found" said nothing to act on. | The cause chain is kept; a timeout says so (the Updates page then reports an unknown status, not a failure); GitHub 404, 403 and rate-limit answers say what they usually mean; the no-source messages say how to set one. |
| U12 | The documented `allow_local_network=true` opt-in for a self-hosted update server could not be saved on a static source (and GitLab's private-host check ignored it). | Both honour the opt-in. Embedded update information still can never set it (tested). |

### Icons

| # | Cause | Fix |
|---|---|---|
| I1 | `.DirIcon` is a **symlink** in nearly every real AppImage, and the extractor skipped every symlink, so the `.DirIcon` fallback never fired. | The picker resolves a symlink by name inside the archive's own listing (never on disk), through up to 8 links, and extracts the target as another member. |
| I2 | Only `<Icon>.png/.svg/.xpm` at the top level was tried. `Icon=app.png` and icons that exist only under `usr/share/icons` gave none; a first-match search could pick a 16 px copy that sorts before the real icon; a file was accepted by its name, not its content. | Candidates in order: the named file, `.DirIcon`, then the theme/pixmaps folders by size. Each is judged by its content (PNG, SVG, XPM); a decoy is skipped. |
| I3 | The GUI drew icons with `Image.file`, which cannot decode SVG, so every SVG-icon app showed a letter. It also set both `cacheWidth` and `cacheHeight`, squashing non-square icons. | SVG is drawn with `flutter_svg` (letter on error, no exception); only the width bounds the decode. |
| I4 | Adoption never read the file, and **refresh read it but discarded the staged icon**, so neither could ever give an app its icon. | Both install it: an integrated app's icon goes beside its entry in the icon theme; an adopted app's goes to the manager's own data folder (`~/.local/share/gosh-appimage-manager/icons/gosh-appimage-<id>.<ext>`), so adoption still writes nothing to the user's menu or icon theme. The name carries the id, so removal still proves ownership and cleans it up. |
| I5 | An XPM was installed with a `.png` name; the Inspect header always drew a letter, never the extracted icon. | An XPM keeps `.xpm`; the Inspect header draws the extracted icon. |
| I6 | Apps already in a library (adopted by an older build, or whose icon file is gone) would stay icon-less. | A quiet pass after start (`heal_library_icons`, each app once per run, no Tasks entry) gives them their icon, fills in a never-read row, re-opens the registry before writing so a concurrent edit is kept, and hashes the file only for a row never read. |

## What I got wrong

I first diagnosed "every download longer than 30 s fails" from the client's 30 s
`timeout()` and built a stall-guard download path on it. I then ran a real
throttled TLS server (1.6 MB at 40 KB/s, 39 s) against the **original** network
code, and it finished. In reqwest 0.12's blocking client each `read` on the
response gets its own deadline, so the 30 s limit was already a per-read stall
limit for streamed downloads, not a total cap. That diagnosis was wrong, and the
rewrite solved a problem that does not exist, so it was removed; the download
path is the original one. The other network changes above were each reproduced
against the original code before being kept (see "Verification").

## Tests added

| Where | What | Count |
|---|---|---|
| `src/versions.rs` | version comparison unit tests | 10 |
| `tests/test_update_embedded.rs` | embedded-source selection, asset choice, tags, architecture, named releases, digest rebuilds, zsync URL, error messages, opt-in, an end-to-end apply on the fake network | 21 |
| `tests/test_icon_real.rs` | icon layouts through real `mksquashfs`/`unsquashfs` | 11 |
| `tests/test_adopt_metadata.rs` | adopt reads the file; icon install, replace and removal; refresh; heal (including that a menu entry that is no longer ours is never rewritten) | 15 |
| `bridge/src/api/library.rs` | heal is quiet, once per app, skips missing files | 3 |
| `flutter/test/icons_and_heal_test.dart` | SVG tile, PNG tile, Inspect header, heal in the model | 15 |

The real-tool tests skip loudly when the squashfs tools are not on `PATH`.

## Verification

Gates, on the final tree (commands as in `docs/verification.md`):

- Root: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` clean;
  `cargo test --no-fail-fast`: **332 passed, 0 failed** (275 before this work).
- Bridge (`bridge/`): fmt and clippy clean; `cargo test`: **23 passed, 0 failed**
  (20 before).
- Bindings: `flutter_rust_bridge_codegen generate` (2.13.0, the pinned version) was
  run and a second run changes nothing, which is what CI's "bindings are up to
  date" step checks.
- Flutter: `dart format --set-exit-if-changed` and `flutter analyze` clean;
  `flutter test`: **374 passed, 1 failed**. The one failure,
  `pages_golden_test.dart: 02 Empty library`, is a 6-pixel (0.00 %) anti-aliasing
  difference that fails identically on the untouched tree on this host; it is a
  host rasterizer difference, not caused by this work. Before this work: 359 passed,
  the same one failed.

End to end:

- **Real CLI, real AppImages.** Four AppImages (SVG icon, PNG icon reached through a
  `.DirIcon` symlink, an icon only under `usr/share/icons`, and one with no icon)
  were adopted with the built binary: three icons were installed in the manager's
  data folder, the fourth correctly has none, and the update strings were stored.
  An update check then took the GitHub path (before: "Static source needs a url").
- **Real GUI, legacy data.** The Linux release bundle was built and run under Xvfb
  against a registry blanked to what an older build leaves (name from the file
  name, no version, icon or update information, icon folder deleted). After the
  startup pass the Library showed the real names and versions, the SVG icon and
  both PNG icons, and a letter tile for the app whose AppImage has no icon. The
  Inspect page, opened from the command line, showed the extracted icon in its
  header.
- **Real TLS, before and after.** Against a local HTTPS server (own CA, trusted
  through `SSL_CERT_FILE`), original code versus this tree:
  - `HEAD` answered 405: original **fails** ("HTTP 405 Method Not Allowed"); this
    tree reports the update through a ranged `GET` (server log: `GET ... 206`).
  - A name whose first address has no listener and whose second works: original
    **fails**; this tree connects.
  - A 39 s throttled download: both **succeed** (see "What I got wrong").
  - The certificate errors are now readable: an unusable server certificate reports
    `CaUsedAsEndEntity` instead of "error sending request".
  The original was patched only enough to talk to the test server (certificate
  trust and the loopback opt-in); its network code was otherwise untouched.

## Not verified, and known limits

- **Live GitHub.** `api.github.com` is blocked by this session's egress policy
  (403 from the proxy, also for `curl`), so the GitHub path was verified on the
  fake network and through the real client against other hosts, not against GitHub.
- **Flatpak.** `flatpak-builder` is not installed here; no Flatpak was built and no
  packaged smoke check was run. No dependency changed (`Cargo.lock`,
  `bridge/Cargo.lock` and `packaging/cargo-sources.json` are untouched). The Linux
  release bundle was built and smoke-run natively.
- **aarch64** was not run; this host is x86_64.
- **IPv6-first resolution** (an IPv6 address first on a host without IPv6) could not
  be reproduced here: the resolver orders IPv4 first because IPv6 is unusable. The
  same limitation was reproduced with a dead first IPv4 address instead.
- **DwarFS AppImages** are still detected but their metadata is not read (documented
  in the README); this work did not change that, so those apps still have no icon
  and no update information from the file.
- **XPM icons** are kept for the desktop but cannot be drawn by the GUI; those apps
  keep their letter tile.
- **Clients behind a proxy that must resolve names** are unchanged: the manager
  resolves the name itself to check it is not a local address.
- The `Icon` row on the Inspect page still says "Extracted from the AppImage"
  whenever the entry names an icon, because the Inspect golden (a mockup-parity
  frame) uses that wording and the goldens cannot be regenerated reliably on this
  host. The header now shows the real icon when one was extracted.
