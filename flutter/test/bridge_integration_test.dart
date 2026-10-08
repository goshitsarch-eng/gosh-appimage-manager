import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/src/rust/api/system.dart';
import 'package:gosh_appimage_flutter/src/rust/frb_generated.dart';

/// The library built by `cargo build` in ../bridge. Tests run from the project
/// root, so the path is relative to it.
const String _libraryPath = '../bridge/target/debug/libgoshaim_bridge.so';

/// These tests drive the real core, so they only run against a scratch home.
/// The core reads GOSHAIM_HOME for every path it writes; without it the tests
/// would use the owner's real files, so they refuse to run instead.
final String? _skipReason = () {
  final home = Platform.environment['GOSHAIM_HOME'];
  if (home == null || home.isEmpty) {
    return 'Set GOSHAIM_HOME to an empty scratch directory to run these tests, '
        'for example: GOSHAIM_HOME=\$(mktemp -d) flutter test '
        'test/bridge_integration_test.dart';
  }
  return null;
}();

/// Writes a 128-byte synthetic AArch64 type-2 AppImage. It is never executed.
File _writeFixture(
  Directory dir, {
  String name = 'Demo-app-1.0.0-aarch64.AppImage',
}) {
  final bytes = Uint8List(128);
  bytes.setRange(0, 4, [0x7f, 0x45, 0x4c, 0x46]); // \x7fELF
  bytes[4] = 2; // 64-bit
  bytes[5] = 1; // little endian
  bytes[6] = 1; // version
  bytes.setRange(8, 11, [0x41, 0x49, 0x02]); // "AI" + type 2 marker
  bytes[18] = 0xB7; // EM_AARCH64
  final file = File('${dir.path}/$name');
  file.writeAsBytesSync(bytes);
  return file;
}

