// QA-1 walkthrough: drives the real app (real native runner, real Rust core
// through the bridge) with the Flutter integration test binding.
//
// Run from flutter/ with fresh scratch homes (see the QA report for the command):
//   flutter test integration_test/qa_walkthrough_test.dart -d linux \
//     --dart-define=QA_SCRATCH=<scratch> --dart-define=QA_MODE=full
//
// QA_MODE=full      fresh home; Phase B walkthrough and Phase C layout sweep.
// QA_MODE=restart   same home as a previous full run; checks persistence.
// QA_HANDSHAKE=<dir> optional. Shared with an external window-capture helper
//                   that acks each state the walkthrough reaches. Unset, the
//                   handshakes are skipped.
//
// Test seams used (none change app code):
// - FilePickers.openAppImages / openFolder are replaced with canned answers so
//   the native GTK chooser is never opened from a test.
// - QaCore extends BridgeCore and only overrides the update *offers* (the core
//   cannot reach a live release feed offline). Everything else is the real core.
// - The drop check sends the same method call the desktop_drop plugin sends
//   from native code, through the real plugin channel.
//
// Every check is recorded as PASS, FAIL or BLOCKED with its inventory ID.

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/app.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/src/rust/frb_generated.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart' as fmt;
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/page_frame.dart';
import 'package:gosh_appimage_flutter/ui/shell.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';
import 'package:integration_test/integration_test.dart';
import 'package:path/path.dart' as p;

// Configuration ---------------------------------------------------------------

const String kScratch = String.fromEnvironment('QA_SCRATCH');
const String kMode = String.fromEnvironment('QA_MODE', defaultValue: 'full');
const String kHandshakeDir = String.fromEnvironment('QA_HANDSHAKE');

final GlobalKey kAppBoundary = GlobalKey();

/// Surface sizes for the layout sweep (Phase C). The last is the smallest size
/// ARCHITECTURE.md claims (360 x 480 minimum).
const List<Size> kSizes = [
  Size(360, 640),
  Size(480, 800),
  Size(640, 480),
  Size(800, 600),
  Size(1024, 700),
  Size(1280, 800),
  Size(1600, 900),
  Size(1920, 1080),
  Size(2560, 1440),
  Size(2400, 420),
  Size(420, 1400),
  Size(360, 480),
];

// Results and logs -------------------------------------------------------------

final List<Map<String, String>> checks = [];
final List<String> logLines = [];
final List<String> layoutErrors = [];
String caseId = '';
String layoutTag = '';
DebugPrintCallback? _originalDebugPrint;

void logLine(String text) {
  final line = '${DateTime.now().toIso8601String()} $text';
  logLines.add(line);
  _originalDebugPrint?.call('QA $line');
}

void record(String inv, String what, String status, [String detail = '']) {
  checks.add({
    'inv': inv,
    'what': what,
    'status': status,
    'detail': detail,
    'case': caseId,
  });
  logLine('CHECK $status $inv $what${detail.isEmpty ? '' : ' :: $detail'}');
}

String _short(Object error) {
  final text = '$error'.replaceAll('\n', ' ');
  return text.length > 500 ? '${text.substring(0, 500)}…' : text;
}

/// Runs one assertion block and records PASS or FAIL. Expectation failures do
/// not stop the walkthrough.
Future<void> check(
  String inv,
  String what,
  FutureOr<void> Function() body,
) async {
  try {
    await body();
    record(inv, what, 'PASS');
  } catch (error) {
    record(inv, what, 'FAIL', _short(error));
  }
}

void blocked(String inv, String what, String reason) {
  record(inv, what, 'BLOCKED', reason);
}

/// Runs a whole case. An uncaught error ends only this case.
Future<void> runCase(
  String id,
  String title,
  Future<void> Function() body,
) async {
  caseId = id;
  logLine('CASE BEGIN $id $title');
  try {
    await body();
  } catch (error, stack) {
    record(id, 'case aborted: $title', 'FAIL', _short(error));
    logLine('CASE STACK $id $stack');
  }
  logLine('CASE END $id');
}

// Environment ------------------------------------------------------------------

String get home => Platform.environment['GOSHAIM_HOME'] ?? '';
String get dataHome => Platform.environment['GOSHAIM_XDG_DATA_HOME'] ?? '';
String get configHome => Platform.environment['GOSHAIM_XDG_CONFIG_HOME'] ?? '';

/// Refuses to run unless every core path is inside the scratch folder, so a
/// missing variable can never reach the owner's real home.
void assertIsolated() {
  if (kScratch.isEmpty) {
    throw StateError('QA_SCRATCH is not set');
  }
  final scratch = p.normalize(kScratch);
  for (final value in [
    home,
    dataHome,
    configHome,
    Platform.environment['GOSHAIM_XDG_CACHE_HOME'] ?? '',
    Platform.environment['XDG_DATA_HOME'] ?? '',
    Platform.environment['XDG_CONFIG_HOME'] ?? '',
  ]) {
    if (value.isEmpty || !p.isWithin(scratch, p.normalize(value))) {
      throw StateError('core path outside scratch: "$value"');
    }
  }
}

// Fixtures ---------------------------------------------------------------------

/// The header layout tests/common.rs uses (an ELF with the AppImage marker at
/// offset 8 and type 2 at offset 10), plus one PT_LOAD header that places the
/// squashfs payload at 4096, as a real type-2 file does.
Uint8List elfHeader(String arch) {
  final header = Uint8List(4096);
  header.setRange(0, 4, [0x7f, 0x45, 0x4c, 0x46]);
  header[4] = 2;
  header[5] = 1;
  header[6] = 1;
  header.setRange(8, 11, [0x41, 0x49, 0x02]);
  final machine = arch == 'x86_64' ? 0x3e : 0xb7;
  header[18] = machine & 0xff;
  header[19] = machine >> 8;
  final data = ByteData.sublistView(header);
  data.setUint64(32, 64, Endian.little); // e_phoff
  data.setUint16(54, 56, Endian.little); // e_phentsize
  data.setUint16(56, 1, Endian.little); // e_phnum
  data.setUint32(64, 1, Endian.little); // p_type PT_LOAD
  data.setUint64(96, 4096, Endian.little); // p_filesz
  data.setUint64(104, 4096, Endian.little); // p_memsz
  return header;
}

/// Builds a squashfs-layout AppImage with mksquashfs (argument arrays only).
String makeAppImage({
  required String dir,
  required String fileName,
  required String appName,
  required String version,
  String arch = 'aarch64',
}) {
  Directory(dir).createSync(recursive: true);
  final tree = Directory(p.join(dir, '.tree', fileName))
    ..createSync(recursive: true);
  final apps = Directory(p.join(tree.path, 'usr', 'share', 'applications'))
    ..createSync(recursive: true);
  File(p.join(apps.path, 'qa-fixture.desktop')).writeAsStringSync(
    '[Desktop Entry]\n'
    'Type=Application\n'
    'Name=$appName\n'
    'Exec=AppRun %U\n'
    'Icon=qa-fixture\n'
    'X-AppImage-Version=$version\n'
    'Categories=Utility;\n',
  );
  File(p.join(tree.path, 'AppRun')).writeAsStringSync('#!/bin/sh\necho qa\n');
  Process.runSync('/usr/bin/chmod', ['+x', p.join(tree.path, 'AppRun')]);
  final squash = p.join(dir, '$fileName.sqfs');
  final made = Process.runSync('/usr/bin/mksquashfs', [
    tree.path,
    squash,
    '-noappend',
    '-all-root',
    '-quiet',
  ]);
  if (made.exitCode != 0) {
    throw StateError('mksquashfs failed: ${made.stderr}');
  }
  final image = File(p.join(dir, fileName));
  final out = BytesBuilder()
    ..add(elfHeader(arch))
    ..add(File(squash).readAsBytesSync());
  image.writeAsBytesSync(out.toBytes());
  Process.runSync('/usr/bin/chmod', ['u+x', image.path]);
  File(squash).deleteSync();
  return image.path;
}

/// A copy of sleep with the AppImage marker, so the core sees an AppImage and
/// the process runs as the managed file (for the running state).
String makeRunningImage(String dir, String fileName) {
  Directory(dir).createSync(recursive: true);
  final bytes = File('/usr/bin/sleep').readAsBytesSync();
  bytes.setRange(8, 11, [0x41, 0x49, 0x02]);
  final path = p.join(dir, fileName);
  File(path).writeAsBytesSync(bytes);
  Process.runSync('/usr/bin/chmod', ['u+x', path]);
  return path;
}

/// A sparse 1 GiB file with a valid header: hashing it takes long enough to see
/// the "Inspecting…" state and to cancel a task.
String makeBigImage(String dir, String fileName, {int gib = 1}) {
  Directory(dir).createSync(recursive: true);
  final path = p.join(dir, fileName);
  final file = File(path)..writeAsBytesSync(elfHeader('aarch64'));
  final raf = file.openSync(mode: FileMode.append);
  raf.truncateSync(gib << 30);
  raf.closeSync();
  return path;
}

// QaCore: the real BridgeCore with update offers supplied at the seam ----------

class QaCore extends BridgeCore {
  QaCore();

  /// uuid -> the version the stub offers. Empty means the real core answers.
  final Map<String, String> offers = {};
  int listCalls = 0;
  int checkCalls = 0;
  int checkOneCalls = 0;

  @override
  Future<LibraryDto> listLibrary() {
    listCalls += 1;
    return super.listLibrary();
  }

  @override
  Future<UpdateScanDto> checkUpdates({required String opId}) async {
    checkCalls += 1;
    final scan = await super.checkUpdates(opId: opId);
    if (offers.isEmpty) {
      return scan;
    }
    final apps = (await super.listLibrary()).apps;
    final have = {for (final offer in scan.offers) offer.uuid};
    final extra = [
      for (final app in apps)
        if (offers.containsKey(app.uuid) && !have.contains(app.uuid))
          _offer(app, offers[app.uuid]!),
    ];
    return UpdateScanDto(
      offers: [...scan.offers, ...extra],
      failures: scan.failures,
      skipped: scan.skipped,
      checked: scan.checked,
      cancelled: scan.cancelled,
    );
  }

  @override
  Future<UpdateCheckDto> checkOneUpdate({required String uuid}) async {
    checkOneCalls += 1;
    final version = offers[uuid];
    if (version == null) {
      return super.checkOneUpdate(uuid: uuid);
    }
    final apps = (await super.listLibrary()).apps;
    final app = apps.firstWhere((candidate) => candidate.uuid == uuid);
    return UpdateCheckDto(
      uuid: uuid,
      currentVersion: app.version,
      availableVersion: version,
      downloadSize: 0,
      reducedVerification: false,
      error: '',
      timedOut: false,
    );
  }

  UpdateOfferDto _offer(AppDto app, String version) => UpdateOfferDto(
    uuid: app.uuid,
    name: app.name,
    currentVersion: app.version,
    availableVersion: version,
    manager: 'static',
    url: '',
    downloadSize: 0,
    digest: '',
    reducedVerification: false,
    digestAlgo: '',
    embeddedSource: '',
    running: app.running,
  );
}

// Pumping, waiting and input ---------------------------------------------------

Future<void> settle(WidgetTester tester, {int ms = 250}) async {
  final steps = math.max(1, ms ~/ 25);
  for (var i = 0; i < steps; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 25)),
    );
    await tester.pump(const Duration(milliseconds: 25));
  }
}

Future<void> until(
  WidgetTester tester,
  bool Function() done, {
  int timeoutMs = 15000,
  String what = 'condition',
}) async {
  final end = DateTime.now().add(Duration(milliseconds: timeoutMs));
  while (!done()) {
    if (DateTime.now().isAfter(end)) {
      throw TestFailure('timed out after ${timeoutMs}ms waiting for $what');
    }
    await settle(tester, ms: 100);
  }
}

bool present(Finder finder) => finder.evaluate().isNotEmpty;

bool showsText(String text) =>
    find.text(text, skipOffstage: true).evaluate().isNotEmpty;

bool showsTextContaining(String text) =>
    find.textContaining(text, skipOffstage: true).evaluate().isNotEmpty;

Finder byKey(String key) => find.byKey(Key(key), skipOffstage: true);

Future<void> tap(WidgetTester tester, Finder finder) async {
  // Off-screen widgets still count here: ensureVisible scrolls them into view.
  if (finder.evaluate().isEmpty) {
    throw TestFailure('not in the tree: $finder');
  }
  await settle(tester, ms: 50);
  try {
    await tester.ensureVisible(finder.first);
  } catch (_) {
    // Not inside a scrollable: the tap below reports a miss if it is off screen.
  }
  await tester.tap(finder.first, warnIfMissed: false);
  await settle(tester);
}

Future<void> tapKey(WidgetTester tester, String key) =>
    tap(tester, find.byKey(Key(key)));

Future<void> tapText(WidgetTester tester, String text) =>
    tap(tester, find.text(text, skipOffstage: true).first);

/// Focuses the field with a tap, then replaces its text. A plain enterText
/// without the tap leaves the text out of the model (see probe P2).
Future<void> typeInto(WidgetTester tester, String key, String text) async {
  final finder = find.byKey(Key(key));
  if (finder.evaluate().isEmpty) {
    throw TestFailure('field not in the tree: $key');
  }
  // Scroll the field into view first: a tap off screen never reaches it.
  try {
    await tester.ensureVisible(finder.first);
  } catch (_) {
    // Not inside a scrollable: the tap below is the only step.
  }
  await settle(tester, ms: 50);
  await tester.tap(finder.first, warnIfMissed: false);
  await settle(tester, ms: 50);
  await tester.enterText(finder.first, text);
  await settle(tester);
}

/// Enter in a single-line field is the platform's "done" action. A synthetic
/// key event does not reach the field's submit path in this test binding, so
/// the action is sent to the text input directly.
Future<void> submitField(WidgetTester tester) async {
  tester.testTextInput.receiveAction(TextInputAction.done);
  await settle(tester);
}

/// Cancel exists only while an inspection result is shown.
Future<void> cancelInspectIfPresent(WidgetTester tester) async {
  if (present(byKey('inspect-cancel'))) {
    await tapKey(tester, 'inspect-cancel');
  }
}

/// Presses Tab to put focus back on the shell's controls. The shortcuts no
/// longer need this: they are handled app-wide, whatever has focus (QA D-08).
Future<void> focusShell(WidgetTester tester) async {
  await press(tester, LogicalKeyboardKey.tab);
}

Future<void> press(
  WidgetTester tester,
  LogicalKeyboardKey key, {
  bool ctrl = false,
  bool shift = false,
}) async {
  if (ctrl) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  }
  if (shift) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  }
  await tester.sendKeyEvent(key);
  if (shift) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  }
  if (ctrl) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  }
  await settle(tester);
}

String? _keyOf(Element element) {
  final key = element.widget.key;
  return key is ValueKey<String> ? key.value : null;
}

/// The key of the focused control: the nearest keyed widget under the focus
/// node, else the nearest keyed ancestor.
String focusedKey() {
  final node = FocusManager.instance.primaryFocus;
  final context = node?.context;
  if (context == null) {
    return '(none)';
  }
  String? hit;
  void walk(Element element, int depth) {
    if (hit != null || depth > 6) {
      return;
    }
    hit = _keyOf(element);
    if (hit == null) {
      element.visitChildElements((child) => walk(child, depth + 1));
    }
  }

  context.visitChildElements((child) => walk(child, 0));
  if (hit != null) {
    return hit!;
  }
  context.visitAncestorElements((element) {
    hit = _keyOf(element);
    return hit == null;
  });
  return hit ?? '(unkeyed)';
}

/// Presses Tab (or Shift+Tab) `count` times and returns each focused key.
Future<List<String>> traverse(
  WidgetTester tester, {
  required int count,
  bool backwards = false,
}) async {
  final seen = <String>[];
  for (var i = 0; i < count; i++) {
    await press(tester, LogicalKeyboardKey.tab, shift: backwards);
    seen.add(focusedKey());
  }
  return seen;
}

// Handshake with part 2 (window capture helper) ---------------------------------

Future<void> handshake(WidgetTester tester, String state) async {
  // Without QA_HANDSHAKE no capture helper is attached: nothing to wait for.
  if (kHandshakeDir.isEmpty) return;
  final dir = Directory(kHandshakeDir)..createSync(recursive: true);
  final stateFile = File(p.join(dir.path, 'state.txt'));
  final ackFile = File(p.join(dir.path, 'ack.txt'));
  await tester.runAsync(() async {
    stateFile.writeAsStringSync('$state ${DateTime.now().toIso8601String()}\n');
  });
  final deadline = DateTime.now().add(const Duration(seconds: 15));
  var acked = false;
  await tester.runAsync(() async {
    while (DateTime.now().isBefore(deadline)) {
      if (ackFile.existsSync()) {
        acked = true;
        ackFile.deleteSync();
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 200));
    }
  });
  record(
    'HANDSHAKE',
    'state $state',
    acked ? 'PASS' : 'BLOCKED',
    acked ? 'ack received' : 'no ack within 15 s (part 2 not running)',
  );
}

// Captures and layout ----------------------------------------------------------

Future<void> capture(WidgetTester tester, String path) async {
  await settle(tester, ms: 200);
  await tester.runAsync(() async {
    final boundary =
        kAppBoundary.currentContext?.findRenderObject()
            as RenderRepaintBoundary?;
    if (boundary == null) {
      throw StateError('no repaint boundary for $path');
    }
    final image = await boundary.toImage(pixelRatio: 1.0);
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    File(path).parent.createSync(recursive: true);
    File(path).writeAsBytesSync(bytes!.buffer.asUint8List());
  });
}

// Model and app lifecycle --------------------------------------------------------

class Session {
  Session(this.tester, this.core);

  final WidgetTester tester;
  final QaCore core;
  AppModel? model;

  AppModel get m => model!;

  Future<void> start() async {
    final created = AppModel(core: core);
    model = created;
    _liveModel = created;
    await tester.pumpWidget(
      RepaintBoundary(
        key: kAppBoundary,
        child: GoshApp(model: created),
      ),
    );
    await settle(tester);
    await tester.runAsync(() => created.start(const []));
    await settle(tester, ms: 400);
  }

  Future<void> restart() async {
    await tester.pumpWidget(const SizedBox.shrink());
    await settle(tester, ms: 200);
    model?.dispose();
    await start();
  }

  Future<void> go(AppPage page) async {
    m.setPage(page);
    await settle(tester, ms: 400);
  }
}

// Settings and state on disk -----------------------------------------------------

Map<String, dynamic> readSettingsFile() {
  final file = File(
    p.join(configHome, 'gosh-appimage-manager', 'settings.json'),
  );
  if (!file.existsSync()) {
    return {};
  }
  final decoded = jsonDecode(file.readAsStringSync());
  return decoded is Map<String, dynamic> ? decoded : {};
}

/// settings.json uses CamelCase keys (MoveSource, ManagedFolder, ...), so a
/// snake_case name is matched with underscores and case removed.
dynamic settingNamed(String part) {
  final map = readSettingsFile();
  String squash(String text) => text.replaceAll('_', '').toLowerCase();
  for (final key in map.keys) {
    if (squash(key).contains(squash(part))) {
      return map[key];
    }
  }
  return 'missing';
}

String autostartPath() => p.join(
  configHome,
  'autostart',
  'com.goshapps.AppImageManager-updates.desktop',
);

String trashFilesDir() => p.join(dataHome, 'Trash', 'files');

// Fixture set ------------------------------------------------------------------

class Fixtures {
  Fixtures(this.root);

  final String root;

  String get quill =>
      p.join(root, 'fixtures', 'a', 'Quill Notes 1.2.0.AppImage');
  String get quillDupB =>
      p.join(root, 'fixtures', 'b', 'Quill Notes 1.2.0.AppImage');
  String get quillDupC =>
      p.join(root, 'fixtures', 'c', 'Quill Notes 1.2.0.AppImage');
  String get brisk =>
      p.join(root, 'fixtures', 'a', 'Brisk Terminal 1.1.4.AppImage');
  String get cinder =>
      p.join(root, 'fixtures', 'a', 'Cinder Chat 0.14.2.AppImage');
  String get unavailable =>
      p.join(root, 'fixtures', 'a', 'Unavailable Sync 2.0.0.AppImage');
  String get eclair =>
      p.join(root, 'fixtures', 'a', 'Éclair Ñandú 0.9.AppImage');
  String get ledger => p.join(root, 'fixtures', 'a', 'Ledgerline 3.3.AppImage');
  String get weird =>
      p.join(root, 'fixtures', 'a', "Weird;name \$(x) 'q'.AppImage");
  String get x86 =>
      p.join(root, 'fixtures', 'a', 'Arch Check 1.0 x86.AppImage');
  String get bigLong =>
      p.join(root, 'fixtures', 'big', 'Big Stub Long 1.0.AppImage');
  String get bigStub =>
      p.join(root, 'fixtures', 'big', 'Big Stub 1.0.AppImage');
  String get textFile => p.join(root, 'fixtures', 'a', 'notes.txt');
  String get directory => p.join(root, 'fixtures', 'a', 'Folder.AppImage');
  String get adoptable =>
      p.join(root, 'fixtures', 'outside', 'Adopt Me 2.0.AppImage');
  String get altManaged => p.join(root, 'managed-alt');

