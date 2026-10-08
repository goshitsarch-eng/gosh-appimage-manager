import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/app.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/dialogs.dart';

import 'support/fakes.dart';

/// The mockup frames, one golden each (SPEC section 2). Goldens are compared
/// at 1x: the desktop frame is 1280 x 800 (the content box of the mockup's
/// 1282 x 802 window), the narrow frame 360 x 760, and the dialogs are their
/// 460 px cards.
final DateTime _now = DateTime(2026, 10, 7, 9, 14);

const Size _desktop = Size(1280, 800);
const Size _narrow = Size(360, 760);

Future<void> _loadFonts() async {
  final onest = FontLoader(AppType.sansFamily);
  for (final file in [
    'assets/fonts/Onest-Regular.ttf',
    'assets/fonts/Onest-Medium.ttf',
    'assets/fonts/Onest-SemiBold.ttf',
    'assets/fonts/Onest-Bold.ttf',
  ]) {
    onest.addFont(rootBundle.load(file));
  }
  await onest.load();
  final mono = FontLoader(AppType.monoFamily);
  for (final file in [
    'assets/fonts/IBMPlexMono-Regular.ttf',
    'assets/fonts/IBMPlexMono-Medium.ttf',
    'assets/fonts/IBMPlexMono-SemiBold.ttf',
  ]) {
    mono.addFont(rootBundle.load(file));
  }
  await mono.load();
}

/// Pumps the app in a window of [size] with the mockup's sample library.
///
/// Background checks are off, the real default, unless [backgroundOn] says
/// otherwise. Only the Settings golden turns them on, to draw the mockup's
/// frame 07, and its name says so.
Future<AppModel> _pump(
  WidgetTester tester, {
  required FakeCore core,
  Size size = _desktop,
  AppPage page = AppPage.library,
  AppearanceChoice appearance = AppearanceChoice.light,
  bool backgroundOn = false,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  core.settings = fakeSettings(
    managedFolder: '/home/someone/AppImages',
    appearance: appearance,
    backgroundUpdateChecks: backgroundOn,
  );
  final model = AppModel(core: core, pollInterval: null, clock: () => _now);
  await model.start(const []);
  await model.checkUpdates();
  // The mockup shows no status line; the check's summary is not part of a frame.
  model.dismissStatus();
  model.setPage(page);
  await tester.pumpWidget(GoshApp(model: model));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
  return model;
}

void main() {
  setUpAll(() async {
    await _loadFonts();
    homeFolderProvider = () => '/home/someone';
  });

  tearDownAll(() {
    homeFolderProvider = () => Platform.environment['HOME'];
  });

  testWidgets('01 Library (light)', (tester) async {
    await _pump(tester, core: mockupCore());
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/01-library.png'),
    );
  });

  testWidgets('02 Empty library', (tester) async {
    await _pump(tester, core: FakeCore());
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/02-empty-library.png'),
    );
  });

  testWidgets('03 Detail (light)', (tester) async {
    final model = await _pump(tester, core: mockupCore());
    model.selectApp('quill');
    await tester.pump();
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/03-detail.png'),
    );
  });

  testWidgets('04 Inspect', (tester) async {
    final core = mockupCore()
      ..inspectResult = fakeInspect(
        name: 'Cinder Chat',
        version: '0.14.2',
        path: '/home/someone/Downloads/Cinder-Chat-0.14.2-x86_64.AppImage',
        sizeBytes: (61.8 * 1024 * 1024).round(),
        sha256:
            '9b1e04d7a6c3f258e0b9d41a7c2f6e83d05a9c17b4e2f80d63a1c7e5b9204f18',
        categories: const ['Network', 'Chat'],
        iconName: 'cinder-chat',
        embeddedUpdate: 'Embedded .upd_info · GitHub · example-org/cinder-chat',
      );
    final model = await _pump(tester, core: core, page: AppPage.inspect);
    await model.startInspect(
      '/home/someone/Downloads/Cinder-Chat-0.14.2-x86_64.AppImage',
    );
    await tester.pump();
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/04-inspect.png'),
    );
  });

  testWidgets('05 Updates', (tester) async {
    await _pump(tester, core: mockupCore(), page: AppPage.updates);
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/05-updates.png'),
    );
  });

  testWidgets('06 Tasks', (tester) async {
    await _pump(tester, core: mockupCore(), page: AppPage.tasks);
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/06-tasks.png'),
    );
  });

  // The mockup's frame 07 has "Check in the background" on. The golden forces
  // that setting on, so its name says so. The real default is off (features_test
  // F88 covers it).
  testWidgets('07 Settings (background checks forced on)', (tester) async {
    await _pump(
      tester,
      core: mockupCore(),
      page: AppPage.settings,
      appearance: AppearanceChoice.system,
      backgroundOn: true,
    );
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/07-settings-background-forced-on.png'),
    );
  });

  testWidgets('08 Library (dark)', (tester) async {
    await _pump(tester, core: mockupCore(), appearance: AppearanceChoice.dark);
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/08-library-dark.png'),
    );
  });

  testWidgets('09 Detail (dark)', (tester) async {
    final model = await _pump(
      tester,
      core: mockupCore(),
      appearance: AppearanceChoice.dark,
    );
    model.selectApp('quill');
    await tester.pump();
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/09-detail-dark.png'),
    );
  });

  testWidgets('10 Library (narrow, 360 px)', (tester) async {
    await _pump(tester, core: mockupCore(), size: _narrow);
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/10-library-narrow.png'),
    );
  });

  testWidgets('11 Remove dialog', (tester) async {
    final model = await _pump(tester, core: mockupCore());
    model.askRemove('quill', permanent: false);
    await tester.pump();
    await expectLater(
      find.byType(AppDialogCard),
      matchesGoldenFile('goldens/11-dialog-remove.png'),
    );
  });

  testWidgets('12 Name conflict dialog', (tester) async {
    final model = await _pump(tester, core: mockupCore());
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
    await expectLater(
      find.byType(AppDialogCard),
      matchesGoldenFile('goldens/12-dialog-conflict.png'),
    );
  });

  testWidgets('13 Running app dialog', (tester) async {
    final model = await _pump(tester, core: mockupCore());
    model.showDialog(
      const UpdateForceDialog(uuid: 'quill', name: 'Quill Notes'),
    );
    await tester.pump();
    await expectLater(
      find.byType(AppDialogCard),
      matchesGoldenFile('goldens/13-dialog-running.png'),
    );
  });

  testWidgets('About (no mockup frame; regression only)', (tester) async {
    await _pump(tester, core: mockupCore(), page: AppPage.about);
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/about.png'),
    );
  });
}
