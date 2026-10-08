import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

import 'support/fakes.dart';
import 'support/harness.dart';

/// A core double that lists an unmanaged AppImage only while "Discover
/// AppImages elsewhere" is on, as the core does (R6-05).
class _DiscoveryCore extends FakeCore {
  _DiscoveryCore(this.found)
    : super(
        library: LibraryDto(apps: mockupApps(), discovered: const []),
      );

  final DiscoveredDto found;

  @override
  Future<LibraryDto> listLibrary() async {
    final base = await super.listLibrary();
    return LibraryDto(
      apps: base.apps,
      discovered: settings.manageOutsideFolder ? [found] : const [],
    );
  }
}

/// The round 6 defects. Each test names its defect and acts as a user does:
/// taps, keys and typed text, with the core calls the fake records.
void main() {
  setUpAll(() async {
    await loadAppFonts();
    homeFolderProvider = () => '/home/someone';
  });

  tearDownAll(() {
    homeFolderProvider = () => null;
  });

  setUp(() {
    FilePickers.openAppImages = () async => [];
    FilePickers.openFolder = () async => null;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('gosh/window'),
          (_) async => null,
        );
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('gosh/window'), null);
    FilePickers.openAppImages = () async => [];
    FilePickers.openFolder = () async => null;
  });

  /// The label of the focused control, as its focus node names it.
  String? focusLabel() => FocusManager.instance.primaryFocus?.debugLabel;

  /// Whether the focused control sits inside the open dialog's focus scope.
  bool focusInDialog() =>
      FocusManager.instance.primaryFocus?.ancestors.any(
        (node) => node.debugLabel == 'app-dialog',
      ) ??
      false;

  Future<void> settleBriefly(WidgetTester tester) async {
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
  }

  /// Starts a bridge operation that stays open until its hold completes, as a
  /// long update check does on a slow network. Mutations queue behind it.
  Future<void> startHeldCheck(WidgetTester tester, AppModel model) async {
    unawaited(model.checkUpdates());
    await tester.pump();
    expect(model.busy, isNotNull, reason: 'the check did not start');
  }

  /// Presses Tab until the focused control has [label].
  Future<void> tabTo(WidgetTester tester, String label, {int max = 60}) async {
    for (var i = 0; i < max && focusLabel() != label; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    expect(focusLabel(), label, reason: 'Tab never reached $label');
  }

  /// The library row for [uuid], scrolled into the table's view first: the
  /// table is a lazy list, so a row below the fold is not built yet.
  Future<Finder> libraryRow(WidgetTester tester, String uuid) async {
    final table = find.byKey(const Key('library-table'));
    final row = find.byKey(Key('row-$uuid'));
    // Back to the top, so the search below always runs the same way.
    for (var i = 0; i < 40; i++) {
      await tester.drag(table, const Offset(0, 200));
    }
    await tester.pump();
    for (var i = 0; i < 40 && row.evaluate().isEmpty; i++) {
      await tester.drag(table, const Offset(0, -120));
      await tester.pump();
    }
    expect(row, findsOneWidget, reason: 'row $uuid never came into view');
    await tester.ensureVisible(row);
    await tester.pump();
    return row;
  }

  // R6-03 -------------------------------------------------------------------

  for (final (choice, label, mode) in [
    (AppearanceChoice.light, 'Light', ThemeMode.light),
    (AppearanceChoice.dark, 'Dark', ThemeMode.dark),
    (AppearanceChoice.system, 'System', ThemeMode.system),
  ]) {
    testWidgets(
      'R6-03 $label saves and applies at once while a check is running',
      (tester) async {
        final core = mockupCore();
        final gate = Completer<void>();
        core.holds['checkUpdates'] = gate;
        final model = await pumpApp(
          tester,
          core: core,
          page: AppPage.settings,
          appearance: label == 'Light'
              ? AppearanceChoice.dark
              : AppearanceChoice.light,
          checked: false,
        );
        await startHeldCheck(tester, model);
        await tester.tap(
          find.descendant(
            of: find.byType(AppSegmented<AppearanceChoice>),
            matching: find.text(label),
          ),
        );
        await settleBriefly(tester);
        expect(core.settings.appearance, choice, reason: '$label not saved');
        expect(
          tester.widget<MaterialApp>(find.byType(MaterialApp)).themeMode,
          mode,
          reason: '$label not applied',
        );
        gate.complete();
        await settleBriefly(tester);
        // Read back on restart: a new model over the same core.
        final restarted = AppModel(
          core: core,
          pollInterval: null,
          clock: () => testNow,
        );
        await restarted.start(const []);
        expect(restarted.settings?.appearance, choice, reason: 'restart');
      },
    );
  }

  // R6-04 -------------------------------------------------------------------

  testWidgets('R6-04 Enter applies a valid maximum size', (tester) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.settings);
    await tester.enterText(find.byKey(const Key('max-size-field')), '512');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await settleBriefly(tester);
    expect(core.lastPatch?.maxAppimageBytes, 512 * 1024 * 1024);
    expect(model.status?.text, 'Maximum size updated');
  });

  testWidgets(
    'R6-04 Enter applies a valid maximum size while a check is running',
    (tester) async {
      final core = mockupCore();
      final gate = Completer<void>();
      core.holds['checkUpdates'] = gate;
      final model = await pumpApp(
        tester,
        core: core,
        page: AppPage.settings,
        checked: false,
      );
      await startHeldCheck(tester, model);
      await tester.enterText(find.byKey(const Key('max-size-field')), '512');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settleBriefly(tester);
      expect(
        core.lastPatch?.maxAppimageBytes,
        512 * 1024 * 1024,
        reason: 'the size was not saved',
      );
      expect(model.status?.text, 'Maximum size updated');
      gate.complete();
    },
  );

  // R6-05 -------------------------------------------------------------------

  testWidgets(
    'R6-05 turning on discovery while a check is running shows the adopt card',
    (tester) async {
      final found = fakeDiscovered(
        path: '/home/someone/Downloads/Found-1.0.AppImage',
        name: 'Found',
      );
      final core = _DiscoveryCore(found);
      final gate = Completer<void>();
      core.holds['checkUpdates'] = gate;
      final model = await pumpApp(
        tester,
        core: core,
        page: AppPage.settings,
        checked: false,
      );
      await startHeldCheck(tester, model);
      await tester.tap(find.byKey(const Key('toggle-discover')));
      await settleBriefly(tester);
      expect(core.settings.manageOutsideFolder, isTrue, reason: 'not saved');
      model.setPage(AppPage.library);
      await settleBriefly(tester);
      expect(
        find.byKey(Key('adopt-button-${found.path}')),
        findsOneWidget,
        reason: 'the adopt card is missing',
      );
      gate.complete();
    },
  );

  testWidgets(
    'R6-05 a folder typed in the managed folder field and submitted is saved while a check is running',
    (tester) async {
      final core = mockupCore();
      final gate = Completer<void>();
      core.holds['checkUpdates'] = gate;
      final model = await pumpApp(
        tester,
        core: core,
        page: AppPage.settings,
        checked: false,
      );
      await startHeldCheck(tester, model);
      await tester.enterText(
        find.byKey(const Key('managed-folder-field')),
        '/home/someone/Elsewhere',
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settleBriefly(tester);
      expect(core.settings.managedFolder, '/home/someone/Elsewhere');
      expect(model.status?.text, 'Managed folder updated');
      gate.complete();
    },
  );

  testWidgets(
    'R6-05 launch arguments typed after an earlier save are sent by Add',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      model.selectApp('quill');
      await settleBriefly(tester);
      Future<void> typeAndAdd(String text) async {
        await tester.tap(
          find.byKey(const Key('arguments-field')).first,
          warnIfMissed: false,
        );
        await settleBriefly(tester);
        await tester.enterText(find.byKey(const Key('arguments-field')), text);
        await settleBriefly(tester);
        // A long line pushes Add below the fold; scroll it into view as a
        // user would.
        await tester.ensureVisible(find.byKey(const Key('launch-options-add')));
        await tester.tap(find.byKey(const Key('launch-options-add')));
        await settleBriefly(tester);
      }

      await typeAndAdd('--flag-one\n--flag-two');
      expect(core.lastArguments, ['--flag-one', '--flag-two']);
      await typeAndAdd('  \nÜnïcode-ärg\n${'x' * 2000}');
      expect(core.lastArguments?.first, 'Ünïcode-ärg');
      await typeAndAdd('--flag-one\n--flag-two');
      expect(core.lastArguments, [
        '--flag-one',
        '--flag-two',
      ], reason: 'the third Add sent the earlier arguments');
    },
  );

  // R6-07 -------------------------------------------------------------------

  testWidgets(
    'R6-07 closing a dialog opened from Detail returns focus to the window',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      model.selectApp('ledger');
      await settleBriefly(tester);
      await tester.tap(find.byKey(const Key('detail-trash')));
      await settleBriefly(tester);
      expect(focusInDialog(), isTrue, reason: 'the dialog did not take focus');
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await settleBriefly(tester);
      expect(model.dialog, isNull);
      expect(
        focusLabel(),
        'window',
        reason: 'focus is on ${focusLabel() ?? 'no control'} after the dialog',
      );
    },
  );

  testWidgets(
    'R6-07 after a dialog closes by Cancel, Tab reaches the sidebar and Enter works',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      model.selectApp('ledger');
      await settleBriefly(tester);
      await tester.tap(find.byKey(const Key('detail-trash')));
      await settleBriefly(tester);
      await tester.tap(find.byKey(const Key('dialog-cancel')));
      await settleBriefly(tester);
      expect(model.dialog, isNull);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await settleBriefly(tester);
      expect(
        focusLabel(),
        'nav-library',
        reason: 'Tab did not start at the top',
      );
      await tabTo(tester, 'nav-settings');
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await settleBriefly(tester);
      expect(model.page, AppPage.settings, reason: 'Enter did not activate');
    },
  );

  testWidgets(
    'R6-07 focus returns to the window when the focused confirm closes a dialog',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      model.askRemove('quill', permanent: false);
      await tester.pump();
      await tabTo(tester, 'Move to Trash');
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await settleBriefly(tester);
      expect(
        model.dialog,
        isNull,
        reason: 'Enter on the confirm did not close',
      );
      expect(
        focusLabel(),
        'window',
        reason: 'focus is on ${focusLabel() ?? 'no control'} after the confirm',
      );
    },
  );

  testWidgets(
    'R6-07 focus returns to the window when Detail closes with a control focused',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      model.selectApp('quill');
      await settleBriefly(tester);
      await tabTo(tester, 'Launch');
      model.closeDetail();
      await settleBriefly(tester);
      expect(
        focusLabel(),
        'window',
        reason:
            'focus is on ${focusLabel() ?? 'no control'} after Detail closed',
      );
    },
  );

  // R6-08 -------------------------------------------------------------------

  testWidgets('R6-08 the About page at 800 by 600 has no overflow', (
    tester,
  ) async {
    await pumpApp(
      tester,
      core: mockupCore(),
      size: const Size(800, 600),
      page: AppPage.about,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('R6-08 the Settings page at 800 by 600 has no overflow', (
    tester,
  ) async {
    await pumpApp(
      tester,
      core: mockupCore(),
      size: const Size(800, 600),
      page: AppPage.settings,
    );
    expect(tester.takeException(), isNull);
  });

  // R6-09 -------------------------------------------------------------------

  testWidgets('R6-09 at 800 by 600 each library row is 72 px or less', (
    tester,
  ) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, size: const Size(800, 600));
    for (final app in core.library.apps) {
      final row = await libraryRow(tester, app.uuid);
      final height = tester.getSize(row).height;
      expect(
        height,
        lessThanOrEqualTo(72),
        reason: '${app.name} row is $height px tall',
      );
    }
  });

  testWidgets('R6-09 at 800 by 600 app names are not cut short', (
    tester,
  ) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, size: const Size(800, 600));
    for (final uuid in ['quill', 'atlas', 'brisk', 'tidemark']) {
      final row = await libraryRow(tester, uuid);
      final app = core.library.apps.firstWhere((app) => app.uuid == uuid);
      final name = find.descendant(of: row, matching: find.text(app.name));
      expect(name, findsOneWidget, reason: '${app.name} is not in its row');
      final paragraph = tester.renderObject<RenderParagraph>(name);
      expect(
        paragraph.didExceedMaxLines,
        isFalse,
        reason: '"${app.name}" is cut short',
      );
    }
  });

  testWidgets(
    'R6-09 at 800 by 600 the Running badge sits on the version line',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, size: const Size(800, 600));
      final row = await libraryRow(tester, 'quill');
      final badge = find.descendant(of: row, matching: find.text('Running'));
      final version = find.descendant(of: row, matching: find.text('2.4.1'));
      expect(badge, findsOneWidget);
      expect(version, findsOneWidget);
      final gap = (tester.getCenter(badge).dy - tester.getCenter(version).dy)
          .abs();
      expect(
        gap,
        lessThanOrEqualTo(1),
        reason: 'badge is $gap px off the line',
      );
    },
  );
}
