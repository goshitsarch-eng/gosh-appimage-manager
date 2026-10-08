import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/ui/dialogs.dart';
import 'package:gosh_appimage_flutter/ui/page_frame.dart';
import 'package:gosh_appimage_flutter/ui/settings_page.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

import 'support/fakes.dart';
import 'support/harness.dart';

/// The FEATURES rows of the mockup (SPEC section 12) for the Remove help on the
/// Detail page, the Inspect, Updates, Tasks and Settings pages and the three
/// confirmation dialogs. A test is
/// named `F<row>` so docs/flutter/PARITY.md can cite it. Each test acts on the
/// app, then checks the model, the core call or the widget property that the
/// action leaves behind.
void main() {
  final originalHome = homeFolderProvider;

  setUpAll(() async {
    await loadAppFonts();
    // The mockup writes the managed folder as ~/AppImages. Pin the home folder
    // so that text does not depend on who runs the test.
    homeFolderProvider = () => '/home/someone';
  });

  tearDownAll(() {
    homeFolderProvider = originalHome;
  });

  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('gosh/window'),
          (call) async => null,
        );
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('gosh/window'), null);
  });

  const demoPath = '/home/someone/Downloads/Demo-1.0.AppImage';
  const atlasPath = '/home/someone/Downloads/Atlas-Viewer-0.9.4.AppImage';
  const reducedNotice = 'Reduced verification: no checksum published';
  const timedOutSentence = 'Timed out. Status is unknown, not up to date.';
  const inspectSubtitle = "Read a file's metadata before you integrate it";

  /// Lets a tap or a model change finish and paint.
  Future<void> settle(WidgetTester tester) async {
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
  }

  /// Taps a finder after scrolling it into view, then lets the change paint.
  Future<void> tapFinder(WidgetTester tester, Finder finder) async {
    await tester.ensureVisible(finder);
    await tester.tap(finder);
    await settle(tester);
  }

  Future<void> tapKey(WidgetTester tester, String key) =>
      tapFinder(tester, find.byKey(Key(key)));

  /// The toggle drawn under a key. The key sits on its tap target.
  AppToggle toggleOf(WidgetTester tester, String key) =>
      tester.widget<AppToggle>(
        find.ancestor(
          of: find.byKey(Key(key)),
          matching: find.byType(AppToggle),
        ),
      );

  /// The button drawn under a key. The key sits on its tap target.
  AppButton buttonOf(WidgetTester tester, String key) =>
      tester.widget<AppButton>(
        find.ancestor(
          of: find.byKey(Key(key)),
          matching: find.byType(AppButton),
        ),
      );

  Finder within(Finder ancestor, Finder target) =>
      find.descendant(of: ancestor, matching: target);

  Finder updateRow(String uuid) => find.byKey(Key('update-row-$uuid'));

  Finder runningCard(String id) => find.byKey(Key('running-$id'));

  Finder finishedRow(String id) => find.byKey(Key('finished-$id'));

  AppProgressBar progressIn(WidgetTester tester, Finder ancestor) => tester
      .widget<AppProgressBar>(within(ancestor, find.byType(AppProgressBar)));

  /// The Text drawn with [text] under [ancestor].
  Text textIn(WidgetTester tester, Finder ancestor, String text) =>
      tester.widget<Text>(within(ancestor, find.text(text)));

  Finder section(String title) => find.byWidgetPredicate(
    (widget) => widget is SettingsCard && widget.title == title,
  );

  /// Types a path into the Inspect page's field, then inspects it.
  Future<void> inspectPath(WidgetTester tester, String path) async {
    await tester.enterText(find.byKey(const Key('inspect-path')), path);
    await settle(tester);
    await tapKey(tester, 'inspect-run');
  }

  /// Replaces the Tidemark Photos update the core reports as running. The
  /// defaults are the mockup's values.
  void setTidemark(
    FakeCore core, {
    int progress = 62,
    int phaseIndex = 1,
    String fromVersion = '3.1.0',
    String toVersion = '3.2.0',
    int bytesDone = 43201331,
    int bytesTotal = 69206016,
  }) {
    final index = core.tasks.indexWhere((task) => task.id == 'op-tidemark');
    core.tasks[index] = fakeTask(
      id: 'op-tidemark',
      kind: TaskKindDto.update,
      state: TaskStateDto.running,
      title: 'Updating',
      target: 'Tidemark Photos',
      progress: progress,
      fromVersion: fromVersion,
      toVersion: toVersion,
      phaseIndex: phaseIndex,
      bytesDone: bytesDone,
      bytesTotal: bytesTotal,
      startedAt: unixSeconds(DateTime(2026, 10, 7, 9, 10)),
    );
  }

  /// Replaces the failed checks in the core's scan and keeps its offers.
  void setFailures(FakeCore core, List<UpdateFailureDto> failures) {
    final scan = core.scan;
    core.scan = UpdateScanDto(
      offers: scan.offers,
      failures: failures,
      skipped: scan.skipped,
      checked: scan.checked,
      cancelled: scan.cancelled,
    );
  }

  /// The core's scan with one Brisk Terminal offer, which is not running.
  UpdateScanDto briskScan({bool reducedVerification = false}) => UpdateScanDto(
    offers: [
      fakeOffer(
        uuid: 'brisk',
        name: 'Brisk Terminal',
        currentVersion: '1.2.0',
        availableVersion: '1.3.0',
        reducedVerification: reducedVerification,
      ),
    ],
    failures: const [],
    skipped: 0,
    checked: 7,
    cancelled: false,
  );

  /// Inspects a file the core reports as a second copy of Atlas Viewer, then
  /// presses Integrate, so the core's conflict answer opens the dialog.
  Future<AppModel> atlasConflict(WidgetTester tester, FakeCore core) async {
    final model = await pumpApp(tester, core: core, page: AppPage.inspect);
    core.inspectResult = fakeInspect(
      name: 'Atlas Viewer',
      version: '0.9.4',
      path: atlasPath,
    );
    core.integrateResult = fakeOutcome(
      ok: false,
      conflict: true,
      conflictUuid: 'atlas',
      conflictName: 'Atlas Viewer',
    );
    await inspectPath(tester, atlasPath);
    await tapKey(tester, 'integrate');
    return model;
  }

  /// Checks that the phase named [active] is drawn as the highlight: weight 500
  /// in the main ink. The other two phases are weight 400 in the secondary ink.
  void expectPhaseHighlight(WidgetTester tester, Finder card, String active) {
    for (final phase in ['1 Download', '2 Verify', '3 Swap in']) {
      final style = tester.widget<Text>(within(card, find.text(phase))).style!;
      if (phase == active) {
        expect(style.fontWeight, FontWeight.w500, reason: phase);
        expect(style.color, const Color(0xFF1D1C1A), reason: phase);
      } else {
        expect(style.fontWeight, FontWeight.w400, reason: phase);
        expect(style.color, const Color(0xFF5F5B54), reason: phase);
      }
    }
  }

  /// A finished row carries its label, its time and the done icon.
  void expectFinishedRow(String id, String label, String when) {
    final row = finishedRow(id);
    expect(within(row, find.text(label)), findsOneWidget);
    expect(within(row, find.text(when)), findsOneWidget);
    expect(
      within(
        row,
        find.byWidgetPredicate(
          (widget) => widget is AppIcon && widget.name == 'status-done',
        ),
      ),
      findsOneWidget,
    );
  }

  // Row 55: Remove app help text -----------------------------------------------

  testWidgets(
    'F55 Delete asks again, and no permanent removal reaches the core until that dialog is confirmed',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      model.selectApp('quill');
      await settle(tester);
      expect(
        find.text('Moves to the Trash. Permanent delete asks again.'),
        findsOneWidget,
      );

      await tapKey(tester, 'detail-delete');
      expect(model.dialog, isA<RemoveDialog>());
      expect((model.dialog! as RemoveDialog).permanent, isTrue);
      expect(core.calls.where((call) => call.startsWith('removeApp')), isEmpty);

      await tapKey(tester, 'dialog-confirm');
      expect(core.calls, contains('removeApp:quill:permanent'));
    },
  );

  // Row 56: Inspect subtitle ---------------------------------------------------

  testWidgets(
    'F56 the Inspect subtitle heads the Inspect page and leaves with it',
    (tester) async {
      final model = await pumpApp(tester, core: mockupCore());
      expect(find.text(inspectSubtitle), findsNothing);

      await tapKey(tester, 'nav-inspect');
      expect(model.page, AppPage.inspect);
      expect(find.text(inspectSubtitle), findsOneWidget);

      await tapKey(tester, 'nav-library');
      expect(model.page, AppPage.library);
      expect(find.text(inspectSubtitle), findsNothing);
    },
  );

  // Row 57: "Nothing is executed or installed" banner ----------------------------

  testWidgets(
    'F57 inspecting a file only asks the core to read it, so the banner holds',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.inspect);
      expect(
        within(
          find.byKey(const Key('inspect-banner')),
          find.text('Nothing is executed or installed'),
        ),
        findsOneWidget,
      );

      final before = core.calls.length;
      await inspectPath(tester, demoPath);
      expect(model.inspect.inspectedOk, isTrue);
      expect(core.calls.sublist(before), [
        'inspectPath:$demoPath',
        'listTasks',
      ]);
    },
  );

  // Row 58: metadata card ------------------------------------------------------

  testWidgets(
    'F58 the metadata card shows the categories and checksum the core inspected',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.inspect);
      final sha = 'ef' * 32;
      core.inspectResult = fakeInspect(
        categories: const ['Utility', 'Network'],
        sha256: sha,
      );
      await inspectPath(tester, demoPath);

      expect(model.inspected!.categories, ['Utility', 'Network']);
      expect(model.inspected!.sha256, sha);
      expect(find.text('Utility, Network'), findsOneWidget);
      expect(find.text(sha), findsOneWidget);
    },
  );

  // Row 59: "· matches" -----------------------------------------------------------

  testWidgets(
    'F59 the architecture reads · matches in green only when the core reports it supported',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.inspect);
      final matches = find.byWidgetPredicate(
        (widget) =>
            widget is RichText &&
            widget.text.toPlainText().contains('· matches'),
      );

      await inspectPath(tester, demoPath);
      expect(model.inspected!.architectureSupported, isTrue);
      expect(matches, findsOneWidget);
      // Text.rich wraps its span in one root span, so the match is two levels
      // down: the root's child is the architecture cell's span.
      final root = tester.widget<RichText>(matches).text as TextSpan;
      final cell = root.children!.single as TextSpan;
      final span = cell.children!.last as TextSpan;
      expect(span.text, ' · matches');
      expect(span.style?.color, const Color(0xFF2E7D4F));

      await tapKey(tester, 'inspect-cancel');
      core.inspectResult = fakeInspect(architectureSupported: false);
      await inspectPath(tester, demoPath);
      expect(model.inspected!.architectureSupported, isFalse);
      expect(matches, findsNothing);
    },
  );

  // Row 60: Integrate into library steps ---------------------------------------

  testWidgets(
    'F60 the Integrate card appears after an inspection and lists steps 1 to 3 in order',
    (tester) async {
      final model = await pumpApp(
        tester,
        core: mockupCore(),
        page: AppPage.inspect,
      );
      expect(find.text('Integrate into library'), findsNothing);

      await inspectPath(tester, demoPath);
      expect(model.inspect.inspectedOk, isTrue);
      expect(find.text('Integrate into library'), findsOneWidget);

      final card = find
          .ancestor(
            of: find.text('Move the original'),
            matching: find.byType(AppCard),
          )
          .first;
      for (final number in ['1', '2', '3']) {
        expect(within(card, find.text(number)), findsOneWidget);
      }
      final first = tester.getTopLeft(
        find.text('Copies the file into ~/AppImages'),
      );
      final second = tester.getTopLeft(
        find.text('Writes a menu entry and installs the icon'),
      );
      final third = tester.getTopLeft(
        find.text('On a name conflict, asks to keep both or replace'),
      );
      expect(first.dy, lessThan(second.dy));
      expect(second.dy, lessThan(third.dy));
    },
  );

  // Row 61: step 1 names the managed folder -------------------------------------

  testWidgets(
    'F61 step 1 names the managed folder the settings hold, and follows a folder change',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.inspect);
      await inspectPath(tester, demoPath);
      expect(model.managedFolderPath, '/home/someone/AppImages');
      expect(find.text('Copies the file into ~/AppImages'), findsOneWidget);

      await model.chooseManagedFolder('/home/someone/Apps');
      await settle(tester);
      expect(core.lastPatch?.managedFolder, '/home/someone/Apps');
      expect(model.managedFolderPath, '/home/someone/Apps');
      expect(find.text('Copies the file into ~/Apps'), findsOneWidget);
      expect(find.text('Copies the file into ~/AppImages'), findsNothing);
    },
  );

  // Row 64: Cancel -------------------------------------------------------------

  testWidgets(
    'F64 Cancel on an update in progress asks the core to cancel that update',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.updates);
      await tapKey(tester, 'cancel-update-tidemark');
      expect(core.calls, contains('cancelTask:op-tidemark'));
      expect(
        core.calls.where((call) => call.startsWith('applyUpdate')),
        isEmpty,
      );
    },
  );

  testWidgets(
    'F64 Cancel on a running task on the Tasks page asks the core to cancel it',
    (tester) async {
      final core = mockupCore();
      await pumpApp(tester, core: core, page: AppPage.tasks);
      await tapKey(tester, 'task-cancel-op-tidemark');
      expect(core.calls, contains('cancelTask:op-tidemark'));
    },
  );

  testWidgets(
    'F64 Cancel on an inspected file forgets it and asks the core for nothing',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.inspect);
      await inspectPath(tester, demoPath);
      expect(model.inspected, isNotNull);

      final before = List.of(core.calls);
      await tapKey(tester, 'inspect-cancel');
      expect(model.inspected, isNull);
      expect(model.inspect.pathInput, isEmpty);
      expect(model.inspect.inspectedOk, isFalse);
      expect(core.calls, before);
    },
  );

  // Row 65: Updates subtitle ---------------------------------------------------

  testWidgets(
    'F65 the Updates subtitle reads Not checked yet until a check runs, then the check time',
    (tester) async {
      final model = await pumpApp(
        tester,
        core: mockupCore(),
        page: AppPage.updates,
        checked: false,
      );
      // The status bar prints its own "Not checked yet", so look in the header.
      final subtitle = find.descendant(
        of: find.byType(PageHeader),
        matching: find.text('Not checked yet'),
      );
      expect(model.lastChecked, isNull);
      expect(subtitle, findsOneWidget);

      await tapKey(tester, 'check-now');
      expect(model.lastChecked, testNow);
      expect(
        find.descendant(
          of: find.byType(PageHeader),
          matching: find.text('Last checked today at 09:14'),
        ),
        findsOneWidget,
      );
      expect(subtitle, findsNothing);
    },
  );

  // Row 70: App is running -----------------------------------------------------

  testWidgets(
    'F70 a running app with an update reads App is running, and the label follows the library',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.updates);
      final row = updateRow('quill');
      expect(
        model.library.firstWhere((app) => app.uuid == 'quill').running,
        isTrue,
      );
      expect(within(row, find.text('App is running')), findsOneWidget);
      expect(
        within(row, find.text("You'll be asked to confirm")),
        findsOneWidget,
      );
      expect(
        textIn(tester, row, 'App is running').style?.color,
        const Color(0xFF2E7D4F),
      );

      core.library = LibraryDto(
        apps: [
          for (final app in mockupApps())
            if (app.uuid == 'quill')
              fakeApp(uuid: 'quill', name: 'Quill Notes', version: '2.4.1')
            else
              app,
        ],
        discovered: const [],
      );
      await model.loadLibrary();
      await settle(tester);
      expect(
        model.library.firstWhere((app) => app.uuid == 'quill').running,
        isFalse,
      );
      expect(within(row, find.text('App is running')), findsNothing);
      expect(within(row, find.text('Update available')), findsOneWidget);
    },
  );

  // Row 71: progress bar -------------------------------------------------------

  testWidgets(
    'F71 the progress bar on a running update shows the core percent and follows it',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.updates);
      final row = updateRow('tidemark');
      final tidemark = model.library.firstWhere(
        (app) => app.uuid == 'tidemark',
      );
      expect(model.updateTaskFor(tidemark)!.progress, 62);
      expect(progressIn(tester, row).percent, 62);
      expect(progressIn(tester, row).height, 6);
      expect(within(row, find.text('62%')), findsOneWidget);

      setTidemark(core, progress: 80);
      await model.refreshTasks();
      await settle(tester);
      expect(model.updateTaskFor(tidemark)!.progress, 80);
      expect(progressIn(tester, row).percent, 80);
      expect(within(row, find.text('80%')), findsOneWidget);
    },
  );

  // Row 72: reduced verification notice ------------------------------------------

  testWidgets(
    'F72 the reduced verification notice follows the offer, in the warning colour',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.updates);
      expect(model.offerFor('tidemark')!.reducedVerification, isTrue);
      final notice = within(updateRow('tidemark'), find.text(reducedNotice));
      expect(notice, findsOneWidget);
      expect(tester.widget<Text>(notice).style?.color, const Color(0xFF7A5200));

      expect(model.offerFor('quill')!.reducedVerification, isFalse);
      expect(
        within(updateRow('quill'), find.text(reducedNotice)),
        findsNothing,
      );

      // Brisk Terminal's offer comes from a source with no checksum, then one
      // with a checksum: the notice follows the offer both ways.
      core.scan = briskScan(reducedVerification: true);
      await tapKey(tester, 'check-now');
      expect(model.offerFor('brisk')!.reducedVerification, isTrue);
      expect(
        within(updateRow('brisk'), find.text(reducedNotice)),
        findsOneWidget,
      );

      core.scan = briskScan();
      await tapKey(tester, 'check-now');
      expect(model.offerFor('brisk')!.reducedVerification, isFalse);
      expect(
        within(updateRow('brisk'), find.text(reducedNotice)),
        findsNothing,
      );
    },
  );

  // Row 74: timed-out status ---------------------------------------------------

  testWidgets(
    'F74 a timed-out check reads the mockup sentence and a refusal reads the core text',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.updates);
      final row = updateRow('orbit');
      final failure = model.failureFor('orbit')!;
      expect(failure.timedOut, isTrue);
      expect(within(row, find.text('Check failed')), findsOneWidget);
      expect(within(row, find.text(timedOutSentence)), findsOneWidget);
      expect(within(row, find.text(failure.error)), findsNothing);
      // The status line is red and the detail line is the secondary ink.
      expect(
        textIn(tester, row, 'Check failed').style?.color,
        const Color(0xFFA8322D),
      );
      expect(
        textIn(tester, row, timedOutSentence).style?.color,
        const Color(0xFF5F5B54),
      );

      setFailures(core, [
        fakeFailure(
          uuid: 'orbit',
          name: 'Orbit Mail',
          error: 'Server refused the request (HTTP 403)',
          timedOut: false,
        ),
      ]);
      await tapKey(tester, 'check-now');
      expect(model.failureFor('orbit')!.timedOut, isFalse);
      expect(
        within(row, find.text('Server refused the request (HTTP 403)')),
        findsOneWidget,
      );
      expect(within(row, find.text(timedOutSentence)), findsNothing);
    },
  );

  testWidgets(
    'F74 an update that timed out and did not apply reads the mockup sentence in the list',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.updates);
      core.batch = BatchDto(
        applied: const [],
        failed: [
          fakeFailure(
            uuid: 'tidemark',
            name: 'Tidemark Photos',
            error: 'Download stalled',
            timedOut: true,
          ),
        ],
        skippedRunning: const [],
        checkFailures: const [],
        cancelled: false,
      );
      await tapKey(tester, 'update-all');
      expect(model.updateFailures.single.timedOut, isTrue);
      expect(find.text('Tidemark Photos: $timedOutSentence'), findsOneWidget);
      expect(find.textContaining('Download stalled'), findsNothing);
    },
  );

  // Row 75: Tasks subtitle -----------------------------------------------------

  testWidgets(
    'F75 the Tasks subtitle counts running and finished tasks from the core',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.tasks);
      expect(model.runningTasks, hasLength(1));
      expect(model.finishedTasks, hasLength(3));
      expect(find.text('1 running, 3 finished'), findsOneWidget);

      core.tasks.add(
        fakeTask(
          id: 'op-brisk-2',
          kind: TaskKindDto.update,
          state: TaskStateDto.running,
          title: 'Updating',
          target: 'Brisk Terminal',
          progress: 10,
          fromVersion: '1.2.0',
          toVersion: '1.3.0',
        ),
      );
      await model.refreshTasks();
      await settle(tester);
      expect(model.runningTasks, hasLength(2));
      expect(find.text('2 running, 3 finished'), findsOneWidget);
    },
  );

  // Row 77: running card -------------------------------------------------------

  testWidgets(
    'F77 the running card names the update and its versions from the core',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.tasks);
      final card = runningCard('op-tidemark');
      expect(model.runningTasks.single.fromVersion, '3.1.0');
      expect(
        within(card, find.text('Updating Tidemark Photos')),
        findsOneWidget,
      );
      expect(within(card, find.text('3.1.0 → 3.2.0')), findsOneWidget);

      setTidemark(core, toVersion: '3.3.0');
      await model.refreshTasks();
      await settle(tester);
      expect(within(card, find.text('3.1.0 → 3.3.0')), findsOneWidget);
      expect(within(card, find.text('3.1.0 → 3.2.0')), findsNothing);

      // A core that did not name both versions shows the state instead.
      setTidemark(core, toVersion: '');
      await model.refreshTasks();
      await settle(tester);
      expect(model.runningTasks.single.toVersion, isEmpty);
      expect(within(card, find.text('running')), findsOneWidget);
    },
  );

  // Row 78: phase highlight ----------------------------------------------------

  testWidgets('F78 the phase highlight follows the core phase index', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.tasks);
    final card = runningCard('op-tidemark');
    expect(model.runningTasks.single.phaseIndex, 1);
    expectPhaseHighlight(tester, card, '1 Download');

    setTidemark(core, phaseIndex: 2);
    await model.refreshTasks();
    await settle(tester);
    expect(model.runningTasks.single.phaseIndex, 2);
    expectPhaseHighlight(tester, card, '2 Verify');

    setTidemark(core, phaseIndex: 3);
    await model.refreshTasks();
    await settle(tester);
    expectPhaseHighlight(tester, card, '3 Swap in');
  });

  // Row 79: byte progress ------------------------------------------------------

  testWidgets(
    'F79 the byte line follows the core byte counts, and hides without a total',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.tasks);
      final card = runningCard('op-tidemark');
      expect(within(card, find.text('41.2 of 66.0 MB')), findsOneWidget);

      setTidemark(core, bytesDone: 52428800);
      await model.refreshTasks();
      await settle(tester);
      expect(model.runningTasks.single.bytesDone, 52428800);
      expect(within(card, find.text('50.0 of 66.0 MB')), findsOneWidget);
      expect(within(card, find.text('41.2 of 66.0 MB')), findsNothing);

      setTidemark(core, bytesTotal: 0);
      await model.refreshTasks();
      await settle(tester);
      expect(within(card, find.textContaining('MB')), findsNothing);
    },
  );

  // Row 80: finished list ------------------------------------------------------

  testWidgets(
    'F80 the finished list labels each task and times it from the core, newest first',
    (tester) async {
      final model = await pumpApp(
        tester,
        core: mockupCore(),
        page: AppPage.tasks,
      );
      expect(model.finishedTasks.map((task) => task.id), [
        'op-cinder',
        'op-brisk',
        'op-remove',
      ]);
      expectFinishedRow(
        'op-cinder',
        'Integrated Cinder Chat 0.14.2',
        'Today 08:52',
      );
      expectFinishedRow(
        'op-brisk',
        'Updated Brisk Terminal 1.1.4 → 1.2.0',
        'Today 08:31',
      );
      expectFinishedRow(
        'op-remove',
        'Moved Old Notes 1.0 to the Trash',
        'Yesterday 17:40',
      );
    },
  );

  testWidgets(
    'F80 the finish time is the one the core recorded, and a task without one shows its state word',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.tasks);
      final index = core.tasks.indexWhere((task) => task.id == 'op-remove');
      void replaceRemove(int finishedAt) {
        core.tasks[index] = fakeTask(
          id: 'op-remove',
          kind: TaskKindDto.remove,
          state: TaskStateDto.succeeded,
          title: 'Removing',
          target: 'Old Notes',
          fromVersion: '1.0',
          finishedAt: finishedAt,
        );
      }

      replaceRemove(unixSeconds(DateTime(2026, 10, 7, 7, 5)));
      await model.refreshTasks();
      await settle(tester);
      expect(
        model.finishedAt(model.finishedTasks.last),
        DateTime(2026, 10, 7, 7, 5),
      );
      expect(
        within(finishedRow('op-remove'), find.text('Today 07:05')),
        findsOneWidget,
      );

      replaceRemove(0);
      await model.refreshTasks();
      await settle(tester);
      expect(model.finishedAt(model.finishedTasks.last), isNull);
      expect(
        within(finishedRow('op-remove'), find.text('succeeded')),
        findsOneWidget,
      );
    },
  );

  // A cancelled task reads cancelled. It is not a failure.
  group('F80 cancelled tasks', () {
    testWidgets('F80 a cancelled task reads cancelled, not failed', (
      tester,
    ) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.tasks);
      core.tasks.add(
        fakeTask(
          id: 'op-cancelled',
          kind: TaskKindDto.update,
          state: TaskStateDto.cancelled,
          title: 'Updating',
          target: 'Brisk Terminal',
          finishedAt: unixSeconds(DateTime(2026, 10, 7, 9, 5)),
        ),
      );
      await model.refreshTasks();
      await settle(tester);
      expect(model.finishedTasks.first.state, TaskStateDto.cancelled);
      expect(
        within(finishedRow('op-cancelled'), find.textContaining('failed')),
        findsNothing,
      );
      expect(
        within(
          finishedRow('op-cancelled'),
          find.text('Updating Brisk Terminal cancelled'),
        ),
        findsOneWidget,
      );
    });
  });

  // Row 81: Settings subtitle ------------------------------------------------

  testWidgets(
    'F81 with only background checks on, every other option reads off',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.settings);
      expect(find.text('Options are off unless noted'), findsOneWidget);
      expect(toggleOf(tester, 'toggle-background').value, isFalse);

      await tapKey(tester, 'toggle-background');
      expect(model.settings!.backgroundUpdateChecks, isTrue);
      expect(toggleOf(tester, 'toggle-background').value, isTrue);
      for (final key in [
        'toggle-move',
        'toggle-discover',
        'toggle-terminal',
        'toggle-login',
        'toggle-debug',
        'toggle-unsafe',
      ]) {
        expect(toggleOf(tester, key).value, isFalse, reason: key);
      }
    },
  );

  // Row 92: Settings section headings ------------------------------------------

  testWidgets(
    'F92 each Settings heading holds the controls it names, and a change there saves to the core',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.settings);
      for (final title in [
        'Appearance',
        'Integration',
        'Updates',
        'Advanced',
      ]) {
        expect(section(title), findsOneWidget, reason: title);
      }

      expect(
        within(section('Integration'), find.byKey(const Key('toggle-move'))),
        findsOneWidget,
      );
      await tapKey(tester, 'toggle-move');
      expect(core.lastPatch?.moveSource, isTrue);

      expect(
        within(section('Updates'), find.byKey(const Key('toggle-background'))),
        findsOneWidget,
      );
      await tapKey(tester, 'toggle-background');
      expect(core.lastPatch?.backgroundUpdateChecks, isTrue);

      expect(
        within(section('Advanced'), find.byKey(const Key('toggle-debug'))),
        findsOneWidget,
      );
      await tapKey(tester, 'toggle-debug');
      expect(core.lastPatch?.debugLogging, isTrue);

      expect(within(section('Appearance'), find.text('Dark')), findsOneWidget);
      await tapFinder(tester, within(section('Appearance'), find.text('Dark')));
      expect(core.lastPatch?.appearance, AppearanceChoice.dark);
      expect(model.settings!.appearance, AppearanceChoice.dark);
    },
  );

  // Row 93: Remove dialog ------------------------------------------------------

  testWidgets(
    'F93 Move to Trash opens the trash confirmation for that app, and Cancel closes it with no core call',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      model.selectApp('quill');
      await settle(tester);

      await tapKey(tester, 'detail-trash');
      expect(model.dialog, isA<RemoveDialog>());
      final dialog = model.dialog! as RemoveDialog;
      expect(dialog.name, 'Quill Notes');
      expect(dialog.permanent, isFalse);
      expect(find.text('Move Quill Notes to the Trash?'), findsOneWidget);
      expect(tester.getSize(find.byType(AppDialogCard)).width, 460);
      expect(
        tester.getCenter(find.byKey(const Key('dialog-cancel'))).dx,
        lessThan(tester.getCenter(find.byKey(const Key('dialog-confirm'))).dx),
      );
      expect(
        buttonOf(tester, 'dialog-confirm').variant,
        AppButtonVariant.primary,
      );

      final before = List.of(core.calls);
      await tapKey(tester, 'dialog-cancel');
      expect(model.dialog, isNull);
      expect(model.selectedUuid, 'quill');
      expect(core.calls, before);
    },
  );

  testWidgets(
    'F93 confirming the trash dialog sends a trash removal for that app to the core',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core);
      model.selectApp('quill');
      await settle(tester);

      await tapKey(tester, 'detail-trash');
      await tapKey(tester, 'dialog-confirm');
      expect(core.calls, contains('removeApp:quill:trash'));
      expect(model.dialog, isNull);
      expect(model.selectedUuid, isNull);
    },
  );

  // Row 94: Name conflict dialog -----------------------------------------------

  testWidgets(
    'F94 a name conflict names the installed app, and Cancel closes it with no core call',
    (tester) async {
      final core = mockupCore();
      final model = await atlasConflict(tester, core);
      expect(model.dialog, isA<IntegrateConflictDialog>());
      expect((model.dialog! as IntegrateConflictDialog).replaceUuid, 'atlas');
      expect(find.text('Atlas Viewer is already integrated'), findsOneWidget);
      expect(
        find.textContaining('The installed copy is 0.9.3.'),
        findsOneWidget,
      );
      expect(
        find.textContaining('The file you are integrating is 0.9.4.'),
        findsOneWidget,
      );
      expect(tester.getSize(find.byType(AppDialogCard)).width, 460);

      final cancelX = tester
          .getCenter(find.byKey(const Key('dialog-cancel')))
          .dx;
      final keepX = tester
          .getCenter(find.byKey(const Key('dialog-keep-both')))
          .dx;
      final replaceX = tester
          .getCenter(find.byKey(const Key('dialog-replace')))
          .dx;
      expect(cancelX, lessThan(keepX));
      expect(keepX, lessThan(replaceX));
      expect(
        buttonOf(tester, 'dialog-replace').variant,
        AppButtonVariant.primary,
      );

      final before = List.of(core.calls);
      await tapKey(tester, 'dialog-cancel');
      expect(model.dialog, isNull);
      expect(core.calls, before);
      expect(model.inspect.inspectedOk, isTrue);
    },
  );

  testWidgets(
    'F94 Keep both integrates the new copy beside the installed one',
    (tester) async {
      final core = mockupCore();
      final model = await atlasConflict(tester, core);
      core.integrateResult = fakeOutcome(
        app: fakeApp(uuid: 'atlas-2', name: 'Atlas Viewer', version: '0.9.4'),
      );
      await tapKey(tester, 'dialog-keep-both');
      expect(core.calls, contains('integrateApp:$atlasPath:keepBoth'));
      expect(model.dialog, isNull);
      expect(model.inspect.inspectedOk, isFalse);
    },
  );

  testWidgets('F94 Replace integrates over the installed copy the core named', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await atlasConflict(tester, core);
    core.integrateResult = fakeOutcome(
      app: fakeApp(uuid: 'atlas', name: 'Atlas Viewer', version: '0.9.4'),
    );
    await tapKey(tester, 'dialog-replace');
    expect(core.calls, contains('integrateApp:$atlasPath:replace'));
    expect(core.lastReplaceUuid, 'atlas');
    expect(model.dialog, isNull);
  });

  // Row 95: Running app dialog -------------------------------------------------

  testWidgets(
    'F95 updating a running app asks first, and Cancel closes it with no core call',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.updates);
      final before = List.of(core.calls);

      await tapKey(tester, 'update-quill');
      expect(model.dialog, isA<UpdateForceDialog>());
      expect((model.dialog! as UpdateForceDialog).name, 'Quill Notes');
      expect(find.text('Quill Notes is running'), findsOneWidget);
      expect(tester.getSize(find.byType(AppDialogCard)).width, 460);
      expect(
        tester.getCenter(find.byKey(const Key('dialog-cancel'))).dx,
        lessThan(tester.getCenter(find.byKey(const Key('dialog-confirm'))).dx),
      );
      expect(
        buttonOf(tester, 'dialog-confirm').variant,
        AppButtonVariant.destructive,
      );
      expect(core.calls, before);

      await tapKey(tester, 'dialog-cancel');
      expect(model.dialog, isNull);
      expect(core.calls, before);
    },
  );

  testWidgets(
    'F95 Update anyway applies the running app\'s update with force',
    (tester) async {
      final core = mockupCore();
      final model = await pumpApp(tester, core: core, page: AppPage.updates);
      await tapKey(tester, 'update-quill');
      await tapKey(tester, 'dialog-confirm');
      expect(core.calls, contains('applyUpdate:quill:force'));
      expect(model.dialog, isNull);
    },
  );

  testWidgets('F95 a stopped app with an offer updates without asking', (
    tester,
  ) async {
    final core = mockupCore();
    final model = await pumpApp(tester, core: core, page: AppPage.updates);
    core.scan = briskScan();
    await tapKey(tester, 'check-now');
    expect(model.offerFor('brisk')!.running, isFalse);

    await tapKey(tester, 'update-brisk');
    expect(model.dialog, isNull);
    expect(core.calls, contains('applyUpdate:brisk:normal'));
  });
}
