import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/app.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';

import 'fakes.dart';

/// The clock every widget test reads. The mockup's frames show 09:14.
final DateTime testNow = DateTime(2026, 10, 7, 9, 14);

const Size desktopSize = Size(1280, 800);
const Size narrowSize = Size(360, 760);

/// Loads the bundled Onest and IBM Plex Mono faces, so text measures as it
/// does in the app.
Future<void> loadAppFonts() async {
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

/// Pumps the whole app over [core] at [size], on [page], with the mockup's
/// library state already loaded (the same state the goldens use).
Future<AppModel> pumpApp(
  WidgetTester tester, {
  required FakeCore core,
  Size size = desktopSize,
  AppPage page = AppPage.library,
  AppearanceChoice appearance = AppearanceChoice.light,
  bool checked = true,
  SettingsDto? settings,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  core.settings = settings ?? fakeSettings(appearance: appearance);
  final model = AppModel(core: core, pollInterval: null, clock: () => testNow);
  await model.start(const []);
  if (checked) {
    await model.checkUpdates();
    model.dismissStatus();
  }
  model.setPage(page);
  await tester.pumpWidget(GoshApp(model: model));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
  return model;
}