  void build() {
    makeAppImage(
      dir: p.dirname(quill),
      fileName: p.basename(quill),
      appName: 'Quill Notes',
      version: '1.2.0',
    );
    makeAppImage(
      dir: p.dirname(quillDupB),
      fileName: p.basename(quillDupB),
      appName: 'Quill Notes',
      version: '1.2.0',
    );
    makeAppImage(
      dir: p.dirname(quillDupC),
      fileName: p.basename(quillDupC),
      appName: 'Quill Notes',
      version: '1.2.0',
    );
    makeRunningImage(p.dirname(brisk), p.basename(brisk));
    makeAppImage(
      dir: p.dirname(cinder),
      fileName: p.basename(cinder),
      appName: 'Cinder Chat',
      version: '0.14.2',
    );
    makeAppImage(
      dir: p.dirname(unavailable),
      fileName: p.basename(unavailable),
      appName: 'Unavailable Sync',
      version: '2.0.0',
    );
    makeAppImage(
      dir: p.dirname(eclair),
      fileName: p.basename(eclair),
      appName: 'Éclair Ñandú',
      version: '0.9',
    );
    makeAppImage(
      dir: p.dirname(ledger),
      fileName: p.basename(ledger),
      appName: 'Ledgerline',
      version: '3.3',
    );
    makeAppImage(
      dir: p.dirname(weird),
      fileName: p.basename(weird),
      appName: 'Weird',
      version: '1.0',
    );
    makeAppImage(
      dir: p.dirname(x86),
      fileName: p.basename(x86),
      appName: 'Arch Check',
      version: '1.0',
      arch: 'x86_64',
    );
    makeBigImage(p.dirname(bigStub), p.basename(bigStub));
    // 6 GiB sparse, used only for the Tasks check: long enough for a running task
    // to stay on screen (hashing takes about a minute here).
    makeBigImage(p.dirname(bigLong), p.basename(bigLong), gib: 6);
    File(textFile).createSync(recursive: true);
    File(textFile).writeAsStringSync('plain text, not an AppImage\n');
    Directory(directory).createSync(recursive: true);
    makeAppImage(
      dir: p.dirname(adoptable),
      fileName: p.basename(adoptable),
      appName: 'Adopt Me',
      version: '2.0',
    );
  }
}

// Phase B: the walkthrough ------------------------------------------------------

final Map<String, String> _notes = {};

Future<void> main() async {
  final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  _integrationBinding = binding;
  _originalDebugPrint = debugPrint;
  debugPrint = (String? message, {int? wrapWidth}) {
    if (message != null) {
      logLines.add('${DateTime.now().toIso8601String()} [debugPrint] $message');
    }
    _originalDebugPrint?.call(message, wrapWidth: wrapWidth);
  };
  final previousOnError = FlutterError.onError;
  FlutterError.onError = (details) {
    final text = details.exceptionAsString();
    logLines.add(
      '${DateTime.now().toIso8601String()} [FlutterError:$caseId$layoutTag] '
      '${text.split('\n').first}',
    );
    if (text.contains('overflowed')) {
      layoutErrors.add('$layoutTag :: ${text.split('\n').first}');
    }
    previousOnError?.call(details);
  };

  testWidgets('QA-1 walkthrough', (tester) async {
    final started = DateTime.now();
    logLine('RUN mode=$kMode scratch=$kScratch');
    try {
      assertIsolated();
      record(
        'SAFETY',
        'core paths are inside the scratch folder',
        'PASS',
        'GOSHAIM_HOME=$home',
      );
    } catch (error) {
      record(
        'SAFETY',
        'core paths are inside the scratch folder',
        'FAIL',
        _short(error),
      );
      rethrow;
    }
    // Replace every chooser with a canned answer. The native GTK chooser
    // would open a window the test cannot drive.
    FilePickers.openAppImages = () async => <String>[];
    FilePickers.openFolder = () async => null;
    await RustLib.init();

    final fixtures = Fixtures(kScratch);
    if (kMode != 'restart') {
      fixtures.build();
      logLine('FIXTURES built under $kScratch/fixtures');
    }
    final session = Session(tester, QaCore());
    try {
      if (kMode == 'full') {
        await runFullWalkthrough(tester, binding, session, fixtures);
      } else if (kMode == 'probe') {
        await runProbes2(tester, session, fixtures);
      } else if (kMode == 'probe3') {
        await runProbes3(tester, session, fixtures);
      } else if (kMode == 'probe4') {
        await runProbes4(tester, session, fixtures);
      } else if (kMode == 'probe5') {
        await runProbes5(tester, session, fixtures);
      } else if (kMode == 'layout') {
        await runLayoutOnly(tester, session, fixtures);
      } else if (kMode == 'probe6') {
        await runProbes6(tester, session, fixtures);
      } else if (kMode == 'keys') {
        await runKeysOnly(tester, session, fixtures);
      } else if (kMode == 'layout800') {
        await runLayout800(tester, session, fixtures);
      } else if (kMode == 'unsafe') {
        await runUnsafeOnly(tester, session, fixtures);
      } else if (kMode == 'detail') {
        await runDetailOnly(tester, session, fixtures);
      } else if (kMode == 'recheck') {
        await runRecheck(tester, session, fixtures);
      } else {
        await runRestartChecks(tester, session, fixtures);
      }
    } finally {
      record(
        'TIME',
        'run wall time',
        'PASS',
        '${DateTime.now().difference(started).inSeconds} s',
      );
      for (final process in [runningApp]) {
        process?.kill();
      }
      _writeOutputs();
    }
  }, timeout: const Timeout(Duration(minutes: 60)));
}

void _writeOutputs() {
  final runDir = Directory(p.join(kScratch, 'runs', kMode))
    ..createSync(recursive: true);
  File(p.join(runDir.path, 'results.json'))
      .writeAsStringSync(const JsonEncoder.withIndent('  ').convert(checks));
  File(p.join(runDir.path, 'log.txt')).writeAsStringSync(logLines.join('\n'));
  File(p.join(runDir.path, 'layout-errors.txt'))
      .writeAsStringSync(layoutErrors.join('\n'));
  File(p.join(runDir.path, 'notes.json'))
      .writeAsStringSync(const JsonEncoder.withIndent('  ').convert(_notes));
}

Future<void> runFullWalkthrough(
  WidgetTester tester,
  IntegrationTestWidgetsFlutterBinding binding,
  Session session,
  Fixtures fixtures,
) async {
  // Screenshot probe: the integration binding's own screenshot on this host.
  // binding.takeScreenshot is not called here: on Linux it throws
  // MissingPluginException for captureScreenshot on plugins.flutter.io/integration_test
  // (run2 log), and that error is reported to the test as a failure. Captures use
  // RepaintBoundary.toImage instead.
  caseId = 'B-00';
  blocked(
    'INV-001',
    'binding.takeScreenshot on Linux',
    'MissingPluginException: no captureScreenshot implementation on '
        'plugins.flutter.io/integration_test for Linux (run2 log); RepaintBoundary.toImage used',
  );
  await session.start();
  // A group that throws is recorded and the run goes on with the next group.
  await guardGroup('B-02', () => caseRunEmptyLaunch(tester, session, fixtures));
  await guardGroup('B-04', () => caseRunInspect(tester, session, fixtures));
  await guardGroup('B-05', () => caseRunLibrary(tester, session, fixtures));
  await guardGroup(
    'B-07',
    () => caseRunRowsAndMenus(tester, session, fixtures),
  );
  await guardGroup('B-09', () => caseRunDetail(tester, session, fixtures));
  await guardGroup('B-13', () => caseRunDialogs(tester, session, fixtures));
  await guardGroup('B-16', () => caseRunKeyboard(tester, session, fixtures));
  await guardGroup(
    'B-17',
    () => caseRunUpdatesTasks(tester, session, fixtures),
  );
  await guardGroup('B-19', () => caseRunSettings(tester, session, fixtures));
  await guardGroup('B-24', () => caseRunAbout(tester, session, fixtures));
  await guardGroup('B-14', () => caseRunNarrow(tester, session, fixtures));
  await guardGroup(
    'B-26',
    () => caseRunSessionState(tester, session, fixtures),
  );
  await guardGroup('U-01', () => caseRunUnsafe(tester, session, fixtures));
  await guardGroup('C', () => caseRunLayoutSweep(tester, session, fixtures));
  await guardGroup('B-01', () => caseRunFinalWindowControls(tester, session));
}

/// Groups named in QA_SKIP (comma separated, for example B-16,C) are recorded
/// as BLOCKED with the reason, instead of run.
const String kSkip = String.fromEnvironment('QA_SKIP');

Future<void> guardGroup(String id, Future<void> Function() body) async {
  if (kSkip.split(',').contains(id)) {
    record(id, 'group not run in this pass', 'BLOCKED', 'QA_SKIP=$kSkip');
    return;
  }
  try {
    await body();
  } catch (error, stack) {
    record(id, 'case group aborted', 'FAIL', _short(error));
    logLine('GROUP STACK $id $stack');
  }
}

// Shared helpers for the walkthrough cases --------------------------------------

IntegrationTestWidgetsFlutterBinding? _integrationBinding;

/// The model of the running session; the layout helpers pump their root with it.
AppModel? _liveModel;

/// Sets the surface size for a layout case. The app root is replaced by
/// [LayoutAppRoot] so that MediaQuery inside the app follows the target size.
Future<void> setSize(WidgetTester tester, Size size) =>
    layoutSize(tester, _liveModel!, size);

/// Restores the normal surface and the normal app root after a layout case.
Future<void> restoreSize(WidgetTester tester) =>
    restoreApp(tester, _liveModel!);

/// The layout root for captures: the same MaterialApp and ShellPage as GoshApp,
/// with MediaQuery set to the target size inside the app. setSurfaceSize alone
/// leaves MediaQuery at the real window size (1280x800), so pages chose desktop
/// layouts under the narrow shell.
class LayoutAppRoot extends StatelessWidget {
  const LayoutAppRoot({super.key, required this.model, required this.size});

  final AppModel model;
  final Size size;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: model,
      builder: (context, _) {
        final mode = switch (model.settings?.appearance ??
            AppearanceChoice.system) {
          AppearanceChoice.light => ThemeMode.light,
          AppearanceChoice.dark => ThemeMode.dark,
          AppearanceChoice.system => ThemeMode.system,
        };
        return MaterialApp(
          title: 'Gosh AppImage Manager',
          debugShowCheckedModeBanner: false,
          themeMode: mode,
          theme: appThemeData(AppPalette.light),
          darkTheme: appThemeData(AppPalette.dark),
          builder: (context, child) {
            final dark = Theme.of(context).brightness == Brightness.dark;
            return MediaQuery(
              data: MediaQuery.of(context).copyWith(size: size),
              child: AppScope(
                palette: dark ? AppPalette.dark : AppPalette.light,
                child: child!,
              ),
            );
          },
          home: Material(
            type: MaterialType.transparency,
            child: ShellPage(model: model),
          ),
        );
      },
    );
  }
}

/// Sets the surface size and pumps [LayoutAppRoot] for it.
Future<void> layoutSize(WidgetTester tester, AppModel model, Size size) async {
  await _integrationBinding!.setSurfaceSize(size);
  await tester.pumpWidget(
    RepaintBoundary(
      key: kAppBoundary,
      child: LayoutAppRoot(model: model, size: size),
    ),
  );
  await settle(tester, ms: 400);
}

/// Restores the normal surface and pumps the normal app root [GoshApp].
Future<void> restoreApp(WidgetTester tester, AppModel model) async {
  await _integrationBinding!.setSurfaceSize(null);
  await tester.pumpWidget(
    RepaintBoundary(
      key: kAppBoundary,
      child: GoshApp(model: model),
    ),
  );
  await settle(tester, ms: 400);
}

/// The MediaQuery size that the app's shell sees (inside MaterialApp).
Size appMediaQuerySize(WidgetTester tester) =>
    MediaQuery.sizeOf(tester.element(find.byType(ShellPage)));

/// Fails the case unless the app's MediaQuery is the target size. Called before
/// each layout PNG.
void expectMediaQuery(WidgetTester tester, Size size) {
  final got = appMediaQuerySize(tester);
  if (got != size) {
    throw TestFailure(
      'MediaQuery in the app is ${got.width.toInt()}x${got.height.toInt()}, '
      'not ${size.width.toInt()}x${size.height.toInt()}',
    );
  }
}

/// Types a path into the Inspect field and runs the inspection with the button.
Future<void> inspectPath(WidgetTester tester, Session s, String path) async {
  await s.go(AppPage.inspect);
  // A finished inspection hides the path field until Cancel forgets it.
  await cancelInspectIfPresent(tester);
  await typeInto(tester, 'inspect-path', path);
  if (s.m.inspect.pathInput != path) {
    throw TestFailure('the typed path did not reach the model');
  }
  await tapKey(tester, 'inspect-run');
  await until(
    tester,
    () =>
        s.m.busy == null &&
        (s.m.inspect.inspectedOk || s.m.inspect.error.isNotEmpty),
    what: 'inspection of $path',
  );
}

/// Inspects and integrates one file through the Inspect page. Returns the
/// status text, or 'conflict' when the conflict dialog opens.
Future<String> integrateThroughGui(
  WidgetTester tester,
  Session s,
  String path,
) async {
  await inspectPath(tester, s, path);
  if (!s.m.inspect.inspectedOk) {
    return 'inspect failed: ${s.m.inspect.error}';
  }
  final before = s.m.status;
  await tapKey(tester, 'integrate');
  await until(
    tester,
    () =>
        s.m.busy == null &&
        (s.m.dialog != null || !identical(s.m.status, before)),
    what: 'integration of $path',
  );
  if (s.m.dialog is IntegrateConflictDialog) {
    return 'conflict';
  }
  return s.m.status?.text ?? 'no status';
}

AppDto? appNamed(AppModel m, String name) {
  for (final app in m.library) {
    if (app.name == name) {
      return app;
    }
  }
  return null;
}

// Case group: empty launch, sidebar, inspect invalid input ------------------------

Future<void> caseRunEmptyLaunch(
  WidgetTester tester,
  Session s,
  Fixtures fixtures,
) async {
  await runCase('B-02', 'fresh launch, empty library, sidebar', () async {
    await check('INV-019', 'empty library heading reads No apps yet', () {
      expect(showsText('No apps yet'), isTrue);
      expect(s.m.library, isEmpty);
    });
    await check('INV-041', 'empty card asks for an AppImage', () {
      expect(present(byKey('empty-card')), isTrue);
      expect(showsText('Drop an AppImage here'), isTrue);
    });
    await handshake(tester, 'library-empty');
    await capture(
      tester,
      p.join(kScratch, 'png', 'phaseB', 'library-empty.png'),
    );
    blocked(
      'INV-127',
      'loading text appears during load',
      'the load finishes before the first frame; state is not observable',
    );
    blocked(
      'INV-001',
      'window title follows the page',
      'the title is set on the native window and cannot be read from Dart',
    );
    await check('INV-110', 'status bar reads 0 installed', () {
      expect(showsText('0 installed'), isTrue);
    });
    await check('INV-113', 'status bar reads Not checked yet', () {
      expect(showsText('Not checked yet'), isTrue);
    });
    await check('INV-018', 'managed folder box names the folder and Empty', () {
      expect(present(byKey('managed-folder-box')), isTrue);
      expect(showsText('Empty'), isTrue);
      expect(showsTextContaining('AppImages'), isTrue);
    });
    await check('INV-012', 'Library nav item is selected at launch', () {
      expect(s.m.page, AppPage.library);
    });
    // Sidebar: every item opens its page, forward then back.
    for (final entry in [
      ('nav-inspect', AppPage.inspect, 'Inspect'),
      ('nav-updates', AppPage.updates, 'Updates'),
      ('nav-tasks', AppPage.tasks, 'Tasks'),
      ('nav-settings', AppPage.settings, 'Settings'),
      ('nav-about', AppPage.about, 'About'),
      ('nav-library', AppPage.library, 'Library'),
    ]) {
      final inv = switch (entry.$2) {
        AppPage.inspect => 'INV-013',
        AppPage.updates => 'INV-014',
        AppPage.tasks => 'INV-015',
        AppPage.settings => 'INV-016',
        AppPage.about => 'INV-017',
        _ => 'INV-012',
      };
      await check(inv, 'sidebar ${entry.$1} opens ${entry.$3}', () async {
        await tapKey(tester, entry.$1);
        expect(s.m.page, entry.$2);
        expect(
          find
              .descendant(
                of: find.byType(PageHeader),
                matching: find.text(entry.$3),
              )
              .evaluate()
              .isNotEmpty,
          isTrue,
          reason: 'page header ${entry.$3} not shown',
        );
      });
    }
    await check(
      'INV-012',
      'quick switching ends on the last tapped page',
      () async {
        await tester.tap(byKey('nav-tasks').first, warnIfMissed: false);
        await tester.tap(byKey('nav-about').first, warnIfMissed: false);
        await tester.tap(byKey('nav-settings').first, warnIfMissed: false);
        await settle(tester, ms: 400);
        expect(s.m.page, AppPage.settings);
        await tapKey(tester, 'nav-library');
      },
    );
    await check(
      'INV-041',
      'empty-library Open Inspect opens Inspect',
      () async {
        await tapKey(tester, 'open-inspect');
        expect(s.m.page, AppPage.inspect);
        await s.go(AppPage.library);
      },
    );
    await check(
      'INV-042',
      'empty-library Browse (card) uses the chooser seam',
      () async {
        var calls = 0;
        FilePickers.openAppImages = () async {
          calls += 1;
          return <String>[];
        };
        await tapKey(tester, 'browse-drop');
        expect(calls, 1, reason: 'the chooser was not opened');
        expect(s.m.page, AppPage.library, reason: 'a cancel must not navigate');
        expect(s.m.library, isEmpty);
        FilePickers.openAppImages = () async => <String>[];
      },
    );
    await check(
      'INV-044',
      'empty-library header Browse uses the chooser seam',
      () async {
        var calls = 0;
        FilePickers.openAppImages = () async {
          calls += 1;
          return <String>[];
        };
        await tapKey(tester, 'browse-empty');
        expect(calls, 1);
        FilePickers.openAppImages = () async => <String>[];
      },
    );
    await check('INV-043', 'Open Inspect button opens Inspect', () async {
      await s.go(AppPage.library);
      await tapKey(tester, 'open-inspect');
      expect(s.m.page, AppPage.inspect);
      await s.go(AppPage.library);
    });
    await check(
      'INV-095',
      'Tasks empty state reads Nothing has run yet.',
      () async {
        await s.go(AppPage.tasks);
        expect(showsText('Nothing has run yet.'), isTrue);
        await s.go(AppPage.library);
      },
    );
    await check(
      'INV-090',
      'Updates empty state reads No AppImages are integrated yet.',
      () async {
        await s.go(AppPage.updates);
        expect(showsText('No AppImages are integrated yet.'), isTrue);
        expect(showsText('Not checked yet'), isTrue);
        await s.go(AppPage.library);
      },
    );
  });
}