void main() {
  const core = BridgeCore();
  late Directory scratch;

  setUpAll(() async {
    if (_skipReason != null) {
      return;
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(_libraryPath));
    scratch = Directory.systemTemp.createTempSync('gosh-bridge-test-');
  });

  tearDownAll(() {
    if (_skipReason == null) {
      scratch.deleteSync(recursive: true);
    }
  });

  test('bridgeVersion is synchronous and returns the crate version', () {
    expect(bridgeVersion(), '0.1.0');
  }, skip: _skipReason);

  test('settings and the managed folder live under GOSHAIM_HOME', () async {
    final settings = await core.loadSettings();
    expect(
      settings.managedFolder,
      startsWith(Platform.environment['GOSHAIM_HOME']!),
    );
    expect(await core.appVersion(), isNotEmpty);
  }, skip: _skipReason);

  test('inspect, integrate, list, then remove permanently', () async {
    final file = _writeFixture(scratch);

    final inspected = await core.inspectPath(
      opId: 'it-inspect-1',
      path: file.path,
    );
    expect(inspected.magicValid, isTrue);
    expect(inspected.sizeBytes, 128);
    expect(inspected.sha256, matches(RegExp(r'^[0-9a-f]{64}$')));
    expect(
      await File(file.path).exists(),
      isTrue,
      reason: 'inspect must not move the file',
    );

    final outcome = await core.integrateApp(
      opId: 'it-integrate-1',
      sourcePath: file.path,
      conflict: ConflictChoice.automatic,
      replaceUuid: '',
      moveSource: false,
    );
    expect(outcome.ok, isTrue, reason: outcome.message);
    final app = outcome.app!;
    expect(File(app.managedPath).existsSync(), isTrue);

    final library = await core.listLibrary();
    expect(library.apps.map((a) => a.uuid), contains(app.uuid));

    final removed = await core.removeApp(
      opId: 'it-remove-1',
      uuid: app.uuid,
      permanent: true,
    );
    expect(removed.ok, isTrue, reason: removed.message);
    expect(File(app.managedPath).existsSync(), isFalse);
    expect(
      (await core.listLibrary()).apps.map((a) => a.uuid),
      isNot(contains(app.uuid)),
    );
  }, skip: _skipReason);

  test(
    'integrating a second copy of the same name reports the replace candidate',
    () async {
      final first = _writeFixture(scratch, name: 'Clash-1.0-aarch64.AppImage');
      final installed = await core.integrateApp(
        opId: 'it-integrate-2',
        sourcePath: first.path,
        conflict: ConflictChoice.automatic,
        replaceUuid: '',
        moveSource: false,
      );
      expect(installed.ok, isTrue, reason: installed.message);

      final second = _writeFixture(
        Directory('${scratch.path}/second')..createSync(),
        name: 'Clash-1.0-aarch64.AppImage',
      );
      final clash = await core.integrateApp(
        opId: 'it-integrate-3',
        sourcePath: second.path,
        conflict: ConflictChoice.automatic,
        replaceUuid: '',
        moveSource: false,
      );
      expect(clash.ok, isFalse);
      expect(clash.conflict, isTrue);
      expect(clash.conflictUuid, installed.app!.uuid);

      await core.removeApp(
        opId: 'it-remove-2',
        uuid: installed.app!.uuid,
        permanent: true,
      );
    },
    skip: _skipReason,
  );

  test(
    'checking updates on an empty library finds nothing and fails nothing',
    () async {
      final scan = await core.checkUpdates(opId: 'it-check-1');
      expect(scan.offers, isEmpty);
      expect(scan.failures, isEmpty);
      expect(scan.cancelled, isFalse);
    },
    skip: _skipReason,
  );

  test('saved preferences round-trip through the core', () async {
    final before = await core.loadSettings();
    final saved = await core.saveSettings(
      patch: const SettingsPatchDto(
        maxAppimageBytes: 16 * 1024 * 1024 * 1024,
        appearance: AppearanceChoice.dark,
      ),
    );
    expect(saved.maxAppimageBytes, 16 * 1024 * 1024 * 1024);
    expect(saved.appearance, AppearanceChoice.dark);

    final restored = await core.saveSettings(
      patch: SettingsPatchDto(
        maxAppimageBytes: before.maxAppimageBytes,
        appearance: before.appearance,
      ),
    );
    expect(restored.maxAppimageBytes, before.maxAppimageBytes);
  }, skip: _skipReason);

  test('inspect maps an empty path to UserInput', () async {
    await expectLater(
      core.inspectPath(opId: 'it-inspect-empty', path: '   '),
      throwsA(
        isA<CoreError>().having((e) => e.kind, 'kind', ErrorKind.userInput),
      ),
    );
  }, skip: _skipReason);

  test(
    'inspect reports a non-ELF file as invalid, with the core reason',
    () async {
      final text = File('${scratch.path}/not-an-appimage.txt')
        ..writeAsStringSync('plain text\n');
      final inspected = await core.inspectPath(
        opId: 'it-inspect-text',
        path: text.path,
      );
      expect(inspected.magicValid, isFalse);
      expect(inspected.error, contains('ELF'));
    },
    skip: _skipReason,
  );

  // Background checks, the login entry, the unsafe fallback and the integration
  // date, against the real core in the scratch home. The default is checked
  // first, before any test changes the settings.
  test(
    'background checks start off in a fresh home (the owner default)',
    () async {
      final settings = await core.loadSettings();
      expect(settings.backgroundUpdateChecks, isFalse);
      expect(settings.autostartEnabled, isFalse);
    },
    skip: _skipReason,
  );

  test('the login check is refused while background checks are off', () async {
    await expectLater(
      core.setAutostart(enabled: true),
      throwsA(
        isA<CoreError>().having(
          (e) => e.message,
          'message',
          contains('Check in the background'),
        ),
      ),
    );
    expect((await core.loadSettings()).autostartEnabled, isFalse);
  }, skip: _skipReason);

  test('turning background checks on stores it, and the login check can then be added', () async {
    final on = await core.saveSettings(
      patch: const SettingsPatchDto(backgroundUpdateChecks: true),
    );
    expect(on.backgroundUpdateChecks, isTrue);
    await core.setAutostart(enabled: true);
    expect((await core.loadSettings()).autostartEnabled, isTrue);
  }, skip: _skipReason);

  test(
    'turning background checks off removes the login entry and stores off',
    () async {
      final off = await core.saveSettings(
        patch: const SettingsPatchDto(backgroundUpdateChecks: false),
      );
      expect(off.backgroundUpdateChecks, isFalse);
      expect(
        off.autostartEnabled,
        isFalse,
        reason: 'the login entry is removed before the setting is stored',
      );
    },
    skip: _skipReason,
  );

  test('the unsafe extraction fallback is off by default, and a saved-on value reads back on', () async {
    // Checked first: no test before this one has changed the setting.
    expect((await core.loadSettings()).unsafeExtractionFallback, isFalse);
    final on = await core.saveSettings(
      patch: const SettingsPatchDto(unsafeExtractionFallback: true),
    );
    expect(on.unsafeExtractionFallback, isTrue);
    expect((await core.loadSettings()).unsafeExtractionFallback, isTrue);
    // Put the default back, so the tests after this one start from it.
    await core.saveSettings(
      patch: const SettingsPatchDto(unsafeExtractionFallback: false),
    );
    expect((await core.loadSettings()).unsafeExtractionFallback, isFalse);
  }, skip: _skipReason);

  test('an integration records its date and source folder, and checking an app changes nothing installed', () async {
    final file = _writeFixture(
      scratch,
      name: 'Dated-app-2.0.0-aarch64.AppImage',
    );
    final before = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    final outcome = await core.integrateApp(
      opId: 'it-integrate-dated',
      sourcePath: file.path,
      conflict: ConflictChoice.automatic,
      replaceUuid: '',
      moveSource: false,
    );
    expect(outcome.ok, isTrue, reason: outcome.message);
    final app = outcome.app!;
    expect(app.integratedAt, greaterThanOrEqualTo(before));
    expect(app.integratedAt, lessThanOrEqualTo(before + 120));
    expect(
      app.integratedFolder,
      scratch.path,
      reason: 'the folder the source was in is recorded',
    );
    final bytesBefore = await File(app.managedPath).readAsBytes();

    // This app has no update source, so the check cannot succeed. It must say
    // so, and it must not download, replace or re-version the installed file.
    final checked = await core.checkOneUpdate(uuid: app.uuid);
    expect(checked.error, isNotEmpty);
    expect(checked.availableVersion, isEmpty);
    expect(await File(app.managedPath).readAsBytes(), bytesBefore);
    final listed = (await core.listLibrary()).apps.firstWhere(
      (candidate) => candidate.uuid == app.uuid,
    );
    expect(listed.version, app.version);

    await core.removeApp(
      opId: 'it-remove-dated',
      uuid: app.uuid,
      permanent: true,
    );
  }, skip: _skipReason);
}
