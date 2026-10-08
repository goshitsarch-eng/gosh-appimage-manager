import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/app.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

import 'support/fakes.dart';
import 'support/harness.dart';

/// The parity rows of the Library and Detail pages (SPEC section 12): rows 3,
/// 10 to 14, 22 to 31 (27b included), 36 to 39, 45, 46 and 52. A test acts on
/// the app or reads the model, then checks what the page shows. Each name
/// starts with its row number, so docs/flutter/PARITY.md can cite it.
void main() {
  setUpAll(() async {
    await loadAppFonts();
    homeFolderProvider = () => '/home/someone';
  });

  tearDownAll(() {
    homeFolderProvider = () => null;
  });

  // Finders and readers -------------------------------------------------------

  /// The Library row of one app (`row-<uuid>`).
  Finder rowOf(String uuid) => find.byKey(Key('row-$uuid'));

  /// The 7 px status dot in a row's status cell. The running badge's dot is 6 px.
  Finder statusDotOf(String uuid) => find.descendant(
    of: rowOf(uuid),
    matching: find.byWidgetPredicate(
      (widget) => widget is StatusDot && widget.size == 7,
    ),
  );

  AppDto appOf(AppModel model, String uuid) =>
      model.library.firstWhere((app) => app.uuid == uuid);

  AppPalette paletteAt(WidgetTester tester, Finder finder) =>
      AppScope.of(tester.element(finder.first));

  TextStyle styleOf(WidgetTester tester, Finder finder) =>
      tester.widget<Text>(finder.first).style!;

  /// The colour of the dot beside [text] (the dot in the same row as the text).
  Color dotBeside(WidgetTester tester, Finder text) => tester
      .widget<StatusDot>(
        find.descendant(
          of: find.ancestor(of: text, matching: find.byType(Row)).first,
          matching: find.byType(StatusDot),
        ),
      )
      .color;

  /// The Detail page's status line: the wrap that holds Running, the update
  /// and the integration. Its spacing tells it apart from other wraps.
  Finder statusLine() => find.byWidgetPredicate(
    (widget) =>
        widget is Wrap && widget.spacing == 14 && widget.runSpacing == 4,
  );

  /// The Record card's label and value pairs, read in the order the card lays
  /// them out (label, then its value).
  Map<String, String> recordValues(WidgetTester tester) {
    final card = find
        .ancestor(of: find.text('Record'), matching: find.byType(AppCard))
        .first;
    final texts = [
      for (final text in tester.widgetList<Text>(
        find.descendant(of: card, matching: find.byType(Text)),
      ))
        text.data ?? '',
    ];
    final first = texts.indexOf('Path');
    return {
      for (var i = first; i + 1 < texts.length; i += 2) texts[i]: texts[i + 1],
    };
  }

  // Actions and fixtures ------------------------------------------------------

  /// Taps the middle of a control. Icon-only buttons need this (see features_test).
  Future<void> tapCenter(WidgetTester tester, Finder finder) =>
      tester.tapAt(tester.getCenter(finder));

  Future<void> openPage(WidgetTester tester, String navKey) async {
    await tester.tap(find.byKey(Key(navKey)));
    await tester.pumpAndSettle();
  }

  /// Presses Check now on the Updates page, then goes back to the Library.
  Future<void> checkNow(WidgetTester tester) async {
    await openPage(tester, 'nav-updates');
    await tester.tap(find.byKey(const Key('check-now')));
    await tester.pumpAndSettle();
    await openPage(tester, 'nav-library');
  }

  /// Puts [apps] in the core's library and reloads it, as the app does after
  /// the core changes something.
  Future<void> setLibrary(
    WidgetTester tester,
    AppModel model,
    FakeCore core,
    List<AppDto> apps,
  ) async {
    core.library = LibraryDto(apps: apps, discovered: const []);
    await model.loadLibrary();
    await tester.pump();
  }

  /// The mockup's seven apps, with the app [uuid] replaced by [change] applied.
  List<AppDto> mockupWith(String uuid, AppDto Function(AppDto app) change) => [
    for (final app in mockupApps()) app.uuid == uuid ? change(app) : app,
  ];

  /// The record of [app] with the given fields changed. The copy is made with
  /// fakeApp, the way the core's records are.
  AppDto copyApp(
    AppDto app, {
    String? version,
    bool? running,
    bool? reducedVerification,
    bool? externalFolder,
    String? managedPath,
    int? sizeBytes,
    int? integratedAt,
    String? integratedFolder,
  }) => fakeApp(
    uuid: app.uuid,
    name: app.name,
    version: version ?? app.version,
    managedPath: managedPath ?? app.managedPath,
    running: running ?? app.running,
    adopted: app.adopted,
    externalFolder: externalFolder ?? app.externalFolder,
    reducedVerification: reducedVerification ?? app.reducedVerification,
    sizeBytes: sizeBytes ?? app.sizeBytes,
    appType: app.appType,
    architecture: app.architecture,
    sha256: app.sha256,
    desktopId: app.desktopId,
    arguments: app.arguments,
    environment: app.environment,
    updateManager: app.updateManager,
    updateConfig: app.updateConfig,
    embeddedUpdate: app.embeddedUpdate,
    integratedAt: integratedAt ?? app.integratedAt,
    integratedFolder: integratedFolder ?? app.integratedFolder,
  );

  /// Pumps the app over [core] as pumpApp does, but with a clock the test can
  /// move on, so a later check stamps a later time.
  Future<AppModel> pumpWithClock(
    WidgetTester tester,
    FakeCore core,
    DateTime Function() clock,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = desktopSize;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    core.settings = fakeSettings(appearance: AppearanceChoice.light);
    final model = AppModel(core: core, pollInterval: null, clock: clock);
    await model.start(const []);
    await model.checkUpdates();
    model.dismissStatus();
    await tester.pumpWidget(GoshApp(model: model));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    return model;
  }

  // Window and sidebar --------------------------------------------------------

  testWidgets(
    'F3 the title-bar logo tile is a 22 by 22 accent tile on every page',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      for (final page in AppPage.values) {
        model.setPage(page);
        await tester.pump();
        expect(model.page, page);
        final logo = find
            .ancestor(
              of: find.descendant(
                of: find.byKey(const Key('title-bar')),
                matching: find.text('G'),
              ),
              matching: find.byType(Container),
            )
            .first;
        expect(tester.getSize(logo), const Size(22, 22), reason: page.name);
        final decoration = tester.widget<Container>(logo).decoration;
        expect(
          (decoration as BoxDecoration).color,
          paletteAt(tester, logo).accent,
          reason: page.name,
        );
      }
    },
  );

  testWidgets(
    'F10 the managed folder box names the folder and the apps it holds',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      final box = find.byKey(const Key('managed-folder-box'));
      expect(model.managedFolderPath, '/home/someone/AppImages');
      expect(
        find.descendant(of: box, matching: find.text('MANAGED FOLDER')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: box, matching: find.text('~/AppImages')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: box, matching: find.text('7 apps · 512 MB')),
        findsOneWidget,
      );
      // The note follows the library: an empty library reads Empty, and the
      // folder stays the one the settings name.
      await setLibrary(tester, model, core, const []);
      expect(
        find.descendant(of: box, matching: find.text('Empty')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: box, matching: find.text('7 apps · 512 MB')),
        findsNothing,
      );
      expect(
        find.descendant(of: box, matching: find.text('~/AppImages')),
        findsOneWidget,
      );
    },
  );

  testWidgets('F10 the box shows the managed folder the core saved', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core);
    model.setManagedFolderInput('/home/someone/Apps');
    await model.applyManagedFolder();
    await tester.pump();
    final box = find.byKey(const Key('managed-folder-box'));
    expect(core.settings.managedFolder, '/home/someone/Apps');
    expect(
      find.descendant(of: box, matching: find.text('~/Apps')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: box, matching: find.text('~/AppImages')),
      findsNothing,
    );
  });

  testWidgets(
    'F11 the status bar counts apps, updates and failed checks, and a new check moves the counts',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      final bar = find.byKey(const Key('status-bar'));
      final installed = find.descendant(
        of: bar,
        matching: find.text('7 installed'),
      );
      final updates = find.descendant(
        of: bar,
        matching: find.text('2 updates'),
      );
      final failed = find.descendant(
        of: bar,
        matching: find.text('1 check failed'),
      );
      expect(model.library, hasLength(7));
      expect(model.updateCount, 2);
      expect(model.checkFailures, hasLength(1));
      expect(installed, findsOneWidget);
      expect(updates, findsOneWidget);
      expect(failed, findsOneWidget);
      final palette = paletteAt(tester, bar);
      expect(dotBeside(tester, updates), palette.accent);
      expect(dotBeside(tester, failed), palette.bad);
      // A check that finds nothing wrong clears the update and failure counts.
      core.scan = UpdateScanDto(
        offers: const [],
        failures: const [],
        skipped: 0,
        checked: 7,
        cancelled: false,
      );
      await checkNow(tester);
      expect(model.updateCount, 0);
      expect(model.checkFailures, isEmpty);
      expect(updates, findsNothing);
      expect(failed, findsNothing);
      expect(installed, findsOneWidget);
    },
  );

  testWidgets(
    'F12 the status bar reads the time of the last check, right aligned in mono',
    (tester) async {
      var now = testNow;
      final model = await pumpWithClock(tester, mockupCore(), () => now);
      final stamp = find.text('Last checked 09:14');
      expect(model.lastChecked, testNow);
      expect(stamp, findsOneWidget);
      expect(styleOf(tester, stamp).fontFamily, AppType.monoFamily);
      final bar = find.byKey(const Key('status-bar'));
      expect(
        tester.getTopRight(stamp).dx,
        closeTo(tester.getTopRight(bar).dx - 16, 0.5),
      );
      // The next check stamps the bar with its own time.
      now = DateTime(2026, 10, 7, 10, 2);
      await checkNow(tester);
      expect(model.lastChecked, now);
      expect(stamp, findsNothing);
      expect(find.text('Last checked 10:02'), findsOneWidget);
    },
  );

  testWidgets('F12 before any check the status bar reads Not checked yet', (
    tester,
  ) async {
    final model = await pumpApp(tester, core: mockupCore(), checked: false);
    expect(model.lastChecked, isNull);
    // The rows say Not checked yet too (QA D-03); the status bar is what this
    // test is about.
    expect(
      find.descendant(
        of: find.byKey(const Key('status-bar')),
        matching: find.text('Not checked yet'),
      ),
      findsOneWidget,
    );
    expect(find.textContaining('Last checked'), findsNothing);
  });

  testWidgets(
    'F13 the status bar reads 0 installed on an empty library, then counts the install',
    (tester) async {
      final core = FakeCore();
      final model = await pumpApp(tester, core: core);
      final bar = find.byKey(const Key('status-bar'));
      expect(model.library, isEmpty);
      expect(
        find.descendant(of: bar, matching: find.text('0 installed')),
        findsOneWidget,
      );
      await setLibrary(tester, model, core, [
        fakeApp(uuid: 'demo', name: 'Demo'),
      ]);
      expect(
        find.descendant(of: bar, matching: find.text('1 installed')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: bar, matching: find.text('0 installed')),
        findsNothing,
      );
    },
  );

  // Library -------------------------------------------------------------------

  testWidgets(
    'F14 the Library heading counts the apps and the adopted ones, and follows the library',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      expect(model.library.where((app) => app.adopted), hasLength(1));
      expect(find.text('7 apps in ~/AppImages and 1 adopted'), findsOneWidget);
      // Without the adopted app the heading drops the adopted count.
      await setLibrary(tester, model, core, [
        for (final app in mockupApps())
          if (!app.adopted) app,
      ]);
      expect(find.text('6 apps in ~/AppImages'), findsOneWidget);
      expect(find.textContaining('adopted'), findsNothing);
    },
  );

  testWidgets(
    'F22 the table headers read APP, VERSION and STATUS over their columns',
    (tester) async {
      await pumpApp(tester, core: mockupCore());
      expect(find.text('APP'), findsOneWidget);
      expect(find.text('VERSION'), findsOneWidget);
      expect(find.text('STATUS'), findsOneWidget);
      // Each header starts where its column starts in a row.
      final tile = find.descendant(
        of: rowOf('quill'),
        matching: find.byType(AppLetterTile),
      );
      final version = find.descendant(
        of: rowOf('quill'),
        matching: find.text('2.4.1'),
      );
      expect(
        tester.getTopLeft(find.text('APP')).dx,
        closeTo(tester.getTopLeft(tile).dx, 0.01),
      );
      expect(
        tester.getTopLeft(find.text('VERSION')).dx,
        closeTo(tester.getTopLeft(version).dx, 0.01),
      );
      expect(
        tester.getTopLeft(find.text('STATUS')).dx,
        closeTo(tester.getTopLeft(statusDotOf('quill')).dx, 0.01),
      );
      // The headers are uppercase IBM Plex Mono labels, semibold at 10.5 px.
      final label = styleOf(tester, find.text('VERSION'));
      expect(label.fontFamily, AppType.monoFamily);
      expect(label.fontSize, 10.5);
      expect(label.fontWeight, FontWeight.w600);
    },
  );

  testWidgets('F22 the table headers are shown only with a table to head', (
    tester,
  ) async {
    await pumpApp(tester, core: FakeCore());
    expect(find.text('APP'), findsNothing);
    expect(find.text('VERSION'), findsNothing);
    expect(find.text('STATUS'), findsNothing);
  });

  testWidgets(
    'F23 an app row shows its tile, name and path, and a tap on the name opens its Detail',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      final row = rowOf('quill');
      expect(
        tester
            .widget<AppLetterTile>(
              find.descendant(of: row, matching: find.byType(AppLetterTile)),
            )
            .letter,
        'Q',
      );
      expect(
        find.descendant(of: row, matching: find.text('Quill Notes')),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: row,
          matching: find.text('~/AppImages/Quill-Notes-x86_64.AppImage'),
        ),
        findsOneWidget,
      );
      expect(model.selectedUuid, isNull);
      await tester.tap(
        find.descendant(of: row, matching: find.text('Quill Notes')),
      );
      await tester.pump();
      expect(model.selectedUuid, 'quill');
      expect(find.byKey(const Key('breadcrumb-library')), findsOneWidget);
    },
  );

  testWidgets('F23 the row path follows the managed path the core reports', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core);
    await setLibrary(
      tester,
      model,
      core,
      mockupWith(
        'quill',
        (app) => copyApp(app, managedPath: '/home/someone/Apps/Quill.AppImage'),
      ),
    );
    expect(
      find.descendant(
        of: rowOf('quill'),
        matching: find.text('~/Apps/Quill.AppImage'),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: rowOf('quill'),
        matching: find.text('~/AppImages/Quill-Notes-x86_64.AppImage'),
      ),
      findsNothing,
    );
  });

  testWidgets(
    'F24 a running app shows a green Running badge, and the badge follows the running flag',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      final badge = find.descendant(
        of: rowOf('quill'),
        matching: find.text('Running'),
      );
      expect(appOf(model, 'quill').running, isTrue);
      expect(badge, findsOneWidget);
      final green = paletteAt(tester, badge).ok;
      expect(styleOf(tester, badge).color, green);
      final dot = tester.widget<StatusDot>(
        find.descendant(
          of: rowOf('quill'),
          matching: find.byWidgetPredicate(
            (widget) => widget is StatusDot && widget.size == 6,
          ),
        ),
      );
      expect(dot.color, green);
      expect(
        find.descendant(of: rowOf('brisk'), matching: find.text('Running')),
        findsNothing,
      );
      // Brisk Terminal starts running: the core reports it and the badge appears.
      await setLibrary(
        tester,
        model,
        core,
        mockupWith('brisk', (app) => copyApp(app, running: true)),
      );
      expect(
        find.descendant(of: rowOf('brisk'), matching: find.text('Running')),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'F25 the version cell shows an arrow to the new version only for an app with an offer',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      expect(model.offerFor('quill')?.availableVersion, '2.5.0');
      expect(model.offerFor('atlas'), isNull);
      final arrow = find.descendant(
        of: rowOf('quill'),
        matching: find.text('→ 2.5.0'),
      );
      expect(arrow, findsOneWidget);
      expect(
        find.descendant(of: rowOf('quill'), matching: find.text('2.4.1')),
        findsOneWidget,
      );
      expect(styleOf(tester, arrow).color, paletteAt(tester, arrow).accent);
      expect(styleOf(tester, arrow).fontWeight, FontWeight.w500);
      expect(
        find.descendant(of: rowOf('atlas'), matching: find.textContaining('→')),
        findsNothing,
      );
      // A check that offers nothing removes the arrow.
      core.scan = UpdateScanDto(
        offers: const [],
        failures: const [],
        skipped: 0,
        checked: 7,
        cancelled: false,
      );
      await checkNow(tester);
      expect(model.offerFor('quill'), isNull);
      expect(find.text('→ 2.5.0'), findsNothing);
    },
  );

  // A successful update drops its offer (app_model.dart _startUpdate and
  // updateAll), so the arrow and "Update available" go until a check finds a
  // newer version.
  testWidgets('F25 an applied update removes its arrow and its update status', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core);
    // Tidemark's update finishes and the core now reports version 3.2.0.
    final index = core.tasks.indexWhere((task) => task.id == 'op-tidemark');
    core.tasks[index] = fakeTask(
      id: 'op-tidemark',
      kind: TaskKindDto.update,
      state: TaskStateDto.succeeded,
      title: 'Updating',
      target: 'Tidemark Photos',
      fromVersion: '3.1.0',
      toVersion: '3.2.0',
    );
    await model.refreshTasks();
    core.library = LibraryDto(
      apps: mockupWith('tidemark', (app) => copyApp(app, version: '3.2.0')),
      discovered: const [],
    );
    // Tidemark's row menu applies its waiting update.
    await tester.tap(find.byKey(const Key('row-menu-tidemark')));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(PopupMenuItem<String>, 'Update…'));
    await tester.pumpAndSettle();
    expect(core.calls, contains('applyUpdate:tidemark:normal'));
    // The app is at 3.2.0 now, so nothing is waiting for it.
    expect(model.offerFor('tidemark'), isNull);
    expect(
      find.descendant(
        of: rowOf('tidemark'),
        matching: find.textContaining('→'),
      ),
      findsNothing,
    );
    expect(
      find.descendant(
        of: rowOf('tidemark'),
        matching: find.text('Update available'),
      ),
      findsNothing,
    );
  });

  testWidgets(
    'F26 the Updates rows show the version change as an arrow, and a check that drops an offer drops its row',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.updates);
      expect(model.updates.map((offer) => offer.uuid), ['quill', 'tidemark']);
      expect(
        find.descendant(
          of: find.byKey(const Key('update-row-quill')),
          matching: find.text('→ 2.5.0'),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: find.byKey(const Key('update-row-tidemark')),
          matching: find.text('→ 3.2.0'),
        ),
        findsOneWidget,
      );
      // Orbit Mail has a failed check and no offer, so its row has no arrow.
      expect(
        find.descendant(
          of: find.byKey(const Key('update-row-orbit')),
          matching: find.textContaining('→'),
        ),
        findsNothing,
      );
      // A check that offers only Quill Notes leaves one arrow and one row.
      core.scan = UpdateScanDto(
        offers: [
          fakeOffer(
            uuid: 'quill',
            name: 'Quill Notes',
            currentVersion: '2.4.1',
            availableVersion: '2.5.0',
            running: true,
          ),
        ],
        failures: const [],
        skipped: 0,
        checked: 7,
        cancelled: false,
      );
      await tester.tap(find.byKey(const Key('check-now')));
      await tester.pumpAndSettle();
      expect(model.updates.map((offer) => offer.uuid), ['quill']);
      expect(find.text('→ 3.2.0'), findsNothing);
      expect(find.text('→ 2.5.0'), findsOneWidget);
      expect(find.byKey(const Key('update-row-tidemark')), findsNothing);
    },
  );

  testWidgets(
    'F27 an available update reads Update available in accent, with the running note',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      final status = model.statusFor(appOf(model, 'quill'));
      expect(status.label, 'Update available');
      expect(status.tone, StatusTone.accent);
      expect(status.detail, 'Confirm required while running');
      final label = find.descendant(
        of: rowOf('quill'),
        matching: find.text('Update available'),
      );
      expect(label, findsOneWidget);
      expect(
        find.descendant(
          of: rowOf('quill'),
          matching: find.text('Confirm required while running'),
        ),
        findsOneWidget,
      );
      expect(
        tester.widget<StatusDot>(statusDotOf('quill')).color,
        paletteAt(tester, label).accent,
      );
      expect(paletteAt(tester, label).accent, const Color(0xFF3D4DB5));
      // An offer for an app that is not running carries no running note.
      core.scan = UpdateScanDto(
        offers: [
          fakeOffer(
            uuid: 'quill',
            name: 'Quill Notes',
            currentVersion: '2.4.1',
            availableVersion: '2.5.0',
          ),
        ],
        failures: [fakeFailure(uuid: 'orbit', name: 'Orbit Mail')],
        skipped: 0,
        checked: 7,
        cancelled: false,
      );
      await checkNow(tester);
      expect(model.statusFor(appOf(model, 'quill')).detail, isEmpty);
      expect(find.text('Confirm required while running'), findsNothing);
      expect(
        find.descendant(
          of: rowOf('quill'),
          matching: find.text('Update available'),
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'F27b a running update reads Updating with its percent, and the percent follows the core task',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      final tidemark = appOf(model, 'tidemark');
      expect(model.updateTaskFor(tidemark)?.progress, 62);
      expect(model.statusFor(tidemark).label, 'Updating · 62%');
      final label = find.descendant(
        of: rowOf('tidemark'),
        matching: find.text('Updating · 62%'),
      );
      expect(label, findsOneWidget);
      expect(
        find.descendant(
          of: rowOf('tidemark'),
          matching: find.text('Reduced verification'),
        ),
        findsOneWidget,
      );
      expect(
        tester.widget<StatusDot>(statusDotOf('tidemark')).color,
        paletteAt(tester, label).accent,
      );
      expect(
        find.descendant(of: rowOf('tidemark'), matching: find.text('→ 3.2.0')),
        findsOneWidget,
      );
      // The core reports 80 per cent for the running update.
      final index = core.tasks.indexWhere((task) => task.id == 'op-tidemark');
      core.tasks[index] = fakeTask(
        id: 'op-tidemark',
        kind: TaskKindDto.update,
        state: TaskStateDto.running,
        title: 'Updating',
        target: 'Tidemark Photos',
        progress: 80,
        fromVersion: '3.1.0',
        toVersion: '3.2.0',
        phaseIndex: 1,
        phase: 'Download',
      );
      await model.refreshTasks();
      await tester.pump();
      expect(
        find.descendant(
          of: rowOf('tidemark'),
          matching: find.text('Updating · 80%'),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: rowOf('tidemark'),
          matching: find.text('Updating · 62%'),
        ),
        findsNothing,
      );
      // When the update ends, the row falls back to the update it was waiting on.
      core.tasks[index] = fakeTask(
        id: 'op-tidemark',
        kind: TaskKindDto.update,
        state: TaskStateDto.succeeded,
        title: 'Updating',
        target: 'Tidemark Photos',
        fromVersion: '3.1.0',
        toVersion: '3.2.0',
        finishedAt: unixSeconds(DateTime(2026, 10, 7, 9, 20)),
      );
      await model.refreshTasks();
      await tester.pump();
      expect(model.updateTaskFor(tidemark), isNull);
      expect(
        find.descendant(
          of: rowOf('tidemark'),
          matching: find.text('Update available'),
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'F28 an up-to-date app reads Up to date in green, with the time of the last check',
    (tester) async {
      var now = testNow;
      final model = await pumpWithClock(tester, mockupCore(), () => now);
      final status = model.statusFor(appOf(model, 'cinder'));
      expect(status.label, 'Up to date');
      expect(status.tone, StatusTone.ok);
      expect(status.detail, 'Checked 09:14');
      final label = find.descendant(
        of: rowOf('cinder'),
        matching: find.text('Up to date'),
      );
      expect(label, findsOneWidget);
      expect(
        find.descendant(
          of: rowOf('cinder'),
          matching: find.text('Checked 09:14'),
        ),
        findsOneWidget,
      );
      expect(
        tester.widget<StatusDot>(statusDotOf('cinder')).color,
        const Color(0xFF2E7D4F),
      );
      // A later check stamps the row with the later time.
      now = DateTime(2026, 10, 7, 10, 2);
      await checkNow(tester);
      expect(model.statusFor(appOf(model, 'cinder')).detail, 'Checked 10:02');
      expect(
        find.descendant(
          of: rowOf('cinder'),
          matching: find.text('Checked 10:02'),
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'F29 a source without a checksum reads Reduced verification in amber, and reads Up to date once it is not marked so',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      final status = model.statusFor(appOf(model, 'atlas'));
      expect(status.label, 'Reduced verification');
      expect(status.tone, StatusTone.warn);
      expect(status.detail, 'Source publishes no checksum');
      final label = find.descendant(
        of: rowOf('atlas'),
        matching: find.text('Reduced verification'),
      );
      expect(label, findsOneWidget);
      expect(
        tester.widget<StatusDot>(statusDotOf('atlas')).color,
        const Color(0xFFB07A10),
      );
      await setLibrary(
        tester,
        model,
        core,
        mockupWith('atlas', (app) => copyApp(app, reducedVerification: false)),
      );
      expect(model.statusFor(appOf(model, 'atlas')).label, 'Up to date');
      expect(
        find.descendant(
          of: rowOf('atlas'),
          matching: find.text('Reduced verification'),
        ),
        findsNothing,
      );
    },
  );

  testWidgets(
    'F30 a failed check reads Check failed in red, never Up to date',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      final status = model.statusFor(appOf(model, 'orbit'));
      expect(status.label, 'Check failed');
      expect(status.tone, StatusTone.bad);
      expect(status.detail, 'Status unknown, not up to date');
      expect(
        find.descendant(
          of: rowOf('orbit'),
          matching: find.text('Check failed'),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: rowOf('orbit'),
          matching: find.text('Status unknown, not up to date'),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(of: rowOf('orbit'), matching: find.text('Up to date')),
        findsNothing,
      );
      expect(
        tester.widget<StatusDot>(statusDotOf('orbit')).color,
        const Color(0xFFA8322D),
      );
      // A check that succeeds for Orbit Mail clears the failure.
      core.scan = UpdateScanDto(
        offers: core.scan.offers,
        failures: const [],
        skipped: 0,
        checked: 7,
        cancelled: false,
      );
      await checkNow(tester);
      expect(model.checkFailures, isEmpty);
      expect(model.statusFor(appOf(model, 'orbit')).label, 'Up to date');
      expect(
        find.descendant(
          of: rowOf('orbit'),
          matching: find.text('Check failed'),
        ),
        findsNothing,
      );
    },
  );

  testWidgets(
    'F31 an adopted app outside the managed folder reads Adopted in grey, with its place',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      final ledger = appOf(model, 'ledger');
      expect(ledger.adopted, isTrue);
      expect(ledger.externalFolder, isTrue);
      final status = model.statusFor(ledger);
      expect(status.label, 'Adopted');
      expect(status.tone, StatusTone.mute);
      expect(status.detail, 'Outside the managed folder');
      expect(
        find.descendant(of: rowOf('ledger'), matching: find.text('Adopted')),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: rowOf('ledger'),
          matching: find.text('Outside the managed folder'),
        ),
        findsOneWidget,
      );
      expect(
        tester.widget<StatusDot>(statusDotOf('ledger')).color,
        const Color(0xFF8F8A82),
      );
      // Once the core reports it inside the managed folder, the detail says so.
      await setLibrary(
        tester,
        model,
        core,
        mockupWith('ledger', (app) => copyApp(app, externalFolder: false)),
      );
      expect(
        find.descendant(
          of: rowOf('ledger'),
          matching: find.text('Managed folder'),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: rowOf('ledger'),
          matching: find.text('Outside the managed folder'),
        ),
        findsNothing,
      );
    },
  );

  testWidgets(
    'F36 the narrow status bar counts apps, updates and failed checks, with the failure in red',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, size: narrowSize);
      final bar = find.byKey(const Key('status-bar'));
      expect(model.library, hasLength(7));
      expect(model.updateCount, 2);
      expect(model.checkFailures, hasLength(1));
      expect(
        find.descendant(of: bar, matching: find.text('7 installed')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: bar, matching: find.text('2 updates')),
        findsOneWidget,
      );
      final failed = find.descendant(
        of: bar,
        matching: find.text('1 check failed'),
      );
      expect(failed, findsOneWidget);
      expect(styleOf(tester, failed).color, paletteAt(tester, failed).bad);
      // The narrow bar leaves out the check time.
      expect(
        find.descendant(of: bar, matching: find.textContaining('Last checked')),
        findsNothing,
      );
      // A check without failures clears the failed count.
      core.scan = UpdateScanDto(
        offers: core.scan.offers,
        failures: const [],
        skipped: 0,
        checked: 7,
        cancelled: false,
      );
      await tapCenter(tester, find.byKey(const Key('menu-button')));
      await tester.pumpAndSettle();
      await openPage(tester, 'nav-updates');
      await tester.tap(find.byKey(const Key('check-now')));
      await tester.pumpAndSettle();
      expect(model.checkFailures, isEmpty);
      expect(
        find.descendant(of: bar, matching: find.text('1 check failed')),
        findsNothing,
      );
    },
  );

  // Empty library -------------------------------------------------------------

  testWidgets(
    'F37 the empty library shows a dashed drop card, and the card goes once an app is installed',
    (tester) async {
      final core = FakeCore();
      final model = await pumpApp(tester, core: core);
      final card = find.byKey(const Key('empty-card'));
      expect(model.library, isEmpty);
      expect(find.text('Drop an AppImage here'), findsOneWidget);
      final border = tester.widget<DashedBorder>(
        find.ancestor(of: card, matching: find.byType(DashedBorder)).first,
      );
      expect(border.width, 1.5);
      expect(border.color, const Color(0xFFCFCCC3));
      expect(find.byKey(const Key('library-table')), findsNothing);
      await setLibrary(tester, model, core, [
        fakeApp(uuid: 'demo', name: 'Demo'),
      ]);
      expect(card, findsNothing);
      expect(find.byKey(const Key('row-demo')), findsOneWidget);
    },
  );

  testWidgets(
    'F38 nothing is installed until Integrate: an inspect reads the file and installs nothing',
    (tester) async {
      final core = FakeCore();
      final model = await pumpApp(tester, core: core);
      expect(
        find.text(
          'Or open one from your file manager. You can inspect it first. Nothing '
          'is installed until you press Integrate.',
        ),
        findsOneWidget,
      );
      await tester.tap(find.byKey(const Key('open-inspect')));
      await tester.pump();
      expect(model.page, AppPage.inspect);
      await tester.enterText(
        find.byKey(const Key('inspect-path')),
        '/home/someone/Downloads/Demo.AppImage',
      );
      await tester.tap(find.byKey(const Key('inspect-run')));
      await tester.pumpAndSettle();
      expect(model.inspect.inspectedOk, isTrue);
      expect(
        core.calls,
        contains('inspectPath:/home/someone/Downloads/Demo.AppImage'),
      );
      // Inspecting installs nothing: no install call, and the library stays empty.
      expect(
        core.calls.where((call) => call.startsWith('integrateApp')),
        isEmpty,
      );
      expect(core.calls.where((call) => call.startsWith('adoptPath')), isEmpty);
      expect(model.library, isEmpty);
      await openPage(tester, 'nav-library');
      expect(find.byKey(const Key('empty-card')), findsOneWidget);
      // Integrate is the press that installs it.
      await openPage(tester, 'nav-inspect');
      await tester.tap(find.byKey(const Key('integrate')));
      await tester.pumpAndSettle();
      expect(
        core.calls.where(
          (call) => call.startsWith(
            'integrateApp:/home/someone/Downloads/Demo.AppImage',
          ),
        ),
        hasLength(1),
      );
    },
  );

  testWidgets(
    'F39 the empty library subtitle reads No apps yet until an app is installed',
    (tester) async {
      final core = FakeCore();
      final model = await pumpApp(tester, core: core);
      expect(model.library, isEmpty);
      expect(find.text('No apps yet'), findsOneWidget);
      await setLibrary(tester, model, core, [
        fakeApp(uuid: 'demo', name: 'Demo'),
      ]);
      expect(find.text('No apps yet'), findsNothing);
      expect(find.text('1 app in ~/AppImages'), findsOneWidget);
    },
  );

  // Detail --------------------------------------------------------------------

  testWidgets(
    'F45 the Detail status line shows Running, the waiting update and the integration date',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      await tester.tap(find.byKey(const Key('open-quill')));
      await tester.pump();
      expect(model.selectedUuid, 'quill');
      final quill = appOf(model, 'quill');
      expect(quill.running, isTrue);
      expect(model.offerFor('quill')?.availableVersion, '2.5.0');
      expect(
        DateTime.fromMillisecondsSinceEpoch(quill.integratedAt * 1000),
        DateTime(2026, 9, 2, 10),
      );
      final palette = paletteAt(tester, statusLine());
      expect(
        find.descendant(of: statusLine(), matching: find.text('Running')),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: statusLine(),
          matching: find.text('Update 2.5.0 available'),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: statusLine(),
          matching: find.text('Integrated 2 Sep 2026'),
        ),
        findsOneWidget,
      );
      expect(
        styleOf(
          tester,
          find.descendant(of: statusLine(), matching: find.text('Running')),
        ).color,
        palette.ok,
      );
      expect(
        styleOf(
          tester,
          find.descendant(
            of: statusLine(),
            matching: find.text('Update 2.5.0 available'),
          ),
        ).color,
        palette.accent,
      );
      // The Running item leaves the line when the core stops reporting the app as running.
      await setLibrary(
        tester,
        model,
        core,
        mockupWith('quill', (app) => copyApp(app, running: false)),
      );
      expect(
        find.descendant(of: statusLine(), matching: find.text('Running')),
        findsNothing,
      );
      expect(
        find.descendant(
          of: statusLine(),
          matching: find.text('Update 2.5.0 available'),
        ),
        findsOneWidget,
      );
      // The update item leaves once a check finds Quill Notes up to date.
      core.checkResults['quill'] = fakeCheck(
        uuid: 'quill',
        currentVersion: '2.4.1',
      );
      await tester.tap(find.byKey(const Key('detail-check')));
      await tester.pumpAndSettle();
      expect(model.offerFor('quill'), isNull);
      expect(
        find.descendant(
          of: statusLine(),
          matching: find.text('Update 2.5.0 available'),
        ),
        findsNothing,
      );
      expect(
        find.descendant(
          of: statusLine(),
          matching: find.text('Integrated 2 Sep 2026'),
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'F45 an app with no recorded install date reads Integrated alone',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      await setLibrary(
        tester,
        model,
        core,
        mockupWith('quill', (app) => copyApp(app, integratedAt: 0)),
      );
      await tester.tap(find.byKey(const Key('open-quill')));
      await tester.pump();
      expect(
        find.descendant(of: statusLine(), matching: find.text('Integrated')),
        findsOneWidget,
      );
      expect(find.textContaining('Integrated 2 Sep'), findsNothing);
    },
  );

  testWidgets('F46 the Record card lists the facts of the app in the library', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core);
    await tester.tap(find.byKey(const Key('open-quill')));
    await tester.pump();
    // The record's own fields are read from the library's record; the path,
    // size, source and provenance are the mockup's formatted values.
    final quill = appOf(model, 'quill');
    expect(recordValues(tester), {
      'Path': '~/AppImages/Quill-Notes-x86_64.AppImage',
      'Desktop ID': quill.desktopId,
      'SHA-256': quill.sha256,
      'Type': quill.appType,
      'Architecture': quill.architecture,
      'Size': '84.2 MB',
      'Update source': 'GitHub · example-org/quill-notes',
      'Provenance': 'Integrated 2 Sep 2026 from ~/Downloads',
    });
    // The path, desktop ID and checksum are monospace values.
    for (final value in [
      '~/AppImages/Quill-Notes-x86_64.AppImage',
      quill.desktopId,
      quill.sha256,
    ]) {
      expect(
        styleOf(tester, find.text(value)).fontFamily,
        AppType.monoFamily,
        reason: value,
      );
    }
    // The card shows the record the library holds.
    await setLibrary(
      tester,
      model,
      core,
      mockupWith('quill', (app) => copyApp(app, sizeBytes: 2048)),
    );
    expect(recordValues(tester)['Size'], '2.0 KB');
  });

  testWidgets(
    'F46b the Provenance line names the source folder, and reads Integrated alone for an older app',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      await tester.tap(find.byKey(const Key('open-quill')));
      await tester.pump();
      // Quill Notes was integrated from ~/Downloads on 2 Sep 2026. The status
      // line keeps the date alone.
      expect(
        recordValues(tester)['Provenance'],
        'Integrated 2 Sep 2026 from ~/Downloads',
      );
      expect(
        find.descendant(
          of: statusLine(),
          matching: find.text('Integrated 2 Sep 2026'),
        ),
        findsOneWidget,
      );
      // Without a stored folder the line keeps the date alone.
      await setLibrary(
        tester,
        model,
        core,
        mockupWith('quill', (app) => copyApp(app, integratedFolder: '')),
      );
      expect(recordValues(tester)['Provenance'], 'Integrated 2 Sep 2026');
      // An app integrated before this build has neither, and reads Integrated.
      await setLibrary(
        tester,
        model,
        core,
        mockupWith(
          'quill',
          (app) => copyApp(app, integratedAt: 0, integratedFolder: ''),
        ),
      );
      expect(recordValues(tester)['Provenance'], 'Integrated');
    },
  );

  testWidgets(
    'F52 the source help sits under the source field, which saves one key=value pair',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      await tester.tap(find.byKey(const Key('open-quill')));
      await tester.pump();
      expect(
        find.text(
          'One key=value pair. Multi-key sources are set from the CLI. Sources '
          'without a published checksum are marked reduced verification.',
        ),
        findsOneWidget,
      );
      // The help sits under the source row, aligned with its selector. The
      // field it describes is one line, so a save sends one pair.
      final field = find.byKey(const Key('source-config-field'));
      final selector = find.byKey(const Key('source-selector'));
      final help = find.text(
        'One key=value pair. Multi-key sources are set from the CLI. Sources '
        'without a published checksum are marked reduced verification.',
      );
      expect(
        tester.getTopLeft(help).dy,
        greaterThan(tester.getBottomLeft(field).dy),
      );
      expect(
        tester.getTopLeft(help).dx,
        closeTo(tester.getTopLeft(selector).dx, 0.5),
      );
      expect(tester.widget<TextField>(field).maxLines, 1);
      await tester.enterText(field, 'repo=quill');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pump();
      expect(model.sourceConfigInput, 'repo=quill');
      expect(core.calls, contains('setUpdateSource:quill:github:repo=quill'));
    },
  );
}