Future<void> caseRunInspect(
  WidgetTester tester,
  Session s,
  Fixtures fixtures,
) async {
  await runCase(
    'B-04',
    'Inspect: invalid input, valid files, integrate',
    () async {
      await s.go(AppPage.inspect);
      await check(
        'INV-071',
        'inspect banner says nothing is executed or installed',
        () {
          expect(present(byKey('inspect-banner')), isTrue);
          expect(showsText('Nothing is executed or installed'), isTrue);
        },
      );
      await check(
        'INV-072',
        'empty path: Inspect shows Choose an AppImage file first',
        () async {
          await tapKey(tester, 'inspect-run');
          expect(s.m.inspect.error, 'Choose an AppImage file first');
          expect(present(byKey('inspect-error')), isTrue);
        },
      );
      await check('INV-072', 'spaces only path is treated as empty', () async {
        await typeInto(tester, 'inspect-path', '     ');
        await tapKey(tester, 'inspect-run');
        expect(s.m.inspect.error, 'Choose an AppImage file first');
      });
      await check(
        'INV-072',
        'Enter in the empty field gives the same message',
        () async {
          await typeInto(tester, 'inspect-path', '');
          await submitField(tester);
          expect(s.m.inspect.error, 'Choose an AppImage file first');
        },
      );
      await check(
        'INV-072',
        'a 2000 character path is refused by the core, not hung',
        () async {
          final long = 'a' * 2000;
          await inspectPath(tester, s, '/tmp/$long.AppImage');
          expect(s.m.inspect.inspectedOk, isFalse);
          expect(s.m.inspect.error, isNotEmpty);
          expect(present(byKey('inspect-error')), isTrue);
        },
      );
      await check(
        'INV-072',
        'Unicode path that does not exist reports an error',
        () async {
          await inspectPath(
            tester,
            s,
            p.join(kScratch, 'fixtures', 'Ñandú 🚀 é.AppImage'),
          );
          expect(s.m.inspect.inspectedOk, isFalse);
          expect(s.m.inspect.error, isNotEmpty);
        },
      );
      await check('INV-072', 'directory path is refused', () async {
        await inspectPath(tester, s, fixtures.directory);
        expect(s.m.inspect.inspectedOk, isFalse);
        expect(s.m.inspect.error, isNotEmpty);
      });
      await check(
        'INV-072',
        'non-AppImage text file is refused as not an AppImage',
        () async {
          await inspectPath(tester, s, fixtures.textFile);
          expect(s.m.inspect.inspectedOk, isFalse);
          expect(s.m.inspect.error, isNotEmpty);
        },
      );
      await check(
        'INV-072',
        'path with special characters that do not exist is refused',
        () async {
          await inspectPath(
            tester,
            s,
            "/tmp/qa;rm -rf \$HOME/'x'\"*?.AppImage",
          );
          expect(s.m.inspect.inspectedOk, isFalse);
        },
      );
      await check(
        'INV-075',
        'an error message is shown in the error style',
        () {
          expect(present(byKey('inspect-error')), isTrue);
        },
      );
      await check(
        'INV-072',
        'weird-name file with a shell-like name inspects by argument',
        () async {
          await inspectPath(tester, s, fixtures.weird);
          expect(s.m.inspect.inspectedOk, isTrue, reason: s.m.inspect.error);
          expect(s.m.inspect.name, isNotEmpty);
          await tapKey(tester, 'inspect-cancel');
        },
      );
      await check('INV-077', 'metadata card shows the file facts', () async {
        await inspectPath(tester, s, fixtures.quill);
        expect(s.m.inspect.inspectedOk, isTrue, reason: s.m.inspect.error);
        expect(showsText('Type'), isTrue);
        expect(showsText('Architecture'), isTrue);
        expect(showsText('SHA-256'), isTrue);
        expect(
          RegExp(r'^[0-9a-f]{64}$').hasMatch(s.m.inspected!.sha256),
          isTrue,
        );
        expect(showsText('Integrate into library'), isTrue);
      });
      await check('INV-081', 'integrate steps 1 to 3 are listed', () {
        expect(showsTextContaining('Copies the file into'), isTrue);
        expect(showsTextContaining('Writes a menu entry'), isTrue);
        expect(showsTextContaining('On a name conflict'), isTrue);
      });
      await check(
        'INV-077',
        'name shows what the core read (metadata, or file name)',
        () {
          _notes['inspect.quill.name'] = s.m.inspect.name;
          _notes['inspect.quill.version'] = s.m.inspect.version;
          _notes['inspect.quill.warnings'] = s.m.inspect.warnings.join(' | ');
          expect(
            s.m.inspect.name,
            'Quill Notes',
            reason:
                'the core read no desktop name; warnings: '
                '${s.m.inspect.warnings.join(' | ')}',
          );
        },
      );
      await check(
        'INV-079',
        'move-the-original toggle saves the setting',
        () async {
          final before = s.m.settings?.moveSource ?? false;
          await tapKey(tester, 'inspect-move-toggle');
          expect(s.m.settings?.moveSource, !before);
          expect(settingNamed('move_source'), !before);
          await tapKey(tester, 'inspect-move-toggle');
          expect(s.m.settings?.moveSource, before);
        },
      );
      await check(
        'INV-078',
        'Integrate copies the file into the managed folder',
        () async {
          final result = await integrateThroughGui(tester, s, fixtures.quill);
          expect(result, startsWith('Integrated '), reason: result);
          expect(s.m.library.length, 1);
          expect(File(s.m.library.first.managedPath).existsSync(), isTrue);
          expect(s.m.library.first.managedPath, startsWith(home));
          expect(
            File(fixtures.quill).existsSync(),
            isTrue,
            reason: 'the default copies; the source must stay',
          );
        },
      );
      await check('INV-115', 'a success shows Done in the status line', () {
        expect(s.m.status?.severity, Severity.success);
        expect(showsTextContaining('Done: Integrated'), isTrue);
      });
      await check('INV-116', 'Dismiss clears the status line', () async {
        await tapText(tester, 'Dismiss');
        expect(s.m.status, isNull);
      });
      await check(
        'INV-066',
        'duplicate name opens the conflict dialog',
        () async {
          final result = await integrateThroughGui(
            tester,
            s,
            fixtures.quillDupB,
          );
          expect(result, 'conflict');
          expect(present(byKey('dialog-keep-both')), isTrue);
          expect(present(byKey('dialog-replace')), isTrue);
        },
      );
      await check('INV-066', 'conflict Cancel changes nothing', () async {
        await tapKey(tester, 'dialog-cancel');
        expect(s.m.dialog, isNull);
        expect(s.m.library.length, 1);
      });
      await check(
        'INV-066',
        'conflict Keep both integrates the second copy',
        () async {
          final result = await integrateThroughGui(
            tester,
            s,
            fixtures.quillDupB,
          );
          expect(result, 'conflict');
          await tapKey(tester, 'dialog-keep-both');
          await until(
            tester,
            () => s.m.busy == null && s.m.library.length == 2,
            what: 'keep-both integration',
          );
          expect(s.m.library.length, 2);
        },
      );
      await check(
        'INV-066',
        'conflict Replace swaps the installed copy',
        () async {
          final before = s.m.library.map((app) => app.uuid).toSet();
          final result = await integrateThroughGui(
            tester,
            s,
            fixtures.quillDupC,
          );
          expect(result, 'conflict');
          await tapKey(tester, 'dialog-replace');
          await until(
            tester,
            () => s.m.busy == null && s.m.library.length == 2,
            what: 'replace integration',
          );
          final after = s.m.library.map((app) => app.uuid).toSet();
          expect(after.length, 2);
          // The core may keep the uuid of the replaced copy; record it, do not assert it.
          _notes['replace.uuid.kept'] = '${after.intersection(before).length}';
        },
      );
      await check(
        'INV-080',
        'Cancel on Inspect forgets the inspected file',
        () async {
          await inspectPath(tester, s, fixtures.brisk);
          expect(s.m.inspect.inspectedOk, isTrue, reason: s.m.inspect.error);
          await tapKey(tester, 'inspect-cancel');
          expect(s.m.inspect.inspectedOk, isFalse);
          expect(s.m.inspect.pathInput, isEmpty);
          expect(present(byKey('inspect-path')), isTrue);
        },
      );
      await check(
        'INV-074',
        'Enter in the path field inspects the file',
        () async {
          await typeInto(tester, 'inspect-path', fixtures.cinder);
          await submitField(tester);
          await until(
            tester,
            () =>
                s.m.busy == null && s.m.inspect.inspectedOk ||
                s.m.inspect.error.isNotEmpty,
            what: 'enter inspection',
          );
          expect(s.m.inspect.inspectedOk, isTrue, reason: s.m.inspect.error);
          await tapKey(tester, 'inspect-cancel');
        },
      );
      await check(
        'INV-076',
        'Inspecting text and busy state appear while hashing',
        () async {
          await typeInto(tester, 'inspect-path', fixtures.bigStub);
          await tapKey(tester, 'inspect-run');
          var sawBusy = false;
          var sawText = false;
          final end = DateTime.now().add(const Duration(seconds: 60));
          // Poll while the inspection runs; stop once it has a result or an error.
          while (DateTime.now().isBefore(end)) {
            if (s.m.busy != null) {
              sawBusy = true;
            }
            if (showsTextContaining('Inspecting') ||
                showsTextContaining('Working:')) {
              sawText = true;
            }
            if (s.m.busy == null &&
                (s.m.inspect.inspectedOk || s.m.inspect.error.isNotEmpty)) {
              break;
            }
            await settle(tester, ms: 50);
          }
          _notes['inspect.bigStub.busySeen'] = '$sawBusy';
          _notes['inspect.bigStub.textSeen'] = '$sawText';
          expect(sawBusy, isTrue, reason: 'no busy state was observed');
          expect(
            sawText,
            isTrue,
            reason: 'no Inspecting or Working text was observed',
          );
        },
      );
      blocked(
        'INV-114',
        'Working line with Cancel during an inspection',
        'the 1 GiB inspection can finish before Cancel is tapped; Cancel is '
            'exercised on a running task in the Tasks case (INV-093)',
      );
      await check(
        'INV-116',
        'Dismiss after a busy inspection leaves no stale status',
        () async {
          await cancelInspectIfPresent(tester);
          expect(s.m.busy, isNull);
        },
      );
      await check(
        'INV-073',
        'Inspect Browse opens the chooser and inspects the pick',
        () async {
          FilePickers.openAppImages = () async => [fixtures.cinder];
          await s.go(AppPage.inspect);
          await tapKey(tester, 'inspect-browse');
          await until(
            tester,
            () => s.m.inspect.inspectedOk || s.m.inspect.error.isNotEmpty,
            what: 'browse inspection',
          );
          expect(s.m.inspect.inspectedOk, isTrue, reason: s.m.inspect.error);
          FilePickers.openAppImages = () async => <String>[];
          await tapKey(tester, 'inspect-cancel');
        },
      );
      await check('INV-073', 'a cancelled chooser changes nothing', () async {
        FilePickers.openAppImages = () async => <String>[];
        await s.go(AppPage.inspect);
        await tapKey(tester, 'inspect-browse');
        expect(s.m.inspect.inspectedOk, isFalse);
        expect(s.m.inspect.pathInput, isEmpty);
      });
      await check('INV-075', 'error text names a cause (not blank)', () async {
        await inspectPath(tester, s, fixtures.textFile);
        expect(s.m.inspect.error.trim(), isNotEmpty);
        await cancelInspectIfPresent(tester);
      });
      blocked(
        'INV-066',
        'Replace hidden without a candidate',
        'the core always names a candidate for a name conflict; the hidden '
            'state needs a conflict without a candidate, which the core does not produce',
      );
      await s.go(AppPage.library);
    },
  );
}

// Library helpers ----------------------------------------------------------------

bool _isRowKey(Key? key) {
  if (key is! ValueKey<String>) {
    return false;
  }
  final value = key.value;
  return value.startsWith('row-') && !value.startsWith('row-menu-');
}

/// The installed-app uuids of the rows on screen, top to bottom.
List<String> libraryRowUuids() {
  final finder = find.byWidgetPredicate(
    (widget) => _isRowKey(widget.key),
    skipOffstage: true,
  );
  final rows = <MapEntry<double, String>>[];
  for (final element in finder.evaluate()) {
    final key = (element.widget.key! as ValueKey<String>).value;
    final box = element.renderObject as RenderBox?;
    final top = box?.localToGlobal(Offset.zero).dy ?? 0;
    rows.add(MapEntry(top, key.substring('row-'.length)));
  }
  rows.sort((a, b) => a.key.compareTo(b.key));
  return [for (final row in rows) row.value];
}

Future<void> searchFor(WidgetTester tester, String text) =>
    typeInto(tester, 'search-field', text);

Future<void> chooseFilter(WidgetTester tester, String label) => tap(
  tester,
  find
      .descendant(
        of: find.byType(AppSegmented<LibraryFilter>),
        matching: find.text(label, skipOffstage: true),
      )
      .first,
);

Future<void> chooseSort(WidgetTester tester, String label) async {
  await tapKey(tester, 'sort-menu');
  await tap(
    tester,
    find.widgetWithText(PopupMenuItem<SortOrder>, label, skipOffstage: true),
  );
}

/// Integrates a fixture through the Inspect page, expecting success.
Future<void> integrateOk(WidgetTester tester, Session s, String path) async {
  final result = await integrateThroughGui(tester, s, path);
  expect(result, startsWith('Integrated '), reason: result);
  await settle(tester, ms: 300);
}

/// The managed file name the core gives a fixture: spaces become underscores
/// (observed: "Ledgerline 3.3.AppImage" is installed as "Ledgerline_3.3.AppImage").
String managedNameOf(String fixturePath) =>
    p.basename(fixturePath).replaceAll(' ', '_');

AppDto appForFile(AppModel m, String fixturePath) {
  final name = managedNameOf(fixturePath);
  for (final app in m.library) {
    if (p.basename(app.managedPath) == name) {
      return app;
    }
  }
  throw TestFailure('no installed app for $name');
}

Process? runningApp;

/// Opens the Detail page of an app by tapping its name in the Library.
Future<void> openDetail(WidgetTester tester, Session s, String uuid) async {
  // A detail left open by an earlier failed check would hide the library rows.
  if (s.m.selectedUuid != null) {
    s.m.closeDetail();
    await settle(tester, ms: 300);
  }
  await s.go(AppPage.library);
  await tapKey(tester, 'open-$uuid');
  expect(s.m.selectedUuid, uuid);
}

Future<void> closeDetail(WidgetTester tester, Session s) async {
  await tapKey(tester, 'breadcrumb-library');
  expect(s.m.selectedUuid, isNull);
}

// Case group: library with seven apps, filters, search and sort ------------------

Future<void> caseRunLibrary(WidgetTester tester, Session s, Fixtures f) async {
  await runCase('B-05', 'Browse and Ctrl+O reach the Inspect flow', () async {
    await s.go(AppPage.library);
    await check(
      'INV-020',
      'Library Browse opens the chooser and inspects the pick',
      () async {
        FilePickers.openAppImages = () async => [f.brisk];
        await tapKey(tester, 'browse');
        await until(
          tester,
          () =>
              s.m.busy == null &&
              (s.m.inspect.inspectedOk || s.m.inspect.error.isNotEmpty),
          what: 'browse inspection',
        );
        expect(s.m.page, AppPage.inspect);
        expect(s.m.inspect.inspectedOk, isTrue, reason: s.m.inspect.error);
        FilePickers.openAppImages = () async => <String>[];
        await tapKey(tester, 'integrate');
        await until(
          tester,
          () => s.m.busy == null && s.m.library.length == 3,
          what: 'brisk integration',
        );
      },
    );
    await check('INV-117', 'Ctrl+O opens the chooser', () async {
      var calls = 0;
      FilePickers.openAppImages = () async {
        calls += 1;
        return <String>[f.cinder];
      };
      await s.go(AppPage.library);
      await focusShell(tester);
      await press(tester, LogicalKeyboardKey.keyO, ctrl: true);
      expect(calls, 1, reason: 'the chooser was not opened by Ctrl+O');
      await until(
        tester,
        () => s.m.busy == null && s.m.inspect.inspectedOk,
        what: 'ctrl-o inspection',
      );
      expect(s.m.inspect.pathInput, f.cinder);
      FilePickers.openAppImages = () async => <String>[];
      await tapKey(tester, 'integrate');
      await until(
        tester,
        () => s.m.busy == null && s.m.library.length == 4,
        what: 'cinder integration',
      );
    });
    await check(
      'INV-072',
      'Enter in the path field integrates a second file',
      () async {
        await integrateOk(tester, s, f.unavailable);
        expect(s.m.library.length, 5);
      },
    );
    await check(
      'INV-072',
      'Eclair (Unicode name) integrates by path',
      () async {
        await integrateOk(tester, s, f.eclair);
        expect(s.m.library.length, 6);
        expect(appForFile(s.m, f.eclair).name, isNotEmpty);
      },
    );
    await check('INV-072', 'Ledgerline integrates by path', () async {
      await integrateOk(tester, s, f.ledger);
      expect(s.m.library.length, 7);
    });
    await check('INV-072', 'the seven fixture apps are installed', () {
      expect(s.m.library.length, 7);
    });
  });

  await runCase('B-03', 'library heading, status bar and running app', () async {
    await s.go(AppPage.library);
    await check('INV-019', 'heading subtitle counts the apps', () {
      expect(showsTextContaining('7 apps in'), isTrue);
    });
    await check('INV-110', 'status bar counts the installed apps', () {
      expect(showsText('7 installed'), isTrue);
    });
    await check('INV-018', 'managed folder box follows the library', () {
      expect(showsTextContaining('apps ·'), isTrue);
    });
    // The running app: a copy of sleep at the managed path, started from there.
    await check('INV-037', 'a running app shows the Running badge', () async {
      final brisk = appForFile(s.m, f.brisk);
      final path = brisk.managedPath;
      runningApp = await tester.runAsync(() async {
        final process = await Process.start(path, ['600']);
        return process;
      });
      await tester.runAsync(() => s.m.loadLibrary());
      await settle(tester, ms: 400);
      expect(
        appForFile(s.m, f.brisk).running,
        isTrue,
        reason: 'the core did not see the process running from $path',
      );
      expect(showsText('Running'), isTrue);
    });
    await handshake(tester, 'library-populated');
    await capture(
      tester,
      p.join(kScratch, 'png', 'phaseB', 'library-populated.png'),
    );
  });

  await runCase('B-06', 'filters, search and sort on seven apps', () async {
    await s.go(AppPage.library);
    await check('INV-021', 'filter All shows every row', () {
      expect(libraryRowUuids().length, 7);
    });
    await check(
      'INV-022',
      'filter Updates is empty before any check',
      () async {
        await chooseFilter(tester, 'Updates');
        expect(s.m.filter, LibraryFilter.updates);
        expect(libraryRowUuids(), isEmpty);
        await chooseFilter(tester, 'All');
      },
    );
    await check(
      'INV-023',
      'filter Needs attention is empty before any check',
      () async {
        await chooseFilter(tester, 'Needs attention');
        expect(s.m.filter, LibraryFilter.attention);
        expect(libraryRowUuids(), isEmpty);
        await chooseFilter(tester, 'All');
        expect(libraryRowUuids().length, 7);
      },
    );
    await check(
      'INV-024',
      'search matches the name, case-insensitively',
      () async {
        await searchFor(tester, 'QUILL');
        expect(libraryRowUuids().length, 2);
        await searchFor(tester, '');
      },
    );
    await check('INV-024', 'search matches the version', () async {
      await searchFor(tester, '1.2.0');
      expect(libraryRowUuids().length, 2);
      await searchFor(tester, '');
    });
    await check('INV-024', 'search matches the path', () async {
      await searchFor(tester, 'AppImages');
      expect(libraryRowUuids().length, 7);
      await searchFor(tester, '');
    });
    await check('INV-024', 'search matches a name with spaces', () async {
      await searchFor(tester, 'Notes');
      expect(libraryRowUuids().length, 2);
      await searchFor(tester, '');
    });
    await check(
      'INV-024',
      'search matches Unicode (accented, lower case)',
      () async {
        await searchFor(tester, 'ñandú');
        expect(libraryRowUuids().length, 1);
        await searchFor(tester, 'ÉCLAIR');
        expect(
          libraryRowUuids().length,
          1,
          reason: 'Unicode upper-case search',
        );
        await searchFor(tester, '');
      },
    );
    await check('INV-024', 'spaces only search is treated as empty', () async {
      await searchFor(tester, '    ');
      expect(libraryRowUuids().length, 7);
      await searchFor(tester, '');
    });
    await check('INV-045', 'no match shows the echoed search text', () async {
      await searchFor(tester, 'zzz-none');
      expect(libraryRowUuids(), isEmpty);
      expect(showsText('No AppImage matches “zzz-none”.'), isTrue);
      await searchFor(tester, '');
    });
    await check(
      'INV-024',
      'a 2000 character search does not break the list',
      () async {
        await searchFor(tester, 'q' * 2000);
        expect(libraryRowUuids(), isEmpty);
        expect(present(byKey('search-field')), isTrue);
        await searchFor(tester, '');
        expect(libraryRowUuids().length, 7);
      },
    );
    await check('INV-024', 'special characters match literally', () async {
      await searchFor(tester, r'[](){}*.+?');
      expect(libraryRowUuids(), isEmpty);
      await searchFor(tester, '');
    });
    await check('INV-025', 'sort by Version reorders the rows', () async {
      await chooseSort(tester, 'Version');
      expect(s.m.sort, SortOrder.version);
      // Natural version order, ties by name (R6-06): 0.9 before 0.14.2.
      final expected = [...s.m.library]
        ..sort((a, b) {
          final byVersion = fmt.compareVersions(a.version, b.version);
          return byVersion != 0
              ? byVersion
              : a.name.toLowerCase().compareTo(b.name.toLowerCase());
        });
      expect(
        libraryRowUuids(),
        expected.map((app) => app.uuid).toList(),
        reason: 'rows are not in version order',
      );
    });
    await check('INV-025', 'sort by Name is A to Z', () async {
      await chooseSort(tester, 'Name');
      expect(s.m.sort, SortOrder.name);
      final expected = [...s.m.library]
        ..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
      expect(libraryRowUuids(), expected.map((app) => app.uuid).toList());
    });
    await check(
      'INV-025',
      'sort menu offers Name, Version and Updates first',
      () async {
        await tapKey(tester, 'sort-menu');
        expect(
          find
              .widgetWithText(PopupMenuItem<SortOrder>, 'Name')
              .evaluate()
              .length,
          1,
        );
        expect(
          find
              .widgetWithText(PopupMenuItem<SortOrder>, 'Version')
              .evaluate()
              .length,
          1,
        );
        expect(
          find
              .widgetWithText(PopupMenuItem<SortOrder>, 'Updates first')
              .evaluate()
              .length,
          1,
        );
        await press(tester, LogicalKeyboardKey.escape);
      },
    );
    await check('INV-026', 'table headers read APP, VERSION, STATUS', () {
      expect(showsText('APP'), isTrue);
      expect(showsText('VERSION'), isTrue);
      expect(showsText('STATUS'), isTrue);
    });
    await handshake(tester, 'library-filtered');
    await s.go(AppPage.library);
  });
}

