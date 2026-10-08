import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';

import 'support/fakes.dart';

/// Lets the unawaited work the model starts finish before the test looks.
Future<void> settle() => Future<void>.delayed(Duration.zero).then((_) {
  return Future<void>.delayed(Duration.zero);
});

AppModel modelWith(FakeCore core) => AppModel(core: core, pollInterval: null);

void main() {
  group('launch', () {
    test('reports the app it launched', () async {
      final core = FakeCore(
        library: LibraryDto(apps: [fakeApp()], discovered: const []),
      );
      final model = modelWith(core);
      await model.loadLibrary();

      await model.launch('u1');

      expect(core.calls, contains('launchApp:u1'));
      expect(model.status?.text, 'Launched Demo');
      expect(model.status?.severity, Severity.success);
    });

    test('reports a failure with the core message', () async {
      final core = FakeCore(
        library: LibraryDto(apps: [fakeApp()], discovered: const []),
      )..failures['launchApp'] = coreError('no such binary');
      final model = modelWith(core);
      await model.loadLibrary();

      await model.launch('u1');

      expect(model.status?.text, 'Launch failed: no such binary');
      expect(model.status?.severity, Severity.error);
    });

    test('names the missing app when it is not in the library', () async {
      final model = modelWith(FakeCore());
      await model.launch('ghost');
      expect(model.status?.text, 'No such application');
    });
  });

  group('removal', () {
    test('a permanent removal asks first, then removes and reports', () async {
      final core = FakeCore(
        library: LibraryDto(apps: [fakeApp()], discovered: const []),
      );
      final model = modelWith(core);
      await model.loadLibrary();

      model.askRemove('u1', permanent: true);
      expect(model.dialog, isA<RemoveDialog>());
      expect(core.calls, isNot(contains('removeApp:u1:permanent')));

      await model.confirmRemove();

      expect(core.calls, contains('removeApp:u1:permanent'));
      expect(model.dialog, isNull);
      expect(model.status?.text, 'Removed');
    });

    test('a refused removal shows the core reason as an error', () async {
      final core = FakeCore(
        library: LibraryDto(apps: [fakeApp()], discovered: const []),
      )..removeResult = fakeOutcome(ok: false, message: 'in use');
      final model = modelWith(core);
      await model.loadLibrary();

      model.askRemove('u1', permanent: false);
      await model.confirmRemove();

      expect(model.status?.text, 'in use');
      expect(model.status?.severity, Severity.error);
    });
  });

  group('integrate', () {
    test(
      'a name conflict with one candidate opens the replace dialog',
      () async {
        final core = FakeCore()
          ..integrateResult = fakeOutcome(
            ok: false,
            message: 'Replace requires an explicit choice',
            conflict: true,
            conflictUuid: 'old',
            conflictName: 'Demo',
          );
        final model = modelWith(core)
          ..inspect.pathInput = '/tmp/Demo.AppImage'
          ..inspect.inspectedOk = true;

        model.integrateFromInspect();
        await settle();

        final dialog = model.dialog;
        expect(dialog, isA<IntegrateConflictDialog>());
        expect((dialog! as IntegrateConflictDialog).replaceLabel, 'Demo');
        expect((dialog as IntegrateConflictDialog).replaceUuid, 'old');
        expect(model.status, isNull);
      },
    );

    test(
      'a conflict with no single candidate is reported as a failure',
      () async {
        final core = FakeCore()
          ..integrateResult = fakeOutcome(
            ok: false,
            message: 'keep-both or replace needed',
            conflict: true,
          );
        final model = modelWith(core)
          ..inspect.pathInput = '/tmp/Demo.AppImage'
          ..inspect.inspectedOk = true;

        model.integrateFromInspect();
        await settle();

        expect(model.dialog, isNull);
        expect(
          model.status?.text,
          'Integrate failed: keep-both or replace needed',
        );
      },
    );

    test('keeping both integrates with the keep-both policy', () async {
      final core = FakeCore()
        ..integrateResult = fakeOutcome(
          ok: true,
          app: fakeApp(managedPath: '/home/someone/AppImages/Demo-2.AppImage'),
        );
      final model = modelWith(core);

      model.keepBothAndIntegrate('/tmp/Demo.AppImage');
      await settle();

      expect(core.calls, contains('integrateApp:/tmp/Demo.AppImage:keepBoth'));
      expect(
        model.status?.text,
        'Integrated /home/someone/AppImages/Demo-2.AppImage',
      );
    });

    test(
      'an empty path asks for a file rather than calling the core',
      () async {
        final core = FakeCore();
        final model = modelWith(core)..integrateFromInspect();
        expect(model.inspect.error, 'Choose an AppImage file first');
        expect(core.calls.where((c) => c.startsWith('integrateApp')), isEmpty);
      },
    );
  });

  group('updates', () {
    test('updating a running app asks before forcing it', () async {
      final core = FakeCore();
      final model = modelWith(core);
      model.updates = [fakeOffer(running: true)];

      await model.updateOne('u1');

      expect(model.dialog, isA<UpdateForceDialog>());
      expect(core.calls, isNot(contains('applyUpdate:u1:normal')));

      await model.confirmForceUpdate();

      expect(core.calls, contains('applyUpdate:u1:force'));
    });

    test(
      'an update all with nothing offered says so and does not run',
      () async {
        final core = FakeCore();
        final model = modelWith(core);

        await model.updateAll();

        expect(model.status?.text, 'Nothing to update');
        expect(core.calls, isNot(contains('applyAllUpdates:normal')));
      },
    );

    test(
      'a batch with a refused app is reported as failures, not success',
      () async {
        final core = FakeCore()
          ..batch = BatchDto(
            applied: const ['Alpha'],
            failed: [
              UpdateFailureDto(
                uuid: 'b',
                name: 'Beta',
                manager: 'github',
                error: 'checksum mismatch',
                timedOut: false,
              ),
            ],
            skippedRunning: const [],
            checkFailures: const [],
            cancelled: false,
          );
        final model = modelWith(core)
          ..updates = [fakeOffer(uuid: 'a', name: 'Alpha')];

        await model.updateAll();

        expect(model.status?.text, 'Updated 1; 1 failed');
        expect(model.status?.severity, Severity.error);
        expect(model.updateFailures.single.name, 'Beta');
      },
    );

    test(
      'a check that could not reach an app says so beside the offers',
      () async {
        final core = FakeCore()
          ..scan = UpdateScanDto(
            offers: const [],
            failures: [
              UpdateFailureDto(
                uuid: 'c',
                name: 'Gamma',
                manager: 'static',
                timedOut: false,
                error: 'timed out',
              ),
            ],
            skipped: 0,
            checked: 1,
            cancelled: false,
          );
        final model = modelWith(core);

        await model.checkUpdates();

        expect(model.checkFailures.single.name, 'Gamma');
        expect(
          model.status?.text,
          '0 update(s) available; 1 app(s) could not be checked',
        );
      },
    );
  });

  group('settings', () {
    test(
      'turning background checks off removes the login entry first',
      () async {
        final core = FakeCore();
        final model = modelWith(core);

        await model.setBackgroundUpdateChecks(false);

        final autostart = core.calls.indexOf('setAutostart:false');
        final save = core.calls.indexOf('saveSettings');
        expect(autostart, isNonNegative);
        expect(save, greaterThan(autostart));
        expect(model.status?.text, 'Background update checks off');
      },
    );

    test('a refused login-entry removal stops the change', () async {
      final core = FakeCore()
        ..failures['setAutostart'] = coreError('permission denied');
      final model = modelWith(core);

      await model.setBackgroundUpdateChecks(false);

      expect(core.calls, isNot(contains('saveSettings')));
      expect(
        model.status?.text,
        'Cannot disable background checks: permission denied',
      );
    });

    test(
      'a failed save names the setting and does not claim success',
      () async {
        final core = FakeCore()
          ..failures['saveSettings'] = coreError('disk full');
        final model = modelWith(core);
        await model.loadSettings(fromStart: true);

        await model.applyManagedFolder();
        model.setManagedFolderInput('/home/someone/Apps');
        await model.applyManagedFolder();

        expect(
          model.status?.text,
          'Could not save the managed folder: disk full',
        );
        expect(model.status?.severity, Severity.error);
      },
    );

    test('a relative managed folder is refused before saving', () async {
      final core = FakeCore();
      final model = modelWith(core)..setManagedFolderInput('relative/path');

      await model.applyManagedFolder();

      expect(model.status?.text, 'Managed folder must be an absolute path');
      expect(core.calls, isNot(contains('saveSettings')));
    });

    test('a size outside the allowed range is refused before saving', () async {
      final core = FakeCore();
      final model = modelWith(core)..setMaxBytesInput('0');

      await model.applyMaxBytes();

      expect(model.status?.text, 'Enter 1–32768 MB (default 8192)');
      expect(core.calls, isNot(contains('saveSettings')));
    });
  });

  group('arguments and environment', () {
    test('an invalid variable name is named and nothing is saved', () async {
      final core = FakeCore(
        library: LibraryDto(apps: [fakeApp()], discovered: const []),
      );
      final model = modelWith(core);
      await model.loadLibrary();
      model.selectApp('u1');
      model.setEnvironmentInput('1BAD=x\nGOOD=1');

      await model.saveArgumentsAndEnvironment();

      expect(model.status?.text, 'Not a valid environment variable name: 1BAD');
      expect(
        core.calls.where((c) => c.startsWith('saveArgumentsAndEnvironment')),
        isEmpty,
      );
    });

    test('a valid save reports what was saved', () async {
      final core = FakeCore(
        library: LibraryDto(apps: [fakeApp()], discovered: const []),
      );
      final model = modelWith(core);
      await model.loadLibrary();
      model.selectApp('u1');
      model.setArgumentsInput('--one\n\n--two');

      await model.saveArgumentsAndEnvironment();

      expect(core.calls, contains('saveArgumentsAndEnvironment:u1'));
      expect(model.status?.text, 'Saved arguments and environment');
    });

    test('selecting an app loads its saved arguments into the form', () async {
      final core = FakeCore(
        library: LibraryDto(
          apps: [
            fakeApp(
              arguments: const ['--a', '--b'],
              environment: [EnvVarDto(name: 'MODE', value: 'fast')],
            ),
          ],
          discovered: const [],
        ),
      );
      final model = modelWith(core);
      await model.loadLibrary();

      model.selectApp('u1');

      expect(model.argumentsInput, '--a\n--b');
      expect(model.environmentInput, 'MODE=fast');
    });
  });

  group('inspect', () {
    test(
      'an invalid file reports the core reason and offers no integrate',
      () async {
        final core = FakeCore()
          ..inspectResult = fakeInspect(magicValid: false, error: 'not ELF');
        final model = modelWith(core);

        await model.startInspect('/tmp/plain.txt');

        expect(model.inspect.error, 'not ELF');
        expect(model.inspect.inspectedOk, isFalse);
      },
    );

    test('a valid file fills the summary and warnings', () async {
      final core = FakeCore()
        ..inspectResult = fakeInspect(warnings: const ['odd icon']);
      final model = modelWith(core);

      await model.startInspect('/tmp/Demo.AppImage');

      expect(model.inspect.inspectedOk, isTrue);
      expect(model.inspect.warnings, ['odd icon']);
      expect(model.inspect.summary.first.key, 'Path');
    });

    test(
      'opening several files inspects the first and queues the rest',
      () async {
        final core = FakeCore();
        final model = modelWith(core);

        model.openPaths([
          '/tmp/a.AppImage',
          '/tmp/b.AppImage',
          '/tmp/c.AppImage',
        ]);
        await settle();

        expect(model.page, AppPage.inspect);
        expect(model.inspect.queued, ['/tmp/b.AppImage', '/tmp/c.AppImage']);
        expect(core.calls, contains('inspectPath:/tmp/a.AppImage'));
      },
    );

    test(
      'a drop while another file awaits a decision queues instead',
      () async {
        final core = FakeCore();
        final model = modelWith(core)..inspect.inspectedOk = true;

        model.dropFiles(['/tmp/d.AppImage']);
        await settle();

        expect(core.calls, isNot(contains('inspectPath:/tmp/d.AppImage')));
        expect(model.inspect.queued, ['/tmp/d.AppImage']);
      },
    );
  });

  test('the Library sorts and filters the way the original does', () async {
    final core = FakeCore(
      library: LibraryDto(
        apps: [
          fakeApp(uuid: 'a', name: 'zeta', version: '2.0'),
          fakeApp(uuid: 'b', name: 'Alpha', version: '10.0'),
          fakeApp(uuid: 'c', name: 'Mid', version: '1.0'),
        ],
        discovered: const [],
      ),
    );
    final model = modelWith(core);
    await model.loadLibrary();

    expect(model.visibleLibrary.map((a) => a.name), ['Alpha', 'Mid', 'zeta']);

    // Versions compare by their numbers: 1.0 < 2.0 < 10.0 (R6-06).
    model.setSort(SortOrder.version);
    expect(model.visibleLibrary.map((a) => a.name), ['Mid', 'zeta', 'Alpha']);

    model.setSort(SortOrder.updatesFirst);
    model.updates = [fakeOffer(uuid: 'c', name: 'Mid')];
    expect(model.visibleLibrary.first.name, 'Mid');

    model.setSort(SortOrder.name);
    model.setSearch('  MID ');
    expect(model.visibleLibrary.map((a) => a.name), ['Mid']);
  });

  test('a load error from the settings file is shown at start', () async {
    final core = FakeCore(
      settings: fakeSettings(loadError: 'settings.json is corrupt'),
    );
    final model = modelWith(core);

    await model.start(const []);

    expect(model.status?.text, 'settings.json is corrupt');
    expect(model.status?.severity, Severity.error);
    expect(model.version, '3.0.0');
  });

  test('dismissing a dialog clears it without touching the core', () async {
    final core = FakeCore(
      library: LibraryDto(apps: [fakeApp()], discovered: const []),
    );
    final model = modelWith(core);
    await model.loadLibrary();
    model.askRemove('u1', permanent: false);

    model.dismissDialog();

    expect(model.dialog, isNull);
    expect(core.calls.where((c) => c.startsWith('removeApp')), isEmpty);
  });
}
