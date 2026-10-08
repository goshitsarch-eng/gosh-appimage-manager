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

/// A byte count in the original's units: `512 B`, `1.5 MiB`, and so on.
String humanSize(int bytes) {
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
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
  MapEntry('Provenance', app.adopted ? 'adopted' : 'integrated here'),
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