// Case group: rows and row menus -------------------------------------------------

Future<void> caseRunRowsAndMenus(
  WidgetTester tester,
  Session s,
  Fixtures f,
) async {
  await runCase(
    'B-07',
    'library rows: open, launch, status, version',
    () async {
      final quill = appForFile(s.m, f.quill);
      await check(
        'INV-027',
        'tapping the app name opens its Detail page',
        () async {
          await openDetail(tester, s, quill.uuid);
          expect(present(byKey('breadcrumb-library')), isTrue);
          await closeDetail(tester, s);
        },
      );
      await check(
        'INV-028',
        'row Launch starts the app and reports the result',
        () async {
          await tapKey(tester, 'launch-${quill.uuid}');
          expect(s.m.status, isNotNull, reason: 'no status after Launch');
          _notes['launch.quill.status'] = s.m.status?.text ?? '';
          expect(
            s.m.status!.text.startsWith('Launched ') ||
                s.m.status!.text.startsWith('Launch failed:'),
            isTrue,
            reason: 'unexpected launch status: ${s.m.status!.text}',
          );
        },
      );
      await check(
        'INV-038',
        'a freshly integrated app does not claim Up to date before any check',
        () {
          final row = find.text('Up to date', skipOffstage: true);
          expect(
            row.evaluate().isEmpty,
            isTrue,
            reason:
                'the row reads "Up to date" but no update check has run '
                '(lastChecked is null in app_model.dart statusFor)',
          );
        },
      );
      await check('INV-038', 'the Running app row reads Running', () {
        expect(showsText('Running'), isTrue);
      });
      await check('INV-039', 'version column shows the installed version', () {
        expect(showsText('1.2.0'), isTrue);
      });
      blocked(
        'INV-039',
        'update arrow on a row',
        'needs a checked offer; covered in the Updates case',
      );
    },
  );

  await runCase('B-08', 'row menu items and their actions', () async {
    final quill = appForFile(s.m, f.quill);
    await check('INV-029', 'the row menu opens with its actions', () async {
      await tapKey(tester, 'row-menu-${quill.uuid}');
      expect(showsText('Launch'), isTrue);
      expect(showsText('Details'), isTrue);
      expect(showsText('Reveal in folder'), isTrue);
      expect(showsText('Refresh metadata'), isTrue);
      expect(showsText('Move to Trash'), isTrue);
      expect(showsText('Delete permanently'), isTrue);
      expect(
        showsText('Update…'),
        isFalse,
        reason: 'no offer, so no Update item',
      );
    });
    await check('INV-029', 'Escape closes the row menu', () async {
      await press(tester, LogicalKeyboardKey.escape);
      expect(showsText('Reveal in folder'), isFalse);
    });
    await check('INV-031', 'row menu Details opens Detail', () async {
      await tapKey(tester, 'row-menu-${quill.uuid}');
      await tapText(tester, 'Details');
      expect(s.m.selectedUuid, quill.uuid);
      await closeDetail(tester, s);
    });
    await check(
      'INV-034',
      'row menu Refresh metadata refreshes the record',
      () async {
        await tapKey(tester, 'row-menu-${quill.uuid}');
        throw TestFailure(
          'not run: in run2 the app process ended during this '
          'native refresh (no Dart error; see probe4 for the reproduction)',
        );
        // ignore: dead_code
        expect(s.m.status, isNotNull);
        // ignore: dead_code
        expect(
          s.m.status!.text,
          contains('Refreshed metadata for'),
          reason: s.m.status!.text,
        );
      },
    );
    await check('INV-030', 'row menu Launch starts the app', () async {
      await tapKey(tester, 'row-menu-${quill.uuid}');
      await tapText(tester, 'Launch');
      expect(s.m.status, isNotNull);
    });
    blocked(
      'INV-032',
      'row menu Reveal in folder',
      'calls xdg-open on the host, which would open a file manager window on the owner desktop',
    );
    await check(
      'INV-035',
      'row menu Move to Trash opens the trash confirmation',
      () async {
        await tapKey(tester, 'row-menu-${quill.uuid}');
        await tapText(tester, 'Move to Trash');
        expect(s.m.dialog, isA<RemoveDialog>());
        expect((s.m.dialog! as RemoveDialog).permanent, isFalse);
        await tapKey(tester, 'dialog-cancel');
        expect(s.m.dialog, isNull);
        expect(s.m.library.length, 7);
      },
    );
    await check(
      'INV-036',
      'row menu Delete permanently opens the permanent confirmation',
      () async {
        await tapKey(tester, 'row-menu-${quill.uuid}');
        await tapText(tester, 'Delete permanently');
        expect(s.m.dialog, isA<RemoveDialog>());
        expect((s.m.dialog! as RemoveDialog).permanent, isTrue);
        await tapKey(tester, 'dialog-cancel');
        expect(s.m.library.length, 7);
      },
    );
    blocked(
      'INV-033',
      'Update… in the row menu appears only with an offer',
      'exercised in the Updates case, after a check creates an offer',
    );
    await check('INV-029', 'row menus open on the running app too', () async {
      final brisk = appForFile(s.m, f.brisk);
      await tapKey(tester, 'row-menu-${brisk.uuid}');
      expect(showsText('Delete permanently'), isTrue);
      await press(tester, LogicalKeyboardKey.escape);
    });
    await handshake(tester, 'library-row-menu');
  });
}

// Case group: Detail page, launch options, update source -------------------------

Future<void> caseRunDetail(WidgetTester tester, Session s, Fixtures f) async {
  final quill = appForFile(s.m, f.quill);
  final unavailable = appForFile(s.m, f.unavailable);
  await runCase('B-09', 'Detail page: header, record, actions', () async {
    await openDetail(tester, s, quill.uuid);
    await handshake(tester, 'detail');
    await capture(tester, p.join(kScratch, 'png', 'phaseB', 'detail.png'));
    await check('INV-049', 'breadcrumb Library returns to the list', () async {
      await tapKey(tester, 'breadcrumb-library');
      expect(s.m.selectedUuid, isNull);
      await openDetail(tester, s, quill.uuid);
    });
    await check('INV-050', 'Reveal in folder button exists', () {
      expect(present(byKey('detail-reveal')), isTrue);
    });
    blocked('INV-050', 'Reveal in folder action', 'calls xdg-open on the host');
    await check(
      'INV-051',
      'Check for update reports what the source says',
      () async {
        await tapKey(tester, 'detail-check');
        expect(
          s.m.status,
          isNotNull,
          reason: 'no status after Check for update',
        );
        _notes['detail.check.quill'] = s.m.status!.text;
      },
    );
    await check('INV-052', 'Detail Launch starts the app', () async {
      await tapKey(tester, 'detail-launch');
      expect(s.m.status, isNotNull);
    });
    await check(
      'INV-055',
      'Refresh on the record card refreshes the metadata',
      () async {
        throw TestFailure(
          'not run: in run2 the app process ended during the '
          'native refresh of the row menu (see probe4); the record card action is the same call',
        );
        // ignore: dead_code
        expect(
          s.m.status!.text,
          contains('Refreshed metadata for'),
          reason: s.m.status!.text,
        );
      },
    );
    await check('INV-053', 'status line names the integration', () {
      expect(showsTextContaining('Integrated'), isTrue);
    });
    await check('INV-054', 'record card lists the file facts', () {
      for (final label in [
        'Path',
        'Desktop ID',
        'SHA-256',
        'Type',
        'Architecture',
        'Size',
        'Update source',
        'Provenance',
      ]) {
        expect(showsText(label), isTrue, reason: 'missing $label');
      }
    });
    await check('INV-054', 'Provenance names the source folder', () {
      expect(showsTextContaining('Integrated'), isTrue);
      expect(
        showsTextContaining(' from '),
        isTrue,
        reason: 'no source folder shown',
      );
    });
    await closeDetail(tester, s);
  });

  await runCase('B-10', 'launch options: arguments and environment', () async {
    await openDetail(tester, s, quill.uuid);
    await check('INV-056', 'arguments save, one per line', () async {
      await typeInto(tester, 'arguments-field', '--flag-one\n--flag-two');
      await tapKey(tester, 'launch-options-add');
      expect(s.m.status!.text, 'Saved arguments and environment');
      final saved = appForFile(s.m, f.quill).arguments;
      expect(saved, ['--flag-one', '--flag-two']);
    });
    await check(
      'INV-056',
      'arguments accept Unicode and a 2000 character line; blanks dropped',
      () async {
        await typeInto(
          tester,
          'arguments-field',
          '  \nÜnïcode-ärg\n${'x' * 2000}',
        );
        await tapKey(tester, 'launch-options-add');
        final saved = appForFile(s.m, f.quill).arguments;
        expect(saved.length, 2, reason: 'blank line should be dropped');
        expect(saved.first, 'Ünïcode-ärg');
        expect(saved.last.length, 2000);
        await typeInto(tester, 'arguments-field', '--flag-one\n--flag-two');
        await tapKey(tester, 'launch-options-add');
      },
    );
    await check(
      'INV-057',
      'an invalid environment name is refused and reported',
      () async {
        await typeInto(
          tester,
          'environment-field',
          'GOOD_NAME=1\n1BAD=2\nbad name=3',
        );
        await tapKey(tester, 'launch-options-add');
        expect(s.m.status!.severity, Severity.error);
        expect(s.m.status!.text, contains('1BAD'));
        expect(
          appForFile(s.m, f.quill).environment,
          isEmpty,
          reason: 'a refused save must not store any variable',
        );
      },
    );
    await check('INV-058', 'a valid environment saves and persists', () async {
      await typeInto(tester, 'environment-field', 'GOOD_NAME=1');
      await tapKey(tester, 'launch-options-add');
      expect(s.m.status!.text, 'Saved arguments and environment');
      final env = appForFile(s.m, f.quill).environment;
      expect(env.map((pair) => '${pair.name}=${pair.value}').toList(), [
        'GOOD_NAME=1',
      ]);
    });
    await check(
      'INV-057',
      'an environment value of 2000 characters is stored whole',
      () async {
        await typeInto(tester, 'environment-field', 'LONG_VALUE=${'v' * 2000}');
        await tapKey(tester, 'launch-options-add');
        final env = appForFile(s.m, f.quill).environment;
        expect(env.single.value.length, 2000);
        await typeInto(tester, 'environment-field', 'GOOD_NAME=1');
        await tapKey(tester, 'launch-options-add');
      },
    );
    await check(
      'INV-056',
      'saved arguments show again after leaving Detail',
      () async {
        await closeDetail(tester, s);
        await openDetail(tester, s, quill.uuid);
        final field = tester.widget<TextField>(byKey('arguments-field').first);
        expect(field.controller?.text, '--flag-one\n--flag-two');
        await closeDetail(tester, s);
      },
    );
  });

  await runCase('B-11', 'update source selector and key=value field', () async {
    await openDetail(tester, s, unavailable.uuid);
    await check(
      'INV-059',
      'source selector offers None and the six managers',
      () async {
        await tapKey(tester, 'source-selector');
        for (final name in [
          'None',
          'GitHub',
          'GitLab',
          'Codeberg',
          'Forgejo',
          'FTP',
          'Static',
        ]) {
          expect(
            find.widgetWithText(PopupMenuItem<String>, name).evaluate().length,
            1,
            reason: name,
          );
        }
        await press(tester, LogicalKeyboardKey.escape);
      },
    );
    await check('INV-059', 'choosing GitHub saves the manager', () async {
      await tapKey(tester, 'source-selector');
      await tap(
        tester,
        find.widgetWithText(
          PopupMenuItem<String>,
          'GitHub',
          skipOffstage: true,
        ),
      );
      await settle(tester, ms: 400);
      // Owner decision (round 3): a GitHub source needs filename=. The GUI refuses
      // the pair and names the exact --set-update-source command.
      expect(
        s.m.status!.text,
        contains('--set-update-source'),
        reason: s.m.status!.text,
      );
    });
    await check('INV-060', 'key=value field saves on Enter', () async {
      await typeInto(
        tester,
        'source-config-field',
        'repo=gosh-qa-nonexistent-0000/unavailable',
      );
      await submitField(tester);
      expect(
        s.m.status!.text,
        contains('--set-update-source'),
        reason: s.m.status!.text,
      );
      final pairs = appForFile(s.m, f.unavailable).updateConfig;
      expect(pairs, isEmpty, reason: 'a refused pair must not be stored');
    });
    await check('INV-061', 'source help sentence is shown', () {
      expect(showsTextContaining('One key=value pair'), isTrue);
    });
    await check('INV-059', 'choosing None removes the source', () async {
      await tapKey(tester, 'source-selector');
      await tap(
        tester,
        find.widgetWithText(PopupMenuItem<String>, 'None', skipOffstage: true),
      );
      await settle(tester, ms: 400);
      expect(s.m.status!.text, 'Update source removed');
      expect(appForFile(s.m, f.unavailable).updateManager, isEmpty);
    });
    await check(
      'INV-060',
      'a line without = is dropped without a message',
      () async {
        await typeInto(tester, 'source-config-field', 'not-a-pair');
        await submitField(tester);
        expect(
          s.m.status!.severity,
          Severity.success,
          reason:
              'the save reports success for an input it did not store: '
              '"${s.m.status!.text}"',
        );
      },
    );
    await check(
      'INV-059',
      'set GitHub again for the unavailable app',
      () async {
        await tapKey(tester, 'source-selector');
        await tap(
          tester,
          find.widgetWithText(
            PopupMenuItem<String>,
            'GitHub',
            skipOffstage: true,
          ),
        );
        await settle(tester, ms: 300);
        await typeInto(
          tester,
          'source-config-field',
          'repo=gosh-qa-nonexistent-0000/unavailable',
        );
        await submitField(tester);
        // Owner decision (round 3): the GUI refuses a GitHub pair and names the command.
        expect(
          s.m.status!.text,
          contains('--set-update-source'),
          reason: s.m.status!.text,
        );
      },
    );
    await closeDetail(tester, s);
  });
}

// Case group: dialogs, trash and permanent removal ------------------------------

Future<void> caseRunDialogs(WidgetTester tester, Session s, Fixtures f) async {
  final eclair = appForFile(s.m, f.eclair);
  final ledger = appForFile(s.m, f.ledger);
  final quillCopies = s.m.library
      .where((app) => app.name.startsWith('Quill'))
      .toList();
  await runCase('B-13', 'dialogs: open and close repeatedly, Escape, scrim, focus', () async {
    await openDetail(tester, s, eclair.uuid);
    for (var i = 0; i < 3; i++) {
      await check(
        'INV-064',
        'trash dialog opens and Cancel closes it (cycle ${i + 1})',
        () async {
          await tapKey(tester, 'detail-trash');
          expect(s.m.dialog, isA<RemoveDialog>());
          expect(
            showsText('Move Éclair Ñandú 0.9 to the Trash?'),
            isTrue,
            reason: 'title does not name the app',
          );
          await tapKey(tester, 'dialog-cancel');
          expect(s.m.dialog, isNull);
          expect(File(eclair.managedPath).existsSync(), isTrue);
        },
      );
    }
    await check('INV-070', 'Escape dismisses the dialog', () async {
      await tapKey(tester, 'detail-trash');
      expect(s.m.dialog, isNotNull);
      await press(tester, LogicalKeyboardKey.escape);
      expect(s.m.dialog, isNull);
    });
    await check(
      'INV-069',
      'a tap on the scrim does not dismiss the dialog',
      () async {
        await tapKey(tester, 'detail-trash');
        await tester.tapAt(const Offset(4, 4));
        await settle(tester);
        expect(s.m.dialog, isNotNull, reason: 'the scrim dismissed the dialog');
        await tapKey(tester, 'dialog-cancel');
      },
    );
    await check('INV-122', 'Tab does not move focus out of an open dialog', () async {
      await tapKey(tester, 'detail-trash');
      final seen = await traverse(tester, count: 12);
      final outside = seen.where((key) => !key.startsWith('dialog-')).toList();
      expect(
        outside,
        isEmpty,
        reason:
            'focus reached controls behind the dialog: ${outside.toSet().toList()}',
      );
      await tapKey(tester, 'dialog-cancel');
    });
    await check(
      'INV-062',
      'Detail Delete… opens the permanent confirmation',
      () async {
        await tapKey(tester, 'detail-delete');
        expect(s.m.dialog, isA<RemoveDialog>());
        expect((s.m.dialog! as RemoveDialog).permanent, isTrue);
        await tapKey(tester, 'dialog-cancel');
      },
    );
    await handshake(tester, 'dialog-trash');
    await capture(
      tester,
      p.join(kScratch, 'png', 'phaseB', 'dialog-trash.png'),
    );
    await check(
      'INV-064',
      'trash confirm moves the app to the Trash, not delete',
      () async {
        await tapKey(tester, 'detail-trash');
        await tapKey(tester, 'dialog-confirm');
        await until(
          tester,
          () => s.m.busy == null && s.m.library.length == 6,
          what: 'trash removal',
        );
        expect(
          File(eclair.managedPath).existsSync(),
          isFalse,
          reason: 'file still at managed path',
        );
        final trashed = File(
          p.join(trashFilesDir(), p.basename(eclair.managedPath)),
        );
        expect(
          trashed.existsSync(),
          isTrue,
          reason: 'not found in the scratch Trash at ${trashed.path}',
        );
        expect(s.m.status!.text, 'Removed', reason: s.m.status!.text);
      },
    );
    await check('INV-064', 'trash removal leaves no app in the library', () {
      expect(s.m.library.any((app) => app.uuid == eclair.uuid), isFalse);
    });
    await check('INV-065', 'permanent delete: Cancel keeps the file', () async {
      await openDetail(tester, s, ledger.uuid);
      await tapKey(tester, 'detail-delete');
      expect(s.m.dialog, isA<RemoveDialog>());
      await tapKey(tester, 'dialog-cancel');
      expect(File(ledger.managedPath).existsSync(), isTrue);
    });
    await handshake(tester, 'dialog-permanent');
    await check(
      'INV-065',
      'permanent delete confirm deletes the file',
      () async {
        await tapKey(tester, 'detail-delete');
        await tapKey(tester, 'dialog-confirm');
        await until(
          tester,
          () => s.m.busy == null && s.m.library.length == 5,
          what: 'permanent removal',
        );
        expect(File(ledger.managedPath).existsSync(), isFalse);
        expect(s.m.status!.text, 'Removed', reason: s.m.status!.text);
      },
    );
    await check('INV-064', 'a failed Trash never deletes the file (AGENTS rule)', () async {
      // An earlier failed check can leave a dialog open; clear it first.
      s.m.dismissDialog();
      await settle(tester, ms: 200);
      // Make the Trash location a regular file so the trash cannot be written.
      final trashDir = Directory(p.join(dataHome, 'Trash'));
      final backup = Directory(p.join(dataHome, 'Trash.qa-backup'));
      if (trashDir.existsSync()) {
        trashDir.renameSync(backup.path);
      }
      File(p.join(dataHome, 'Trash')).writeAsStringSync('blocker');
      try {
        final copy = quillCopies.last;
        await openDetail(tester, s, copy.uuid);
        await tapKey(tester, 'detail-trash');
        await tapKey(tester, 'dialog-confirm');
        await until(
          tester,
          () => s.m.busy == null && s.m.status != null,
          what: 'failed trash',
        );
        _notes['trash.fail.status'] = s.m.status!.text;
        expect(
          File(copy.managedPath).existsSync(),
          isTrue,
          reason: 'the file was deleted although the Trash failed',
        );
        expect(
          s.m.status!.severity,
          Severity.error,
          reason: 'the failure was not reported: ${s.m.status!.text}',
        );
      } finally {
        File(p.join(dataHome, 'Trash')).deleteSync();
        if (backup.existsSync()) {
          backup.renameSync(trashDir.path);
        }
      }
      await closeDetail(tester, s);
    });
  });
}

