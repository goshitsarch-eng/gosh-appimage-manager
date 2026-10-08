import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/ui/library_page.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

import 'support/fakes.dart';
import 'support/harness.dart';

/// The FEATURES rows of the mockup (SPEC section 12). A test is named `F<row>`
/// so docs/flutter/PARITY.md can cite it. Most rows have a test that acts on
/// the app and checks the state or the core call that results; a row that
/// only shows text is also covered by the frame goldens in
/// pages_golden_test.dart. docs/flutter/PARITY.md names the test for each row.
void main() {
  setUpAll(() async {
    await loadAppFonts();
    homeFolderProvider = () => '/home/someone';
  });

  tearDownAll(() {
    homeFolderProvider = () => null;
  });

  late List<MethodCall> windowCalls;
  setUp(() {
    windowCalls = [];
    FilePickers.openAppImages = () async => [];
    FilePickers.openFolder = () async => null;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('gosh/window'), (
          call,
        ) async {
          windowCalls.add(call);
          return null;
        });
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('gosh/window'), null);
    FilePickers.openAppImages = () async => [];
    FilePickers.openFolder = () async => null;
  });

  /// The text in an input field (find.text only sees Text widgets).
  String fieldText(WidgetTester tester, String key) =>
      tester.widget<TextField>(find.byKey(Key(key))).controller!.text;

  /// The toggle drawn under a key. The key sits on its tap target, so the
  /// widget is found from above.
  AppToggle toggleOf(WidgetTester tester, String key) =>
      tester.widget<AppToggle>(
        find.ancestor(
          of: find.byKey(Key(key)),
          matching: find.byType(AppToggle),
        ),
      );

  /// Taps the middle of a control. Icon buttons are tapped this way; a plain
  /// tap on their keyed box misses in the test harness.
  Future<void> tapCenter(WidgetTester tester, Finder finder) =>
      tester.tapAt(tester.getCenter(finder));

  Finder inSegment<T>(String text) => find.descendant(
    of: find.byType(AppSegmented<T>),
    matching: find.text(text),
  );

  // Window and sidebar ------------------------------------------------------

  testWidgets('F1 window minimize, maximize and close act on the window', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    await tapCenter(tester, find.byKey(const Key('window-minimize')));
    await tester.pump(const Duration(milliseconds: 400));
    await tapCenter(tester, find.byKey(const Key('window-maximize')));
    await tester.pump(const Duration(milliseconds: 400));
    await tapCenter(tester, find.byKey(const Key('window-close')));
    await tester.pump();
    expect(windowCalls.map((call) => call.method), [
      'setTitle',
      'minimize',
      'toggleMaximize',
      'close',
    ]);
  });

  testWidgets('F2 and F3 the title bar names the app and its logo tile', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Gosh AppImage Manager'), findsWidgets);
    expect(find.byKey(const Key('title-bar')), findsOneWidget);
    expect(find.text('G'), findsWidgets);
    expect(
      windowCalls.where((call) => call.method == 'setTitle').last.arguments,
      'Gosh AppImage Manager — Library',
    );
  });

  testWidgets(
    'F4 the Library nav item shows its count and is the current page',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      expect(
        find.descendant(
          of: find.byKey(const Key('nav-library')),
          matching: find.text('7'),
        ),
        findsOneWidget,
      );
      expect(model.page, AppPage.library);
    },
  );

  testWidgets('F5 the Inspect nav item opens Inspect', (tester) async {
    final model = await pumpApp(tester, core: mockupCore());
    await tester.tap(find.byKey(const Key('nav-inspect')));
    await tester.pump();
    expect(model.page, AppPage.inspect);
    expect(
      find.text("Read a file's metadata before you integrate it"),
      findsOneWidget,
    );
  });

  testWidgets('F6 the Updates nav item opens Updates and shows its pill', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    expect(
      find.descendant(
        of: find.byKey(const Key('nav-updates')),
        matching: find.text('2'),
      ),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const Key('nav-updates')));
    await tester.pump();
    expect(model.page, AppPage.updates);
  });

  testWidgets('F7 the Tasks nav item opens Tasks and shows the running count', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    expect(
      find.descendant(
        of: find.byKey(const Key('nav-tasks')),
        matching: find.text('1'),
      ),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const Key('nav-tasks')));
    await tester.pump();
    expect(model.page, AppPage.tasks);
  });

  testWidgets('F8 the Settings nav item opens Settings', (tester) async {
    final model = await pumpApp(tester, core: mockupCore());
    await tester.tap(find.byKey(const Key('nav-settings')));
    await tester.pump();
    expect(model.page, AppPage.settings);
  });

  testWidgets(
    'F9 the About nav item opens About with its identity and license',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      await tester.tap(find.byKey(const Key('nav-about')));
      await tester.pump();
      expect(model.page, AppPage.about);
      // The original About wording (HEAD), with the toolkit reference removed.
      expect(
        find.text(
          'Licensed under the GNU General Public License, version 3 or later. '
          'This program comes with absolutely no warranty.',
        ),
        findsOneWidget,
      );
      expect(find.text('No telemetry of any kind.'), findsOneWidget);
      expect(find.text('Made by'), findsOneWidget);
      // The identifiers and the footer line, as the About page had them.
      expect(find.text('Application ID'), findsOneWidget);
      expect(find.text('com.goshapps.AppImageManager'), findsOneWidget);
      expect(find.text('Homepage'), findsOneWidget);
      expect(find.text('https://goshapps.com'), findsOneWidget);
      expect(find.text('Licensed under GPL-3.0-or-later.'), findsOneWidget);
    },
  );

  testWidgets('F10 the managed folder box shows the folder and its note', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.byKey(const Key('managed-folder-box')), findsOneWidget);
    expect(find.text('MANAGED FOLDER'), findsOneWidget);
    expect(find.text('~/AppImages'), findsWidgets);
    expect(find.text('7 apps · 512 MB'), findsOneWidget);
  });

  testWidgets(
    'F11 the status bar counts installed apps, updates and failed checks',
    (tester) async {
      await pumpApp(tester, core: mockupCore());
      expect(find.text('7 installed'), findsOneWidget);
      expect(find.text('2 updates'), findsOneWidget);
      expect(find.text('1 check failed'), findsOneWidget);
    },
  );

  testWidgets('F12 the status bar shows when updates were last checked', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Last checked 09:14'), findsOneWidget);
  });

  testWidgets('F13 the status bar reads 0 installed on an empty library', (
    tester,
  ) async {
    await pumpApp(tester, core: FakeCore());
    expect(find.text('0 installed'), findsOneWidget);
    expect(find.text('7 installed'), findsNothing);
  });

  // Library -----------------------------------------------------------------

  testWidgets(
    'F14 the Library heading summarises the folder and adopted apps',
    (tester) async {
      await pumpApp(tester, core: mockupCore());
      expect(find.text('7 apps in ~/AppImages and 1 adopted'), findsOneWidget);
    },
  );

  testWidgets(
    'F15 Browse… opens the file chooser and inspects what is picked',
    (tester) async {
      FilePickers.openAppImages = () async => [
        '/home/someone/Downloads/Demo.AppImage',
      ];
      final model = await pumpApp(tester, core: mockupCore());
      await tester.tap(find.byKey(const Key('browse')));
      await tester.pumpAndSettle();
      expect(model.page, AppPage.inspect);
      expect(model.inspect.pathInput, '/home/someone/Downloads/Demo.AppImage');
    },
  );

  testWidgets('F16 Ctrl O is the Browse shortcut and shows its key hint', (
    tester,
  ) async {
    var chooses = 0;
    FilePickers.openAppImages = () async {
      chooses += 1;
      return [];
    };
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Ctrl O'), findsWidgets);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyO);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pumpAndSettle();
    expect(chooses, 1);
  });

  testWidgets(
    'F17 the filter control narrows the table to updates or attention',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      expect(inSegment<LibraryFilter>('Needs attention'), findsOneWidget);
      await tester.tap(inSegment<LibraryFilter>('Updates'));
      await tester.pump();
      expect(model.filter, LibraryFilter.updates);
      expect(find.byKey(const Key('row-quill')), findsOneWidget);
      expect(find.byKey(const Key('row-atlas')), findsNothing);
      await tester.tap(inSegment<LibraryFilter>('Needs attention'));
      await tester.pump();
      expect(find.byKey(const Key('row-atlas')), findsOneWidget);
      expect(find.byKey(const Key('row-orbit')), findsOneWidget);
      expect(find.byKey(const Key('row-quill')), findsNothing);
    },
  );

  testWidgets('F18 the narrow filter tabs read All, Updates and Attention', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore(), size: narrowSize);
    expect(find.text('All 7'), findsOneWidget);
    expect(find.text('Updates 2'), findsOneWidget);
    expect(find.text('Attention 2'), findsOneWidget);
    await tester.tap(find.text('Attention 2'));
    await tester.pump();
    expect(model.filter, LibraryFilter.attention);
    expect(find.byKey(const Key('row-orbit')), findsOneWidget);
    expect(find.byKey(const Key('row-quill')), findsNothing);
  });

  testWidgets('F19 the search field filters by name, version or path', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    expect(find.text('Search apps'), findsOneWidget);
    await tester.enterText(find.byKey(const Key('search-field')), 'orbit');
    await tester.pump();
    expect(model.search, 'orbit');
    expect(find.byKey(const Key('row-orbit')), findsOneWidget);
    expect(find.byKey(const Key('row-quill')), findsNothing);
  });

  testWidgets('F20 the narrow search field takes name, version or path', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), size: narrowSize);
    expect(find.text('Search name, version, or path'), findsOneWidget);
    await tester.enterText(find.byKey(const Key('search-field')), '0.14.2');
    await tester.pump();
    expect(find.byKey(const Key('row-cinder')), findsOneWidget);
    expect(find.byKey(const Key('row-quill')), findsNothing);
  });

  testWidgets('F21 the sort dropdown reads Sort: Name and changes the order', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    expect(find.text('Sort:'), findsOneWidget);
    expect(find.text('Name'), findsOneWidget);
    await tester.tap(find.byKey(const Key('sort-menu')));
    await tester.pumpAndSettle();
    await tester.tap(
      find.widgetWithText(PopupMenuItem<SortOrder>, 'Version').last,
    );
    await tester.pumpAndSettle();
    expect(model.sort, SortOrder.version);
  });

  testWidgets('F22 the table headers read APP, VERSION and STATUS', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('APP'), findsOneWidget);
    expect(find.text('VERSION'), findsOneWidget);
    expect(find.text('STATUS'), findsOneWidget);
  });

  testWidgets('F23 an app row shows its tile, name and path', (tester) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Quill Notes'), findsOneWidget);
    expect(
      find.text('~/AppImages/Quill-Notes-x86_64.AppImage'),
      findsOneWidget,
    );
    expect(find.text('Q'), findsWidgets);
  });

  testWidgets('F24 a running app shows the Running badge', (tester) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Running'), findsWidgets);
    expect(
      find.descendant(
        of: find.byKey(const Key('row-quill')),
        matching: find.text('Running'),
      ),
      findsOneWidget,
    );
  });

  testWidgets(
    'F25 the version cell shows the update arrow to the new version',
    (tester) async {
      await pumpApp(tester, core: mockupCore());
      expect(
        find.descendant(
          of: find.byKey(const Key('row-quill')),
          matching: find.text('→ 2.5.0'),
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets('F26 the Updates page shows the version change as an arrow', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.updates);
    expect(find.text('→ 2.5.0'), findsOneWidget);
    expect(find.text('→ 3.2.0'), findsOneWidget);
  });

  testWidgets('F27 an available update reads Update available', (tester) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Update available'), findsOneWidget);
    expect(find.text('Confirm required while running'), findsOneWidget);
  });

  testWidgets('F28 an up-to-date app reads Up to date with its check time', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Up to date'), findsWidgets);
    expect(find.text('Checked 09:14'), findsWidgets);
  });

  testWidgets('F29 a source without a checksum reads Reduced verification', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Reduced verification'), findsWidgets);
    expect(find.text('Source publishes no checksum'), findsOneWidget);
  });

  testWidgets('F30 a failed check reads Check failed, never up to date', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Check failed'), findsWidgets);
    expect(find.text('Status unknown, not up to date'), findsOneWidget);
  });

  testWidgets('F31 an adopted app outside the folder reads Adopted', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Adopted'), findsOneWidget);
    expect(find.text('Outside the managed folder'), findsOneWidget);
  });

  testWidgets('F27b a running update reads Updating with its percent', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    expect(find.text('Updating · 62%'), findsOneWidget);
  });

  testWidgets('F32 Launch on a row starts the app', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core);
    await tester.tap(find.byKey(const Key('launch-quill')));
    await tester.pump();
    expect(core.calls, contains('launchApp:quill'));
  });

  testWidgets('F33 the row menu button opens the app actions', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core);
    await tester.tap(find.byKey(const Key('row-menu-atlas')));
    await tester.pumpAndSettle();
    expect(
      find.widgetWithText(PopupMenuItem<String>, 'Launch'),
      findsOneWidget,
    );
    expect(
      find.widgetWithText(PopupMenuItem<String>, 'Move to Trash'),
      findsOneWidget,
    );
    expect(
      find.widgetWithText(PopupMenuItem<String>, 'Delete permanently'),
      findsOneWidget,
    );
    await tester.tap(find.widgetWithText(PopupMenuItem<String>, 'Launch'));
    await tester.pumpAndSettle();
    expect(core.calls, contains('launchApp:atlas'));
  });

  testWidgets('F34 the narrow menu button opens the six pages', (tester) async {
    await pumpApp(tester, core: mockupCore(), size: narrowSize);
    expect(find.byKey(const Key('nav-library')), findsNothing);
    await tapCenter(tester, find.byKey(const Key('menu-button')));
    await tester.pumpAndSettle();
    for (final key in [
      'nav-library',
      'nav-inspect',
      'nav-updates',
      'nav-tasks',
      'nav-settings',
      'nav-about',
    ]) {
      expect(find.byKey(Key(key)), findsOneWidget, reason: key);
    }
    await tester.tap(find.byKey(const Key('nav-settings')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('nav-settings')), findsNothing);
  });

  testWidgets('F35 narrow rows collapse to name, version and one status line', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), size: narrowSize);
    expect(find.text('Update to 2.5.0'), findsOneWidget);
    expect(find.text(' · Running'), findsOneWidget);
    expect(find.text('Reduced verification'), findsOneWidget);
    expect(find.text('Launch'), findsNothing);
    expect(find.byKey(const Key('row-menu-quill')), findsOneWidget);
  });

  testWidgets(
    'F36 the narrow status bar counts apps, updates and failed checks',
    (tester) async {
      await pumpApp(tester, core: mockupCore(), size: narrowSize);
      expect(find.text('7 installed'), findsOneWidget);
      expect(find.text('2 updates'), findsOneWidget);
      expect(find.text('1 check failed'), findsOneWidget);
    },
  );

  testWidgets('F37 the empty library card asks for an AppImage', (
    tester,
  ) async {
    await pumpApp(tester, core: FakeCore());
    expect(find.text('Drop an AppImage here'), findsOneWidget);
    expect(find.byKey(const Key('empty-card')), findsOneWidget);
  });

  testWidgets('F38 the empty card says nothing is installed until Integrate', (
    tester,
  ) async {
    await pumpApp(tester, core: FakeCore());
    expect(
      find.text(
        'Or open one from your file manager. You can inspect it first. Nothing '
        'is installed until you press Integrate.',
      ),
      findsOneWidget,
    );
  });

  testWidgets('F39 the empty library subtitle reads No apps yet', (
    tester,
  ) async {
    await pumpApp(tester, core: FakeCore());
    expect(find.text('No apps yet'), findsOneWidget);
  });

  testWidgets('F40 Open Inspect on the empty card opens Inspect', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: FakeCore());
    await tester.tap(find.byKey(const Key('open-inspect')));
    await tester.pump();
    expect(model.page, AppPage.inspect);
  });

  // Detail ------------------------------------------------------------------

  Future<AppModel> openDetail(WidgetTester tester, FakeCore core) async {
    final model = await pumpApp(tester, core: core);
    await tester.tap(find.byKey(const Key('open-quill')));
    await tester.pump();
    return model;
  }

  testWidgets('F41 the breadcrumb returns from Quill Notes to Library', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await openDetail(tester, core);
    expect(find.text('Quill Notes'), findsWidgets);
    await tester.tap(find.byKey(const Key('breadcrumb-library')));
    await tester.pump();
    expect(model.selectedUuid, isNull);
    expect(find.byKey(const Key('row-quill')), findsOneWidget);
  });

  testWidgets('F42 Reveal in folder reveals the app', (tester) async {
    final core = mockupCore();
    await openDetail(tester, core);
    await tester.tap(find.byKey(const Key('detail-reveal')));
    await tester.pump();
    expect(core.calls, contains('revealApp:quill'));
  });

  testWidgets('F43 Check for update asks the source and applies nothing', (
    tester,
  ) async {
    final core = mockupCore()
      ..checkResults['quill'] = fakeCheck(
        uuid: 'quill',
        currentVersion: '2.4.1',
        availableVersion: '2.5.0',
        downloadSize: 66000000,
      );
    final model = await openDetail(tester, core);
    expect(
      model.library.firstWhere((app) => app.uuid == 'quill').version,
      '2.4.1',
    );
    await tester.tap(find.byKey(const Key('detail-check')));
    await tester.pump();
    // The check reached the core, and no update or confirmation followed.
    expect(core.calls, contains('checkOneUpdate:quill'));
    expect(core.calls.where((call) => call.startsWith('applyUpdate')), isEmpty);
    expect(find.text('Quill Notes is running'), findsNothing);
    // The installed version is the one it was.
    expect(
      model.library.firstWhere((app) => app.uuid == 'quill').version,
      '2.4.1',
    );
    expect(model.status?.text, 'Quill Notes: 2.5.0 available');
  });

  testWidgets(
    'F43b Check for update on an app that is current clears its offer',
    (tester) async {
      final core = mockupCore()
        ..checkResults['quill'] = fakeCheck(
          uuid: 'quill',
          currentVersion: '2.4.1',
        );
      final model = await openDetail(tester, core);
      expect(model.offerFor('quill'), isNotNull);
      await tester.tap(find.byKey(const Key('detail-check')));
      await tester.pump();
      expect(model.offerFor('quill'), isNull);
      expect(model.status?.text, 'Quill Notes is up to date');
      expect(
        core.calls.where((call) => call.startsWith('applyUpdate')),
        isEmpty,
      );
    },
  );

  testWidgets(
    'F43c a check that times out reads Check failed, not up to date',
    (tester) async {
      final core = mockupCore()
        ..checkResults['quill'] = fakeCheck(
          uuid: 'quill',
          currentVersion: '2.4.1',
          error: 'Network request failed: timed out',
          timedOut: true,
        );
      final model = await openDetail(tester, core);
      await tester.tap(find.byKey(const Key('detail-check')));
      await tester.pump();
      expect(model.failureFor('quill'), isNotNull);
      expect(model.offerFor('quill'), isNull);
      expect(
        model.status?.text,
        'Quill Notes: Timed out. Status is unknown, not up to date.',
      );
    },
  );

  testWidgets('F44 Launch on the Detail header starts the app', (tester) async {
    final core = mockupCore();
    await openDetail(tester, core);
    await tester.tap(find.byKey(const Key('detail-launch')));
    await tester.pump();
    expect(core.calls, contains('launchApp:quill'));
  });

  testWidgets(
    'F45 the Detail status line shows running, the update and the integration',
    (tester) async {
      await openDetail(tester, mockupCore());
      expect(find.text('Running'), findsWidgets);
      expect(find.text('Update 2.5.0 available'), findsOneWidget);
      expect(find.text('Integrated 2 Sep 2026'), findsWidgets);
    },
  );

  testWidgets('F46 the Record card lists the app facts', (tester) async {
    await openDetail(tester, mockupCore());
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
      expect(find.text(label), findsWidgets, reason: label);
    }
    expect(find.text('GitHub · example-org/quill-notes'), findsOneWidget);
  });

  testWidgets('F47 Refresh on the Record card refreshes the metadata', (
    tester,
  ) async {
    final core = mockupCore();
    await openDetail(tester, core);
    await tester.tap(find.byKey(const Key('detail-refresh')));
    await tester.pump();
    expect(core.calls, contains('refreshMetadata:quill'));
  });

  testWidgets('F48 Launch options shows the arguments and saves them', (
    tester,
  ) async {
    final core = mockupCore();
    await openDetail(tester, core);
    expect(fieldText(tester, 'arguments-field'), '--ozone-platform=wayland');
    expect(find.text('Arguments'), findsOneWidget);
    await tester.enterText(
      find.byKey(const Key('arguments-field')),
      '--new-flag',
    );
    await tester.tap(find.byKey(const Key('launch-options-add')));
    await tester.pump();
    expect(core.calls, contains('saveArgumentsAndEnvironment:quill'));
    expect(core.lastArguments, ['--new-flag']);
  });

  testWidgets('F49 Launch options shows the environment and Add saves it', (
    tester,
  ) async {
    final core = mockupCore();
    await openDetail(tester, core);
    expect(fieldText(tester, 'environment-field'), 'QT_QPA_PLATFORM=wayland');
    expect(find.text('Add'), findsOneWidget);
    await tester.tap(find.byKey(const Key('launch-options-add')));
    await tester.pump();
    expect(
      core.calls.where(
        (call) => call.startsWith('saveArgumentsAndEnvironment'),
      ),
      isNotEmpty,
    );
  });

  testWidgets(
    'F50 the update source selector offers the managers and saves the choice',
    (tester) async {
      final core = mockupCore();
      await openDetail(tester, core);
      expect(find.text('GitHub'), findsOneWidget);
      await tester.tap(find.byKey(const Key('source-selector')));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(PopupMenuItem<String>, 'GitLab'));
      await tester.pumpAndSettle();
      expect(
        core.calls.where(
          (call) => call.startsWith('setUpdateSource:quill:gitlab'),
        ),
        isNotEmpty,
      );
    },
  );

  testWidgets(
    'F51 the source key=value field shows the pair and saves on Enter',
    (tester) async {
      final core = mockupCore();
      await openDetail(tester, core);
      expect(
        fieldText(tester, 'source-config-field'),
        'repo=example-org/quill-notes',
      );
      await tester.enterText(
        find.byKey(const Key('source-config-field')),
        'repo=quill',
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pump();
      expect(
        core.calls.where(
          (call) => call.startsWith('setUpdateSource:quill:github:repo=quill'),
        ),
        isNotEmpty,
      );
    },
  );

  testWidgets(
    'F52 the source help says one key=value pair and reduced verification',
    (tester) async {
      await openDetail(tester, mockupCore());
      expect(
        find.text(
          'One key=value pair. Multi-key sources are set from the CLI. Sources '
          'without a published checksum are marked reduced verification.',
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets('F53 Move to Trash asks, then trashes the app', (tester) async {
    final core = mockupCore();
    await openDetail(tester, core);
    await tester.tap(find.byKey(const Key('detail-trash')));
    await tester.pump();
    expect(find.text('Move Quill Notes to the Trash?'), findsOneWidget);
    expect(
      find.text(
        'The AppImage moves to the Trash and its menu entry is removed.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const Key('dialog-confirm')));
    await tester.pump();
    expect(core.calls, contains('removeApp:quill:trash'));
  });

  testWidgets('F54 Delete… asks again before it deletes permanently', (
    tester,
  ) async {
    final core = mockupCore();
    await openDetail(tester, core);
    await tester.tap(find.byKey(const Key('detail-delete')));
    await tester.pump();
    expect(find.text('Permanently delete Quill Notes?'), findsOneWidget);
    expect(core.calls, isNot(contains('removeApp:quill:permanent')));
    await tester.tap(find.byKey(const Key('dialog-confirm')));
    await tester.pump();
    expect(core.calls, contains('removeApp:quill:permanent'));
  });

  testWidgets('F55 the remove card says it moves to the Trash and asks again', (
    tester,
  ) async {
    await openDetail(tester, mockupCore());
    expect(
      find.text('Moves to the Trash. Permanent delete asks again.'),
      findsOneWidget,
    );
  });

  testWidgets(
    'trash that fails deletes nothing (AGENTS: never delete when Trash fails)',
    (tester) async {
      final core = mockupCore()
        ..failures['removeApp'] = coreError('Trash unavailable');
      final model = await openDetail(tester, core);
      await tester.tap(find.byKey(const Key('detail-trash')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('dialog-confirm')));
      await tester.pumpAndSettle();
      expect(model.status?.text, 'Trash unavailable');
      expect(model.library.any((app) => app.uuid == 'quill'), isTrue);
      expect(core.calls, isNot(contains('removeApp:quill:permanent')));
    },
  );

  // Inspect -----------------------------------------------------------------

  Future<AppModel> inspectCinder(WidgetTester tester, FakeCore core) async {
    final model = await pumpApp(tester, core: core, page: AppPage.inspect);
    await model.startInspect(
      '/home/someone/Downloads/Cinder-Chat-0.14.2-x86_64.AppImage',
    );
    await tester.pump();
    return model;
  }

  testWidgets('F56 the Inspect subtitle explains what it reads', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.inspect);
    expect(
      find.text("Read a file's metadata before you integrate it"),
      findsOneWidget,
    );
  });

  testWidgets('F57 the banner says nothing is executed or installed', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.inspect);
    expect(find.byKey(const Key('inspect-banner')), findsOneWidget);
    expect(find.text('Nothing is executed or installed'), findsOneWidget);
  });

  testWidgets('F58 the metadata card lists the file facts', (tester) async {
    final core = mockupCore()
      ..inspectResult = fakeInspect(
        name: 'Cinder Chat',
        version: '0.14.2',
        categories: const ['Network', 'Chat'],
      );
    await inspectCinder(tester, core);
    for (final label in [
      'Type',
      'Architecture',
      'Size',
      'Categories',
      'Icon',
      'Update source',
      'SHA-256',
    ]) {
      expect(find.text(label), findsWidgets, reason: label);
    }
    expect(find.text('Network, Chat'), findsOneWidget);
  });

  testWidgets('F59 a matching architecture reads · matches', (tester) async {
    await inspectCinder(tester, mockupCore());
    expect(find.textContaining('· matches'), findsOneWidget);
  });

  testWidgets('F60 the Integrate card lists the three steps', (tester) async {
    await inspectCinder(tester, mockupCore());
    expect(find.text('Integrate into library'), findsOneWidget);
    expect(
      find.text('Writes a menu entry and installs the icon'),
      findsOneWidget,
    );
    expect(
      find.text('On a name conflict, asks to keep both or replace'),
      findsOneWidget,
    );
  });

  testWidgets('F61 step one names the managed folder', (tester) async {
    await inspectCinder(tester, mockupCore());
    expect(find.text('Copies the file into ~/AppImages'), findsOneWidget);
  });

  testWidgets(
    'F62 Move the original toggles the setting and says the source is never hard-deleted',
    (tester) async {
      final core = mockupCore();
      await inspectCinder(tester, core);
      expect(
        find.text(
          'After a verified copy the source goes to the Trash. It is never hard-deleted.',
        ),
        findsOneWidget,
      );
      await tester.tap(find.byKey(const Key('inspect-move-toggle')));
      await tester.pump();
      expect(core.lastPatch?.moveSource, isTrue);
    },
  );

  testWidgets('F63 Integrate copies the inspected file into the library', (
    tester,
  ) async {
    final core = mockupCore();
    await inspectCinder(tester, core);
    await tester.tap(find.byKey(const Key('integrate')));
    await tester.pumpAndSettle();
    expect(
      core.calls.where(
        (call) =>
            call.startsWith('integrateApp:/home/someone/Downloads/Cinder'),
      ),
      isNotEmpty,
    );
  });

  testWidgets('F64 Cancel on Inspect forgets the inspected file', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await inspectCinder(tester, core);
    await tester.tap(find.byKey(const Key('inspect-cancel')));
    await tester.pump();
    expect(model.inspect.inspectedOk, isFalse);
    expect(model.inspect.summary, isEmpty);
  });

  testWidgets('F59b a name conflict asks to keep both or replace', (
    tester,
  ) async {
    final core = mockupCore()
      ..integrateResult = fakeOutcome(
        ok: false,
        conflict: true,
        conflictUuid: 'atlas',
        conflictName: 'Atlas Viewer',
      );
    await inspectCinder(tester, core);
    await tester.tap(find.byKey(const Key('integrate')));
    await tester.pumpAndSettle();
    expect(find.text('Atlas Viewer is already integrated'), findsOneWidget);
    expect(find.text('Keep both'), findsOneWidget);
    expect(find.text('Replace'), findsOneWidget);
    await tester.tap(find.byKey(const Key('dialog-keep-both')));
    await tester.pumpAndSettle();
    expect(core.calls.where((call) => call.endsWith(':keepBoth')), isNotEmpty);
  });

  testWidgets('F59c Replace on a name conflict replaces the installed copy', (
    tester,
  ) async {
    final core = mockupCore()
      ..integrateResult = fakeOutcome(
        ok: false,
        conflict: true,
        conflictUuid: 'atlas',
        conflictName: 'Atlas Viewer',
      );
    await inspectCinder(tester, core);
    await tester.tap(find.byKey(const Key('integrate')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('dialog-replace')));
    await tester.pumpAndSettle();
    expect(core.calls.where((call) => call.endsWith(':replace')), isNotEmpty);
    expect(core.lastReplaceUuid, 'atlas');
  });

  testWidgets('F59d Browse… on Inspect picks a file to inspect', (
    tester,
  ) async {
    FilePickers.openAppImages = () async => [
      '/home/someone/Downloads/Other.AppImage',
    ];
    final model = await pumpApp(
      tester,
      core: mockupCore(),
      page: AppPage.inspect,
    );
    await tester.tap(find.byKey(const Key('inspect-browse')));
    await tester.pumpAndSettle();
    expect(model.inspect.pathInput, '/home/someone/Downloads/Other.AppImage');
  });

  // Updates -----------------------------------------------------------------

  testWidgets('F65 the Updates subtitle gives the check time', (tester) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.updates);
    expect(find.text('Last checked today at 09:14'), findsOneWidget);
  });

  testWidgets('F66 Check now checks every source again', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.updates);
    await tester.tap(find.byKey(const Key('check-now')));
    await tester.pumpAndSettle();
    expect(
      core.calls.where((call) => call == 'checkUpdates').length,
      greaterThanOrEqualTo(2),
    );
  });

  testWidgets('F67 Update all applies every update', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.updates);
    await tester.tap(find.byKey(const Key('update-all')));
    await tester.pumpAndSettle();
    expect(core.calls, contains('applyAllUpdates:normal'));
  });

  testWidgets('F68 the summary cards count available, up to date and unknown', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.updates);
    expect(find.text('Available'), findsOneWidget);
    expect(find.text('Up to date'), findsWidgets);
    expect(find.text('Unknown, check failed'), findsOneWidget);
    final available = tester.widget<Text>(
      find
          .descendant(
            of: find.byKey(const Key('summary-available')),
            matching: find.byType(Text),
          )
          .last,
    );
    final unknown = tester.widget<Text>(
      find
          .descendant(
            of: find.byKey(const Key('summary-unknown')),
            matching: find.byType(Text),
          )
          .last,
    );
    expect(available.data, '2');
    expect(unknown.data, '1');
  });

  testWidgets('F69 Update… on a running app asks before it updates', (
    tester,
  ) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.updates);
    await tester.tap(find.byKey(const Key('update-quill')));
    await tester.pumpAndSettle();
    expect(find.text('Quill Notes is running'), findsOneWidget);
    await tester.tap(find.byKey(const Key('dialog-confirm')));
    await tester.pumpAndSettle();
    expect(core.calls, contains('applyUpdate:quill:force'));
  });

  testWidgets('F70 a running app reads App is running and asks to confirm', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.updates);
    expect(find.text('App is running'), findsOneWidget);
    expect(find.text("You'll be asked to confirm"), findsOneWidget);
  });

  testWidgets('F71 an update in progress shows its percent on a bar', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.updates);
    expect(find.text('62%'), findsOneWidget);
    expect(find.byType(AppProgressBar), findsWidgets);
  });

  testWidgets('F72 reduced verification is noted in amber', (tester) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.updates);
    expect(
      find.text('Reduced verification: no checksum published'),
      findsOneWidget,
    );
  });

  testWidgets('F73 Retry checks the failed app again', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.updates);
    await tester.tap(find.byKey(const Key('retry-orbit')));
    await tester.pumpAndSettle();
    expect(
      core.calls.where((call) => call == 'checkUpdates').length,
      greaterThanOrEqualTo(2),
    );
  });

  testWidgets('F74 a timed-out check reads unknown, not up to date', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.updates);
    expect(
      find.text('Timed out. Status is unknown, not up to date.'),
      findsOneWidget,
    );
    expect(find.text('Check failed'), findsWidgets);
  });

  testWidgets('Cancel on a running update cancels that task', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.updates);
    await tester.tap(find.byKey(const Key('cancel-update-tidemark')));
    await tester.pump();
    expect(core.calls, contains('cancelTask:op-tidemark'));
  });

  // Tasks -------------------------------------------------------------------

  testWidgets('F75 the Tasks subtitle counts running and finished work', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.tasks);
    expect(find.text('1 running, 3 finished'), findsOneWidget);
  });

  testWidgets('F76 Clear finished removes completed entries only', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.tasks);
    await tester.tap(find.byKey(const Key('clear-finished')));
    await tester.pumpAndSettle();
    expect(core.calls, contains('clearFinishedTasks'));
    expect(model.finishedTasks, isEmpty);
    expect(model.runningTasks.length, 1);
  });

  testWidgets('F77 a running update shows its title, target and versions', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.tasks);
    expect(find.text('Updating Tidemark Photos'), findsOneWidget);
    expect(find.text('3.1.0 → 3.2.0'), findsOneWidget);
    expect(find.byKey(const Key('running-op-tidemark')), findsOneWidget);
  });

  testWidgets('F78 the update phase highlight follows the core phase index', (
    tester,
  ) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.tasks);
    FontWeight weightOf(String text) =>
        tester.widget<Text>(find.text(text)).style!.fontWeight!;
    // The mockup's state: phase 1, Download, is the active one.
    expect(find.text('1 Download'), findsOneWidget);
    expect(find.text('2 Verify'), findsOneWidget);
    expect(find.text('3 Swap in'), findsOneWidget);
    expect(weightOf('1 Download'), FontWeight.w500);
    expect(weightOf('2 Verify'), FontWeight.w400);

    // The core moves the update on to Swap in; the highlight moves with it.
    core.tasks
      ..removeWhere((task) => task.id == 'op-tidemark')
      ..add(
        fakeTask(
          id: 'op-tidemark',
          kind: TaskKindDto.update,
          state: TaskStateDto.running,
          title: 'Updating',
          target: 'Tidemark Photos',
          progress: 95,
          fromVersion: '3.1.0',
          toVersion: '3.2.0',
          phaseIndex: 3,
          phase: 'Swap in',
        ),
      );
    final model = await pumpApp(tester, core: core, page: AppPage.tasks);
    await model.refreshTasks();
    await tester.pump();
    expect(weightOf('3 Swap in'), FontWeight.w500);
    expect(weightOf('1 Download'), FontWeight.w400);
  });

  testWidgets('F79 the byte line reads the core byte counts', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.tasks);
    expect(find.text('41.2 of 66.0 MB'), findsOneWidget);

    core.tasks
      ..removeWhere((task) => task.id == 'op-tidemark')
      ..add(
        fakeTask(
          id: 'op-tidemark',
          kind: TaskKindDto.update,
          state: TaskStateDto.running,
          title: 'Updating',
          target: 'Tidemark Photos',
          progress: 25,
          fromVersion: '3.1.0',
          toVersion: '3.2.0',
          phaseIndex: 1,
          phase: 'Download',
          bytesDone: 16777216,
          bytesTotal: 69206016,
        ),
      );
    final model = await pumpApp(tester, core: core, page: AppPage.tasks);
    await model.refreshTasks();
    await tester.pump();
    expect(find.text('16.0 of 66.0 MB'), findsOneWidget);
    expect(find.text('41.2 of 66.0 MB'), findsNothing);
  });

  testWidgets('F80 finished work lists what happened with the time it ended', (
    tester,
  ) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.tasks);
    expect(find.text('Integrated Cinder Chat 0.14.2'), findsOneWidget);
    expect(find.text('Updated Brisk Terminal 1.1.4 → 1.2.0'), findsOneWidget);
    expect(find.text('Moved Old Notes 1.0 to the Trash'), findsOneWidget);
    expect(find.text('Today 08:52'), findsOneWidget);
    expect(find.text('Today 08:31'), findsOneWidget);
    expect(find.text('Yesterday 17:40'), findsOneWidget);
  });

  testWidgets('F80b the finish time is the one the core recorded', (
    tester,
  ) async {
    final core = mockupCore();
    final brisk = core.tasks.indexWhere((task) => task.id == 'op-brisk');
    core.tasks[brisk] = fakeTask(
      id: 'op-brisk',
      kind: TaskKindDto.update,
      state: TaskStateDto.succeeded,
      title: 'Updating',
      target: 'Brisk Terminal',
      fromVersion: '1.1.4',
      toVersion: '1.2.0',
      finishedAt: unixSeconds(DateTime(2026, 10, 7, 12, 5)),
    );
    await pumpApp(tester, core: core, page: AppPage.tasks);
    expect(find.text('Today 12:05'), findsOneWidget);
    expect(find.text('Today 08:31'), findsNothing);
  });

  // Settings ----------------------------------------------------------------

  testWidgets('F81 the Settings subtitle says options are off unless noted', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), page: AppPage.settings);
    expect(find.text('Options are off unless noted'), findsOneWidget);
  });

  testWidgets(
    'F82 the theme control offers System, Light and Dark and saves the choice',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.settings);
      expect(inSegment<AppearanceChoice>('System'), findsOneWidget);
      expect(inSegment<AppearanceChoice>('Light'), findsOneWidget);
      expect(inSegment<AppearanceChoice>('Dark'), findsOneWidget);
      await tester.tap(inSegment<AppearanceChoice>('Dark'));
      await tester.pumpAndSettle();
      expect(core.lastPatch?.appearance, AppearanceChoice.dark);
    },
  );

  testWidgets('F83 Change… picks the managed folder and saves it', (
    tester,
  ) async {
    final core = mockupCore();
    FilePickers.openFolder = () async => '/home/someone/Other';
    await pumpApp(tester, core: core, page: AppPage.settings);
    await tester.tap(find.byKey(const Key('change-folder')));
    await tester.pumpAndSettle();
    expect(core.lastPatch?.managedFolder, '/home/someone/Other');
  });

  testWidgets('F84 the maximum file size takes whole megabytes and says MB', (
    tester,
  ) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.settings);
    expect(find.text('MB'), findsOneWidget);
    expect(find.text('1 to 32768 MB'), findsOneWidget);
    await tester.enterText(find.byKey(const Key('max-size-field')), '1000');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(core.lastPatch?.maxAppimageBytes, 1000 * 1024 * 1024);
  });

  testWidgets('F85 Move originals instead of copying toggles on and off', (
    tester,
  ) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.settings);
    expect(find.text('Move originals instead of copying'), findsOneWidget);
    await tester.tap(find.byKey(const Key('toggle-move')));
    await tester.pump();
    expect(core.lastPatch?.moveSource, isTrue);
  });

  testWidgets('F86 Discover AppImages elsewhere toggles', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.settings);
    expect(find.text('Discover AppImages elsewhere'), findsOneWidget);
    await tester.tap(find.byKey(const Key('toggle-discover')));
    await tester.pump();
    expect(core.lastPatch?.manageOutsideFolder, isTrue);
  });

  testWidgets('F87 Drop .AppImage from terminal app names toggles', (
    tester,
  ) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.settings);
    expect(find.text('Drop .AppImage from terminal app names'), findsOneWidget);
    await tester.tap(find.byKey(const Key('toggle-terminal')));
    await tester.pump();
    expect(core.lastPatch?.terminalOmitSuffix, isTrue);
  });

  testWidgets('F88 Check in the background starts off and switches on', (
    tester,
  ) async {
    // The real default (off, owner decision). Nothing here forces it on.
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.settings);
    expect(
      find.text('Notifies only. Never downloads or applies.'),
      findsOneWidget,
    );
    expect(core.settings.backgroundUpdateChecks, isFalse);
    expect(toggleOf(tester, 'toggle-background').value, isFalse);

    await tester.tap(find.byKey(const Key('toggle-background')));
    await tester.pump();
    // The toggle stores true and the Settings page shows the stored value.
    expect(core.settings.backgroundUpdateChecks, isTrue);
    expect(core.lastPatch?.backgroundUpdateChecks, isTrue);
    expect(toggleOf(tester, 'toggle-background').value, isTrue);
    expect(model.settings?.backgroundUpdateChecks, isTrue);
  });

  testWidgets(
    'F88b the background setting shows its stored value after a restart',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.settings);
      await tester.tap(find.byKey(const Key('toggle-background')));
      await tester.pump();
      // A new model over the same stored settings is a restart.
      final restarted = AppModel(
        core: core,
        pollInterval: null,
        clock: () => testNow,
      );
      await restarted.start(const []);
      expect(restarted.settings?.backgroundUpdateChecks, isTrue);
    },
  );

  testWidgets(
    'F88c turning background checks off removes the login entry first',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.settings);
      await tester.tap(find.byKey(const Key('toggle-background')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('toggle-login')));
      await tester.pump();
      expect(core.settings.autostartEnabled, isTrue);

      await tester.tap(find.byKey(const Key('toggle-background')));
      await tester.pump();
      final removed = core.calls.indexOf('setAutostart:false');
      final stored = core.calls.lastIndexOf('saveSettings');
      expect(removed, isNonNegative);
      expect(
        removed,
        lessThan(stored),
        reason: 'the entry goes before the setting',
      );
      expect(core.settings.backgroundUpdateChecks, isFalse);
      expect(core.settings.autostartEnabled, isFalse);
    },
  );

  testWidgets(
    'F89 Also check at login is disabled until background checks are on',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.settings);
      expect(
        find.text('Adds an autostart entry for the same check'),
        findsOneWidget,
      );
      // Off by default, and disabled: a tap changes nothing.
      expect(toggleOf(tester, 'toggle-login').onChanged, isNull);
      await tester.tap(find.byKey(const Key('toggle-login')));
      await tester.pump();
      expect(
        core.calls.where((call) => call.startsWith('setAutostart')),
        isEmpty,
      );
      expect(core.settings.autostartEnabled, isFalse);

      // Once background checks are on, the login check can be switched on.
      await tester.tap(find.byKey(const Key('toggle-background')));
      await tester.pump();
      expect(toggleOf(tester, 'toggle-login').onChanged, isNotNull);
      await tester.tap(find.byKey(const Key('toggle-login')));
      await tester.pump();
      expect(core.calls, contains('setAutostart:true'));
      expect(toggleOf(tester, 'toggle-login').value, isTrue);
    },
  );

  testWidgets(
    'F89b the app does not ask the core for a login check while background checks are off',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.settings);
      await model.setAutostart(true);
      await tester.pump();
      expect(
        core.calls.where((call) => call.startsWith('setAutostart')),
        isEmpty,
      );
      expect(core.settings.autostartEnabled, isFalse);
      expect(model.status?.severity, Severity.error);
    },
  );

  testWidgets('F90 Verbose diagnostics toggles', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.settings);
    expect(find.text('Per-operation lines on stderr'), findsOneWidget);
    await tester.tap(find.byKey(const Key('toggle-debug')));
    await tester.pump();
    expect(core.lastPatch?.debugLogging, isTrue);
  });

  testWidgets(
    'F91 the unsafe extraction fallback is off, and turning it on asks first',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.settings);
      expect(find.text('Unsafe extraction fallback'), findsOneWidget);
      expect(find.text('Unavailable'), findsNothing);
      expect(
        find.text(
          'Runs the AppImage itself to read its files when safe extraction '
          'fails. It asks before each file. Only turn this on for AppImages '
          'you trust.',
        ),
        findsOneWidget,
      );
      expect(core.settings.unsafeExtractionFallback, isFalse);
      await tester.tap(find.byKey(const Key('toggle-unsafe')));
      await tester.pumpAndSettle();
      expect(model.dialog, isNotNull, reason: 'the warning asks first');
      expect(core.calls.where((call) => call == 'saveSettings'), isEmpty);
    },
  );

  testWidgets(
    'F92 the Settings cards are Appearance, Integration, Updates and Advanced',
    (tester) async {
      await pumpApp(tester, core: mockupCore(), page: AppPage.settings);
      for (final title in [
        'Appearance',
        'Integration',
        'Updates',
        'Advanced',
      ]) {
        expect(find.text(title), findsWidgets, reason: title);
      }
    },
  );

  testWidgets('F64 Cancel on a running task stops that task', (tester) async {
    final core = mockupCore();
    await pumpApp(tester, core: core, page: AppPage.tasks);
    await tester.tap(find.byKey(const Key('task-cancel-op-tidemark')));
    await tester.pump();
    expect(core.calls, contains('cancelTask:op-tidemark'));
  });

  testWidgets(
    'F64b Cancel closes the name-conflict dialog without integrating',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      model.showDialog(
        const IntegrateConflictDialog(
          path: '/tmp/Atlas.AppImage',
          conflictName: 'Atlas Viewer',
          incomingVersion: '0.9.3',
          replaceUuid: 'atlas',
          replaceLabel: 'Atlas Viewer',
          installedVersion: '0.8.9',
        ),
      );
      await tester.pump();
      await tester.tap(find.byKey(const Key('dialog-cancel')));
      await tester.pump();
      expect(model.dialog, isNull);
      expect(
        core.calls.where((call) => call.startsWith('integrateApp')),
        isEmpty,
      );
    },
  );

  testWidgets('F64c Cancel closes the running-app dialog without updating', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core);
    model.showDialog(
      const UpdateForceDialog(uuid: 'quill', name: 'Quill Notes'),
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('dialog-cancel')));
    await tester.pump();
    expect(model.dialog, isNull);
    expect(core.calls.where((call) => call.startsWith('applyUpdate')), isEmpty);
  });

  testWidgets(
    'ADOPT an AppImage found outside the library is adopted through its dialog',
    (tester) async {
      final found = fakeDiscovered(
        path: '/home/someone/Downloads/Found-1.0.AppImage',
        name: 'Found',
      );
      final core = FakeCore(
        library: LibraryDto(apps: mockupApps(), discovered: [found]),
      );
      final model = await pumpApp(tester, core: core);
      expect(find.text('Not managed yet'), findsOneWidget);
      await tester.tap(find.byKey(Key('adopt-button-${found.path}')));
      await tester.pump();
      expect(find.text('Adopt this AppImage?'), findsOneWidget);
      expect(model.dialog, isA<AdoptDialog>());
      await tester.tap(find.byKey(const Key('dialog-confirm')));
      await tester.pumpAndSettle();
      expect(core.calls, contains('adoptPath:${found.path}'));
    },
  );

  // Dialogs (frames 11-13) and the narrow layout -----------------------------

  testWidgets('S197 the narrow Library shows its page title and Browse…', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore(), size: narrowSize);
    expect(find.text('Library'), findsWidgets);
    expect(find.byKey(const Key('browse-narrow')), findsOneWidget);
  });

  testWidgets(
    'F203 the Remove dialog names the app and says it moves to the Trash',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      model.askRemove('quill', permanent: false);
      await tester.pump();
      expect(find.text('Move Quill Notes to the Trash?'), findsOneWidget);
      expect(find.byKey(const Key('dialog-cancel')), findsOneWidget);
      expect(find.text('Move to Trash'), findsOneWidget);
    },
  );

  testWidgets('S204 the Remove dialog body and Cancel leave the app alone', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core);
    model.askRemove('quill', permanent: false);
    await tester.pump();
    expect(
      find.text(
        'The AppImage moves to the Trash and its menu entry is removed.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const Key('dialog-cancel')));
    await tester.pump();
    expect(model.dialog, isNull);
    expect(core.calls, isNot(contains('removeApp:quill:trash')));
  });

  testWidgets('S205 the name conflict title names the installed app', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    model.showDialog(
      const IntegrateConflictDialog(
        path: '/home/someone/Downloads/Atlas-Viewer-0.9.3-x86_64.AppImage',
        conflictName: 'Atlas Viewer',
        incomingVersion: '0.9.3',
        replaceUuid: 'atlas',
        replaceLabel: 'Atlas Viewer',
        installedVersion: '0.8.9',
      ),
    );
    await tester.pump();
    expect(find.text('Atlas Viewer is already integrated'), findsOneWidget);
    expect(find.textContaining('The installed copy is 0.8.9.'), findsOneWidget);
  });

  testWidgets(
    'F206 the name conflict body gives both versions and what each choice does',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      model.showDialog(
        const IntegrateConflictDialog(
          path: '/tmp/Atlas.AppImage',
          conflictName: 'Atlas Viewer',
          incomingVersion: '0.9.3',
          replaceUuid: 'atlas',
          replaceLabel: 'Atlas Viewer',
          installedVersion: '0.8.9',
        ),
      );
      await tester.pump();
      expect(
        find.textContaining('The file you are integrating is 0.9.3.'),
        findsOneWidget,
      );
      expect(
        find.textContaining(
          'Keep both adds the new copy beside the existing one.',
        ),
        findsOneWidget,
      );
      expect(
        find.textContaining('Replace swaps the installed copy out.'),
        findsOneWidget,
      );
    },
  );

  testWidgets('S207 Keep both on the conflict dialog is a secondary choice', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    model.showDialog(
      const IntegrateConflictDialog(
        path: '/tmp/Atlas.AppImage',
        conflictName: 'Atlas Viewer',
        incomingVersion: '0.9.3',
        replaceUuid: 'atlas',
        replaceLabel: 'Atlas Viewer',
        installedVersion: '0.8.9',
      ),
    );
    await tester.pump();
    expect(find.text('Keep both'), findsOneWidget);
  });

  testWidgets('S208 Replace on the conflict dialog is the primary choice', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    model.showDialog(
      const IntegrateConflictDialog(
        path: '/tmp/Atlas.AppImage',
        conflictName: 'Atlas Viewer',
        incomingVersion: '0.9.3',
        replaceUuid: 'atlas',
        replaceLabel: 'Atlas Viewer',
        installedVersion: '0.8.9',
      ),
    );
    await tester.pump();
    expect(find.byKey(const Key('dialog-replace')), findsOneWidget);
  });

  testWidgets('S209 the running-app dialog names the running app', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore());
    model.showDialog(
      const UpdateForceDialog(uuid: 'quill', name: 'Quill Notes'),
    );
    await tester.pump();
    expect(find.text('Quill Notes is running'), findsOneWidget);
  });

  testWidgets(
    'F210 the running-app dialog body asks before replacing an open app',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      model.showDialog(
        const UpdateForceDialog(uuid: 'quill', name: 'Quill Notes'),
      );
      await tester.pump();
      expect(
        find.text(
          'Updating now replaces an app that is open. Close it first, or update anyway.',
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets('S211 Update anyway applies the update over the running app', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core);
    model.showDialog(
      const UpdateForceDialog(uuid: 'quill', name: 'Quill Notes'),
    );
    await tester.pump();
    expect(find.text('Update anyway'), findsOneWidget);
    await tester.tap(find.byKey(const Key('dialog-confirm')));
    await tester.pumpAndSettle();
    expect(core.calls, contains('applyUpdate:quill:force'));
  });

  testWidgets('Escape dismisses a pending dialog', (tester) async {
    final model = await pumpApp(tester, core: mockupCore());
    model.askRemove('quill', permanent: false);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(model.dialog, isNull);
  });

  testWidgets(
    'The narrow breakpoint sits between the 360 px and 1280 px frames',
    (tester) async {
      await pumpApp(tester, core: mockupCore(), size: desktopSize);
      expect(find.byKey(const Key('nav-library')), findsOneWidget);
      expect(find.byKey(const Key('menu-button')), findsNothing);
      tester.view.physicalSize = narrowSize;
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('nav-library')), findsNothing);
      expect(find.byKey(const Key('menu-button')), findsOneWidget);
      expect(narrowBreakpoint, 800);
    },
  );

  testWidgets('the Library rows hold their columns at the desktop width', (
    tester,
  ) async {
    await pumpApp(tester, core: mockupCore());
    final table = tester.getRect(find.byKey(const Key('library-table')));
    expect(table.width, closeTo(977, 1));
    expect(find.byType(LibraryPage), findsOneWidget);
  });

  testWidgets('F69b a stale running flag still asks before the update', (
    tester,
  ) async {
    // The cached offer says the app is not running. The core knows better.
    final core = mockupCore()
      ..scan = UpdateScanDto(
        offers: [
          fakeOffer(
            uuid: 'quill',
            name: 'Quill Notes',
            currentVersion: '2.4.1',
            availableVersion: '2.5.0',
          ),
        ],
        failures: const [],
        skipped: 0,
        checked: 7,
        cancelled: false,
      );
    final model = await pumpApp(tester, core: core);
    expect(model.offerFor('quill')!.running, isFalse);

    await model.updateOne('quill');
    await tester.pump();

    // The core refused, and the running-app dialog is up, not the CLI text.
    expect(core.calls, contains('applyUpdate:quill:normal'));
    expect(model.dialog, isA<UpdateForceDialog>());
    expect(find.text('Quill Notes is running'), findsOneWidget);
    expect(model.status?.text, isNot(contains('--force')));

    await model.confirmForceUpdate();
    expect(core.calls, contains('applyUpdate:quill:force'));
  });

  testWidgets(
    'D30 the dialog buttons are 30 px high, as section 10 specifies',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      model.askRemove('quill', permanent: false);
      await tester.pump();
      // The declared height is 30. A bordered secondary button draws its 1 px
      // border outside that height (the mockup's CSS box), so Cancel renders
      // at 32; the borderless primary renders at 30.
      AppButton buttonUnder(String key) => tester.widget<AppButton>(
        find.ancestor(
          of: find.byKey(Key(key)),
          matching: find.byType(AppButton),
        ),
      );
      expect(buttonUnder('dialog-cancel').height, 30);
      expect(buttonUnder('dialog-confirm').height, 30);
      expect(
        tester.getSize(find.byKey(const Key('dialog-confirm'))).height,
        30,
      );
    },
  );

  testWidgets(
    'F91b the unsafe row is enabled: its text is not faded and its toggle is not dimmed',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.settings);
      final toggleWidget = find.ancestor(
        of: find.byKey(const Key('toggle-unsafe')),
        matching: find.byType(AppToggle),
      );
      // An enabled toggle is drawn at full opacity, and no row fade sits over
      // its title.
      final toggleFades = [
        for (final fade in tester.widgetList<Opacity>(
          find.descendant(of: toggleWidget, matching: find.byType(Opacity)),
        ))
          fade.opacity,
      ];
      expect(toggleFades, [1.0]);
      expect(
        find.ancestor(
          of: find.text('Unsafe extraction fallback'),
          matching: find.byWidgetPredicate(
            (widget) => widget is Opacity && widget.opacity < 1,
          ),
        ),
        findsNothing,
      );
      await tester.tap(find.byKey(const Key('toggle-unsafe')));
      await tester.pumpAndSettle();
      expect(
        model.dialog,
        isNotNull,
        reason: 'a tap asks, rather than doing nothing',
      );
    },
  );

  testWidgets(
    'F9b the About rows use the Settings label and help roles and keep the original wording',
    (tester) async {
      await pumpApp(tester, core: mockupCore(), page: AppPage.about);
      final label = tester.widget<Text>(find.text('Made by')).style!;
      expect(label.fontSize, 13);
      expect(label.fontWeight, FontWeight.w500);
      final help = tester
          .widget<Text>(find.text('No telemetry of any kind.'))
          .style!;
      expect(help.fontSize, 12);
      final footer = tester
          .widget<Text>(find.text('Licensed under GPL-3.0-or-later.'))
          .style!;
      expect(footer.fontSize, 12, reason: 'the footer takes the help role');
      expect(
        find.text(
          'Native application for safely inspecting, integrating, launching, '
          'organizing, updating, and removing AppImages.',
        ),
        findsOneWidget,
      );
      expect(find.textContaining('COSMIC'), findsNothing);
    },
  );
}
