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
}