// Case group: keyboard -----------------------------------------------------------

Future<void> caseRunKeyboard(WidgetTester tester, Session s, Fixtures f) async {
  await runCase(
    'B-16',
    'keyboard shortcuts, focus, Enter and Space, drop',
    () async {
      await s.go(AppPage.library);
      await check('INV-118', 'Ctrl+R reloads the library', () async {
        final before = s.core.listCalls;
        await focusShell(tester);
        await press(tester, LogicalKeyboardKey.keyR, ctrl: true);
        await settle(tester, ms: 300);
        expect(s.core.listCalls, greaterThan(before));
      });
      await check('INV-119', 'F5 reloads the library', () async {
        final before = s.core.listCalls;
        await focusShell(tester);
        await press(tester, LogicalKeyboardKey.f5);
        await settle(tester, ms: 300);
        expect(s.core.listCalls, greaterThan(before));
      });
      await check(
        'INV-120',
        'Ctrl+F checks for updates (undocumented)',
        () async {
          final before = s.core.checkCalls;
          await focusShell(tester);
          await press(tester, LogicalKeyboardKey.keyF, ctrl: true);
          await until(
            tester,
            () => s.m.busy == null && s.core.checkCalls > before,
            what: 'Ctrl+F check',
          );
          expect(s.core.checkCalls, greaterThan(before));
        },
      );
      await check(
        'INV-122',
        'Tab moves focus through the library controls',
        () async {
          await s.go(AppPage.library);
          final seen = (await traverse(tester, count: 30)).toSet();
          _notes['tab.library.reached'] = seen.join(', ');
          expect(seen.contains('browse'), isTrue, reason: 'Browse not reached');
          expect(
            seen.contains('search-field'),
            isTrue,
            reason: 'search not reached',
          );
          expect(
            seen.contains('sort-menu'),
            isTrue,
            reason: 'sort not reached',
          );
        },
      );
      await check('INV-122', 'Tab reaches the sidebar navigation', () async {
        await s.go(AppPage.library);
        final seen = (await traverse(tester, count: 40)).toSet();
        final nav = seen.where((key) => key.startsWith('nav-')).toList();
        expect(
          nav,
          isNotEmpty,
          reason:
              'the sidebar items are GestureDetectors and cannot take focus; '
              'keys reached: ${seen.toList()}',
        );
      });
      await check('INV-122', 'Shift+Tab moves focus backwards', () async {
        await s.go(AppPage.library);
        await traverse(tester, count: 6);
        final forward = focusedKey();
        final back = await traverse(tester, count: 2, backwards: true);
        expect(back.last, isNot(forward));
      });
      await check(
        'INV-123',
        'Enter on a focused primary button activates it',
        () async {
          await s.go(AppPage.inspect);
          await typeInto(tester, 'inspect-path', f.cinder);
          final reached = <String>[];
          for (var i = 0; i < 8 && focusedKey() != 'inspect-run'; i++) {
            await press(tester, LogicalKeyboardKey.tab);
            reached.add(focusedKey());
          }
          expect(focusedKey(), 'inspect-run', reason: 'Tab path: $reached');
          // A button takes Enter as a key event (ActivateIntent), not as text input.
          await press(tester, LogicalKeyboardKey.enter);
          await until(
            tester,
            () => s.m.busy == null && s.m.inspect.inspectedOk,
            what: 'enter inspect',
          );
          await tapKey(tester, 'inspect-cancel');
        },
      );
      await check(
        'INV-124',
        'Space toggles a focused settings toggle',
        () async {
          await s.go(AppPage.settings);
          final before = s.m.settings?.moveSource ?? false;
          final reached = <String>[];
          for (var i = 0; i < 40 && focusedKey() != 'toggle-move'; i++) {
            await press(tester, LogicalKeyboardKey.tab);
            reached.add(focusedKey());
          }
          expect(
            focusedKey(),
            'toggle-move',
            reason:
                'toggle-move did not take focus (AppToggle is focusable since '
                'QA D-06); keys reached: ${reached.toSet().toList()}',
          );
          await press(tester, LogicalKeyboardKey.space);
          expect(s.m.settings?.moveSource, !before);
          await press(tester, LogicalKeyboardKey.space);
        },
      );
      await check('INV-125', 'Enter in the size field applies it', () async {
        await typeInto(tester, 'max-size-field', '');
        await submitField(tester);
        expect(s.m.status!.text, 'Maximum size cannot be empty');
      });
      await check('INV-126', 'a dropped AppImage opens in Inspect', () async {
        await s.go(AppPage.library);
        await simulateDrop(tester, [f.weird]);
        await until(
          tester,
          () =>
              s.m.inspect.pathInput == f.weird || s.m.inspect.error.isNotEmpty,
          what: 'drop',
        );
        expect(s.m.page, AppPage.inspect);
        expect(s.m.inspect.pathInput, f.weird);
        await tapKey(tester, 'inspect-cancel');
      });
      await check(
        'INV-126',
        'a drop of a non-AppImage reports the refusal',
        () async {
          await simulateDrop(tester, [f.textFile]);
          await until(
            tester,
            () => s.m.inspect.error.isNotEmpty || s.m.inspect.inspectedOk,
            what: 'drop text',
          );
          expect(s.m.inspect.error, isNotEmpty);
          await cancelInspectIfPresent(tester);
        },
      );
      await handshake(tester, 'inspect-after-drop');
    },
  );
}

/// The same method call desktop_drop's native side sends (performOperation_linux).
Future<void> simulateDrop(WidgetTester tester, List<String> paths) async {
  const codec = StandardMethodCodec();
  final uris = paths.map((path) => Uri.file(path).toString()).join('\n');
  final message = codec.encodeMethodCall(
    MethodCall('performOperation_linux', [
      uris,
      [100.0, 100.0],
    ]),
  );
  await tester.runAsync(() async {
    final done = Completer<void>();
    tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'desktop_drop',
      message,
      (ByteData? reply) {
        if (!done.isCompleted) {
          done.complete();
        }
      },
    );
    await done.future.timeout(const Duration(seconds: 5), onTimeout: () {});
  });
  await settle(tester, ms: 300);
}

// Case group: updates and tasks --------------------------------------------------

Future<void> caseRunUpdatesTasks(
  WidgetTester tester,
  Session s,
  Fixtures f,
) async {
  final brisk = appForFile(s.m, f.brisk);
  final cinder = appForFile(s.m, f.cinder);
  final unavailable = appForFile(s.m, f.unavailable);
  s.core.offers[brisk.uuid] = '1.2.0';
  s.core.offers[cinder.uuid] = '0.15.0';

  await runCase('B-17', 'Updates page: check, rows, summary, actions', () async {
    await s.go(AppPage.updates);
    await check('INV-082', 'subtitle reads Not checked yet before a check', () {
      expect(showsText('Not checked yet'), isTrue);
    });
    await handshake(tester, 'updates');
    await capture(tester, p.join(kScratch, 'png', 'phaseB', 'updates.png'));
    await check('INV-083', 'Check now checks every source', () async {
      final before = s.core.checkCalls;
      await tapKey(tester, 'check-now');
      await until(
        tester,
        () => s.m.busy == null && s.m.lastChecked != null,
        what: 'check now',
      );
      expect(s.core.checkCalls, greaterThan(before));
      expect(showsTextContaining('Last checked today at'), isTrue);
    });
    await check('INV-111', 'status bar counts the updates', () {
      expect(showsText('2 updates'), isTrue);
    });
    await check('INV-112', 'status bar counts the failed checks in red', () {
      _notes['check.failures'] = '${s.m.checkFailures.length}';
      expect(
        s.m.checkFailures.length,
        1,
        reason:
            'expected the unavailable source to fail; failures: '
            '${s.m.checkFailures.map((f) => '${f.name}: ${f.error}').join(' | ')}',
      );
      expect(showsText('1 check failed'), isTrue);
    });
    await check('INV-113', 'status bar shows the last check time', () {
      expect(showsTextContaining('Last checked '), isTrue);
    });
    await check(
      'INV-085',
      'summary cards count available, up to date and unknown',
      () {
        expect(
          find.byKey(const Key('summary-available')).evaluate().isNotEmpty,
          isTrue,
        );
        final available = find.descendant(
          of: byKey('summary-available'),
          matching: find.text('2'),
        );
        final unknown = find.descendant(
          of: byKey('summary-unknown'),
          matching: find.text('1'),
        );
        final upToDate = find.descendant(
          of: byKey('summary-up-to-date'),
          matching: find.text('4'),
        );
        expect(available.evaluate().length, 1);
        expect(unknown.evaluate().length, 1);
        expect(upToDate.evaluate().length, 1);
      },
    );
    await check('INV-086', 'the running update row reads App is running', () {
      expect(showsText('App is running'), isTrue);
    });
    await check('INV-088', 'the failed row offers Retry', () {
      expect(present(byKey('retry-${unavailable.uuid}')), isTrue);
    });
    await check('INV-082', 'subtitle reads the check time after a check', () {
      expect(showsTextContaining('Last checked today at'), isTrue);
    });
    blocked(
      'INV-089',
      'Some updates did not apply list',
      'only applyAllUpdates records failed updates; it needs a real failing source',
    );
    await check(
      'INV-086',
      'Update… on a stopped app runs the update',
      () async {
        await tapKey(tester, 'update-${cinder.uuid}');
        await until(
          tester,
          () => s.m.busy == null && s.m.status != null,
          what: 'update',
        );
        _notes['update.cinder.status'] = s.m.status!.text;
        expect(s.m.status!.text, isNotEmpty);
      },
    );
    await check(
      'INV-067',
      'Update… on a running app asks first and Cancel changes nothing',
      () async {
        await tapKey(tester, 'update-${brisk.uuid}');
        expect(s.m.dialog, isA<UpdateForceDialog>());
        expect(
          showsText('Brisk Terminal 1.1.4 is running'),
          isTrue,
          reason: 'title does not name the running app',
        );
        await tapKey(tester, 'dialog-cancel');
        expect(s.m.dialog, isNull);
        expect(appForFile(s.m, f.brisk).running, isTrue);
      },
    );
    await handshake(tester, 'dialog-force');
    await capture(
      tester,
      p.join(kScratch, 'png', 'phaseB', 'dialog-force.png'),
    );
    await check(
      'INV-067',
      'Update anyway confirms and runs the forced update',
      () async {
        await tapKey(tester, 'update-${brisk.uuid}');
        await tapKey(tester, 'dialog-confirm');
        await until(
          tester,
          () => s.m.busy == null && s.m.status != null,
          what: 'forced update',
        );
        _notes['update.brisk.status'] = s.m.status!.text;
        expect(s.m.status!.text, isNotEmpty);
      },
    );
    await check('INV-088', 'Retry runs the check again', () async {
      final before = s.core.checkCalls;
      await tapKey(tester, 'retry-${unavailable.uuid}');
      await until(
        tester,
        () => s.m.busy == null && s.core.checkCalls > before,
        what: 'retry',
      );
      expect(s.core.checkCalls, greaterThan(before));
    });
    await check(
      'INV-084',
      'Update all with offers reports its outcome',
      () async {
        await tapKey(tester, 'update-all');
        await until(
          tester,
          () => s.m.busy == null && s.m.status != null,
          what: 'update all',
        );
        _notes['update.all.status'] = s.m.status!.text;
        expect(s.m.status!.text, isNotEmpty);
      },
    );
    await check(
      'INV-084',
      'Update all with nothing to update says so',
      () async {
        s.core.offers.clear();
        await tapKey(tester, 'check-now');
        await until(
          tester,
          () => s.m.busy == null && s.m.updates.isEmpty,
          what: 'empty check',
        );
        await tapKey(tester, 'update-all');
        expect(s.m.status!.text, 'Nothing to update');
      },
    );
    await check(
      'INV-111',
      'filter Updates lists the offers (after a check)',
      () async {
        s.core.offers[brisk.uuid] = '1.2.0';
        s.core.offers[cinder.uuid] = '0.15.0';
        await tapKey(tester, 'check-now');
        await until(
          tester,
          () => s.m.busy == null && s.m.updates.length == 2,
          what: 'offers',
        );
        await s.go(AppPage.library);
        await chooseFilter(tester, 'Updates');
        expect(libraryRowUuids().length, 2);
        await chooseFilter(tester, 'Needs attention');
        expect(libraryRowUuids().length, 1);
        await chooseFilter(tester, 'All');
        await chooseSort(tester, 'Updates first');
        final ordered = libraryRowUuids();
        expect(ordered.take(2).toSet(), {
          brisk.uuid,
          cinder.uuid,
        }, reason: 'updates should sort first');
        await chooseSort(tester, 'Name');
      },
    );
    await check(
      'INV-039',
      'the version cell shows the arrow to the new version',
      () {
        expect(showsText('→ 0.15.0'), isTrue);
      },
    );
    await check(
      'INV-040',
      'the update arrow reads the new version (Brisk)',
      () {
        expect(showsText('→ 1.2.0'), isTrue);
      },
    );
    await check(
      'INV-038',
      'a failed check reads Check failed in the library',
      () {
        expect(showsText('Check failed'), isTrue);
      },
    );
    await check(
      'INV-038',
      'an update available row reads Update available',
      () {
        expect(showsText('Update available'), isTrue);
      },
    );
    await check(
      'INV-029',
      'the row menu offers Update… when an offer exists',
      () async {
        await s.go(AppPage.library);
        await tapKey(tester, 'row-menu-${cinder.uuid}');
        expect(showsText('Update…'), isTrue);
        await press(tester, LogicalKeyboardKey.escape);
      },
    );
    await check('INV-033', 'row menu Update… runs the update', () async {
      await tapKey(tester, 'row-menu-${cinder.uuid}');
      await tapText(tester, 'Update…');
      await until(
        tester,
        () => s.m.busy == null && s.m.status != null,
        what: 'row update',
      );
      expect(s.m.status!.text, isNotEmpty);
    });
    await check('INV-022', 'updates filter count matches the offers', () async {
      await chooseFilter(tester, 'Updates');
      expect(libraryRowUuids().length, s.m.updates.length);
      await chooseFilter(tester, 'All');
    });
    await handshake(tester, 'library-updates');
    blocked(
      'INV-087',
      'Cancel on a running update',
      'no update runs long enough to cancel offline; Cancel on a running task is INV-093',
    );
  });

  await runCase(
    'B-18',
    'Tasks page: list, times, cancel, clear finished',
    () async {
      await s.go(AppPage.tasks);
      await handshake(tester, 'tasks');
      await capture(tester, p.join(kScratch, 'png', 'phaseB', 'tasks.png'));
      await check('INV-091', 'subtitle counts running and finished tasks', () {
        expect(showsTextContaining('running,'), isTrue);
        expect(showsTextContaining('finished'), isTrue);
      });
      await check('INV-094', 'finished rows name the work and its time', () {
        expect(showsTextContaining('Integrated Quill Notes'), isTrue);
        expect(showsTextContaining('Today '), isTrue);
      });
      await check(
        'INV-094',
        'a removal to the Trash reads as moved to the Trash',
        () {
          expect(showsTextContaining('Moved Éclair'), isTrue);
        },
      );
      await check(
        'INV-094',
        'a permanent delete does not read as moved to the Trash',
        () {
          expect(
            showsTextContaining('Moved Ledgerline'),
            isFalse,
            reason: 'the Ledgerline permanent delete is listed as "Moved … to the Trash"',
          );
        },
      );
      await check(
        'INV-093',
        'Cancel stops a running task (Tasks page)',
        () async {
          final big = f.bigLong;
          await s.go(AppPage.inspect);
          await typeInto(tester, 'inspect-path', big);
          await tapKey(tester, 'inspect-run');
          await s.go(AppPage.tasks);
          Finder cancel() => find.byWidgetPredicate(
            (widget) =>
                widget.key is ValueKey<String> &&
                (widget.key! as ValueKey<String>).value.startsWith(
                  'task-cancel-',
                ),
            skipOffstage: true,
          );
          await until(
            tester,
            () => cancel().evaluate().isNotEmpty || s.m.busy == null,
            timeoutMs: 20000,
            what: 'running task',
          );
          // Duplicate listing check: a running task must appear once on Tasks.
          final runningCards = find
              .byWidgetPredicate(
                (widget) =>
                    widget.key is ValueKey<String> &&
                    (widget.key! as ValueKey<String>).value.startsWith(
                      'running-',
                    ),
                skipOffstage: true,
              )
              .evaluate()
              .length;
          _notes['tasks.running.cards'] = '$runningCards';
          if (runningCards > 0) {
            record(
              'TASKDUP',
              'a running task is listed once on Tasks',
              runningCards == 1 ? 'PASS' : 'FAIL',
              'listed $runningCards times',
            );
          }
          if (cancel().evaluate().isEmpty) {
            blocked(
              'INV-093',
              'Cancel stops a running task',
              'the inspection finished before the Tasks page showed it running',
            );
            return;
          }
          await tap(tester, cancel());
          await until(
            tester,
            () => s.m.busy == null,
            timeoutMs: 20000,
            what: 'cancel',
          );
          await settle(tester, ms: 400);
          expect(showsTextContaining('Inspected Big Stub'), isTrue);
          expect(
            showsTextContaining('cancelled'),
            isTrue,
            reason: 'a cancelled inspection should read cancelled, not failed',
          );
        },
      );
      await check(
        'INV-092',
        'Clear finished removes finished work only',
        () async {
          await s.go(AppPage.tasks);
          await tapKey(tester, 'clear-finished');
          await settle(tester, ms: 300);
          expect(s.m.finishedTasks, isEmpty);
          expect(showsTextContaining('0 finished'), isTrue);
        },
      );
      await check(
        'INV-092',
        'Clear finished is disabled when nothing has finished',
        () {
          final buttons = find.byWidgetPredicate(
            (widget) =>
                widget is AppButton &&
                widget.buttonKey == const Key('clear-finished'),
            skipOffstage: true,
          );
          expect(buttons.evaluate().length, 1);
          final button = buttons.evaluate().first.widget as AppButton;
          expect(button.onPressed, isNull);
        },
      );
      await check('INV-095', 'Tasks empty text after clearing', () {
        expect(s.m.tasks.where((task) => s.m.isFinishedTask(task)), isEmpty);
      });
      await s.go(AppPage.library);
    },
  );
}

// Case group: settings and adopt -------------------------------------------------

