import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/app.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';

import 'support/fakes.dart';

/// The window the original opens at, at its 2x scale: 1024 by 768 logical.
const Size _window = Size(1024, 768);

/// A library that looks like the one the original was captured with: three
/// installed apps and one available update.
FakeCore _populatedCore() {
  final apps = [
    fakeApp(
      uuid: 'a',
      name: 'Alpha-1.2.0-aarch64',
      managedPath: '/home/someone/AppImages/Alpha-1.2.0-aarch64.AppImage',
    ),
    fakeApp(
      uuid: 'b',
      name: 'Beta-3.0-aarch64',
      managedPath: '/home/someone/AppImages/Beta-3.0-aarch64.AppImage',
      running: true,
    ),
    fakeApp(
      uuid: 'c',
      name: 'Gamma-0.9.1-aarch64',
      managedPath: '/home/someone/AppImages/Gamma-0.9.1-aarch64.AppImage',
      adopted: true,
    ),
  ];
  return FakeCore(
      library: LibraryDto(apps: apps, discovered: const []),
    )
    ..scan = UpdateScanDto(
      offers: [fakeOffer(uuid: 'b', name: 'Beta-3.0-aarch64')],
      failures: const [],
      skipped: 0,
      checked: 3,
      cancelled: false,
    )
    ..tasks.addAll([
      TaskDto(
        id: 'op-1',
        kind: TaskKindDto.checkUpdate,
        state: TaskStateDto.succeeded,
        title: 'Checking for updates',
        target: '',
        progress: 100,
        statusText: '',
        error: '',
        retryable: false,
      ),
      TaskDto(
        id: 'op-2',
        kind: TaskKindDto.update,
        state: TaskStateDto.running,
        title: 'Updating',
        target: 'Beta-3.0-aarch64',
        progress: 40,
        statusText: '',
        error: '',
        retryable: false,
      ),
    ]);
}

Future<AppModel> _pump(
  WidgetTester tester, {
  required FakeCore core,
  Size size = _window,
  AppPage page = AppPage.library,
}) async {
  tester.view.devicePixelRatio = 2;
  tester.view.physicalSize = size * 2;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  final model = AppModel(core: core, pollInterval: null);
  await model.start(const []);
  await model.checkUpdates();
  model.setPage(page);
  await tester.pumpWidget(GoshApp(model: model));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
  return model;
}

void main() {
  // The bundled Fira Sans, so captures use the interface font the original does.
  setUpAll(() async {
    final loader = FontLoader(CosmicType.family);
    for (final file in [
      'assets/fonts/FiraSans-Regular.ttf',
      'assets/fonts/FiraSans-Medium.ttf',
      'assets/fonts/FiraSans-SemiBold.ttf',
      'assets/fonts/FiraSans-Bold.ttf',
    ]) {
      loader.addFont(rootBundle.load(file));
    }
    await loader.load();
  });

  for (final page in AppPage.values) {
    testWidgets('the ${page.name} page lays out at the original window size', (
      tester,
    ) async {
      await _pump(tester, core: _populatedCore(), page: page);
      await expectLater(
        find.byType(MaterialApp),
        matchesGoldenFile('goldens/${page.name}.png'),
      );
    });
  }

  testWidgets('the library lays out when the window is condensed', (
    tester,
  ) async {
    await _pump(tester, core: _populatedCore(), size: const Size(600, 700));
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/library-condensed.png'),
    );
  });

  testWidgets('the details page lays out with an app selected', (tester) async {
    final model = await _pump(tester, core: _populatedCore());
    model.selectApp('a');
    await tester.pump();
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/details.png'),
    );
  });

  testWidgets('the inspect page lays out with a file inspected', (
    tester,
  ) async {
    final model = await _pump(
      tester,
      core: _populatedCore(),
      page: AppPage.inspect,
    );
    await model.startInspect(
      '/home/someone/Downloads/Demo-2.0-x86_64.AppImage',
    );
    await tester.pump();
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/inspect-result.png'),
    );
  });

  testWidgets('the removal dialog sits over the library', (tester) async {
    final core = _populatedCore();
    final model = await _pump(tester, core: core);
    model.askRemove('a', permanent: true);
    await tester.pump();
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile('goldens/remove-dialog.png'),
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(model.dialog, isNull);
  });
}
