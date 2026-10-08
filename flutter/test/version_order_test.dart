import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';

import 'support/fakes.dart';

/// R6-06: versions compare by their dotted numbers, so 1.9.0 is older than
/// 1.10.0. Plain text order put 1.10.0 first.
void main() {
  group('compareVersions', () {
    test('ascending: numbers compare as numbers, not as text', () {
      expect(compareVersions('1.9.0', '1.10.0'), lessThan(0));
      expect(compareVersions('1.9', '1.10'), lessThan(0));
      expect(compareVersions('2.0.0', '10.0.0'), lessThan(0));
      expect(compareVersions('0.14.2', '0.9.3'), greaterThan(0));
    });

    test('descending: the reverse comparison agrees', () {
      expect(compareVersions('1.10.0', '1.9.0'), greaterThan(0));
      expect(compareVersions('10.0.0', '2.0.0'), greaterThan(0));
      expect(compareVersions('0.9.3', '0.14.2'), lessThan(0));
    });

    test('equal versions compare as equal, and a longer one is newer', () {
      expect(compareVersions('2.4.1', '2.4.1'), 0);
      expect(compareVersions('2.4', '2.4.0'), lessThan(0));
      expect(compareVersions('2.4.0', '2.4'), greaterThan(0));
    });

    test('a segment that is not a number compares as text', () {
      expect(compareVersions('1.0.0-beta', '1.0.0-alpha'), greaterThan(0));
      expect(compareVersions('1.0.0-alpha', '1.0.0-beta'), lessThan(0));
    });

    test('a missing version sorts before any version', () {
      expect(compareVersions('', '0.1'), lessThan(0));
    });
  });

  test('the Library sorts by version numerically, not as text', () async {
    final core = FakeCore(
      library: LibraryDto(
        apps: [
          fakeApp(
            uuid: 'ten',
            name: 'Ten',
            version: '1.10.0',
            managedPath: '/home/someone/AppImages/Ten.AppImage',
          ),
          fakeApp(
            uuid: 'nine',
            name: 'Nine',
            version: '1.9.0',
            managedPath: '/home/someone/AppImages/Nine.AppImage',
          ),
        ],
        discovered: const [],
      ),
    );
    final model = AppModel(core: core, pollInterval: null);
    await model.start(const []);
    model.setSort(SortOrder.version);
    expect(
      [for (final app in model.visibleLibrary) app.version],
      ['1.9.0', '1.10.0'],
    );
  });
}
