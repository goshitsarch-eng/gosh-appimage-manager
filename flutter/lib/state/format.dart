import 'dart:io';

import 'package:gosh_appimage_flutter/core/core_api.dart';

/// Limits the core enforces, mirrored so the form can explain them before a
/// call is made. The core stays the authority; these only shape the input.
const int maxEnvPairs = 32;
const int maxArguments = 64;
const int minMaxAppimageMb = 1;
const int absoluteMaxAppimageMb = 32768;
const int defaultMaxAppimageMb = 8192;

/// Whole megabytes for the Settings field (8 GB default renders as "8192").
int maxAppimageMbFor(int bytes) => bytes ~/ (1024 * 1024);

/// A byte count as the mockup writes it: `512 B`, `84.2 MB`, and so on.
String humanSize(int bytes) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  var value = bytes < 0 ? 0.0 : bytes.toDouble();
  var unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit == 0) {
    return '$bytes ${units[0]}';
  }
  return '${value.toStringAsFixed(1)} ${units[unit]}';
}

String nonEmpty(String value, String fallback) =>
    value.isEmpty ? fallback : value;

/// A file manager passing a `file://` URL gives a URL, not a path. Anything
/// that does not parse as a local file URL is returned unchanged.
String normaliseOpenTarget(String raw) {
  final uri = Uri.tryParse(raw);
  if (uri != null && uri.scheme == 'file') {
    try {
      return uri.toFilePath();
    } on UnsupportedError {
      // A remote host or a non-file form: keep the text, as the original does.
    }
  }
  return raw;
}

/// Whether `name` is a legal environment variable name: a letter or an
/// underscore, then letters, digits or underscores, at most 128 characters.
bool isValidEnvName(String name) {
  if (name.isEmpty || name.length > 128) {
    return false;
  }
  bool isAlpha(int c) => (c >= 0x41 && c <= 0x5a) || (c >= 0x61 && c <= 0x7a);
  bool isDigit(int c) => c >= 0x30 && c <= 0x39;
  final codes = name.codeUnits;
  if (!(isAlpha(codes.first) || codes.first == 0x5f)) {
    return false;
  }
  return codes.every((c) => isAlpha(c) || isDigit(c) || c == 0x5f);
}

/// The environment parsed from the form, with the names that were refused.
class ParsedEnvironment {
  const ParsedEnvironment(this.pairs, this.rejected);

  final List<EnvVarDto> pairs;
  final List<String> rejected;
}

/// Parses `NAME=value` lines. Names that are not valid are reported rather
/// than dropped, so the user sees what was refused.
ParsedEnvironment parseEnvironment(String text) {
  final pairs = <EnvVarDto>[];
  final rejected = <String>[];
  for (final raw in text.split('\n').take(maxEnvPairs * 2)) {
    final line = raw.trim();
    if (line.isEmpty || line.startsWith('#')) {
      continue;
    }
    final equals = line.indexOf('=');
    if (equals < 0) {
      rejected.add(line);
      continue;
    }
    final name = line.substring(0, equals).trim();
    final value = line.substring(equals + 1).trim();
    if (isValidEnvName(name)) {
      pairs.add(EnvVarDto(name: name, value: value));
    } else {
      rejected.add(name);
    }
  }
  return ParsedEnvironment(pairs, rejected);
}

/// One argument per line. Blank lines are dropped and the count is capped.
List<String> parseArguments(String text) => text
    .split('\n')
    .map((line) => line.trim())
    .where((line) => line.isNotEmpty)
    .take(maxArguments)
    .toList();

/// `key=value` lines for an update source, sorted by key. Lines without an
/// `=` are ignored, as in the original.
List<KeyValueDto> parseConfigLines(String text) {
  final map = <String, String>{};
  for (final raw in text.split('\n')) {
    final line = raw.trim();
    if (line.isEmpty || line.startsWith('#')) {
      continue;
    }
    final equals = line.indexOf('=');
    if (equals < 0) {
      continue;
    }
    map[line.substring(0, equals).trim()] = line.substring(equals + 1).trim();
  }
  final keys = map.keys.toList()..sort();
  return [for (final key in keys) KeyValueDto(key: key, value: map[key]!)];
}

