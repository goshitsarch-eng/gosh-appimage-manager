import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';

import 'support/fakes.dart';
import 'support/harness.dart';

/// The refusal the core gives for a repo-only GitHub pair (QA1-011 / D-10): the
/// text names the exact CLI command, and the GUI must show all of it.
const String _githubRefusal =
    'GitHub needs filename=<release asset name, * and ? allowed>, for example '
    'filename=<app>-x86_64.AppImage. Run: gosh-appimage-manager '
    '--set-update-source /home/someone/AppImages/Quill-Notes-x86_64.AppImage '
    '--manager github repo=example-org/quill-notes filename=<asset name>';

/// The round 7 defects. Each test names its defect and acts as a user does.
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

  Future<void> settleBriefly(WidgetTester tester) async {
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
  }

  /// Whether [rect] lies inside a window of [size], edges included.
  bool insideWindow(Rect rect, Size size) =>
      rect.left >= 0 &&
      rect.top >= 0 &&
      rect.right <= size.width &&
      rect.bottom <= size.height;

  // QA3-008 -----------------------------------------------------------------

  testWidgets(
    'QA3-008 at 800 by 600 the Move to Trash and Delete buttons are inside the window, with no overflow',
    (tester) async {
      const size = Size(800, 600);
      final model = await pumpApp(tester, core: mockupCore(), size: size);
      model.selectApp('quill');
      await settleBriefly(tester);
      await tester.ensureVisible(find.byKey(const Key('detail-trash')));
      await settleBriefly(tester);
      expect(
        tester.takeException(),
        isNull,
        reason: 'the Detail page overflows at 800 by 600',
      );
      for (final key in ['detail-trash', 'detail-delete']) {
        final rect = tester.getRect(find.byKey(Key(key)));
        expect(
          insideWindow(rect, size),
          isTrue,
          reason: '$key is outside the 800 by 600 window at $rect',
        );
      }
    },
  );

  // QA3-007 -----------------------------------------------------------------

  testWidgets(
    'QA3-007 at 800 by 600 a running app with no version reads "no version" in full',
    (tester) async {
      final apps = [
        for (final app in mockupApps())
          if (app.uuid == 'brisk')
            fakeApp(
              uuid: 'brisk',
              name: 'Brisk Terminal',
              version: '',
              running: true,
              managedPath:
                  '/home/someone/AppImages/Brisk-Terminal-x86_64.AppImage',
            )
          else
            app,
      ];
      await pumpApp(
        tester,
        core: FakeCore(
          library: LibraryDto(apps: apps, discovered: const []),
        ),
        size: const Size(800, 600),
      );
      final row = find.byKey(const Key('row-brisk'));
      expect(row, findsOneWidget);
      final version = find.descendant(
        of: row,
        matching: find.text('no version'),
      );
      expect(version, findsOneWidget);
      final paragraph = tester.renderObject<RenderParagraph>(version);
      expect(
        paragraph.didExceedMaxLines,
        isFalse,
        reason: '"no version" wraps past its line',
      );
      expect(
        paragraph.getMaxIntrinsicWidth(double.infinity),
        lessThanOrEqualTo(paragraph.size.width + 0.5),
        reason:
            '"no version" is cut to ${paragraph.size.width.toStringAsFixed(1)} px',
      );
      expect(
        find.descendant(of: row, matching: find.text('Running')),
        findsOneWidget,
        reason: 'the Running badge is missing',
      );
    },
  );

  // QA1-011 / D-10 ----------------------------------------------------------

  for (final size in const [Size(1280, 800), Size(800, 600), Size(360, 640)]) {
    testWidgets(
      'QA1-011 at ${size.width.toInt()} by ${size.height.toInt()} the GitHub refusal shows the Run command in full, inline and on the status line',
      (tester) async {
        final core = mockupCore();
        core.failures['setUpdateSource'] = coreError(
          _githubRefusal,
          kind: ErrorKind.validation,
        );
        final model = await pumpApp(
          tester,
          core: core,
          size: size,
          page: AppPage.library,
        );
        model.selectApp('quill');
        await settleBriefly(tester);
        await tester.enterText(
          find.byKey(const Key('source-config-field')),
          'repo=example-org/quill-notes',
        );
        await tester.testTextInput.receiveAction(TextInputAction.done);
        await settleBriefly(tester);
        expect(
          model.sourceError,
          _githubRefusal,
          reason: 'the model cut the text',
        );
        const command = 'Run: gosh-appimage-manager --set-update-source';
        for (final key in ['source-error', 'status-line']) {
          final finder = find.byKey(Key(key));
          expect(finder, findsOneWidget, reason: '$key is not on screen');
          // The inline error sits at the foot of a scrolling page: bring it into view.
          await tester.ensureVisible(finder);
          await settleBriefly(tester);
          final shown = tester.widget<Text>(finder).data ?? '';
          expect(shown, contains(command), reason: '$key lacks the command');
          final paragraph = tester.renderObject<RenderParagraph>(finder);
          expect(
            paragraph.overflow,
            isNot(TextOverflow.ellipsis),
            reason: '$key ends in an ellipsis',
          );
          expect(
            paragraph.didExceedMaxLines,
            isFalse,
            reason: '$key is cut short',
          );
          expect(
            insideWindow(
              tester.getRect(finder),
              tester.view.physicalSize / tester.view.devicePixelRatio,
            ),
            isTrue,
            reason: '$key is not inside the window',
          );
        }
        expect(
          find.textContaining(command),
          findsNWidgets(2),
          reason: 'the command is not on screen in both places',
        );
      },
    );
  }
}
