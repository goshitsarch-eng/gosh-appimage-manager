import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/app.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/platform/window_channel.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/ui/shell.dart';

import 'support/fakes.dart';

/// Builds the whole application over a fake core, at the given window size.
Future<AppModel> pumpApp(
  WidgetTester tester, {
  FakeCore? core,
  Size size = const Size(1024, 768),
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  final model = AppModel(core: core ?? FakeCore(), pollInterval: null);
  await model.start(const []);
  await tester.pumpWidget(GoshApp(model: model));
  await tester.pump();
  return model;
}

Finder inRail(String label) =>
    find.descendant(of: find.byType(NavRail), matching: find.text(label));

void main() {
  testWidgets('the window opens on an empty Library with its rail', (
    tester,
  ) async {
    await pumpApp(tester);

    expect(inRail('Library'), findsOneWidget);
    expect(inRail('About'), findsOneWidget);
    expect(find.text('No AppImages yet'), findsOneWidget);
    expect(
      find.text(
        'Open one from the Inspect page. Opening a file never integrates or '
        'executes it.',
      ),
      findsOneWidget,
    );
  });

  testWidgets('choosing Settings shows the original sections', (tester) async {
    await pumpApp(tester);

    await tester.tap(inRail('Settings'));
    await tester.pump();

    expect(find.text('Appearance'), findsOneWidget);
    expect(find.text('Integration folder'), findsOneWidget);
    expect(find.text('Behaviour'), findsOneWidget);
    expect(find.text('Update checks'), findsOneWidget);
    expect(find.text('Unsafe extraction fallback'), findsOneWidget);
  });

  testWidgets('a library row offers Launch, Details and Trash', (tester) async {
    final core = FakeCore(
      library: LibraryDto(apps: [fakeApp()], discovered: const []),
    );
    await pumpApp(tester, core: core);

    expect(find.text('Demo'), findsOneWidget);
    expect(find.text('Launch'), findsOneWidget);
    expect(find.text('Details'), findsOneWidget);
    expect(find.text('Trash'), findsOneWidget);
  });

  testWidgets('a pending dialog is dismissed by Escape', (tester) async {
    final core = FakeCore(
      library: LibraryDto(apps: [fakeApp()], discovered: const []),
    );
    final model = await pumpApp(tester, core: core);

    model.askRemove('u1', permanent: false);
    await tester.pump();
    expect(find.text('Move Demo to Trash?'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();

    expect(find.text('Move Demo to Trash?'), findsNothing);
    expect(model.dialog, isNull);
  });

  testWidgets('a running operation is named in the status line with Cancel', (
    tester,
  ) async {
    final core = FakeCore();
    final model = await pumpApp(tester, core: core);

    model.busy = const BusyState('op-1', 'Checking for updates');
    model.notifyListeners();
    await tester.pump();

    expect(find.text('Working: Checking for updates…'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pump();

    expect(core.calls, contains('cancelTask:op-1'));
    expect(model.status?.text, 'Cancelling…');
  });

  testWidgets(
    'a condensed window keeps the rail as a drawer the header opens',
    (tester) async {
      await pumpApp(tester, size: const Size(600, 700));

      expect(find.byType(NavRail), findsNothing);
      // The runner's header button calls back into Dart with toggleNav.
      WindowChannel.onToggleNav?.call();
      await tester.pump();
      expect(find.byType(NavRail), findsOneWidget);
    },
  );
}
