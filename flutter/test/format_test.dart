import 'package:flutter_test/flutter_test.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/format.dart';

import 'support/fakes.dart';

void main() {
  group('humanSize', () {
    test('keeps small counts in bytes', () {
      expect(humanSize(512), '512 B');
    });

    test('scales to the largest unit that keeps the value under 1024', () {
      expect(humanSize(1536), '1.5 KB');
      expect(humanSize(3 * 1024 * 1024), '3.0 MB');
      expect(humanSize(5 * 1024 * 1024 * 1024), '5.0 GB');
    });

    test('shows a negative count as zero bytes, as the original does', () {
      expect(humanSize(-4), '-4 B');
    });
  });

  group('parseMaxAppimageMb', () {
    test('converts whole megabytes to bytes', () {
      expect(parseMaxAppimageMb(' 8192 '), 8192 * 1024 * 1024);
    });

    test('refuses an empty field', () {
      expect(
        () => parseMaxAppimageMb('  '),
        throwsA(
          isA<FormatException>().having(
            (e) => e.message,
            'message',
            'Maximum size cannot be empty',
          ),
        ),
      );
    });

    test('refuses something that is not a whole number', () {
      expect(
        () => parseMaxAppimageMb('1.5'),
        throwsA(
          isA<FormatException>().having(
            (e) => e.message,
            'message',
            '"1.5" is not a whole number of megabytes',
          ),
        ),
      );
    });

    test('refuses a value outside 1 to 32768 MB, naming the range', () {
      for (final input in ['0', '32769']) {
        expect(
          () => parseMaxAppimageMb(input),
          throwsA(
            isA<FormatException>().having(
              (e) => e.message,
              'message',
              'Enter 1–32768 MB (default 8192)',
            ),
          ),
        );
      }
    });
  });

  group('parseEnvironment', () {
    test('keeps valid pairs and reports invalid names', () {
      final parsed = parseEnvironment('''
# a comment
GOOD=1
  _ALSO_GOOD = two words
1BAD=x
no-equals-sign
''');
      expect(parsed.pairs.map((p) => '${p.name}=${p.value}'), [
        'GOOD=1',
        '_ALSO_GOOD=two words',
      ]);
      expect(parsed.rejected, ['1BAD', 'no-equals-sign']);
    });

    test('takes the value after the first equals sign only', () {
      final parsed = parseEnvironment('KEY=a=b');
      expect(parsed.pairs.single.value, 'a=b');
    });
  });

  group('isValidEnvName', () {
    test(
      'accepts a letter or underscore, then letters, digits, underscores',
      () {
        expect(isValidEnvName('PATH'), isTrue);
        expect(isValidEnvName('_x9'), isTrue);
      },
    );

    test('refuses a leading digit, a dash, and an empty name', () {
      expect(isValidEnvName('9X'), isFalse);
      expect(isValidEnvName('A-B'), isFalse);
      expect(isValidEnvName(''), isFalse);
    });
  });

  test('parseArguments drops blank lines and trims each argument', () {
    expect(parseArguments('  --one \n\n--two\n'), ['--one', '--two']);
  });

  test('parseConfigLines sorts by key and ignores lines without =', () {
    final config = parseConfigLines('b=2\n# note\na = 1\nnonsense\n');
    expect(config.map((kv) => '${kv.key}=${kv.value}'), ['a=1', 'b=2']);
  });

  group('normaliseOpenTarget', () {
    test('turns a local file URL into a path', () {
      expect(
        normaliseOpenTarget('file:///home/me/My%20Apps/Demo.AppImage'),
        '/home/me/My Apps/Demo.AppImage',
      );
    });

    test('leaves a plain path alone', () {
      expect(
        normaliseOpenTarget('/home/me/Demo.AppImage'),
        '/home/me/Demo.AppImage',
      );
    });

    test('leaves a remote URL alone rather than guessing a path', () {
      expect(
        normaliseOpenTarget('https://example.invalid/Demo.AppImage'),
        'https://example.invalid/Demo.AppImage',
      );
    });
  });

  group('planDrop', () {
    test('an idle drop starts its first file and queues the rest', () {
      final plan = planDrop(
        queued: const [],
        busy: false,
        hasUnconfirmed: false,
        incoming: const ['a', 'b', 'c'],
      );
      expect(plan.start, 'a');
      expect(plan.queue, ['b', 'c']);
    });

    test('a busy worker is never pre-empted by a drop', () {
      final plan = planDrop(
        queued: const ['q'],
        busy: true,
        hasUnconfirmed: false,
        incoming: const ['a', 'b'],
      );
      expect(plan.start, isNull);
      expect(plan.queue, ['q', 'a', 'b']);
    });

    test('an unconfirmed inspection is not discarded by a drop', () {
      final plan = planDrop(
        queued: const [],
        busy: false,
        hasUnconfirmed: true,
        incoming: const ['a'],
      );
      expect(plan.start, isNull);
      expect(plan.queue, ['a']);
    });

    test('blank drops change nothing', () {
      final plan = planDrop(
        queued: const ['q'],
        busy: false,
        hasUnconfirmed: false,
        incoming: const ['  ', ''],
      );
      expect(plan.start, isNull);
      expect(plan.queue, ['q']);
    });
  });

  test('badgesFor lists the markers that apply, in the original order', () {
    final app = fakeApp(running: true, adopted: true, externalFolder: true);
    final offers = [fakeOffer()];
    expect(
      badgesFor(app, offers),
      'running · update available · external folder · adopted',
    );
    expect(badgesFor(fakeApp(), const <UpdateOfferDto>[]), '');
  });

  test('inspectSummary reports unknown names and a missing update source', () {
    final rows = {
      for (final row in inspectSummary(fakeInspect(name: '', version: '')))
        row.key: row.value,
    };
    expect(rows['Name'], '(unknown)');
    expect(rows['Update source'], '(none embedded)');
    expect(rows['Already managed'], 'no');
  });

  test('appFacts falls back to the original wording for empty fields', () {
    final facts = {
      for (final fact in appFacts(fakeApp())) fact.key: fact.value,
    };
    expect(facts['Update manager'], 'none configured');
    expect(facts['Embedded source'], 'none');
    // No integration date was recorded, so none is invented.
    expect(facts['Provenance'], 'Integrated');
    expect(facts['Size'], '4.0 KB');
  });

  test('a Detail status line reads the integration date the core stored', () {
    expect(
      integrationLabel(
        fakeApp(integratedAt: unixSeconds(DateTime(2026, 9, 2, 10))),
      ),
      'Integrated 2 Sep 2026',
    );
    expect(
      integrationLabel(
        fakeApp(
          adopted: true,
          integratedAt: unixSeconds(DateTime(2026, 9, 2, 10)),
        ),
      ),
      'Adopted 2 Sep 2026',
    );
    expect(
      integrationLabel(fakeApp()),
      'Integrated',
      reason: 'no date is invented',
    );
  });

  test('provenanceLabel names the folder an app was integrated from', () {
    final dated = unixSeconds(DateTime(2026, 9, 2, 10));
    expect(
      provenanceLabel(
        fakeApp(integratedAt: dated, integratedFolder: '/home/gosh/Downloads'),
        '/home/gosh',
      ),
      'Integrated 2 Sep 2026 from ~/Downloads',
    );
    expect(
      provenanceLabel(
        fakeApp(integratedAt: dated, integratedFolder: '/mnt/disk/Downloads'),
        '/home/gosh',
      ),
      'Integrated 2 Sep 2026 from /mnt/disk/Downloads',
      reason: 'a folder outside the home folder is written in full',
    );
    expect(
      provenanceLabel(fakeApp(integratedAt: dated), '/home/gosh'),
      'Integrated 2 Sep 2026',
      reason: 'no folder was stored, so none is shown',
    );
    expect(
      provenanceLabel(fakeApp(), '/home/gosh'),
      'Integrated',
      reason: 'no date or folder was stored, so none is invented',
    );
  });

  test('shortenHome writes only the home folder and paths inside it as ~', () {
    expect(shortenHome('/home/gosh', '/home/gosh'), '~');
    expect(shortenHome('/home/gosh/Downloads', '/home/gosh'), '~/Downloads');
    expect(shortenHome('/home/gosh/Downloads', '/home/gosh/'), '~/Downloads');
    expect(
      shortenHome('/home/gosher/Downloads', '/home/gosh'),
      '/home/gosher/Downloads',
      reason: 'a sibling folder that shares the name is not the home folder',
    );
    expect(shortenHome('/home/gosh/Downloads', null), '/home/gosh/Downloads');
    expect(shortenHome('/home/gosh/Downloads', ''), '/home/gosh/Downloads');
  });

  test('the byte line of a running update reads the core counts in MB', () {
    expect(byteProgressLabel(43201331, 69206016), '41.2 of 66.0 MB');
  });

  test('a timed-out check reads the mockup sentence; any other failure reads the core text', () {
    expect(
      checkFailureText('Network request failed: timed out', timedOut: true),
      'Timed out. Status is unknown, not up to date.',
    );
    expect(
      checkFailureText(
        'Network request failed: DNS resolution',
        timedOut: false,
      ),
      'Network request failed: DNS resolution',
    );
  });
}