/// Parses the Settings "max size (MB)" field into bytes. The messages are the
/// core's own, so the two front ends refuse the same inputs in the same words.
int parseMaxAppimageMb(String input) {
  final trimmed = input.trim();
  if (trimmed.isEmpty) {
    throw const FormatException('Maximum size cannot be empty');
  }
  final mb = int.tryParse(trimmed);
  if (mb == null) {
    throw FormatException('"$trimmed" is not a whole number of megabytes');
  }
  if (mb < minMaxAppimageMb || mb > absoluteMaxAppimageMb) {
    throw FormatException(
      'Enter $minMaxAppimageMb–$absoluteMaxAppimageMb MB '
      '(default $defaultMaxAppimageMb)',
    );
  }
  return mb * 1024 * 1024;
}

/// Orders two version strings as a person reads them. Dotted segments compare
/// as numbers when both are numbers, so 1.9.0 is older than 1.10.0; otherwise
/// they compare as text. A version with fewer segments comes first when the
/// rest is equal, and an empty version comes before any other (R6-06).
/// Negative when [a] is older, positive when it is newer, zero when equal.
int compareVersions(String a, String b) {
  final left = a.split('.');
  final right = b.split('.');
  for (var i = 0; i < left.length && i < right.length; i++) {
    final x = left[i];
    final y = right[i];
    final nx = int.tryParse(x);
    final ny = int.tryParse(y);
    final order = nx != null && ny != null ? nx.compareTo(ny) : x.compareTo(y);
    if (order != 0) {
      return order;
    }
  }
  return left.length.compareTo(right.length);
}

/// The small markers shown after an app's name and version in the Library.
String badgesFor(AppDto app, Iterable<UpdateOfferDto> updates) {
  final badges = <String>[];
  if (app.running) {
    badges.add('running');
  }
  if (updates.any((offer) => offer.uuid == app.uuid)) {
    badges.add('update available');
  }
  if (app.externalFolder) {
    badges.add('external folder');
  }
  if (app.adopted) {
    badges.add('adopted');
  }
  return badges.join(' · ');
}

/// The label/value pairs on the Inspect page for a file that passed inspection.
List<MapEntry<String, String>> inspectSummary(InspectDto result) => [
  MapEntry('Path', result.path),
  MapEntry('Size', humanSize(result.sizeBytes)),
  MapEntry('Type', result.appType),
  MapEntry('Architecture', result.architecture),
  MapEntry('SHA-256', result.sha256),
  MapEntry('Name', nonEmpty(result.name, '(unknown)')),
  MapEntry('Version', nonEmpty(result.version, '(unknown)')),
  MapEntry('Update source', nonEmpty(result.embeddedUpdate, '(none embedded)')),
  MapEntry('Already managed', result.alreadyManaged ? 'yes' : 'no'),
];

/// The facts listed in an app's Details section, in the original's order.
List<MapEntry<String, String>> appFacts(AppDto app) => [
  MapEntry('Path', app.managedPath),
  MapEntry('Desktop ID', app.desktopId),
  MapEntry('SHA-256', app.sha256),
  MapEntry('Type', app.appType),
  MapEntry('Architecture', app.architecture),
  MapEntry('Version', app.version),
  MapEntry('Size', humanSize(app.sizeBytes)),
  MapEntry('Update manager', nonEmpty(app.updateManager, 'none configured')),
  MapEntry('Embedded source', nonEmpty(app.embeddedUpdate, 'none')),
  MapEntry('Provenance', provenanceLabel(app, homeFolder())),
];

/// The words the Tasks page uses for each state.
String taskStateWord(TaskStateDto state) => switch (state) {
  TaskStateDto.queued => 'queued',
  TaskStateDto.running => 'running',
  TaskStateDto.cancelling => 'cancelling',
  TaskStateDto.succeeded => 'succeeded',
  TaskStateDto.failed => 'failed',
  TaskStateDto.cancelled => 'cancelled',
};

/// The outcome of planning a drop: the file to inspect now, if any, and the
/// files that wait behind it.
class DropPlan {
  const DropPlan({this.start, required this.queue});

  final String? start;
  final List<String> queue;
}