Future<void> caseRunSettings(WidgetTester tester, Session s, Fixtures f) async {
  _notes['settings.file.start'] = jsonEncode(readSettingsFile());
  await runCase('B-19', 'Settings: subtitle and theme', () async {
    await s.go(AppPage.settings);
    await check('INV-096', 'subtitle reads Options are off unless noted', () {
      expect(showsText('Options are off unless noted'), isTrue);
    });
    await handshake(tester, 'settings');
    await capture(tester, p.join(kScratch, 'png', 'phaseB', 'settings.png'));
    // Ends on Dark: the restart check expects it to persist.
    for (final entry in [
      ('System', 'system', ThemeMode.system),
      ('Light', 'light', ThemeMode.light),
      ('Dark', 'dark', ThemeMode.dark),
    ]) {
      await check(
        'INV-097',
        'theme ${entry.$1} saves and applies live',
        () async {
          await tap(
            tester,
            find
                .descendant(
                  of: find.byType(AppSegmented<AppearanceChoice>),
                  matching: find.text(entry.$1),
                )
                .first,
          );
          await settle(
            tester,
            ms: 1500,
          ); // the save is queued behind the core lock
          expect(settingNamed('appearance'), entry.$2);
          expect(
            tester.widget<MaterialApp>(find.byType(MaterialApp)).themeMode,
            entry.$3,
          );
        },
      );
    }
  });

  await runCase('B-20', 'managed folder: Change…', () async {
    await s.go(AppPage.settings);
    await check('INV-098', 'Change… picks a folder and saves it', () async {
      Directory(f.altManaged).createSync(recursive: true);
      FilePickers.openFolder = () async => f.altManaged;
      await tapKey(tester, 'change-folder');
      await settle(tester, ms: 1500);
      expect(
        s.m.status?.text,
        'Managed folder updated',
        reason: s.m.status?.text,
      );
      expect(settingNamed('managed_folder'), f.altManaged);
    });
    await check(
      'INV-098',
      'a file picked as the managed folder is refused',
      () async {
        FilePickers.openFolder = () async => f.textFile;
        await tapKey(tester, 'change-folder');
        await settle(tester, ms: 1500);
        expect(
          s.m.status?.severity,
          Severity.error,
          reason:
              'a regular file was accepted as the managed folder: '
              '${s.m.status?.text}',
        );
        expect(settingNamed('managed_folder'), f.altManaged);
      },
    );
    await check('INV-098', 'a cancelled chooser changes nothing', () async {
      FilePickers.openFolder = () async => null;
      await tapKey(tester, 'change-folder');
      await settle(tester, ms: 1500);
      expect(settingNamed('managed_folder'), f.altManaged);
    });
    await check(
      'INV-098',
      'the new folder shows in the sidebar and the library heading',
      () async {
        await s.go(AppPage.settings);
        expect(showsTextContaining('managed-alt'), isTrue);
      },
    );
    FilePickers.openFolder = () async => null;
  });

  await runCase('B-21', 'maximum file size: validation and save', () async {
    await s.go(AppPage.settings);
    final cases = <(String, String?)>[
      ('', 'Maximum size cannot be empty'),
      ('   ', 'Maximum size cannot be empty'),
      ('abc', '"abc" is not a whole number of megabytes'),
      ('1.5', '"1.5" is not a whole number of megabytes'),
      ('0', 'Enter 1–32768 MB (default 8192)'),
      ('32769', 'Enter 1–32768 MB (default 8192)'),
      (
        '99999999999999999999',
        '"99999999999999999999" is not a whole number of megabytes',
      ),
    ];
    for (final (input, message) in cases) {
      await check(
        'INV-099',
        'max size "$input" is refused with its message',
        () async {
          await typeInto(tester, 'max-size-field', input);
          await submitField(tester);
          expect(s.m.status?.text, message);
          expect(s.m.status?.severity, Severity.error);
        },
      );
    }
    await check('INV-099', 'a valid size (512) saves in bytes', () async {
      await typeInto(tester, 'max-size-field', '  512  ');
      await submitField(tester);
      expect(s.m.status?.text, 'Maximum size updated');
      expect(s.m.settings?.maxAppimageBytes, 512 * 1024 * 1024);
      expect(settingNamed('max_appimage_bytes'), 512 * 1024 * 1024);
    });
    await check(
      'INV-099',
      'a typed size without Enter is not applied',
      () async {
        await typeInto(tester, 'max-size-field', '999');
        expect(s.m.settings?.maxAppimageBytes, 512 * 1024 * 1024);
        await typeInto(tester, 'max-size-field', '512');
      },
    );
  });

  await runCase(
    'B-22',
    'toggles: move, discover, terminal, verbose, unsafe',
    () async {
      await s.go(AppPage.settings);
      final toggles = <(String, String, bool Function(), String)>[
        (
          'toggle-move',
          'move_source',
          () => s.m.settings?.moveSource ?? false,
          'INV-100',
        ),
        (
          'toggle-terminal',
          'terminal_omit_suffix',
          () => s.m.settings?.terminalOmitSuffix ?? false,
          'INV-102',
        ),
        (
          'toggle-debug',
          'debug_logging',
          () => s.m.settings?.debugLogging ?? false,
          'INV-105',
        ),
      ];
      for (final (key, fileKey, read, inv) in toggles) {
        await check(inv, '$key flips on and off and is saved', () async {
          final before = read();
          await tapKey(tester, key);
          expect(read(), !before);
          expect(
            settingNamed(fileKey),
            !before,
            reason: 'settings file $fileKey',
          );
          await tapKey(tester, key);
          expect(read(), before);
          await tapKey(tester, key);
          expect(read(), !before);
        });
      }
      // Owner decision: the unsafe fallback is an enabled, opt-in switch. A tap
      // asks first, and Cancel leaves it off.
      await check(
        'INV-106',
        'the unsafe extraction toggle asks first, and Cancel leaves it off',
        () async {
          await tapKey(tester, 'toggle-unsafe');
          expect(s.m.dialog, isNotNull);
          await tapKey(tester, 'dialog-cancel');
          expect(s.m.dialog, isNull);
          expect(s.m.settings?.unsafeExtractionFallback, isFalse);
          expect(
            settingNamed('unsafe_extraction_fallback'),
            anyOf(false, 'missing'),
          );
          expect(showsText('Unavailable'), isFalse);
        },
      );
    },
  );

  await runCase('B-12', 'discovery and adoption', () async {
    await s.go(AppPage.settings);
    final desktop = File(
      p.join(dataHome, 'applications', 'qa-adopt-me.desktop'),
    );
    desktop.parent.createSync(recursive: true);
    desktop.writeAsStringSync(
      '[Desktop Entry]\nType=Application\nName=Adopt Me\nExec="${f.adoptable}" %U\n',
    );
    await check(
      'INV-101',
      'Discover AppImages elsewhere toggles and reloads the library',
      () async {
        final before = s.core.listCalls;
        await tapKey(tester, 'toggle-discover');
        expect(s.m.settings?.manageOutsideFolder, isTrue);
        expect(settingNamed('manage_outside_folder'), isTrue);
        expect(s.core.listCalls, greaterThan(before));
      },
    );
    await s.go(AppPage.library);
    await check(
      'INV-040',
      'an AppImage outside the folder appears under Not managed yet',
      () {
        expect(
          showsText('Not managed yet'),
          isTrue,
          reason:
              'the adoptable AppImage did not appear; discovered: '
              '${s.m.discovered.map((d) => d.path).join(', ')}',
        );
      },
    );
    final beforeAdopt = s.m.library.length;
    await check('INV-068', 'Adopt dialog: Cancel registers nothing', () async {
      await tapKey(tester, 'adopt-button-${f.adoptable}');
      expect(s.m.dialog, isA<AdoptDialog>());
      await tapKey(tester, 'dialog-cancel');
      expect(s.m.library.length, beforeAdopt);
    });
    await handshake(tester, 'dialog-adopt');
    await check(
      'INV-068',
      'Adopt registers the app and changes nothing on disk',
      () async {
        await tapKey(tester, 'adopt-button-${f.adoptable}');
        await tapKey(tester, 'dialog-confirm');
        await until(
          tester,
          () => s.m.busy == null && s.m.library.length == beforeAdopt + 1,
          what: 'adopt',
        );
        expect(
          File(f.adoptable).existsSync(),
          isTrue,
          reason: 'adopt must not move the file',
        );
        expect(s.m.status?.severity, Severity.success);
      },
    );
    await check('INV-031', 'an adopted app shows the Adopted status', () {
      expect(showsText('Adopted'), isTrue);
    });
  });

  await runCase('B-23', 'background checks and login entry', () async {
    await s.go(AppPage.settings);
    await check(
      'INV-104',
      'login toggle is disabled while background checks are off',
      () async {
        expect(settingNamed('background_update_checks'), isFalse);
        await tapKey(tester, 'toggle-login');
        expect(
          File(autostartPath()).existsSync(),
          isFalse,
          reason: 'the login entry was written while background checks are off',
        );
      },
    );
    await check(
      'INV-103',
      'background checks on, then login entry on',
      () async {
        await tapKey(tester, 'toggle-background');
        expect(settingNamed('background_update_checks'), isTrue);
        await tapKey(tester, 'toggle-login');
        expect(File(autostartPath()).existsSync(), isTrue);
        expect(
          s.m.status?.text,
          'Update checks will run at login (notify only)',
        );
      },
    );
    await check(
      'INV-103',
      'background off removes the login entry first',
      () async {
        await tapKey(tester, 'toggle-background');
        expect(settingNamed('background_update_checks'), isFalse);
        expect(
          File(autostartPath()).existsSync(),
          isFalse,
          reason:
              'the login entry survived the background checks being turned off',
        );
      },
    );
    await check(
      'INV-103',
      'background checks back on, login back on for the restart check',
      () async {
        await tapKey(tester, 'toggle-background');
        await tapKey(tester, 'toggle-login');
        expect(File(autostartPath()).existsSync(), isTrue);
      },
    );
    await handshake(tester, 'settings-after-toggles');
  });
}

// Case group: About --------------------------------------------------------------

Future<void> caseRunAbout(WidgetTester tester, Session s, Fixtures f) async {
  await runCase('B-24', 'About page: identity, statements, licence', () async {
    await s.go(AppPage.about);
    await handshake(tester, 'about');
    await capture(tester, p.join(kScratch, 'png', 'phaseB', 'about.png'));
    await check(
      'INV-107',
      'identity card shows version, ID, homepage and purpose',
      () {
        expect(showsText('Application ID'), isTrue);
        expect(showsText('com.goshapps.AppImageManager'), isTrue);
        expect(showsText('https://goshapps.com'), isTrue);
        expect(showsText('Made by'), isTrue);
        expect(
          showsTextContaining('Native application for safely inspecting'),
          isTrue,
        );
        expect(s.m.version, isNotNull);
        expect(showsText(s.m.version!), isTrue);
      },
    );
    await check(
      'INV-108',
      'About this application, Credits and License cards',
      () {
        expect(
          showsTextContaining(
            'Opening an AppImage never integrates or executes it',
          ),
          isTrue,
        );
        expect(showsText('No telemetry of any kind.'), isTrue);
        expect(showsTextContaining('Gear Lever by Lorenzo Paderi'), isTrue);
        expect(
          showsTextContaining('GNU General Public License, version 3'),
          isTrue,
        );
      },
    );
    await check('INV-109', 'footer names the licence', () {
      expect(showsText('Licensed under GPL-3.0-or-later.'), isTrue);
    });
  });
}

// Case group: narrow frame -------------------------------------------------------

Future<void> caseRunNarrow(WidgetTester tester, Session s, Fixtures f) async {
  await setSize(tester, const Size(360, 640));
  try {
    await runCase(
      'B-14',
      'narrow frame: menu drawer, Browse, window controls',
      () async {
        await s.go(AppPage.library);
        await check('INV-007', 'the menu button opens the drawer', () async {
          await tapKey(tester, 'menu-button');
          expect(present(byKey('menu-scrim')), isTrue);
        });
        await check(
          'INV-011',
          'a drawer item opens its page and closes the drawer',
          () async {
            await tapKey(tester, 'nav-tasks');
            expect(s.m.page, AppPage.tasks);
            expect(present(byKey('menu-scrim')), isFalse);
          },
        );
        await check('INV-010', 'the scrim closes the drawer', () async {
          await s.go(AppPage.library);
          await tapKey(tester, 'menu-button');
          await tester.tapAt(const Offset(340, 400));
          await settle(tester);
          expect(present(byKey('menu-scrim')), isFalse);
        });
        await check('INV-010', 'Escape closes the drawer', () async {
          await tapKey(tester, 'menu-button');
          await press(tester, LogicalKeyboardKey.escape);
          expect(present(byKey('menu-scrim')), isFalse);
        });
        await check(
          'INV-008',
          'narrow Browse uses the chooser seam and cancels cleanly',
          () async {
            var calls = 0;
            FilePickers.openAppImages = () async {
              calls += 1;
              return <String>[];
            };
            await tapKey(tester, 'browse-narrow');
            expect(calls, 1);
            FilePickers.openAppImages = () async => <String>[];
          },
        );
        await check('INV-009', 'narrow title keeps a maximize control', () {
          expect(present(byKey('window-maximize')), isTrue);
        });
        blocked(
          'INV-009',
          'narrow minimize and close',
          'close ends the run; minimize is dispatched at the end of the run',
        );
        await check('INV-046', 'narrow search placeholder', () {
          expect(showsText('Search name, version, or path'), isTrue);
        });
        await check(
          'INV-047',
          'narrow filter tabs read All, Updates and Attention with counts',
          () {
            expect(showsText('All 7'), isTrue);
            expect(showsTextContaining('Updates '), isTrue);
            expect(showsTextContaining('Attention '), isTrue);
          },
        );
        await check('INV-048', 'a narrow row opens Detail on tap', () async {
          await tapKey(tester, 'open-${appForFile(s.m, f.quill).uuid}');
          expect(s.m.selectedUuid, appForFile(s.m, f.quill).uuid);
          await closeDetail(tester, s);
        });
        await check('INV-048', 'a narrow row keeps its menu', () {
          expect(
            present(byKey('row-menu-${appForFile(s.m, f.quill).uuid}')),
            isTrue,
          );
        });
        await check('INV-048', 'the narrow status line keeps the counts', () {
          expect(showsTextContaining('7 installed'), isTrue);
        });
        await handshake(tester, 'narrow-menu');
        await capture(
          tester,
          p.join(kScratch, 'png', 'phaseB', 'narrow-library.png'),
        );
      },
    );
  } finally {
    await restoreSize(tester);
  }
}

// Case group: session-only state -------------------------------------------------

Future<void> caseRunSessionState(
  WidgetTester tester,
  Session s,
  Fixtures f,
) async {
  await runCase(
    'B-26',
    'session-only state before the restart check',
    () async {
      await s.go(AppPage.library);
      await searchFor(tester, 'Quill');
      await chooseFilter(tester, 'Updates');
      await chooseSort(tester, 'Version');
      await check(
        'INV-024',
        'search, filter and sort are set for the restart check',
        () {
          expect(s.m.search, 'Quill');
          expect(s.m.filter, LibraryFilter.updates);
          expect(s.m.sort, SortOrder.version);
        },
      );
      final snapshot = {
        'apps': [
          for (final app in s.m.library)
            {
              'uuid': app.uuid,
              'name': app.name,
              'managedPath': app.managedPath,
              'arguments': app.arguments,
              'environment': [
                for (final e in app.environment) '${e.name}=${e.value}',
              ],
              'updateManager': app.updateManager,
            },
        ],
        'settings': {
          'appearance': s.m.settings?.appearance.name,
          'managedFolder': s.m.settings?.managedFolder,
          'maxAppimageBytes': s.m.settings?.maxAppimageBytes.toString(),
          'moveSource': s.m.settings?.moveSource,
          'manageOutsideFolder': s.m.settings?.manageOutsideFolder,
          'terminalOmitSuffix': s.m.settings?.terminalOmitSuffix,
          'backgroundUpdateChecks': s.m.settings?.backgroundUpdateChecks,
          'debugLogging': s.m.settings?.debugLogging,
          'autostart': s.m.settings?.autostartEnabled,
        },
      };
      _notes['snapshot.json'] = jsonEncode(snapshot);
      final runDir = Directory(p.join(kScratch, 'runs', 'full'))
        ..createSync(recursive: true);
      File(
        p.join(runDir.path, 'state.json'),
      ).writeAsStringSync(const JsonEncoder.withIndent('  ').convert(snapshot));
    },
  );
}

// Phase C: layout sweep ----------------------------------------------------------

Future<void> caseRunLayoutSweep(
  WidgetTester tester,
  Session s,
  Fixtures f,
) async {
  final brisk = appForFile(s.m, f.brisk);
  final quill = appForFile(s.m, f.quill);
  for (final size in kSizes) {
    final label = '${size.width.toInt()}x${size.height.toInt()}';
    caseId = 'C-$label';
    await setSize(tester, size);
    final states = <(String, Future<void> Function())>[
      ('library', () async => s.go(AppPage.library)),
      (
        'detail',
        () async {
          await s.go(AppPage.library);
          await tapKey(tester, 'open-${quill.uuid}');
        },
      ),
      (
        'inspect',
        () async {
          await s.go(AppPage.inspect);
          await typeInto(tester, 'inspect-path', f.cinder);
          await tapKey(tester, 'inspect-run');
          await until(
            tester,
            () => s.m.busy == null && s.m.inspect.inspectedOk,
            what: 'layout inspect',
          );
        },
      ),
      ('updates', () async => s.go(AppPage.updates)),
      ('tasks', () async => s.go(AppPage.tasks)),
      ('settings', () async => s.go(AppPage.settings)),
      ('about', () async => s.go(AppPage.about)),
      (
        'dialog-trash',
        () async {
          await s.go(AppPage.library);
          await tapKey(tester, 'open-${quill.uuid}');
          await tapKey(tester, 'detail-trash');
        },
      ),
      (
        'dialog-permanent',
        () async {
          await tapKey(tester, 'open-${quill.uuid}');
          await tapKey(tester, 'detail-delete');
        },
      ),
      (
        'dialog-conflict',
        () async {
          s.m.showDialog(
            IntegrateConflictDialog(
              path: f.quillDupB,
              conflictName: 'Quill Notes',
              incomingVersion: '1.2.0',
              installedVersion: '1.2.0',
              replaceUuid: quill.uuid,
              replaceLabel: 'Quill Notes',
            ),
          );
          await settle(tester, ms: 200);
        },
      ),
      (
        'dialog-force',
        () async {
          s.m.showDialog(UpdateForceDialog(uuid: brisk.uuid, name: brisk.name));
          await settle(tester, ms: 200);
        },
      ),
      (
        'dialog-adopt',
        () async {
          s.m.showDialog(AdoptDialog(path: f.adoptable));
          await settle(tester, ms: 200);
        },
      ),
    ];
    for (final (name, open) in states) {
      layoutTag = ' [$label $name]';
      layoutErrors.clear();
      try {
        s.m.dismissDialog();
        s.m.closeDetail();
        s.m.clearInspect();
        // Earlier cases leave the library search, filter and sort set. Clear
        // them so each state shows every app and its open target.
        s.m.setFilter(LibraryFilter.all);
        s.m.setSort(SortOrder.name);
        s.m.setSearch('');
        await s.go(AppPage.library);
        await settle(tester, ms: 100);
        await open();
        expectMediaQuery(tester, size);
        await capture(tester, p.join(kScratch, 'png', label, '$name.png'));
        final errors = List<String>.of(layoutErrors);
        record(
          'LAYOUT',
          '$label $name renders without overflow',
          errors.isEmpty ? 'PASS' : 'FAIL',
          errors.take(3).join(' | '),
        );
      } catch (error) {
        record('LAYOUT', '$label $name', 'FAIL', _short(error));
      }
      s.m.dismissDialog();
      await settle(tester, ms: 100);
    }
    layoutTag = '';
    if (size.width < narrowBreakpoint) {
      try {
        s.m.setFilter(LibraryFilter.all);
        s.m.setSort(SortOrder.name);
        s.m.setSearch('');
        await s.go(AppPage.library);
        await tapKey(tester, 'menu-button');
        expectMediaQuery(tester, size);
        await capture(
          tester,
          p.join(kScratch, 'png', label, 'narrow-menu.png'),
        );
        final errors = List<String>.of(layoutErrors);
        record(
          'LAYOUT',
          '$label narrow-menu renders without overflow',
          errors.isEmpty ? 'PASS' : 'FAIL',
          errors.take(3).join(' | '),
        );
        await press(tester, LogicalKeyboardKey.escape);
      } catch (error) {
        record('LAYOUT', '$label narrow-menu', 'FAIL', _short(error));
      }
    }
  }
  caseId = 'C-reset';
  await restoreSize(tester);
  await s.go(AppPage.library);
}

Future<void> caseRunFinalWindowControls(WidgetTester tester, Session s) async {
  await runCase(
    'B-01',
    'window controls (dispatch only; native effect not readable)',
    () async {
      await s.go(AppPage.library);
      await check(
        'INV-004',
        'maximize control toggles twice without error',
        () async {
          await tapKey(tester, 'window-maximize');
          await tapKey(tester, 'window-maximize');
          expect(present(byKey('window-maximize')), isTrue);
        },
      );
      blocked(
        'INV-005',
        'window close',
        'would end the test run; the close path is WindowChannel.close',
      );
      blocked(
        'INV-002',
        'title bar drag',
        'drag starts a native move; not injectable from the test',
      );
      blocked(
        'INV-006',
        'window edge resize grips',
        'native resize; not injectable from the test',
      );
      await check(
        'INV-003',
        'minimize control dispatches the window call',
        () async {
          await tapKey(tester, 'window-minimize');
          expect(present(byKey('window-minimize')), isTrue);
        },
      );
    },
  );
}

// Diagnostic probes (QA_MODE=probe): focus, typing and status checks, logged only.

