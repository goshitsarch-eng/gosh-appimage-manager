import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/src/rust/api/app.dart';
import 'package:gosh_appimage_flutter/src/rust/frb_generated.dart';

/// The library built by `cargo build` in ../bridge. Tests run from the project
/// root, so the path is relative to it.
const String _libraryPath = '../bridge/target/debug/libgoshaim_bridge.so';

/// Writes a 128-byte synthetic AArch64 type-2 AppImage. It is never executed.
File _writeFixture(Directory dir) {
  final bytes = Uint8List(128);
  bytes.setRange(0, 4, [0x7f, 0x45, 0x4c, 0x46]); // \x7fELF
  bytes[4] = 2; // 64-bit
  bytes[5] = 1; // little endian
  bytes[6] = 1; // version
  bytes.setRange(8, 11, [0x41, 0x49, 0x02]); // "AI" + type 2 marker
  bytes[18] = 0xB7; // EM_AARCH64
  final file = File('${dir.path}/Demo-app-1.0.0-aarch64.AppImage');
  file.writeAsBytesSync(bytes);
  return file;
}

void main() {
  late Directory scratch;

  setUpAll(() async {
    await RustLib.init(externalLibrary: ExternalLibrary.open(_libraryPath));
    scratch = Directory.systemTemp.createTempSync('gosh-bridge-test-');
  });

  tearDownAll(() => scratch.deleteSync(recursive: true));

  test('bridgeVersion is synchronous and returns the crate version', () {
    expect(bridgeVersion(), '0.1.0');
  });

  test(
    'inspectPath describes a synthetic AppImage without executing it',
    () async {
      final file = _writeFixture(scratch);
      final summary = await inspectPath(path: file.path);
      expect(summary.sizeBytes, BigInt.from(128));
      expect(summary.appType, 'type2');
      expect(summary.architecture, 'aarch64');
      expect(summary.magicValid, isTrue);
      expect(summary.sha256, matches(RegExp(r'^[0-9a-f]{64}$')));
    },
  );

  test('inspectPath maps a missing file to NotFound', () async {
    await expectLater(
      inspectPath(path: '${scratch.path}/missing.AppImage'),
      throwsA(
        isA<CoreError>().having((e) => e.kind, 'kind', ErrorKind.notFound),
      ),
    );
  });

  test('inspectPath maps an empty path to UserInput', () async {
    await expectLater(
      inspectPath(path: '   '),
      throwsA(
        isA<CoreError>().having((e) => e.kind, 'kind', ErrorKind.userInput),
      ),
    );
  });

  test(
    'inspectPath maps a non-ELF file to Validation with the core message',
    () async {
      final text = File('${scratch.path}/not-an-appimage.txt')
        ..writeAsStringSync('plain text\n');
      await expectLater(
        inspectPath(path: text.path),
        throwsA(
          isA<CoreError>()
              .having((e) => e.kind, 'kind', ErrorKind.validation)
              .having((e) => e.message, 'message', contains('ELF')),
        ),
      );
    },
  );

  test('countTicks streams values in order and then completes', () async {
    expect(await countTicks(total: 3).toList(), [0, 1, 2]);
  });

  test(
    'a Rust panic reaches Dart as Internal, and the bridge keeps working',
    () async {
      await expectLater(
        panicForContractTest(),
        throwsA(
          isA<CoreError>()
              .having((e) => e.kind, 'kind', ErrorKind.internal)
              .having((e) => e.details, 'details', contains('deliberate')),
        ),
      );
      expect(bridgeVersion(), '0.1.0');
    },
  );
}
