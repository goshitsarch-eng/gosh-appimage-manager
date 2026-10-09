import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/ui/detail_page.dart';
import 'package:gosh_appimage_flutter/ui/inspect_page.dart';

import 'support/fakes.dart';
import 'support/harness.dart';

/// App icons that are not PNGs, and the quiet pass that gives icon-less apps
/// their icon. Many AppImages ship an SVG icon, which the raster decoder cannot
/// read, so those apps showed a letter in place of their icon.
void main() {
  setUpAll(() async {
    await loadAppFonts();
  });

  /// Lets the loaders finish. File reads run outside the test's fake clock.
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

  /// An app's own SVG icon inside [within]. The interface draws its own icons
  /// with SvgPicture too, but from assets; an icon read from a file is the one
  /// with a file loader.
  Finder fileSvg(Finder within) => find.descendant(
    of: within,
    matching: find.byWidgetPredicate(
      (widget) => widget is SvgPicture && widget.bytesLoader is SvgFileLoader,
    ),
  );

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

  group('an SVG icon', () {
    testWidgets('shows in the library row in place of the letter', (
      tester,
    ) async {
      await pumpApp(tester, core: coreWithIcon(iconFixture('valid.svg')));
      await settleIcons(tester);
      final row = find.byKey(const Key('row-quill'));
      expect(fileSvg(row), findsOneWidget);
      expect(find.descendant(of: row, matching: find.text('Q')), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('shows in the Detail header too', (tester) async {
      final model = await pumpApp(
        tester,
        core: coreWithIcon(iconFixture('valid.svg')),
      );
      model.selectApp('quill');
      await tester.pump();
      await settleIcons(tester);
      expect(fileSvg(find.byType(DetailPage)), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('that is not valid shows the letter, with no error', (
      tester,
    ) async {
      await pumpApp(tester, core: coreWithIcon(iconFixture('broken.svg')));
      await settleIcons(tester);
      final row = find.byKey(const Key('row-quill'));
      expect(
        find.descendant(of: row, matching: find.text('Q')),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('that is missing shows the letter, with no error', (
      tester,
    ) async {
      await pumpApp(
        tester,
        core: coreWithIcon('/home/someone/.local/share/icons/not-there.svg'),
      );
      await settleIcons(tester);
      final row = find.byKey(const Key('row-quill'));
      expect(
        find.descendant(of: row, matching: find.text('Q')),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('is chosen by the extension, in any case', (tester) async {
      final copy = File(
        '${Directory.systemTemp.createTempSync('gosh-icon').path}/Icon.SVG',
      )..writeAsBytesSync(File(iconFixture('valid.svg')).readAsBytesSync());
      addTearDown(() => copy.parent.deleteSync(recursive: true));
      await pumpApp(tester, core: coreWithIcon(copy.path));
      await settleIcons(tester);
      expect(fileSvg(find.byKey(const Key('row-quill'))), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('a PNG icon', () {
    testWidgets('is still drawn by the image decoder, not the SVG reader', (
      tester,
    ) async {
      await pumpApp(tester, core: coreWithIcon(iconFixture('valid.png')));
      await settleIcons(tester);
      final row = find.byKey(const Key('row-quill'));
      expect(
        find.descendant(of: row, matching: find.byType(Image)),
        findsOneWidget,
      );
      expect(fileSvg(row), findsNothing);
    });

    testWidgets('keeps its shape: only the width bounds the decode', (
      tester,
    ) async {
      await pumpApp(tester, core: coreWithIcon(iconFixture('valid.png')));
      await settleIcons(tester);
      final image = tester.widget<Image>(
        find.descendant(
          of: find.byKey(const Key('row-quill')),
          matching: find.byType(Image),
        ),
      );
      final provider = image.image;
      expect(provider, isA<ResizeImage>());
      final resized = provider as ResizeImage;
      expect(resized.width, isNotNull);
      expect(resized.height, isNull);
    });
  });

  group('the Inspect page', () {
    Future<void> inspectWith(
      WidgetTester tester, {
      List<int>? bytes,
      String format = '',
    }) async {
      final core = mockupCore()
        ..inspectResult = fakeInspect(
          name: 'Cinder Chat',
          version: '0.14.2',
          iconName: 'cinder-chat',
          iconFormat: format,
          iconBytes: bytes == null ? null : Uint8List.fromList(bytes),
        );
      final model = await pumpApp(tester, core: core, page: AppPage.inspect);
      await model.startInspect('/home/someone/Downloads/Cinder.AppImage');
      await tester.pump();
      await settleIcons(tester);
    }

    testWidgets('shows the extracted raster icon in its header', (
      tester,
    ) async {
      await inspectWith(
        tester,
        bytes: File(iconFixture('valid.png')).readAsBytesSync(),
        format: 'png',
      );
      final page = find.byType(InspectPage);
      expect(
        find.descendant(of: page, matching: find.byType(Image)),
        findsOneWidget,
      );
      expect(find.descendant(of: page, matching: find.text('C')), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('shows an extracted SVG icon in its header', (tester) async {
      await inspectWith(
        tester,
        bytes: File(iconFixture('valid.svg')).readAsBytesSync(),
        format: 'svg',
      );
      final page = find.byType(InspectPage);
      expect(
        find.descendant(
          of: page,
          matching: find.byWidgetPredicate(
            (widget) =>
                widget is SvgPicture && widget.bytesLoader is SvgBytesLoader,
          ),
        ),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('keeps the letter when the AppImage had no icon', (
      tester,
    ) async {
      await inspectWith(tester);
      final page = find.byType(InspectPage);
      expect(
        find.descendant(of: page, matching: find.text('C')),
        findsOneWidget,
      );
    });

    testWidgets('keeps the letter when the bytes are not an image', (
      tester,
    ) async {
      await inspectWith(tester, bytes: [1, 2, 3, 4], format: 'png');
      final page = find.byType(InspectPage);
      expect(
        find.descendant(of: page, matching: find.text('C')),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });
  });

  group('healing icons', () {
    test('reloads the Library when the core changed a record', () async {
      final core = mockupCore()..healedIcons = 2;
      final model = AppModel(core: core, pollInterval: null);
      await model.start(const []);
      final before = core.calls.where((c) => c == 'listLibrary').length;

      await model.healIcons();

      expect(core.calls, contains('healLibraryIcons'));
      expect(core.calls.where((c) => c == 'listLibrary').length, before + 1);
    });

    test('leaves the Library alone when nothing changed', () async {
      final core = mockupCore()..healedIcons = 0;
      final model = AppModel(core: core, pollInterval: null);
      await model.start(const []);
      final before = core.calls.where((c) => c == 'listLibrary').length;

      await model.healIcons();

      expect(core.calls, contains('healLibraryIcons'));
      expect(core.calls.where((c) => c == 'listLibrary').length, before);
    });

    test('is quiet when the core fails: no status, no error', () async {
      final core = mockupCore()
        ..failures['healLibraryIcons'] = CoreError(
          kind: ErrorKind.failure,
          message: 'could not read',
          details: '',
        );
      final model = AppModel(core: core, pollInterval: null);
      await model.start(const []);
      model.dismissStatus();

      await model.healIcons();

      expect(model.status, isNull);
      expect(model.libraryError, isNull);
    });

    test('shows the icon an app was given', () async {
      final core = mockupCore()
        ..library = LibraryDto(
          apps: [fakeApp(uuid: 'quill', name: 'Quill Notes')],
          discovered: const [],
        )
        ..healedIcons = 1;
      final model = AppModel(core: core, pollInterval: null);
      await model.start(const []);
      expect(model.library.single.iconPath, isEmpty);
      core.library = LibraryDto(
        apps: [
          fakeApp(
            uuid: 'quill',
            name: 'Quill Notes',
            iconPath: '/data/icons/gosh-appimage-quill.png',
          ),
        ],
        discovered: const [],
      );

      await model.healIcons();

      expect(
        model.library.single.iconPath,
        '/data/icons/gosh-appimage-quill.png',
      );
    });
  });
}