Future<void> runProbes(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'P';
  await s.start();
  // P1: can typed text reach the Inspect path field?
  await s.go(AppPage.inspect);
  final path = byKey('inspect-path');
  logLine(
    'PROBE path field count=${path.evaluate().length} rect=${path.evaluate().isEmpty ? '-' : tester.getRect(path.first)}',
  );
  await tester.tap(path.first, warnIfMissed: false);
  await settle(tester);
  final focus = FocusManager.instance.primaryFocus;
  logLine(
    'PROBE after tap focus=${focus?.debugLabel} editable=${focus?.context?.findAncestorStateOfType<EditableTextState>() != null}',
  );
  await tester.enterText(path.first, '/tmp/qa-probe.AppImage');
  await settle(tester);
  final field = tester.widget<TextField>(path.first);
  logLine(
    'PROBE controller="${field.controller?.text}" model="${s.m.inspect.pathInput}"',
  );
  await tapKey(tester, 'inspect-run');
  logLine(
    'PROBE after inspect error="${s.m.inspect.error}" busy=${s.m.busy?.title}',
  );
  // P2: do shortcuts still work after leaving a text field?
  var calls = 0;
  FilePickers.openAppImages = () async {
    calls += 1;
    return <String>[];
  };
  await s.go(AppPage.settings);
  await tapKey(tester, 'max-size-field');
  logLine(
    'PROBE focus in max-size field=${FocusManager.instance.primaryFocus?.debugLabel}',
  );
  await tapKey(tester, 'nav-library');
  await press(tester, LogicalKeyboardKey.keyO, ctrl: true);
  logLine(
    'PROBE ctrl-o after leaving a text field calls=$calls focus=${FocusManager.instance.primaryFocus?.debugLabel}',
  );
  await tap(tester, find.text('Gosh AppImage Manager').first);
  await press(tester, LogicalKeyboardKey.keyO, ctrl: true);
  logLine('PROBE ctrl-o after clicking the title bar calls=$calls');
  FilePickers.openAppImages = () async => <String>[];
  // P3: a never-checked app: which status does the row show?
  final core = BridgeCore();
  final outcome = await tester.runAsync(
    () => core.integrateApp(
      opId: 'probe-1',
      sourcePath: f.quill,
      conflict: ConflictChoice.automatic,
      replaceUuid: '',
      moveSource: false,
    ),
  );
  logLine('PROBE core integrate ok=${outcome?.ok} message=${outcome?.message}');
  await tester.runAsync(() => s.m.loadLibrary());
  await s.go(AppPage.library);
  logLine(
    'PROBE row shows "Up to date"=${showsText('Up to date')} "Checked"=${showsTextContaining('Checked ')}',
  );
  _writeOutputs();
}

/// Replays the B-04 typed steps and logs what reached the model after each one.
Future<void> runProbes2(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'P2';
  await s.start();
  var calls = 0;
  FilePickers.openAppImages = () async {
    calls += 1;
    return <String>[];
  };
  // Baseline: shortcuts right after launch.
  await press(tester, LogicalKeyboardKey.keyO, ctrl: true);
  logLine('PROBE2 baseline ctrl-o on launch calls=$calls');
  await s.go(AppPage.inspect);
  final steps = <(String, String)>[
    ('typed', ''),
    ('typed', '     '),
    ('typed', f.quill),
    ('typed', '/tmp/${'a' * 2000}.AppImage'),
    ('typed', f.textFile),
    ('typed', f.weird),
    ('typed', f.bigStub),
  ];
  for (final (_, value) in steps) {
    await typeInto(tester, 'inspect-path', value);
    final model = s.m.inspect.pathInput;
    final shown =
        tester
            .widget<TextField>(byKey('inspect-path').first)
            .controller
            ?.text ??
        '';
    logLine(
      'PROBE2 typed len=${value.length} model_len=${model.length} shown_len=${shown.length} equal=${model == value}',
    );
    await tapKey(tester, 'inspect-run');
    logLine(
      'PROBE2 after run error="${s.m.inspect.error.length > 80 ? s.m.inspect.error.substring(0, 80) : s.m.inspect.error}" ok=${s.m.inspect.inspectedOk}',
    );
    if (present(byKey('inspect-cancel'))) {
      await tapKey(tester, 'inspect-cancel');
    }
  }
  FilePickers.openAppImages = () async => <String>[];
  _writeOutputs();
}

/// Reproduces the code-reading candidates (QA_MODE=probe3). Logged, not asserted.
Future<void> runProbes3(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'P3';
  await s.start();
  final core = BridgeCore();
  final vDir = p.join(kScratch, 'fixtures', 'versions');
  final alpha = makeAppImage(
    dir: vDir,
    fileName: 'Vtest Alpha 1.9.0.AppImage',
    appName: 'Vtest Alpha',
    version: '1.9.0',
  );
  final beta = makeAppImage(
    dir: vDir,
    fileName: 'Vtest Beta 1.10.0.AppImage',
    appName: 'Vtest Beta',
    version: '1.10.0',
  );
  Future<void> install(String path) async {
    final outcome = await tester.runAsync(
      () => core.integrateApp(
        opId: 'p3-${DateTime.now().microsecondsSinceEpoch}',
        sourcePath: path,
        conflict: ConflictChoice.automatic,
        replaceUuid: '',
        moveSource: false,
      ),
    );
    logLine(
      'PROBE3 install ${p.basename(path)} ok=${outcome?.ok} ${outcome?.message ?? ''}',
    );
  }

  await install(f.ledger);
  await install(f.unavailable);
  await install(alpha);
  await install(beta);
  final listed = (await tester.runAsync(() => core.listLibrary()))!;
  final unavailable = listed.apps.firstWhere(
    (a) => p.basename(a.managedPath) == managedNameOf(f.unavailable),
  );
  await tester.runAsync(
    () => core.setUpdateSource(
      uuid: unavailable.uuid,
      manager: 'github',
      config: [
        KeyValueDto(key: 'repo', value: 'gosh-qa-nonexistent-0000/unavailable'),
      ],
    ),
  );
  await tester.runAsync(() => s.m.loadLibrary());
  await settle(tester, ms: 300);

  // Permanent delete: which label does the Tasks page give it?
  await s.go(AppPage.library);
  final ledger = appForFile(s.m, f.ledger);
  await openDetail(tester, s, ledger.uuid);
  await tapKey(tester, 'detail-delete');
  await tapKey(tester, 'dialog-confirm');
  await until(
    tester,
    () => s.m.busy == null && s.m.library.every((a) => a.uuid != ledger.uuid),
    what: 'permanent delete',
  );
  await s.go(AppPage.tasks);
  logLine(
    'PROBE3 permanent delete: task label "Moved Ledgerline" shown=${showsTextContaining('Moved Ledgerline')} "to the Trash" shown=${showsTextContaining('to the Trash')}',
  );

  // Tab order: library, settings, and inside an open dialog.
  await s.go(AppPage.library);
  logLine(
    'PROBE3 tab order library: ${(await traverse(tester, count: 40)).toSet().toList()}',
  );
  await s.go(AppPage.settings);
  final settingsKeys = (await traverse(tester, count: 40)).toSet().toList();
  logLine('PROBE3 tab order settings: $settingsKeys');
  if (settingsKeys.contains('toggle-move')) {
    final before = s.m.settings?.moveSource;
    while (focusedKey() != 'toggle-move') {
      await press(tester, LogicalKeyboardKey.tab);
    }
    await press(tester, LogicalKeyboardKey.space);
    logLine(
      'PROBE3 space on focused toggle-move: before=$before after=${s.m.settings?.moveSource}',
    );
  } else {
    logLine(
      'PROBE3 toggle-move is not reachable by Tab, so Space cannot be pressed on it',
    );
  }
  await s.go(AppPage.library);
  final alphaApp = appForFile(s.m, alpha);
  await openDetail(tester, s, alphaApp.uuid);
  await tapKey(tester, 'detail-trash');
  logLine(
    'PROBE3 tab order inside the trash dialog: ${(await traverse(tester, count: 12)).toSet().toList()}',
  );
  await tapKey(tester, 'dialog-cancel');
  await closeDetail(tester, s);

  // Retry on a failing check.
  await s.go(AppPage.updates);
  await tapKey(tester, 'check-now');
  await until(
    tester,
    () => s.m.busy == null && s.m.lastChecked != null,
    what: 'probe check',
  );
  logLine(
    'PROBE3 failures=${s.m.checkFailures.map((x) => x.name).toList()} retryShown=${present(byKey('retry-${unavailable.uuid}'))}',
  );
  if (present(byKey('retry-${unavailable.uuid}'))) {
    final before = s.core.checkCalls;
    await tapKey(tester, 'retry-${unavailable.uuid}');
    await until(
      tester,
      () => s.m.busy == null && s.core.checkCalls > before,
      what: 'probe retry',
    );
    logLine(
      'PROBE3 retry ran a check: calls $before->${s.core.checkCalls} status=${s.m.status?.text}',
    );
  }

  // Version sort: 1.9.0 should come before 1.10.0.
  await s.go(AppPage.library);
  await tapKey(tester, 'sort-menu');
  await tap(
    tester,
    find.widgetWithText(
      PopupMenuItem<SortOrder>,
      'Version',
      skipOffstage: true,
    ),
  );
  final order = [
    for (final uuid in libraryRowUuids())
      s.m.library.firstWhere((a) => a.uuid == uuid).version,
  ];
  logLine('PROBE3 version sort order (versions, top to bottom): $order');

  // Size field: a typed value without Enter.
  await s.go(AppPage.settings);
  final fileBefore = settingNamed('max_appimage_bytes');
  await typeInto(tester, 'max-size-field', '999');
  logLine(
    'PROBE3 size typed without Enter: model="${s.m.maxBytesInput}" file=$fileBefore settings=${s.m.settings?.maxAppimageBytes}',
  );
  await tapText(tester, 'Maximum file size');
  logLine(
    'PROBE3 size after tapping the label: model="${s.m.maxBytesInput}" file=${settingNamed('max_appimage_bytes')} settings=${s.m.settings?.maxAppimageBytes}',
  );

  // Shortcuts after leaving a text field, then after one Tab.
  var calls = 0;
  FilePickers.openAppImages = () async {
    calls += 1;
    return <String>[];
  };
  await s.go(AppPage.library);
  await press(tester, LogicalKeyboardKey.keyO, ctrl: true);
  logLine(
    'PROBE3 ctrl-o after leaving the size field: calls=$calls focus=${FocusManager.instance.primaryFocus?.debugLabel}',
  );
  await focusShell(tester);
  await press(tester, LogicalKeyboardKey.keyO, ctrl: true);
  logLine('PROBE3 ctrl-o after one Tab: calls=$calls');
  FilePickers.openAppImages = () async => <String>[];
  _writeOutputs();
}

/// Reproduces the Refresh metadata crash seen in run2 (QA_MODE=probe4).
Future<void> runProbes4(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'P4';
  await s.start();
  final core = BridgeCore();
  final outcome = await tester.runAsync(
    () => core.integrateApp(
      opId: 'p4-install',
      sourcePath: f.quill,
      conflict: ConflictChoice.automatic,
      replaceUuid: '',
      moveSource: false,
    ),
  );
  logLine('PROBE4 installed ok=${outcome?.ok}');
  await tester.runAsync(() => s.m.loadLibrary());
  await s.go(AppPage.library);
  final quill = appForFile(s.m, f.quill);
  logLine(
    'PROBE4 before refresh: name=${quill.name} version="${quill.version}"',
  );
  await tapKey(tester, 'row-menu-${quill.uuid}');
  logLine('PROBE4 menu open, tapping Refresh metadata');
  await tapText(tester, 'Refresh metadata');
  await until(
    tester,
    () => s.m.busy == null && s.m.status != null,
    timeoutMs: 20000,
    what: 'refresh',
  );
  logLine(
    'PROBE4 after refresh: status="${s.m.status?.text}" severity=${s.m.status?.severity}',
  );
  _writeOutputs();
}

/// Isolates the two actions that ended run3 (QA_MODE=probe5): Enter in the size
/// field with an empty value, and a dropped file through the desktop_drop channel.
Future<void> runProbes5(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'P5';
  await s.start();
  await s.go(AppPage.settings);
  logLine('PROBE5 before size submit: status="${s.m.status?.text}"');
  await typeInto(tester, 'max-size-field', '');
  logLine('PROBE5 typed empty size, about to submit');
  await submitField(tester);
  logLine('PROBE5 after size submit: status="${s.m.status?.text}"');
  await s.go(AppPage.library);
  logLine('PROBE5 about to drop ${f.weird}');
  await simulateDrop(tester, [f.weird]);
  logLine(
    'PROBE5 after drop: page=${s.m.page} path="${s.m.inspect.pathInput}"',
  );
  _writeOutputs();
}

/// Phase C only (QA_MODE=layout): seven apps installed through the core, then
/// the layout sweep over every size and state. Used because the full run ends
/// early in B-16 (see the QA report).
Future<void> runLayoutOnly(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'C-setup';
  await s.start();
  final core = BridgeCore();
  for (final path in [
    f.quill,
    f.quillDupB,
    f.brisk,
    f.cinder,
    f.unavailable,
    f.eclair,
    f.ledger,
  ]) {
    final outcome = await tester.runAsync(
      () => core.integrateApp(
        opId: 'layout-${DateTime.now().microsecondsSinceEpoch}',
        sourcePath: path,
        conflict: ConflictChoice.keepBoth,
        replaceUuid: '',
        moveSource: false,
      ),
    );
    logLine('LAYOUT setup install ${p.basename(path)} ok=${outcome?.ok}');
  }
  await tester.runAsync(() => s.m.loadLibrary());
  await s.go(AppPage.library);
  await caseRunLayoutSweep(tester, s, f);
  _writeOutputs();
}

/// Detail and dialog groups alone on a populated library (QA_MODE=detail).
Future<void> runDetailOnly(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'D-setup';
  await s.start();
  final core = BridgeCore();
  for (final path in [
    f.quill,
    f.quillDupB,
    f.brisk,
    f.cinder,
    f.unavailable,
    f.eclair,
    f.ledger,
  ]) {
    await tester.runAsync(
      () => core.integrateApp(
        opId: 'detail-${DateTime.now().microsecondsSinceEpoch}',
        sourcePath: path,
        conflict: ConflictChoice.keepBoth,
        replaceUuid: '',
        moveSource: false,
      ),
    );
  }
  await tester.runAsync(() => s.m.loadLibrary());
  await s.go(AppPage.library);
  await guardGroup('B-09', () => caseRunDetail(tester, s, f));
  await guardGroup('B-13', () => caseRunDialogs(tester, s, f));
  _writeOutputs();
}

/// Keyboard group alone on a populated library (QA_MODE=keys), so a stop inside
/// it can be told apart from a stop caused by the earlier groups.
Future<void> runKeysOnly(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'K-setup';
  await s.start();
  final core = BridgeCore();
  for (final path in [
    f.quill,
    f.brisk,
    f.cinder,
    f.unavailable,
    f.eclair,
    f.ledger,
  ]) {
    await tester.runAsync(
      () => core.integrateApp(
        opId: 'keys-${DateTime.now().microsecondsSinceEpoch}',
        sourcePath: path,
        conflict: ConflictChoice.keepBoth,
        replaceUuid: '',
        moveSource: false,
      ),
    );
  }
  await tester.runAsync(() => s.m.loadLibrary());
  await s.go(AppPage.library);
  await guardGroup('B-16', () => caseRunKeyboard(tester, s, f));
  _writeOutputs();
}

/// Checks whether MediaQuery follows the surface size used by the sweep (QA_MODE=probe6).
Future<void> runProbes6(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'P6';
  await s.start();
  for (final size in [
    const Size(420, 1400),
    const Size(360, 480),
    const Size(1280, 800),
  ]) {
    await setSize(tester, size);
    await settle(tester, ms: 500);
    final mq = MediaQuery.sizeOf(kAppBoundary.currentContext!);
    logLine(
      'PROBE6 surface=${size.width.toInt()}x${size.height.toInt()} MediaQuery=${mq.width.toInt()}x${mq.height.toInt()} '
      'menuButton=${present(byKey('menu-button'))} browseHeader=${present(byKey('browse'))} '
      'browseNarrow=${present(byKey('browse-narrow'))}',
    );
  }
  await restoreSize(tester);
  _writeOutputs();
}

/// Owner decisions (round 3) at 800 px with seven apps (QA_MODE=layout800): library
/// rows at most 72 px high, and the Running badge on the version line.
Future<void> runLayout800(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'L800';
  await s.start();
  final core = BridgeCore();
  for (final path in [
    f.quill,
    f.quillDupB,
    f.brisk,
    f.cinder,
    f.unavailable,
    f.eclair,
    f.ledger,
  ]) {
    await tester.runAsync(
      () => core.integrateApp(
        opId: 'l800-${DateTime.now().microsecondsSinceEpoch}',
        sourcePath: path,
        conflict: ConflictChoice.keepBoth,
        replaceUuid: '',
        moveSource: false,
      ),
    );
  }
  await tester.runAsync(() => s.m.loadLibrary());
  final brisk = appForFile(s.m, f.brisk);
  runningApp = await tester.runAsync(
    () => Process.start(brisk.managedPath, ['600']),
  );
  await tester.runAsync(() => s.m.loadLibrary());
  await setSize(tester, const Size(800, 600));
  await s.go(AppPage.library);
  await settle(tester, ms: 500);
  // The list builds only the rows in view, so scroll through it and keep the
  // height of every row seen. All seven apps must be measured.
  final rowsInView = find.byWidgetPredicate(
    (widget) => _isRowKey(widget.key),
    skipOffstage: true,
  );
  final scrollList = find.ancestor(
    of: rowsInView.first,
    matching: find.byType(Scrollable),
  );
  final heights = <String, double>{};
  for (var pass = 0; pass < 8; pass++) {
    for (final element in rowsInView.evaluate()) {
      final box = element.renderObject;
      final key = element.widget.key;
      if (box is RenderBox && key is ValueKey<String>) {
        heights[key.value] = box.size.height;
      }
    }
    await tester.drag(scrollList.last, const Offset(0, -150));
    await settle(tester, ms: 300);
  }
  var tallest = 0.0;
  for (final height in heights.values) {
    if (height > tallest) {
      tallest = height;
    }
  }
  record(
    'QA2-ROW',
    'library rows at 800 px are 72 px or less (seven apps)',
    heights.length == 7 && tallest <= 72 ? 'PASS' : 'FAIL',
    '${heights.length} rows, tallest ${tallest.toStringAsFixed(1)} px',
  );
  await tester.drag(scrollList.last, const Offset(0, 2000));
  await settle(tester, ms: 300);
  final versionText = brisk.version.isEmpty ? 'no version' : brisk.version;
  final rowFinder = byKey('row-${brisk.uuid}');
  final badge = find.descendant(
    of: rowFinder,
    matching: find.text('Running', skipOffstage: true),
  );
  final version = find.descendant(
    of: rowFinder,
    matching: find.text(versionText, skipOffstage: true),
  );
  if (badge.evaluate().isEmpty || version.evaluate().isEmpty) {
    record(
      'QA2-BADGE',
      'Running badge sits on the version line at 800 px',
      'FAIL',
      'badge found=${badge.evaluate().isNotEmpty} version found=${version.evaluate().isNotEmpty}',
    );
  } else {
    final gap =
        (tester.getTopLeft(badge.first).dy -
                tester.getTopLeft(version.first).dy)
            .abs();
    record(
      'QA2-BADGE',
      'Running badge sits on the version line at 800 px',
      gap <= 4 ? 'PASS' : 'FAIL',
      'vertical gap ${gap.toStringAsFixed(1)} px',
    );
  }
  expectMediaQuery(tester, const Size(800, 600));
  await capture(tester, p.join(kScratch, 'png', 'l800', 'library-800x600.png'));
  _writeOutputs();
}

/// Builds a broken-payload AppImage-layout file: a real ELF with the AppImage
/// marker and no squashfs payload. Its code writes [markerPath] when it runs. Run
/// with --appimage-extract it also writes a squashfs-root desktop entry, which is
/// what the unsafe fallback reads.
String makeBrokenAppImage(String dir, String fileName, String markerPath) {
  Directory(dir).createSync(recursive: true);
  final source = p.join(dir, 'broken-payload.rs');
  File(source).writeAsStringSync('''
use std::fs;
fn main() {
    let _ = fs::write(r"$markerPath", b"ran\\n");
    let _ = fs::create_dir_all("squashfs-root");
    let _ = fs::write(
        "squashfs-root/broken-payload.desktop",
        "[Desktop Entry]\\nType=Application\\nName=Broken Payload App\\nExec=broken-payload %U\\nX-AppImage-Version=9.9\\nCategories=Utility;\\n",
    );
}
''');
  final binary = p.join(dir, 'broken-payload-bin');
  final built = Process.runSync('rustc', [
    '--edition',
    '2021',
    '-O',
    '-o',
    binary,
    source,
  ]);
  if (built.exitCode != 0) {
    throw StateError('rustc failed: ${built.stderr}');
  }
  final bytes = File(binary).readAsBytesSync();
  bytes.setRange(8, 11, [0x41, 0x49, 0x02]);
  final out = p.join(dir, fileName);
  File(out).writeAsBytesSync(bytes);
  Process.runSync('/usr/bin/chmod', ['+x', out]);
  return out;
}

