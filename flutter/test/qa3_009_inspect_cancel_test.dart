import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';

import 'support/fakes.dart';
import 'support/harness.dart';

/// QA3-009 (major): a stopped inspection must not read as a finished one. A run
/// cancelled from the status bar showed OK, an empty hash row, an enabled
/// Integrate and "Cancelling..." after busy had cleared.
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
  });

  const path = '/home/someone/Downloads/Big-Stub-1.0.AppImage';

  Future<void> settleBriefly(WidgetTester tester) async {
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
  }

  /// Asserts the Inspect page claims nothing for [path]: no OK, no Integrate,
  /// and no "Cancelling" left on screen.
  void expectNoClaim(WidgetTester tester, AppModel model) {
    expect(model.inspect.inspectedOk, isFalse, reason: 'the run claims OK');
    expect(
      find.byKey(const Key('integrate')),
      findsNothing,
      reason: 'Integrate is offered',
    );
    expect(
      find.textContaining('Cancelling'),
      findsNothing,
      reason: '"Cancelling" is still on screen',
    );
  }

  testWidgets(
    'QA3-009 a run cancelled from the status bar shows its error, claims no OK, and offers no Integrate',
    (tester) async {
      final core = mockupCore();
      final gate = Completer<void>();
      core.holds['inspectPath'] = gate;
      // What a stopped run returns today: it parses, with no error and no hash.
      core.inspectResult = fakeInspect(path: path, hashed: false);
      final model = await pumpApp(
        tester,
        core: core,
        page: AppPage.inspect,
        checked: false,
      );
      unawaited(model.startInspect(path));
      await settleBriefly(tester);
      expect(model.busy, isNotNull, reason: 'the inspection did not start');
      model.cancelBusy();
      await settleBriefly(tester);
      gate.complete();
      await settleBriefly(tester);
      await tester.pump(const Duration(seconds: 3));
      expect(model.busy, isNull, reason: 'busy did not clear');
      expect(
        model.inspect.error,
        isNotEmpty,
        reason: 'the stopped run shows no error',
      );
      expect(find.byKey(const Key('inspect-error')), findsOneWidget);
      expectNoClaim(tester, model);
    },
  );

  testWidgets(
    'QA3-009 a stopped run the bridge reports as failed shows the bridge error and no Integrate',
    (tester) async {
      const bridgeError =
          'Inspection stopped before the file was read to the end';
      final core = mockupCore();
      final gate = Completer<void>();
      core.holds['inspectPath'] = gate;
      core.inspectResult = fakeInspect(
        path: path,
        hashed: false,
        error: bridgeError,
      );
      final model = await pumpApp(
        tester,
        core: core,
        page: AppPage.inspect,
        checked: false,
      );
      unawaited(model.startInspect(path));
      await settleBriefly(tester);
      model.cancelBusy();
      await settleBriefly(tester);
      gate.complete();
      await settleBriefly(tester);
      await tester.pump(const Duration(seconds: 3));
      expect(model.busy, isNull, reason: 'busy did not clear');
      expect(
        model.inspect.error,
        bridgeError,
        reason: 'the bridge error is lost',
      );
      expect(find.text(bridgeError), findsOneWidget);
      expectNoClaim(tester, model);
    },
  );

  testWidgets(
    'QA3-009 a failed inspection is not OK even when the file parsed',
    (tester) async {
      const failure = 'The metadata could not be read';
      final core = mockupCore()
        ..inspectResult = fakeInspect(path: path, error: failure);
      final model = await pumpApp(
        tester,
        core: core,
        page: AppPage.inspect,
        checked: false,
      );
      await model.startInspect(path);
      await settleBriefly(tester);
      expect(model.inspect.error, failure, reason: 'the failure is not shown');
      expectNoClaim(tester, model);
    },
  );
}
