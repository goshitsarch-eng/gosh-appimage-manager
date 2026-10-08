import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';
import 'package:gosh_appimage_flutter/ui/detail_page.dart';

import 'support/fakes.dart';
import 'support/harness.dart';

/// Regression tests for the Flutter defects of the 2026-10-08 QA audit. Each
/// test is named for its defect (D-03 to D-17). They act as a user does: keys,
/// Tab, Enter and Space, taps, and the core calls the fake records.
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

  /// A status text in a library row is laid out in full: it is not cut short
  /// and does not end in an ellipsis, and it is on screen (QA2-011).
  void expectReadsInFull(WidgetTester tester, String rowKey, String text) {
    final cell = find.descendant(
      of: find.byKey(Key(rowKey)),
      matching: find.text(text),
    );
    expect(cell, findsOneWidget, reason: '"$text" is not in $rowKey');
    final paragraph = tester.renderObject<RenderParagraph>(cell);
    expect(
      paragraph.didExceedMaxLines,
      isFalse,
      reason: '"$text" is cut short',
    );
    expect(
      paragraph.overflow,
      isNot(TextOverflow.ellipsis),
      reason: '"$text" ends in an ellipsis',
    );
    final box = tester.getRect(cell);
    expect(box.top, greaterThanOrEqualTo(0));
    expect(
      box.bottom,
      lessThanOrEqualTo(
        tester.view.physicalSize.height / tester.view.devicePixelRatio,
      ),
      reason: '"$text" is below the window',
    );
  }

  /// Whether the focused control is a text field.
  bool textFieldFocused(WidgetTester tester) => tester
      .widgetList<EditableText>(find.byType(EditableText))
      .any((field) => field.focusNode.hasFocus);

  Future<void> pressTab(WidgetTester tester) async {
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
  }

  /// Presses Tab until the focused control has [label].
  Future<void> tabTo(WidgetTester tester, String label, {int max = 60}) async {
    for (var i = 0; i < max && focusLabel() != label; i++) {
      await pressTab(tester);
    }
    expect(focusLabel(), label, reason: 'Tab never reached $label');
  }

  Future<void> pressCtrl(WidgetTester tester, LogicalKeyboardKey key) async {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(key);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
  }

  // D-08 -------------------------------------------------------------------

  testWidgets(
    'D-08 Ctrl O still opens the chooser after focus leaves the search field',
    (tester) async {
      var chooses = 0;
      FilePickers.openAppImages = () async {
        chooses += 1;
        return [];
      };
      await pumpApp(tester, core: mockupCore());
      await tester.tap(find.byKey(const Key('search-field')));
      await tester.pump();
      expect(textFieldFocused(tester), isTrue);
      FocusManager.instance.primaryFocus!.unfocus();
      await tester.pump();
      expect(textFieldFocused(tester), isFalse);
      await pressCtrl(tester, LogicalKeyboardKey.keyO);
      await tester.pumpAndSettle();
      expect(chooses, 1);
    },
  );

  testWidgets(
    'D-08 F5, Ctrl R and Ctrl F work after the search field has gone',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      await tester.tap(find.byKey(const Key('search-field')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('nav-settings')));
      await tester.pump();
      expect(model.page, AppPage.settings);
      expect(textFieldFocused(tester), isFalse);

      int listed() => core.calls.where((call) => call == 'listLibrary').length;
      final before = listed();
      await tester.sendKeyEvent(LogicalKeyboardKey.f5);
      await tester.pumpAndSettle();
      expect(listed(), before + 1, reason: 'F5 did not reload the library');

      await pressCtrl(tester, LogicalKeyboardKey.keyR);
      await tester.pumpAndSettle();
      expect(listed(), before + 2, reason: 'Ctrl R did not reload');

      await pressCtrl(tester, LogicalKeyboardKey.keyF);
      await tester.pumpAndSettle();
      expect(core.calls, contains('checkUpdates'));
    },
  );

  testWidgets(
    'D-08 Escape closes a dialog after focus leaves the search field',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      await tester.tap(find.byKey(const Key('search-field')));
      await tester.pump();
      FocusManager.instance.primaryFocus!.unfocus();
      await tester.pump();
      model.showDialog(
        const AdoptDialog(path: '/home/someone/Downloads/Demo.AppImage'),
      );
      await tester.pump();
      expect(find.text('Adopt this AppImage?'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(model.dialog, isNull);
    },
  );

  // D-05 -------------------------------------------------------------------

  testWidgets('D-05 Tab visits each sidebar item in order', (tester) async {
    await pumpApp(tester, core: mockupCore());
    const order = [
      'nav-library',
      'nav-inspect',
      'nav-updates',
      'nav-tasks',
      'nav-settings',
      'nav-about',
    ];
    for (final label in order) {
      await pressTab(tester);
      expect(focusLabel(), label, reason: 'Tab order at $label');
    }
  });

  testWidgets('D-05 Enter and Space activate the focused sidebar item', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    await tabTo(tester, 'nav-inspect');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(model.page, AppPage.inspect);
    await tabTo(tester, 'nav-settings');
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pumpAndSettle();
    expect(model.page, AppPage.settings);
  });

  testWidgets('D-05 the focused sidebar item shows a visible focus ring', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.byKey(const ValueKey('nav-updates-focus-ring')), findsNothing);
    await tabTo(tester, 'nav-updates');
    await tester.pump();
    expect(
      find.byKey(const ValueKey('nav-updates-focus-ring')),
      findsOneWidget,
    );
  });

  // D-06 -------------------------------------------------------------------

  testWidgets('D-06 Space toggles a setting when its toggle has focus', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.settings);
    expect(model.settings!.moveSource, isFalse);
    await tabTo(tester, 'toggle-move');
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pumpAndSettle();
    expect(core.settings.moveSource, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(core.settings.moveSource, isFalse);
  });

  testWidgets('D-06 Enter and Space choose a settings segment', (tester) async {
    final core = mockupCore();
    final model = await pumpApp(
      tester,
      core: core,
      page: AppPage.settings,
      appearance: AppearanceChoice.light,
    );
    await tabTo(tester, 'segment-Dark');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(model.settings!.appearance, AppearanceChoice.dark);
    await tabTo(tester, 'segment-Light');
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pumpAndSettle();
    expect(model.settings!.appearance, AppearanceChoice.light);
  });

  // D-07 -------------------------------------------------------------------

  testWidgets('D-07 Tab keeps focus inside an open dialog', (tester) async {
    final model = await pumpApp(tester, core: mockupCore());
    model.askRemove('quill', permanent: false);
    await tester.pump();
    expect(find.text('Move Quill Notes to the Trash?'), findsOneWidget);
    expect(focusLabel(), 'Cancel', reason: 'initial focus');
    for (var i = 0; i < 30; i++) {
      await pressTab(tester);
      expect(
        focusInDialog(),
        isTrue,
        reason: 'Tab press ${i + 1} left the dialog at ${focusLabel()}',
      );
    }
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    for (var i = 0; i < 10; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(focusInDialog(), isTrue, reason: 'Shift+Tab left the dialog');
    }
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  });

  testWidgets('D-07 Escape closes the dialog and focus leaves it', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    model.askRemove('quill', permanent: false);
    await tester.pump();
    expect(focusInDialog(), isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(model.dialog, isNull);
    expect(focusInDialog(), isFalse);
  });

  testWidgets('D-07 Tab works again once the dialog has closed', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    model.askRemove('quill', permanent: false);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(model.dialog, isNull);
    await pressTab(tester);
    expect(focusLabel(), 'nav-library', reason: 'Tab after the dialog closed');
  });

  // D-03 -------------------------------------------------------------------

  testWidgets('D-03 an app no check has covered is not up to date', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), checked: false);
    expect(find.text('Up to date'), findsNothing);
    expect(
      find.descendant(
        of: find.byKey(const Key('row-brisk')),
        matching: find.text('Not checked yet'),
      ),
      findsOneWidget,
    );
  });

  testWidgets(
    'D-03 the Updates page does not count unchecked apps as up to date',
    (tester) async {
      await pumpApp(
        tester,
        core: mockupCore(),
        checked: false,
        page: AppPage.updates,
      );
      expect(
        find.descendant(
          of: find.byKey(const Key('summary-up-to-date')),
          matching: find.text('0'),
        ),
        findsOneWidget,
      );
      expect(find.text('Everything is up to date.'), findsNothing);
    },
  );

  // D-13 -------------------------------------------------------------------

  // The name column must keep a readable width. Each threshold is the width of
  // the name box: about ten characters at 800, and the mockup's column at 1280.
  for (final (width, minimum) in [
    (800.0, 96.0),
    (900.0, 130.0),
    (1280.0, 240.0),
  ]) {
    testWidgets(
      'D-13 names keep a readable width in the library table at ${width.toInt()} px',
      (tester) async {
        // A long name fills its column, so the box measures the column's text width.
        const long = 'Beta_Long_Application_Name_That_Should_Be_Truncated';
        final core = mockupCore();
        core.library = LibraryDto(
          apps: [
            ...core.library.apps,
            fakeApp(uuid: 'long', name: long, version: '1.0.0'),
          ],
          discovered: const [],
        );
        await pumpApp(tester, core: core, size: Size(width, 800));
        expect(tester.takeException(), isNull);
        final name = tester.getSize(find.text(long));
        expect(
          name.width,
          greaterThanOrEqualTo(minimum),
          reason: 'name box is ${name.width.toStringAsFixed(1)} px wide',
        );
      },
    );
  }

  // D-13 toolbar: the search field keeps its placeholder and 200 px -------

  testWidgets(
    'D-13 the library search field keeps 200 px at 800 px, on its own row',
    (tester) async {
      await pumpApp(tester, core: mockupCore(), size: const Size(800, 800));
      expect(tester.takeException(), isNull);
      final search = find.ancestor(
        of: find.byKey(const Key('search-field')),
        matching: find.byType(AppTextField),
      );
      expect(
        tester.getSize(search).width,
        greaterThanOrEqualTo(200),
        reason: 'search is ${tester.getSize(search).width} px wide',
      );
      expect(find.text('Search apps'), findsOneWidget);
      expect(
        tester
            .renderObject<RenderParagraph>(find.text('Search apps'))
            .didExceedMaxLines,
        isFalse,
        reason: 'the placeholder is truncated',
      );
      // The filter sits above the search; the sort shares the search's row.
      final filter = tester.getCenter(find.byType(AppSegmented<LibraryFilter>));
      final searchAt = tester.getCenter(search);
      expect(searchAt.dy, greaterThan(filter.dy + 20));
      expect(
        tester.getCenter(find.byKey(const Key('sort-menu'))).dy,
        closeTo(searchAt.dy, 2),
      );
    },
  );

  testWidgets(
    'D-13 the library search field stays beside the filter at 1280 px, 342 px wide',
    (tester) async {
      await pumpApp(tester, core: mockupCore());
      final search = find.ancestor(
        of: find.byKey(const Key('search-field')),
        matching: find.byType(AppTextField),
      );
      expect(tester.getSize(search).width, 342);
      expect(find.text('Search apps'), findsOneWidget);
      expect(
        tester
            .renderObject<RenderParagraph>(find.text('Search apps'))
            .didExceedMaxLines,
        isFalse,
      );
      expect(
        tester.getCenter(search).dy,
        closeTo(
          tester.getCenter(find.byType(AppSegmented<LibraryFilter>)).dy,
          2,
        ),
      );
    },
  );

  // Round 4: the unsafe fallback asks for each file (owner decision) -----------

  const pendingFile = '/home/someone/Downloads/Odd-App-1.0.AppImage';
  const askTitle = 'Run this AppImage to read it?';

  List<String> callsStarting(FakeCore core, String prefix) =>
      core.calls.where((call) => call.startsWith(prefix)).toList();

  testWidgets(
    'round 4: a pending inspect asks for the file by name, with Cancel focused',
    (tester) async {
      final core = mockupCore()
        ..inspectResult = fakeInspect(
          path: pendingFile,
          name: '',
          fallbackPending: true,
        );
      final model = await pumpApp(tester, core: core);
      await model.startInspect(pendingFile);
      await tester.pumpAndSettle();
      expect(find.text(askTitle), findsOneWidget);
      expect(
        find.textContaining(
          'Safe extraction could not read Odd-App-1.0.AppImage.',
        ),
        findsOneWidget,
      );
      expect(focusLabel(), 'Cancel', reason: 'Cancel takes the initial focus');
    },
  );

  testWidgets(
    'round 4: Cancel makes no second inspect call, and the safe result stays',
    (tester) async {
      final core = mockupCore()
        ..inspectResult = fakeInspect(
          path: pendingFile,
          name: '',
          fallbackPending: true,
        );
      final model = await pumpApp(tester, core: core);
      await model.startInspect(pendingFile);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('dialog-cancel')));
      await tester.pumpAndSettle();
      expect(model.dialog, isNull);
      expect(callsStarting(core, 'inspectPath:'), ['inspectPath:$pendingFile']);
      expect(
        model.inspect.name,
        isEmpty,
        reason: 'the safe result has no metadata',
      );
    },
  );

  testWidgets(
    'round 4: Run anyway inspects again with confirmUnsafe, and the metadata appears',
    (tester) async {
      final core = mockupCore()
        ..inspectResult = fakeInspect(
          path: pendingFile,
          name: '',
          fallbackPending: true,
        )
        ..inspectUnsafeResult = fakeInspect(
          path: pendingFile,
          name: 'Odd App',
          version: '1.0',
        );
      final model = await pumpApp(tester, core: core);
      await model.startInspect(pendingFile);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('dialog-confirm')));
      await tester.pumpAndSettle();
      expect(callsStarting(core, 'inspectPath:'), [
        'inspectPath:$pendingFile',
        'inspectPath:$pendingFile:confirmUnsafe',
      ]);
      expect(model.inspect.name, 'Odd App');
      expect(model.dialog, isNull);
    },
  );

  testWidgets(
    'round 4: Escape cancels a pending inspect question, and runs nothing',
    (tester) async {
      final core = mockupCore()
        ..inspectResult = fakeInspect(
          path: pendingFile,
          name: '',
          fallbackPending: true,
        );
      final model = await pumpApp(tester, core: core);
      await model.startInspect(pendingFile);
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(model.dialog, isNull);
      expect(callsStarting(core, 'inspectPath:'), ['inspectPath:$pendingFile']);
    },
  );

  testWidgets(
    'round 4: a pending integrate asks for the file and installs nothing',
    (tester) async {
      final core = mockupCore()
        ..integrateResult = fakeOutcome(ok: false, fallbackPending: true);
      final model = await pumpApp(tester, core: core);
      model.inspect.pathInput = pendingFile;
      model.integrateFromInspect();
      await tester.pumpAndSettle();
      expect(find.text(askTitle), findsOneWidget);
      expect(callsStarting(core, 'integrateApp:'), [
        'integrateApp:$pendingFile:automatic',
      ]);
      expect(model.status, isNull, reason: 'nothing was integrated');
    },
  );

  testWidgets('round 4: Cancel leaves the file where it is', (tester) async {
    final core = mockupCore()
      ..integrateResult = fakeOutcome(ok: false, fallbackPending: true);
    final model = await pumpApp(tester, core: core);
    model.inspect.pathInput = pendingFile;
    model.integrateFromInspect();
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('dialog-cancel')));
    await tester.pumpAndSettle();
    expect(model.dialog, isNull);
    expect(callsStarting(core, 'integrateApp:'), [
      'integrateApp:$pendingFile:automatic',
    ]);
    expect(callsStarting(core, 'removeApp:'), isEmpty);
  });

  testWidgets('round 4: Run anyway integrates the file', (tester) async {
    final core = mockupCore()
      ..integrateResult = fakeOutcome(ok: false, fallbackPending: true)
      ..integrateUnsafeResult = fakeOutcome(
        ok: true,
        app: fakeApp(
          uuid: 'odd',
          name: 'Odd App',
          managedPath: '/home/someone/AppImages/Odd-App-1.0.AppImage',
        ),
      );
    final model = await pumpApp(tester, core: core);
    model.inspect.pathInput = pendingFile;
    model.integrateFromInspect();
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('dialog-confirm')));
    await tester.pumpAndSettle();
    expect(callsStarting(core, 'integrateApp:'), [
      'integrateApp:$pendingFile:automatic',
      'integrateApp:$pendingFile:automatic:confirmUnsafe',
    ]);
    expect(model.status?.severity, Severity.success);
  });

  // QA2-016: long names at the stacked width ------------------------------------

  testWidgets(
    'QA2-016 at 800 px, a 30-character name with a running app shows in full, with no ellipsis',
    (tester) async {
      const name = 'Zeta Music Player Studio Pro X';
      expect(name.length, 30);
      final core = mockupCore()
        ..library = LibraryDto(
          apps: [
            fakeApp(uuid: 'zeta', name: name, version: '1.2.3', running: true),
          ],
          discovered: const [],
        )
        ..runningUuids.add('zeta');
      await pumpApp(
        tester,
        core: core,
        size: const Size(800, 800),
        checked: false,
      );
      expect(tester.takeException(), isNull);
      final row = find.byKey(const Key('row-zeta'));
      final text = find.descendant(of: row, matching: find.text(name));
      expect(text, findsOneWidget);
      final paragraph = tester.renderObject<RenderParagraph>(text);
      expect(
        paragraph.didExceedMaxLines,
        isFalse,
        reason: 'the name is cut short',
      );
      expect(
        paragraph.overflow,
        isNot(TextOverflow.ellipsis),
        reason: 'the name ends in an ellipsis',
      );
      expect(
        tester.getSize(text).height,
        greaterThan(20),
        reason: 'the name is on one line, so it is cut',
      );
      expect(
        find.descendant(of: row, matching: find.text('Running')),
        findsOneWidget,
      );
      expect(tester.getSize(row).height, lessThanOrEqualTo(72));
    },
  );

  // The Running badge at the stacked width ----------------------------------

  testWidgets(
    'a running app at 800 px shows its full name, and the Running badge on the version line',
    (tester) async {
      await pumpApp(
        tester,
        core: mockupCore(),
        size: const Size(800, 800),
        checked: false,
      );
      expect(tester.takeException(), isNull);
      final row = find.byKey(const Key('row-quill'));
      final name = find.descendant(of: row, matching: find.text('Quill Notes'));
      expect(name, findsOneWidget);
      expect(
        tester.renderObject<RenderParagraph>(name).didExceedMaxLines,
        isFalse,
        reason: 'the name is cut short',
      );
      final badge = find.descendant(of: row, matching: find.text('Running'));
      expect(badge, findsOneWidget);
      // The badge sits under the name, on the version line, and inside the
      // name cell, not clipped.
      expect(
        tester.getCenter(badge).dy,
        greaterThan(tester.getCenter(name).dy),
      );
      expect(
        tester.getRect(badge).right,
        lessThanOrEqualTo(
          tester.getRect(find.byKey(const Key('open-quill'))).right,
        ),
        reason: 'the badge runs out of its cell',
      );
      expect(tester.takeException(), isNull);
    },
  );

  // QA2-013: the row height at 800 px -------------------------------------------

  testWidgets(
    'QA2-013 at 800 px with 7 apps, every row is 72 px or less and the status texts read in full',
    (tester) async {
      final model = await pumpApp(
        tester,
        core: mockupCore(),
        size: const Size(800, 800),
        checked: false,
      );
      expect(tester.takeException(), isNull);
      const uuids = [
        'quill',
        'atlas',
        'brisk',
        'tidemark',
        'cinder',
        'orbit',
        'ledger',
      ];
      void expectRowsFit(String when) {
        for (final uuid in uuids) {
          final row = find.byKey(Key('row-$uuid'));
          expect(
            row,
            findsOneWidget,
            reason: '$when: row $uuid is not on screen',
          );
          expect(
            tester.getSize(row).height,
            lessThanOrEqualTo(72),
            reason: '$when: row $uuid is ${tester.getSize(row).height} px tall',
          );
        }
      }

      expectRowsFit('before a check');
      expectReadsInFull(tester, 'row-brisk', 'Not checked yet');
      await model.checkUpdates();
      await tester.pump();
      expectRowsFit('after a check');
      expectReadsInFull(tester, 'row-brisk', 'Up to date');
      expectReadsInFull(tester, 'row-orbit', 'Check failed');
      expect(tester.takeException(), isNull);
    },
  );

  // QA2-011 and QA2-012: cosmetic ---------------------------------------------

  testWidgets(
    'QA2-011 at 800 px the status texts read in full, with no ellipsis',
    (tester) async {
      final model = await pumpApp(
        tester,
        core: mockupCore(),
        size: const Size(800, 900),
        checked: false,
      );
      expect(tester.takeException(), isNull);
      expectReadsInFull(tester, 'row-brisk', 'Not checked yet');
      expectReadsInFull(tester, 'row-brisk', 'Status unknown, not up to date');
      await model.checkUpdates();
      await tester.pump();
      expectReadsInFull(tester, 'row-brisk', 'Up to date');
      expectReadsInFull(tester, 'row-orbit', 'Check failed');
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'QA2-012 an unreadable managed folder reads Unreadable with its error',
    (tester) async {
      const message =
          'Cannot read the managed folder /home/someone/AppImages: Permission denied';
      final core = unreadableFolderCore(message);
      final model = await pumpApp(tester, core: core, checked: false);
      final box = find.byKey(const Key('managed-folder-box'));
      expect(
        find.descendant(of: box, matching: find.text('Unreadable')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: box, matching: find.text(message)),
        findsOneWidget,
      );
      expect(
        find.descendant(of: box, matching: find.text('Empty')),
        findsNothing,
      );
      expect(
        model.status?.text,
        message,
        reason: 'the error banner is still shown',
      );

      core.failures.remove('listLibrary');
      await model.loadLibrary();
      await tester.pump();
      expect(
        find.descendant(of: box, matching: find.text('Unreadable')),
        findsNothing,
      );
      expect(
        find.descendant(of: box, matching: find.textContaining('apps')),
        findsOneWidget,
      );
    },
  );

  // App icons (owner decision) -----------------------------------------------

  /// Lets Flutter's image decoder finish reading a file icon. Real file I/O
  /// runs outside the test's fake clock, so the pumps alone do not reach it.
  Future<void> settleIcons(WidgetTester tester) async {
    for (var i = 0; i < 25; i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 20)),
      );
      await tester.pump();
    }
  }

  String iconFixture(String name) =>
      File('test/fixtures/icons/$name').absolute.path;

  FakeCore coreWithIcon(String iconPath) => mockupCore()
    ..library = LibraryDto(
      apps: [
        fakeApp(
          uuid: 'quill',
          name: 'Quill Notes',
          version: '2.4.1',
          iconPath: iconPath,
        ),
      ],
      discovered: const [],
    );

  testWidgets(
    'app icons: a valid PNG icon shows in the library row in place of the letter',
    (tester) async {
      await pumpApp(tester, core: coreWithIcon(iconFixture('valid.png')));
      await settleIcons(tester);
      final row = find.byKey(const Key('row-quill'));
      expect(
        find.descendant(of: row, matching: find.byType(Image)),
        findsOneWidget,
      );
      expect(find.descendant(of: row, matching: find.text('Q')), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('app icons: a missing icon shows the letter, with no error', (
    tester,
  ) async {
    await pumpApp(
      tester,
      core: coreWithIcon('/home/someone/.local/share/icons/not-there.png'),
    );
    await settleIcons(tester);
    final row = find.byKey(const Key('row-quill'));
    expect(find.descendant(of: row, matching: find.text('Q')), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'app icons: an icon that does not decode shows the letter, with no error',
    (tester) async {
      await pumpApp(tester, core: coreWithIcon(iconFixture('broken.png')));
      await settleIcons(tester);
      final row = find.byKey(const Key('row-quill'));
      expect(
        find.descendant(of: row, matching: find.text('Q')),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('app icons: the Detail header shows a valid icon too', (
    tester,
  ) async {
    final model = await pumpApp(
      tester,
      core: coreWithIcon(iconFixture('valid.png')),
    );
    model.selectApp('quill');
    await tester.pump();
    await settleIcons(tester);
    expect(
      find.descendant(
        of: find.byType(DetailPage),
        matching: find.byType(Image),
      ),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });

  // Unsafe extraction fallback (owner decision) -------------------------------

  const unsafeHelp =
      'Runs the AppImage itself to read its files when safe extraction fails. '
      'It asks before each file. Only turn this on for AppImages you trust.';

  bool unsafeToggleOn(WidgetTester tester) => tester
      .widget<AppToggle>(
        find.ancestor(
          of: find.byKey(const Key('toggle-unsafe')),
          matching: find.byType(AppToggle),
        ),
      )
      .value;

  testWidgets(
    'unsafe fallback is off by default, with its help and no Unavailable pill',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.settings);
      expect(unsafeToggleOn(tester), isFalse);
      expect(core.settings.unsafeExtractionFallback, isFalse);
      expect(find.text('Unavailable'), findsNothing);
      expect(find.text(unsafeHelp), findsOneWidget);
    },
  );

  testWidgets(
    'unsafe fallback: turning it on asks first, and Cancel leaves it off',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.settings);
      await tester.tap(find.byKey(const Key('toggle-unsafe')));
      await tester.pumpAndSettle();
      expect(
        find.text('Turn on the unsafe extraction fallback?'),
        findsOneWidget,
      );
      expect(model.dialog, isNotNull, reason: 'the warning asks first');
      expect(focusLabel(), 'Cancel', reason: 'Cancel takes the initial focus');
      await tester.tap(find.byKey(const Key('dialog-cancel')));
      await tester.pumpAndSettle();
      expect(model.dialog, isNull);
      expect(core.settings.unsafeExtractionFallback, isFalse);
      expect(core.calls.where((call) => call == 'saveSettings'), isEmpty);
    },
  );

  testWidgets('unsafe fallback: Turn on saves it', (tester) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.settings);
    await tester.tap(find.byKey(const Key('toggle-unsafe')));
    await tester.pumpAndSettle();
    expect(find.text('Turn on'), findsOneWidget);
    await tester.tap(find.byKey(const Key('dialog-confirm')));
    await tester.pumpAndSettle();
    expect(core.settings.unsafeExtractionFallback, isTrue);
    expect(model.settings!.unsafeExtractionFallback, isTrue);
    expect(unsafeToggleOn(tester), isTrue);
  });

  testWidgets('unsafe fallback: Turn off saves at once, with no warning', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.settings);
    await tester.tap(find.byKey(const Key('toggle-unsafe')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('dialog-confirm')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('toggle-unsafe')));
    await tester.pumpAndSettle();
    expect(model.dialog, isNull, reason: 'turning off asks nothing');
    expect(core.settings.unsafeExtractionFallback, isFalse);
    expect(unsafeToggleOn(tester), isFalse);
  });

  testWidgets(
    'unsafe fallback: once on, its row takes focus and Space turns it off',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.settings);
      await tester.tap(find.byKey(const Key('toggle-unsafe')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('dialog-confirm')));
      await tester.pumpAndSettle();
      await tabTo(tester, 'toggle-unsafe');
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(model.dialog, isNull);
      expect(core.settings.unsafeExtractionFallback, isFalse);
    },
  );

  // Managed folder: the typed entry (owner decision) -------------------------

  Finder folderField() => find.byKey(const Key('managed-folder-field'));

  testWidgets(
    'managed folder: the field shows the saved path, and a typed absolute path saves on Enter',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.settings);
      expect(
        tester.widget<TextField>(folderField()).controller!.text,
        '/home/someone/AppImages',
      );
      await tester.enterText(folderField(), '/home/someone/Elsewhere');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(core.settings.managedFolder, '/home/someone/Elsewhere');
      expect(model.status?.text, 'Managed folder updated');
      expect(find.byKey(const Key('managed-folder-error')), findsNothing);
    },
  );

  testWidgets(
    'managed folder: a typed absolute path saves when the field loses focus',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.settings);
      await tester.enterText(folderField(), '/home/someone/Elsewhere');
      await tester.pump();
      FocusManager.instance.primaryFocus!.unfocus();
      await tester.pumpAndSettle();
      expect(core.settings.managedFolder, '/home/someone/Elsewhere');
    },
  );

  testWidgets(
    'managed folder: a relative path shows an inline error and is not saved',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.settings);
      await tester.enterText(folderField(), 'AppImages/Relative');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(core.calls.where((call) => call == 'saveSettings'), isEmpty);
      expect(core.settings.managedFolder, '/home/someone/AppImages');
      expect(
        find.text('Managed folder must be an absolute path'),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'managed folder: an empty path shows an inline error and is not saved',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.settings);
      await tester.enterText(folderField(), '   ');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(core.calls.where((call) => call == 'saveSettings'), isEmpty);
      expect(find.text('Managed folder cannot be empty'), findsOneWidget);
    },
  );

  testWidgets(
    'managed folder: a path the core refuses shows the core text inline',
    (tester) async {
      const message = 'The managed folder cannot be written: Permission denied';
      final core = mockupCore()
        ..failures['saveSettings'] = const CoreError(
          kind: ErrorKind.permission,
          message: message,
          details: '',
        );
      await pumpApp(tester, core: core, page: AppPage.settings);
      await tester.enterText(folderField(), '/home/someone/Locked');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(core.settings.managedFolder, '/home/someone/AppImages');
      expect(
        tester.widget<Text>(find.byKey(const Key('managed-folder-error'))).data,
        message,
      );
    },
  );

  // D-09 -------------------------------------------------------------------

  for (final size in [const Size(1280, 800), const Size(2400, 420)]) {
    testWidgets(
      'D-09 a long managed folder path stays inside its row at ${size.width.toInt()} px',
      (tester) async {
        final longPath =
            '/home/someone/${List.filled(40, 'AppImages-segment').join('/')}';
        await pumpApp(
          tester,
          core: mockupCore(),
          size: size,
          page: AppPage.settings,
          settings: fakeSettings(managedFolder: longPath),
        );
        expect(tester.takeException(), isNull);
        final button = tester.getRect(find.byKey(const Key('change-folder')));
        expect(button.right, lessThanOrEqualTo(size.width));
        // The path is typed into the managed folder field, beside Change…
        final path = tester.getRect(
          find.byKey(const Key('managed-folder-field')),
        );
        expect(
          path.right,
          lessThanOrEqualTo(button.left),
          reason: 'the path runs under the Change button',
        );
        expect(
          path.width,
          greaterThanOrEqualTo(120),
          reason: 'the path gets ${path.width.toStringAsFixed(1)} px',
        );
      },
    );
  }

  // D-10 -------------------------------------------------------------------

  testWidgets('D-10 a GitHub source shows repo=owner/name as its hint', (
    tester,
  ) async {
    final core = mockupCore()
      ..library = LibraryDto(
        apps: [fakeApp(uuid: 'u1', name: 'Demo', updateManager: 'github')],
        discovered: const [],
      );
    final model = await pumpApp(tester, core: core);
    model.selectApp('u1');
    await tester.pump();
    expect(find.byKey(const Key('source-config-field')), findsOneWidget);
    expect(find.text('repo=owner/name'), findsOneWidget);
  });

  testWidgets(
    'D-10 the core refusal shows its message beside the source field',
    (tester) async {
      final core = mockupCore()
        ..library = LibraryDto(
          apps: [fakeApp(uuid: 'u1', name: 'Demo')],
          discovered: const [],
        )
        ..failures['setUpdateSource'] = const CoreError(
          kind: ErrorKind.validation,
          message: 'GitHub username is not valid',
          details: '',
        );
      final model = await pumpApp(tester, core: core);
      model.selectApp('u1');
      await tester.pump();
      await tester.tap(find.byKey(const Key('source-selector')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('GitHub').last);
      await tester.pumpAndSettle();
      expect(
        core.calls.where((call) => call.startsWith('setUpdateSource')),
        isNotEmpty,
        reason: 'choosing GitHub did not save the source',
      );
      expect(model.sourceError, 'GitHub username is not valid');
      expect(find.byKey(const Key('source-error')), findsOneWidget);
      expect(
        tester.widget<Text>(find.byKey(const Key('source-error'))).data,
        'GitHub username is not valid',
      );
    },
  );

  testWidgets('D-10 a core that accepts repo=owner/name saves that form', (
    tester,
  ) async {
    final core = mockupCore()
      ..library = LibraryDto(
        apps: [fakeApp(uuid: 'u1', name: 'Demo', updateManager: 'github')],
        discovered: const [],
      );
    final model = await pumpApp(tester, core: core);
    model.selectApp('u1');
    await tester.pump();
    await tester.enterText(
      find.byKey(const Key('source-config-field')),
      'repo=owner/name',
    );
    await tester.pump();
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(core.calls, contains('setUpdateSource:u1:github:repo=owner/name'));
    expect(model.status?.text, 'Update source saved');
  });

  // D-17 -------------------------------------------------------------------

  testWidgets('D-17 the maximum size is applied when its field loses focus', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.settings);
    await tester.enterText(find.byKey(const Key('max-size-field')), '500');
    await tester.pump();
    FocusManager.instance.primaryFocus!.unfocus();
    await tester.pumpAndSettle();
    expect(core.settings.maxAppimageBytes, 500 * 1024 * 1024);
    expect(model.status?.text, 'Maximum size updated');
  });

  testWidgets(
    'D-17 an invalid maximum size shows an error on focus loss and is not saved',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.settings);
      final before = core.settings.maxAppimageBytes;
      await tester.enterText(find.byKey(const Key('max-size-field')), 'abc');
      await tester.pump();
      FocusManager.instance.primaryFocus!.unfocus();
      await tester.pumpAndSettle();
      expect(core.calls, isNot(contains('saveSettings')));
      expect(core.settings.maxAppimageBytes, before);
      final status = tester.widget<Text>(find.byKey(const Key('status-line')));
      expect(status.data, contains('is not a whole number of megabytes'));
    },
  );

  // D-15 -------------------------------------------------------------------

  testWidgets(
    'D-15 an unmanaged AppImage the core lists shows the Adopt card',
    (tester) async {
      const found = '/home/someone/Downloads/Found-1.0.AppImage';
      final core = mockupCore();
      core.library = LibraryDto(
        apps: core.library.apps,
        discovered: [fakeDiscovered(path: found, name: 'Found')],
      );
      final model = await pumpApp(tester, core: core);
      expect(find.text('Not managed yet'), findsOneWidget);
      await tester.tap(find.byKey(const Key('adopt-button-$found')));
      await tester.pumpAndSettle();
      expect(find.text('Adopt this AppImage?'), findsOneWidget);
      await tester.tap(find.byKey(const Key('dialog-confirm')));
      await tester.pumpAndSettle();
      expect(core.calls, contains('adoptPath:$found'));
      expect(model.dialog, isNull);
    },
  );

  testWidgets(
    'D-15 the Add button saves the arguments typed for the open app',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      model.selectApp('quill');
      await tester.pump();
      await tester.enterText(
        find.byKey(const Key('arguments-field')),
        '--new-flag',
      );
      await tester.pump();
      await tester.tap(find.byKey(const Key('launch-options-add')));
      await tester.pumpAndSettle();
      expect(core.calls, contains('saveArgumentsAndEnvironment:quill'));
      expect(core.lastArguments, ['--new-flag']);
      expect(model.status?.text, 'Saved arguments and environment');
    },
  );

  testWidgets('D-15 Change… saves the managed folder the user picks', (
    tester,
  ) async {
    FilePickers.openFolder = () async => '/home/someone/Elsewhere';
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.settings);
    await tester.tap(find.byKey(const Key('change-folder')));
    await tester.pumpAndSettle();
    expect(core.settings.managedFolder, '/home/someone/Elsewhere');
    expect(model.status?.text, 'Managed folder updated');
  });

  // D-16 -------------------------------------------------------------------

  testWidgets('D-16 the Updates page lays out at 360 by 480 without overflow', (
    tester,
  ) async {
    await pumpApp(
      tester,
      core: mockupCore(),
      size: const Size(360, 480),
      page: AppPage.updates,
    );
    expect(tester.takeException(), isNull);
    expect(find.text('APP'), findsOneWidget);
  });

  // D-04 -------------------------------------------------------------------

  testWidgets(
    'D-04 a permanent removal is listed as Deleted, not as moved to the Trash',
    (tester) async {
      final core = mockupCore();
      core.tasks.add(
        fakeTask(
          id: 'op-delete',
          kind: TaskKindDto.remove,
          state: TaskStateDto.succeeded,
          title: 'Removing',
          target: 'Quill Notes',
          fromVersion: '2.0',
          permanent: true,
          finishedAt: unixSeconds(DateTime(2026, 10, 6, 18, 5)),
        ),
      );
      await pumpApp(tester, core: core, page: AppPage.tasks);
      expect(find.text('Deleted Quill Notes 2.0'), findsOneWidget);
      expect(find.text('Moved Quill Notes 2.0 to the Trash'), findsNothing);
    },
  );

  testWidgets('D-04 a Trash removal is still listed as moved to the Trash', (
    tester,
  ) async {
    final core = mockupCore();
    core.tasks.add(
      fakeTask(
        id: 'op-trashed',
        kind: TaskKindDto.remove,
        state: TaskStateDto.succeeded,
        title: 'Removing',
        target: 'Quill Notes',
        fromVersion: '2.0',
        finishedAt: unixSeconds(DateTime(2026, 10, 6, 18, 5)),
      ),
    );
    await pumpApp(tester, core: core, page: AppPage.tasks);
    expect(find.text('Moved Quill Notes 2.0 to the Trash'), findsOneWidget);
    expect(find.text('Deleted Quill Notes 2.0'), findsNothing);
  });
}