/// Owner decision (round 4): the unsafe fallback asks per file before the AppImage
/// runs. Cancel runs nothing; Run anyway runs it and proceeds. Turning the setting
/// off asks nothing and nothing runs.
Future<void> caseRunUnsafe(WidgetTester tester, Session s, Fixtures f) async {
  final dir = p.join(kScratch, 'unsafe');
  final marker = p.join(dir, 'MARKER-ran.txt');
  Directory(dir).createSync(recursive: true);
  if (File(marker).existsSync()) {
    File(marker).deleteSync();
  }
  final broken = makeBrokenAppImage(dir, 'Broken Payload 9.9.AppImage', marker);
  await runCase(
    'U-01',
    'unsafe fallback confirmation on a broken payload',
    () async {
      await s.go(AppPage.settings);
      await check(
        'INV-106',
        'turning the fallback on asks first, and Turn on saves it',
        () async {
          await tapKey(tester, 'toggle-unsafe');
          expect(showsTextContaining('unsafe extraction fallback?'), isTrue);
          await tapText(tester, 'Turn on');
          await settle(tester, ms: 800);
          expect(s.m.settings?.unsafeExtractionFallback, isTrue);
        },
      );
      await check(
        'U-CONFIRM',
        'inspect asks "Run this AppImage to read it?" and names the file',
        () async {
          await inspectPath(tester, s, broken);
          await until(
            tester,
            () => s.m.dialog != null,
            timeoutMs: 15000,
            what: 'fallback dialog',
          );
          expect(s.m.dialog, isA<FallbackConfirmDialog>());
          expect(showsText('Run this AppImage to read it?'), isTrue);
          expect(showsTextContaining('Broken Payload 9.9.AppImage'), isTrue);
        },
      );
      await check(
        'U-CANCEL',
        'Cancel shows the safe result, runs nothing, leaves no marker',
        () async {
          await tapKey(tester, 'dialog-cancel');
          expect(s.m.dialog, isNull);
          expect(
            File(marker).existsSync(),
            isFalse,
            reason: 'the AppImage ran after Cancel',
          );
          expect(
            s.m.inspect.name,
            isEmpty,
            reason: 'the safe result carries no metadata',
          );
        },
      );
      await check(
        'U-RUN',
        'Run anyway runs it (marker present) and shows its metadata',
        () async {
          await inspectPath(tester, s, broken);
          await until(
            tester,
            () => s.m.dialog != null,
            timeoutMs: 15000,
            what: 'fallback dialog again',
          );
          await tapKey(tester, 'dialog-confirm');
          await until(
            tester,
            () => s.m.busy == null && s.m.inspect.name.isNotEmpty,
            timeoutMs: 60000,
            what: 'run anyway inspection',
          );
          expect(
            File(marker).existsSync(),
            isTrue,
            reason: 'Run anyway did not run the AppImage',
          );
          expect(s.m.inspect.name, 'Broken Payload App');
        },
      );
      await check(
        'U-INT-CANCEL',
        'Integrate: Cancel installs nothing and keeps the source',
        () async {
          await inspectPath(tester, s, broken);
          await until(
            tester,
            () => s.m.dialog != null,
            timeoutMs: 15000,
            what: 'dialog before integrate',
          );
          await tapKey(tester, 'dialog-cancel');
          final before = s.m.library.length;
          await tapKey(tester, 'integrate');
          await until(
            tester,
            () => s.m.dialog != null,
            timeoutMs: 30000,
            what: 'integrate fallback dialog',
          );
          expect(s.m.dialog, isA<FallbackConfirmDialog>());
          await tapKey(tester, 'dialog-cancel');
          await settle(tester, ms: 500);
          expect(
            s.m.library.length,
            before,
            reason: 'Cancel installed something',
          );
          expect(
            File(broken).existsSync(),
            isTrue,
            reason: 'the source was removed',
          );
        },
      );
      await check(
        'U-INT-RUN',
        'Integrate: Run anyway integrates the file',
        () async {
          final before = s.m.library.length;
          await tapKey(tester, 'integrate');
          await until(
            tester,
            () => s.m.dialog != null,
            timeoutMs: 30000,
            what: 'integrate dialog',
          );
          await tapKey(tester, 'dialog-confirm');
          await until(
            tester,
            () => s.m.busy == null && s.m.library.length == before + 1,
            timeoutMs: 60000,
            what: 'run anyway integration',
          );
          expect(s.m.library.length, before + 1);
        },
      );
      await check(
        'U-OFF',
        'turning the fallback off asks nothing; nothing runs',
        () async {
          await s.go(AppPage.settings);
          await tapKey(tester, 'toggle-unsafe');
          await settle(tester, ms: 800);
          expect(s.m.dialog, isNull, reason: 'turning it off asked something');
          expect(s.m.settings?.unsafeExtractionFallback, isFalse);
          if (File(marker).existsSync()) {
            File(marker).deleteSync();
          }
          await inspectPath(tester, s, broken);
          await settle(tester, ms: 800);
          expect(
            s.m.dialog,
            isNull,
            reason: 'a confirmation appeared with the fallback off',
          );
          expect(
            File(marker).existsSync(),
            isFalse,
            reason: 'the AppImage ran with the fallback off',
          );
        },
      );
    },
  );
}

Future<void> runUnsafeOnly(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'U-setup';
  await s.start();
  await guardGroup('U-01', () => caseRunUnsafe(tester, s, f));
  _writeOutputs();
}

// Restart run: persistence and session-only state -----------------------------------

/// Targeted re-check of the round 7 fixes, run with QA_MODE=recheck on a fresh
/// scratch root: QA3-006 Cancel (Tasks card and status bar), QA3-007 and QA3-008
/// at 800x600, and the INV-059 GitHub refusal shown on the Detail page.
Future<void> runRecheck(WidgetTester tester, Session s, Fixtures f) async {
  caseId = 'RC';
  await s.start();
  final core = BridgeCore();
  for (final path in [f.quill, f.brisk]) {
    await tester.runAsync(
      () => core.integrateApp(
        opId: 'rc-${DateTime.now().microsecondsSinceEpoch}',
        sourcePath: path,
        conflict: ConflictChoice.keepBoth,
        replaceUuid: '',
        moveSource: false,
      ),
    );
  }
  await tester.runAsync(() => s.m.loadLibrary());
  final quill = appForFile(s.m, f.quill);
  final brisk = appForFile(s.m, f.brisk);
  runningApp = await tester.runAsync(
    () => Process.start(brisk.managedPath, ['600']),
  );
  await tester.runAsync(() => s.m.loadLibrary());
  final big = makeBigImage(
    p.join(kScratch, 'recheck', 'big'),
    'Recheck Big 6.AppImage',
    gib: 6,
  );
  const small = Size(800, 600);

  // 800x600: the stacked Library row and the Detail remove card.
  await runCase('RC-LAYOUT', '800x600 library row and Detail remove card', () async {
    try {
      await setSize(tester, small);
      await s.go(AppPage.library);
      await settle(tester, ms: 500);
      final row = byKey('row-${brisk.uuid}');
      final version = find.descendant(
        of: row,
        matching: find.text('no version', skipOffstage: true),
      );
      final badge = find.descendant(
        of: row,
        matching: find.text('Running', skipOffstage: true),
      );
      if (version.evaluate().isEmpty || badge.evaluate().isEmpty) {
        record(
          'QA3-007',
          'no version beside the Running badge at 800x600',
          'FAIL',
          'version found=${version.evaluate().isNotEmpty}, '
              'badge found=${badge.evaluate().isNotEmpty}',
        );
      } else {
        final paragraph = tester.renderObject<RenderParagraph>(version.first);
        final natural = TextPainter(
          text: paragraph.text,
          textDirection: TextDirection.ltr,
          maxLines: 1,
        )..layout();
        final gap =
            (tester.getTopLeft(badge.first).dy -
                    tester.getTopLeft(version.first).dy)
                .abs();
        final whole =
            natural.width <= paragraph.size.width + 0.5 &&
            !paragraph.didExceedMaxLines;
        record(
          'QA3-007',
          'no version shown in full beside the Running badge at 800x600',
          whole && gap <= 4 ? 'PASS' : 'FAIL',
          'text ${natural.width.toStringAsFixed(1)} px in a '
              '${paragraph.size.width.toStringAsFixed(1)} px box; '
              'badge gap ${gap.toStringAsFixed(1)} px',
        );
      }
      expectMediaQuery(tester, small);
      await capture(
        tester,
        p.join(kScratch, 'png', 'recheck', 'library-800x600.png'),
      );

      await openDetail(tester, s, quill.uuid);
      await settle(tester, ms: 300);
      for (final key in ['detail-trash', 'detail-delete']) {
        try {
          await tester.ensureVisible(find.byKey(Key(key)).first);
        } catch (_) {
          // Not inside a scrollable: the rectangles below report it.
        }
      }
      await settle(tester, ms: 300);
      final trash = tester.getRect(find.byKey(const Key('detail-trash')).first);
      final remove = tester.getRect(
        find.byKey(const Key('detail-delete')).first,
      );
      final trashIn =
          trash.left >= 0 &&
          trash.top >= 0 &&
          trash.right <= small.width &&
          trash.bottom <= small.height;
      final removeIn =
          remove.left >= 0 &&
          remove.top >= 0 &&
          remove.right <= small.width &&
          remove.bottom <= small.height;
      record(
        'QA3-008',
        'Move to Trash and Delete fully inside the 800x600 window',
        trashIn && removeIn ? 'PASS' : 'FAIL',
        'Move to Trash ${trash.left.toStringAsFixed(0)},'
            '${trash.top.toStringAsFixed(0)} to ${trash.right.toStringAsFixed(0)},'
            '${trash.bottom.toStringAsFixed(0)}; Delete '
            '${remove.left.toStringAsFixed(0)},${remove.top.toStringAsFixed(0)} to '
            '${remove.right.toStringAsFixed(0)},${remove.bottom.toStringAsFixed(0)}',
      );
      expectMediaQuery(tester, small);
      await capture(
        tester,
        p.join(kScratch, 'png', 'recheck', 'detail-remove-800x600.png'),
      );
    } finally {
      await restoreSize(tester);
    }
  });

  // INV-059: a repo-only GitHub pair is refused on the Detail page, which must
  // show the exact command to run.
  await runCase(
    'RC-GH',
    'Detail refusal shows the --set-update-source command',
    () async {
      await openDetail(tester, s, quill.uuid);
      await tapKey(tester, 'source-selector');
      await tap(
        tester,
        find.widgetWithText(
          PopupMenuItem<String>,
          'GitHub',
          skipOffstage: true,
        ),
      );
      await settle(tester, ms: 400);
      await typeInto(tester, 'source-config-field', 'repo=owner/name');
      await submitField(tester);
      await until(
        tester,
        () => present(byKey('source-error')),
        timeoutMs: 8000,
        what: 'the refusal under the source field',
      );
      final shown = tester.widget<Text>(byKey('source-error')).data ?? '';
      record(
        'INV-059',
        'Detail refusal shows Run: gosh-appimage-manager --set-update-source',
        shown.contains('Run: gosh-appimage-manager --set-update-source')
            ? 'PASS'
            : 'FAIL',
        shown,
      );
      try {
        await tester.ensureVisible(byKey('source-error').first);
      } catch (_) {
        // Not inside a scrollable: the capture shows whatever is on screen.
      }
      await settle(tester, ms: 200);
      await capture(
        tester,
        p.join(kScratch, 'png', 'recheck', 'github-refusal.png'),
      );
    },
  );

  // QA3-006: Cancel on the Tasks card of a 6 GiB inspection.
  Finder taskCancel() => find.byWidgetPredicate(
    (widget) =>
        widget.key is ValueKey<String> &&
        (widget.key! as ValueKey<String>).value.startsWith('task-cancel-'),
    skipOffstage: true,
  );

  await runCase('RC-TASKS', 'Tasks Cancel stops a 6 GiB inspection', () async {
    await s.go(AppPage.inspect);
    await cancelInspectIfPresent(tester);
    await typeInto(tester, 'inspect-path', big);
    await tapKey(tester, 'inspect-run');
    await s.go(AppPage.tasks);
    await until(
      tester,
      () => present(taskCancel()),
      timeoutMs: 20000,
      what: 'a running task with a Cancel button',
    );
    final clock = Stopwatch()..start();
    await tap(tester, taskCancel());
    var stopped = true;
    try {
      await until(
        tester,
        () => s.m.busy == null,
        timeoutMs: 5000,
        what: 'the Tasks Cancel to stop the inspection',
      );
    } catch (_) {
      stopped = false;
    }
    record(
      'QA3-006',
      'Tasks Cancel stops the inspection within 5 s',
      stopped ? 'PASS' : 'FAIL',
      'busy cleared after ${clock.elapsedMilliseconds} ms',
    );
    var cardText = '';
    try {
      await until(
        tester,
        () => present(find.textContaining('ancelled', skipOffstage: true)),
        timeoutMs: 3000,
        what: 'the task card to read cancelled',
      );
      cardText =
          tester
              .widget<Text>(
                find.textContaining('ancelled', skipOffstage: true).first,
              )
              .data ??
          '';
    } catch (_) {
      // Recorded below as a failure.
    }
    record(
      'QA3-006',
      'the Tasks card reads cancelled',
      cardText.isNotEmpty ? 'PASS' : 'FAIL',
      cardText.isEmpty ? 'no cancelled text on the card' : cardText,
    );
    await capture(
      tester,
      p.join(kScratch, 'png', 'recheck', 'tasks-cancelled.png'),
    );
  });

  // QA3-006: the status-bar Cancel of a second 6 GiB inspection.
  await runCase('RC-BAR', 'status-bar Cancel stops a 6 GiB inspection', () async {
    await s.go(AppPage.inspect);
    await cancelInspectIfPresent(tester);
    await typeInto(tester, 'inspect-path', big);
    await tapKey(tester, 'inspect-run');
    await until(
      tester,
      () => present(byKey('busy-line')),
      timeoutMs: 20000,
      what: 'the status-bar busy line',
    );
    final cancels = find.text('Cancel', skipOffstage: true);
    expect(
      cancels.evaluate().isNotEmpty,
      isTrue,
      reason: 'no Cancel while busy',
    );
    final barY = tester.getCenter(byKey('busy-line')).dy;
    var pick = 0;
    var lowest = double.negativeInfinity;
    for (var i = 0; i < cancels.evaluate().length; i++) {
      final y = tester.getCenter(cancels.at(i)).dy;
      if (y > lowest) {
        lowest = y;
        pick = i;
      }
    }
    record(
      'QA3-006',
      'status-bar Cancel sits on the status line',
      (lowest - barY).abs() < 40 ? 'PASS' : 'FAIL',
      'cancel y ${lowest.toStringAsFixed(0)}, status y ${barY.toStringAsFixed(0)}',
    );
    final clock = Stopwatch()..start();
    await tap(tester, cancels.at(pick));
    var stopped = true;
    try {
      await until(
        tester,
        () => s.m.busy == null,
        timeoutMs: 5000,
        what: 'the status-bar Cancel to stop the inspection',
      );
    } catch (_) {
      stopped = false;
    }
    record(
      'QA3-006',
      'status-bar Cancel stops the inspection within 5 s',
      stopped ? 'PASS' : 'FAIL',
      'busy cleared after ${clock.elapsedMilliseconds} ms; status: '
          '${s.m.status?.text ?? ''}',
    );
    await settle(tester, ms: 300);
    await capture(
      tester,
      p.join(kScratch, 'png', 'recheck', 'status-bar-cancel.png'),
    );
    // QA3-009: what the Inspect page shows 3 s after the stop.
    await settle(tester, ms: 3000);
    final stopError = s.m.inspect.error;
    final integrate = find.widgetWithText(
      AppButton,
      'Integrate',
      skipOffstage: true,
    );
    final integrateOffered =
        integrate.evaluate().isNotEmpty &&
        tester.widget<AppButton>(integrate.first).onPressed != null;
    final cancelling =
        (s.m.status?.text ?? '').contains('Cancelling') ||
        showsTextContaining('Cancelling');
    final stopVisible =
        stopError.isNotEmpty &&
        showsTextContaining(
          stopError.length > 30 ? stopError.substring(0, 30) : stopError,
        );
    record(
      'QA3-009',
      'item 8: stop error shown, not OK',
      stopVisible && !s.m.inspect.inspectedOk ? 'PASS' : 'FAIL',
      'error "$stopError"; visible $stopVisible; '
          'inspectedOk ${s.m.inspect.inspectedOk}; '
          'status "${s.m.status?.text ?? ''}"',
    );
    record(
      'QA3-009',
      'item 9: no Integrate offered, no Cancelling shown',
      !integrateOffered && !cancelling ? 'PASS' : 'FAIL',
      'Integrate offered $integrateOffered; Cancelling shown $cancelling',
    );
    await capture(
      tester,
      p.join(kScratch, 'png', 'recheck', 'status-bar-cancel-3s.png'),
    );
  });
}

Future<void> runRestartChecks(
  WidgetTester tester,
  Session s,
  Fixtures f,
) async {
  await s.start();
  final snapshotFile = File(p.join(kScratch, 'runs', 'full', 'state.json'));
  final snapshot = snapshotFile.existsSync()
      ? jsonDecode(snapshotFile.readAsStringSync()) as Map<String, dynamic>
      : <String, dynamic>{};
  caseId = 'R-01';
  await runCase('R-01', 'restart: persisted settings and library', () async {
    await check('INV-097', 'appearance Dark persisted', () {
      expect(s.m.settings?.appearance, AppearanceChoice.dark);
      expect(
        tester.widget<MaterialApp>(find.byType(MaterialApp)).themeMode,
        ThemeMode.dark,
      );
    });
    final expectedSettings =
        (snapshot['settings'] as Map<String, dynamic>?) ?? {};
    await check('INV-099', 'maximum size persisted', () {
      expect(s.m.settings?.maxAppimageBytes, 512 * 1024 * 1024);
      expect(expectedSettings['maxAppimageBytes'] ?? '', isNotNull);
    });
    await check('INV-098', 'managed folder persisted', () {
      expect(s.m.settings?.managedFolder, f.altManaged);
    });
    await check('INV-100', 'move originals persisted', () {
      expect(s.m.settings?.moveSource, expectedSettings['moveSource']);
    });
    await check('INV-101', 'discover persisted', () {
      expect(
        s.m.settings?.manageOutsideFolder,
        expectedSettings['manageOutsideFolder'],
      );
    });
    await check('INV-102', 'terminal suffix persisted', () {
      expect(
        s.m.settings?.terminalOmitSuffix,
        expectedSettings['terminalOmitSuffix'],
      );
    });
    await check('INV-103', 'background checks persisted', () {
      expect(s.m.settings?.backgroundUpdateChecks, isTrue);
    });
    await check('INV-104', 'login entry persisted (autostart file)', () {
      expect(File(autostartPath()).existsSync(), isTrue);
      expect(s.m.settings?.autostartEnabled, isTrue);
    });
    await check('INV-105', 'verbose diagnostics persisted', () {
      expect(s.m.settings?.debugLogging, expectedSettings['debugLogging']);
    });
    final apps = (snapshot['apps'] as List<dynamic>?) ?? const [];
    await check('INV-038', 'library holds the same apps after restart', () {
      expect(s.m.library.length, apps.length);
    });
    await check('INV-056', 'arguments persisted in the registry', () {
      final quill = appForFile(s.m, f.quill);
      expect(quill.arguments, ['--flag-one', '--flag-two']);
    });
    await check('INV-057', 'environment persisted in the registry', () {
      final quill = appForFile(s.m, f.quill);
      expect(quill.environment.map((e) => '${e.name}=${e.value}').toList(), [
        'GOOD_NAME=1',
      ]);
    });
    // Owner decision (round 3): the GUI cannot store a GitHub source (CLI only).
    await check('INV-060', 'update source persisted (unavailable app)', () {
      expect(appForFile(s.m, f.unavailable).updateManager, isEmpty);
    });
    await check('INV-068', 'adopted app persisted', () {
      expect(s.m.library.any((app) => app.adopted), isTrue);
    });
  });

  await runCase('R-02', 'restart: session-only state resets', () async {
    await check('INV-012', 'the page resets to Library after restart', () {
      expect(s.m.page, AppPage.library);
    });
    await s.go(AppPage.library);
    await check('INV-024', 'search text resets to empty', () {
      expect(s.m.search, isEmpty);
      expect(
        tester
                .widget<TextField>(byKey('search-field').first)
                .controller
                ?.text ??
            '',
        isEmpty,
      );
    });
    await check('INV-022', 'filter resets to All', () {
      expect(s.m.filter, LibraryFilter.all);
    });
    await check('INV-025', 'sort resets to Name', () {
      expect(s.m.sort, SortOrder.name);
    });
    await check(
      'INV-095',
      'task history is session-only (Tasks empty after restart)',
      () async {
        await s.go(AppPage.tasks);
        expect(showsText('Nothing has run yet.'), isTrue);
        expect(s.m.tasks, isEmpty);
      },
    );
    await check('INV-012', 'the page resets to Library', () {
      expect(s.m.page, AppPage.library);
    });
  });
}