/// Decides whether dropped files start at once or wait. A busy worker or an
/// unconfirmed inspection is never discarded by a drop, so the drop queues.
DropPlan planDrop({
  required List<String> queued,
  required bool busy,
  required bool hasUnconfirmed,
  required List<String> incoming,
}) {
  final fresh = incoming
      .map((path) => path.trim())
      .where((path) => path.isNotEmpty)
      .toList();
  if (fresh.isEmpty) {
    return DropPlan(queue: List.of(queued));
  }
  if (busy || hasUnconfirmed) {
    return DropPlan(queue: [...queued, ...fresh]);
  }
  return DropPlan(start: fresh.first, queue: [...queued, ...fresh.skip(1)]);
}

String _two(int value) => value.toString().padLeft(2, '0');

/// The clock time the mockup prints, `09:14`.
String clockLabel(DateTime when) => '${_two(when.hour)}:${_two(when.minute)}';

const List<String> _monthNames = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
];

/// A calendar date as the Detail page writes it: `2 Sep 2026`.
String dateLabel(DateTime when) =>
    '${when.day} ${_monthNames[when.month - 1]} ${when.year}';

/// How the Detail page says an app came to be here: `Integrated 2 Sep 2026`,
/// or `Adopted 2 Sep 2026`. The date is left out when the core never recorded
/// one, rather than inventing it.
String integrationLabel(AppDto app) {
  final verb = app.adopted ? 'Adopted' : 'Integrated';
  if (app.integratedAt <= 0) {
    return verb;
  }
  final when = DateTime.fromMillisecondsSinceEpoch(app.integratedAt * 1000);
  return '$verb ${dateLabel(when)}';
}

/// The Detail page's Provenance line: [integrationLabel], then the folder the
/// app was integrated from, with the home folder as `~`, as the Library rows
/// write a path: `Integrated 2 Sep 2026 from ~/Downloads`. The folder is left
/// out when the core never recorded one, so an older app reads as it did.
String provenanceLabel(AppDto app, String? home) {
  final label = integrationLabel(app);
  if (app.integratedFolder.isEmpty) {
    return label;
  }
  return '$label from ${shortenHome(app.integratedFolder, home)}';
}

/// The mockup's sentence for a check that ran out of time. The status is
/// unknown, so it must not read as "up to date".
const String timedOutText = 'Timed out. Status is unknown, not up to date.';

/// What a failed check says. A timeout gets the mockup's sentence; any other
/// failure shows the core's message.
String checkFailureText(String error, {required bool timedOut}) =>
    timedOut ? timedOutText : error;

/// A running update's byte line in the mockup's form: `41.2 of 66.0 MB`.
String byteProgressLabel(int done, int total) {
  const mb = 1024 * 1024;
  return '${(done / mb).toStringAsFixed(1)} of '
      '${(total / mb).toStringAsFixed(1)} MB';
}

/// A task's end time: `Today 08:52`, `Yesterday 17:40`, or a short date.
String taskTimeLabel(DateTime when, DateTime now) {
  final day = DateTime(when.year, when.month, when.day);
  final today = DateTime(now.year, now.month, now.day);
  final gap = today.difference(day).inDays;
  if (gap == 0) {
    return 'Today ${clockLabel(when)}';
  }
  if (gap == 1) {
    return 'Yesterday ${clockLabel(when)}';
  }
  return '${when.day} ${_monthNames[when.month - 1]} ${clockLabel(when)}';
}

/// A path with the home folder written as `~`, as the mockup shows it. Only the
/// home folder itself, or a path inside it, is shortened: with the home folder
/// `/home/gosh`, `/home/gosher/x` stays as it is.
String shortenHome(String path, String? home) {
  final root = (home ?? '').replaceFirst(RegExp(r'/+$'), '');
  if (root.isEmpty) {
    return path;
  }
  if (path == root) {
    return '~';
  }
  if (path.startsWith('$root/')) {
    return '~${path.substring(root.length)}';
  }
  return path;
}

/// Whole megabytes, as the mockup's sizes read (`512 MB`).
String megabytes(int bytes) => '${(bytes / (1024 * 1024)).round()} MB';

/// The sidebar's folder note: `7 apps · 512 MB`, or `Empty`.
String folderNote(int apps, int bytes) =>
    apps == 0 ? 'Empty' : '$apps apps · ${megabytes(bytes)}';

/// The user's home folder, or null when the environment does not name one.
String? homeFolder() => homeFolderProvider();

/// Where the home folder comes from. Tests replace it so a golden does not
/// depend on who runs it.
String? Function() homeFolderProvider = () => Platform.environment['HOME'];
