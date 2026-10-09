import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:path/path.dart' as p;

/// The six pages of the sidebar, in the order the original lists them.
enum AppPage { library, inspect, updates, tasks, settings, about }

enum SortOrder { name, version, updatesFirst }

/// The Library's segmented filter: All, Updates, Needs attention.
enum LibraryFilter { all, updates, attention }

/// What a library row says about its app: a label, a tone for its dot, and
/// an optional second line.
class RowStatus {
  const RowStatus(this.label, this.tone, {this.detail = ''});

  final String label;
  final StatusTone tone;
  final String detail;
}

/// The status line carries severity as text as well as colour.
enum Severity { info, success, error }

class StatusMessage {
  const StatusMessage(this.severity, this.text);

  final Severity severity;
  final String text;
}

/// The operation the status bar reports as "Working", and the id Cancel uses.
class BusyState {
  const BusyState(this.opId, this.title);

  final String opId;
  final String title;
}

/// A modal question. Every destructive choice is one of these.
sealed class PendingDialog {
  const PendingDialog();
}

class IntegrateConflictDialog extends PendingDialog {
  const IntegrateConflictDialog({
    required this.path,
    required this.conflictName,
    required this.incomingVersion,
    required this.replaceUuid,
    required this.replaceLabel,
    required this.installedVersion,
  });

  final String path;
  final String conflictName;
  final String incomingVersion;
  final String installedVersion;

  /// Empty when no single installation is implicated, which hides Replace.
  final String replaceUuid;
  final String replaceLabel;
}

class RemoveDialog extends PendingDialog {
  const RemoveDialog({
    required this.uuid,
    required this.name,
    required this.path,
    required this.permanent,
  });

  final String uuid;
  final String name;
  final String path;
  final bool permanent;
}

class UpdateForceDialog extends PendingDialog {
  const UpdateForceDialog({required this.uuid, required this.name});

  final String uuid;
  final String name;
}

class AdoptDialog extends PendingDialog {
  const AdoptDialog({required this.path});

  final String path;
}

/// Asks before the unsafe extraction fallback is turned on: once on, the
/// AppImage itself runs to read its files (owner decision). Turning it off
/// asks nothing.
class UnsafeFallbackDialog extends PendingDialog {
  const UnsafeFallbackDialog();
}

/// Asks, for one file, before the unsafe fallback runs the AppImage itself to
/// read it (owner decision, round 4). Run anyway repeats the request with the
/// choice made for that file only. Cancel and Escape run nothing.
class FallbackConfirmDialog extends PendingDialog {
  const FallbackConfirmDialog({
    required this.fileName,
    required this.onRunAnyway,
  });

  final String fileName;
  final Future<void> Function() onRunAnyway;
}

/// One app whose update could not be checked, or one update that did not
/// apply. Both are listed on the Updates page, never reported as "up to date".
class UpdateProblem {
  const UpdateProblem(
    this.name,
    this.error, {
    this.uuid = '',
    this.timedOut = false,
  });

  final String uuid;
  final String name;
  final String error;

  /// The source did not answer in time. The status is unknown, not up to date.
  final bool timedOut;
}

/// The Inspect page's state: the path being looked at, what it found, and
/// the files queued behind it.
class InspectState {
  String pathInput = '';
  String name = '';
  String version = '';
  List<MapEntry<String, String>> summary = const [];
  List<String> warnings = const [];
  String error = '';
  bool inspectedOk = false;
  List<String> queued = const [];
}

/// Holds every piece of state the pages show, and runs every workflow. The
/// pages render it and call into it; nothing here draws anything.
///
/// Workflows mirror the original GUI's messages, including its status texts,
/// so the same action leaves the same words on screen.
class AppModel extends ChangeNotifier {
  AppModel({
    required this.core,
    this.pollInterval = const Duration(milliseconds: 400),
    DateTime Function()? clock,
  }) : _clock = clock ?? DateTime.now;

  final DateTime Function() _clock;

  final CoreApi core;
  final Duration? pollInterval;
  Timer? _poll;
  bool _disposed = false;

  /// Mutations run one at a time. The core serialises them under its own
  /// lock; queuing them here keeps the bridge calls in the order the user
  /// made them and lets the status bar name the operation in progress.
  Future<void> _mutations = Future.value();

  /// The queue for preference saves. It is separate from [_mutations], so a
  /// setting does not wait for a running check or install (R6-03, R6-04).
  Future<void> _preferences = Future.value();

  /// The operation the user last asked to cancel. Whatever a cancelled run
  /// returns describes part of the file, so that run is never OK (QA3-009).
  String? _cancelledOpId;

  /// The status-bar note while a cancel is on its way.
  static const String _cancellingText = 'Cancelling…';

  /// What a stopped inspection says when the bridge gives no reason.
  static const String _stoppedInspectionText =
      'Inspection stopped before it finished. The file was not inspected.';

  AppPage page = AppPage.library;

  List<AppDto> library = const [];

  /// The core's error for the last library read, while the managed folder
  /// cannot be read. Null once a read succeeds (QA2-012).
  String? libraryError;
  List<DiscoveredDto> discovered = const [];
  bool loadingLibrary = false;
  String search = '';
  SortOrder sort = SortOrder.name;

  /// The app open on the Details page, if any.
  String? selectedUuid;
  String argumentsInput = '';
  String environmentInput = '';
  String sourceManagerInput = '';
  String sourceConfigInput = '';

  /// The core's refusal of the last update-source save, shown beside the field
  /// until the next save or another app is chosen (QA D-10).
  String? sourceError;

  final InspectState inspect = InspectState();

  List<UpdateOfferDto> updates = const [];
  List<UpdateProblem> checkFailures = const [];
  List<UpdateProblem> updateFailures = const [];

  /// The Tasks page, newest first.
  List<TaskDto> tasks = const [];

  SettingsDto? settings;
  String? version;
  String managedFolderInput = '';

  /// Why the typed or chosen managed folder was not saved, shown beside the
  /// field until the next save succeeds.
  String? managedFolderError;
  String maxBytesInput = '';

  LibraryFilter filter = LibraryFilter.all;
  DateTime? lastChecked;

  /// The apps a check has covered: every installed app at a full check, and
  /// the app named by a single check. An app outside this set is not checked
  /// yet, and never reads as up to date (QA D-03).
  final Set<String> _checkedUuids = {};
  InspectDto? inspected;

  BusyState? busy;
  StatusMessage? status;
  PendingDialog? dialog;

  // Derived views -----------------------------------------------------------

  AppDto? get selectedApp {
    final uuid = selectedUuid;
    if (uuid == null) {
      return null;
    }
    for (final app in library) {
      if (app.uuid == uuid) {
        return app;
      }
    }
    return null;
  }

  /// The library rows after search and sort, as the Library page lists them.
  List<AppDto> get visibleLibrary {
    final needle = search.trim().toLowerCase();
    final rows = library.where((app) {
      if (!matchesFilter(app)) {
        return false;
      }
      return needle.isEmpty ||
          app.name.toLowerCase().contains(needle) ||
          app.managedPath.toLowerCase().contains(needle) ||
          app.version.toLowerCase().contains(needle);
    }).toList();
    switch (sort) {
      case SortOrder.name:
        rows.sort(
          (a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()),
        );
      case SortOrder.version:
        rows.sort((a, b) {
          final byVersion = compareVersions(a.version, b.version);
          if (byVersion != 0) {
            return byVersion;
          }
          return a.name.toLowerCase().compareTo(b.name.toLowerCase());
        });
      case SortOrder.updatesFirst:
        bool hasUpdate(AppDto app) =>
            updates.any((offer) => offer.uuid == app.uuid);
        rows.sort((a, b) {
          final ua = hasUpdate(a);
          final ub = hasUpdate(b);
          if (ua != ub) {
            return ua ? -1 : 1;
          }
          return a.name.toLowerCase().compareTo(b.name.toLowerCase());
        });
    }
    return rows;
  }

  /// Discovered AppImages that are not registered yet, offered for adoption.
  List<DiscoveredDto> get adoptable =>
      discovered.where((found) => !found.managed).toList();

  bool get autostartEnabled => settings?.autostartEnabled ?? false;

  /// The managed folder as saved, for the sidebar and Settings.
  String get managedFolderPath => settings?.managedFolder ?? '';

  /// Installed AppImages' total size, for the sidebar's folder note.
  int get totalBytes => library.fold(0, (sum, app) => sum + app.sizeBytes);

  // Status and counts shown by the mockup ----------------------------------

  UpdateOfferDto? offerFor(String uuid) {
    for (final offer in updates) {
      if (offer.uuid == uuid) {
        return offer;
      }
    }
    return null;
  }

  UpdateProblem? failureFor(String uuid) {
    for (final failure in checkFailures) {
      if (failure.uuid == uuid) {
        return failure;
      }
    }
    return null;
  }

  /// Installed apps with an update waiting.
  int get updateCount =>
      library.where((app) => offerFor(app.uuid) != null).length;

  /// Whether a check has covered the app.
  bool isChecked(AppDto app) => _checkedUuids.contains(app.uuid);

  /// Installed apps no check has covered, and that have no offer or failure
  /// to show. The Updates page does not count them as up to date.
  int get notCheckedCount => library
      .where(
        (app) =>
            !isChecked(app) &&
            offerFor(app.uuid) == null &&
            failureFor(app.uuid) == null,
      )
      .length;

  /// A failed check, or a source that publishes no checksum with no update
  /// waiting. An update in progress is never counted here.
  bool needsAttention(AppDto app) =>
      failureFor(app.uuid) != null ||
      (app.reducedVerification && offerFor(app.uuid) == null);

  int get attentionCount => library.where(needsAttention).length;

  bool matchesFilter(AppDto app) => switch (filter) {
    LibraryFilter.all => true,
    LibraryFilter.updates => offerFor(app.uuid) != null,
    LibraryFilter.attention => needsAttention(app),
  };

  void setFilter(LibraryFilter next) {
    filter = next;
    _notify();
  }

  bool isFinishedTask(TaskDto task) =>
      task.state == TaskStateDto.succeeded ||
      task.state == TaskStateDto.failed ||
      task.state == TaskStateDto.cancelled;

  /// Tasks still queued, running or cancelling, newest first.
  List<TaskDto> get runningTasks => [
    for (final task in tasks)
      if (!isFinishedTask(task)) task,
  ];

  /// Tasks that have ended, newest first.
  List<TaskDto> get finishedTasks => [
    for (final task in tasks)
      if (isFinishedTask(task)) task,
  ];

  /// When a task ended, as the core recorded it. Null while it runs.
  DateTime? finishedAt(TaskDto task) => task.finishedAt > 0
      ? DateTime.fromMillisecondsSinceEpoch(task.finishedAt * 1000)
      : null;

  /// The update task running for an app, matched by its name, uuid or path.
  TaskDto? updateTaskFor(AppDto app) {
    for (final task in tasks) {
      if (task.kind == TaskKindDto.update &&
          task.state == TaskStateDto.running &&
          (task.target == app.name ||
              task.target == app.uuid ||
              task.target == app.managedPath)) {
        return task;
      }
    }
    return null;
  }

  /// The status a library row shows: what the app is doing, or what is wrong.
  RowStatus statusFor(AppDto app) {
    final task = updateTaskFor(app);
    if (task != null) {
      return RowStatus(
        'Updating · ${task.progress}%',
        StatusTone.accent,
        detail: app.reducedVerification ? 'Reduced verification' : '',
      );
    }
    if (failureFor(app.uuid) != null) {
      return const RowStatus(
        'Check failed',
        StatusTone.bad,
        detail: 'Status unknown, not up to date',
      );
    }
    final offer = offerFor(app.uuid);
    if (offer != null) {
      return RowStatus(
        'Update available',
        StatusTone.accent,
        detail: offer.running
            ? 'Confirm required while running'
            : (offer.reducedVerification ? 'Reduced verification' : ''),
      );
    }
    if (app.reducedVerification) {
      return const RowStatus(
        'Reduced verification',
        StatusTone.warn,
        detail: 'Source publishes no checksum',
      );
    }
    if (app.adopted) {
      return RowStatus(
        'Adopted',
        StatusTone.mute,
        detail: app.externalFolder
            ? 'Outside the managed folder'
            : 'Managed folder',
      );
    }
    if (!isChecked(app)) {
      return const RowStatus(
        'Not checked yet',
        StatusTone.mute,
        detail: 'Status unknown, not up to date',
      );
    }
    final checked = lastChecked;
    return RowStatus(
      'Up to date',
      StatusTone.ok,
      detail: checked == null ? '' : 'Checked ${clockLabel(checked)}',
    );
  }

  /// Cancels the update running for one app (the Updates page's Cancel).
  Future<void> cancelUpdate(AppDto app) async {
    final task = updateTaskFor(app);
    if (task != null) {
      await _cancel(task.id);
    }
  }

  /// The folder chooser's answer: the same checks and save as a typed path.
  Future<void> chooseManagedFolder(String path) {
    managedFolderInput = path;
    return applyManagedFolder();
  }

  // Lifecycle ---------------------------------------------------------------

  /// Load settings and the library, then open any files given on the command
  /// line. The first opens in Inspect; the rest wait behind it.
  Future<void> start(List<String> initialFiles) async {
    try {
      version = await core.appVersion();
    } on CoreError {
      // The About page shows no version rather than a wrong one.
    }
    await loadSettings(fromStart: true);
    await loadLibrary();
    await refreshTasks();
    if (initialFiles.isNotEmpty) {
      openPaths(initialFiles);
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _poll?.cancel();
    super.dispose();
  }

  void _notify() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  void _setStatus(Severity severity, String text) {
    status = StatusMessage(severity, text);
    _notify();
  }

  void setStatusError(String text) => _setStatus(Severity.error, text);

  // Operations --------------------------------------------------------------

  /// Runs `work` as a bridge operation: it appears in the status bar and the
  /// Tasks page while it runs, and the queue keeps it from overlapping another
  /// mutation.
  Future<T> _operation<T>(
    String title,
    Future<T> Function(String opId) work,
  ) async {
    final opId = OperationIds.next();
    busy = BusyState(opId, title);
    _startPolling();
    _notify();
    try {
      return await _serial(() => work(opId));
    } finally {
      if (busy?.opId == opId) {
        busy = null;
      }
      // A cancelled run must not leave "Cancelling…" behind (QA3-009). The
      // caller that knows the outcome may replace this note with its own.
      if (_cancelledOpId == opId && status?.text == _cancellingText) {
        status = StatusMessage(Severity.info, '$title cancelled');
      }
      _stopPolling();
      _notify();
      await refreshTasks();
    }
  }

  /// Runs a bridge call that takes no operation id. It is still queued behind
  /// other mutations, but it does not claim the status bar.
  Future<T> _quick<T>(Future<T> Function() work) => _serial(work);

  Future<T> _serial<T>(Future<T> Function() action) {
    final previous = _mutations;
    final done = Completer<void>();
    _mutations = done.future;
    return previous.then((_) => action()).whenComplete(() => done.complete());
  }

  /// Runs a preference save on its own lane. The core reads the settings file
  /// afresh for each call and takes no lock, so a preference need not wait for
  /// a check or an install to finish. Saves still run one at a time.
  Future<T> _savePreference<T>(Future<T> Function() action) {
    final previous = _preferences;
    final done = Completer<void>();
    _preferences = done.future;
    return previous.then((_) => action()).whenComplete(() => done.complete());
  }

  void _startPolling() {
    final interval = pollInterval;
    if (interval != null) {
      _poll ??= Timer.periodic(interval, (_) => refreshTasks());
    }
  }

  void _stopPolling() {
    _poll?.cancel();
    _poll = null;
  }

  Future<void> refreshTasks() async {
    try {
      final history = await core.listTasks();
      tasks = history.reversed.toList();
    } on CoreError {
      // The Tasks list is a view of the core's history; keep what was shown.
    }
    _notify();
  }

  Future<void> clearFinishedTasks() async {
    try {
      await core.clearFinishedTasks();
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    await refreshTasks();
  }

  void cancelBusy() {
    final current = busy;
    if (current == null) {
      return;
    }
    unawaited(_cancel(current.opId));
    _setStatus(Severity.info, _cancellingText);
  }

  Future<void> _cancel(String opId) async {
    _cancelledOpId = opId;
    try {
      await core.cancelTask(opId: opId);
    } on CoreError {
      // The operation may already have finished; nothing to cancel.
    }
    await refreshTasks();
  }

  // Navigation and library -------------------------------------------------

  void setPage(AppPage next) {
    page = next;
    _notify();
  }

  /// Gives apps that have no icon file another look at their AppImage, and
  /// shows what that finds. Quiet: it sets no status and reports no error,
  /// because an icon that cannot be found is only a letter tile. An app adopted
  /// before adoption read the file, or integrated before icons were found in
  /// every layout, gets its icon here without the person doing anything.
  Future<void> healIcons() async {
    try {
      final changed = await core.healLibraryIcons();
      if (changed <= 0) {
        return;
      }
      final loaded = await core.listLibrary();
      library = loaded.apps;
      discovered = loaded.discovered;
      _notify();
    } on CoreError {
      // The letter tiles stay; a refresh from an app's page can try again.
    }
  }

  Future<void> loadLibrary() async {
    loadingLibrary = true;
    _notify();
    try {
      final loaded = await core.listLibrary();
      library = loaded.apps;
      discovered = loaded.discovered;
      libraryError = null;
    } on CoreError catch (error) {
      libraryError = error.message;
      _setStatus(Severity.error, error.message);
    } finally {
      loadingLibrary = false;
      _notify();
    }
  }

  void setSearch(String text) {
    search = text;
    _notify();
  }

  void setSort(SortOrder order) {
    sort = order;
    _notify();
  }

  void selectApp(String uuid) {
    AppDto? app;
    for (final candidate in library) {
      if (candidate.uuid == uuid) {
        app = candidate;
      }
    }
    selectedUuid = uuid;
    if (app != null) {
      argumentsInput = app.arguments.join('\n');
      environmentInput = app.environment
          .map((pair) => '${pair.name}=${pair.value}')
          .join('\n');
      sourceManagerInput = app.updateManager;
      sourceConfigInput = app.updateConfig
          .map((pair) => '${pair.key}=${pair.value}')
          .join('\n');
    }
    sourceError = null;
    _notify();
  }

  void closeDetail() {
    selectedUuid = null;
    _notify();
  }

  void setArgumentsInput(String text) => argumentsInput = text;
  void setEnvironmentInput(String text) => environmentInput = text;
  void setSourceManagerInput(String text) => sourceManagerInput = text;
  void setSourceConfigInput(String text) => sourceConfigInput = text;

  Future<void> launch(String uuid) async {
    final app = _appNamed(uuid);
    if (app == null) {
      _setStatus(Severity.error, 'No such application');
      return;
    }
    try {
      await core.launchApp(uuid: uuid);
      _setStatus(Severity.success, 'Launched ${app.name}');
    } on CoreError catch (error) {
      _setStatus(Severity.error, 'Launch failed: ${error.message}');
    }
  }

  Future<void> reveal(String uuid) async {
    try {
      await core.revealApp(uuid: uuid);
    } on CoreError catch (error) {
      _setStatus(Severity.error, 'Cannot reveal: ${error.message}');
    }
  }

  void askRemove(String uuid, {required bool permanent}) {
    final app = _appNamed(uuid);
    if (app == null) {
      return;
    }
    dialog = RemoveDialog(
      uuid: uuid,
      name: app.name,
      path: app.managedPath,
      permanent: permanent,
    );
    _notify();
  }

  Future<void> confirmRemove() async {
    final pending = dialog;
    if (pending is! RemoveDialog) {
      return;
    }
    dialog = null;
    selectedUuid = null;
    _notify();
    try {
      final outcome = await _operation(
        'Removing',
        (opId) => core.removeApp(
          opId: opId,
          uuid: pending.uuid,
          permanent: pending.permanent,
        ),
      );
      _setStatus(
        outcome.ok ? Severity.success : Severity.error,
        outcome.ok ? 'Removed' : outcome.message,
      );
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    await loadLibrary();
  }

  Future<void> refreshMetadata(String uuid) async {
    try {
      final app = await _operation(
        'Refreshing metadata',
        (opId) => core.refreshMetadata(opId: opId, uuid: uuid),
      );
      _setStatus(Severity.success, 'Refreshed metadata for ${app.name}');
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    await loadLibrary();
  }

  void askAdopt(String path) {
    dialog = AdoptDialog(path: path);
    _notify();
  }

  Future<void> confirmAdopt() async {
    final pending = dialog;
    if (pending is! AdoptDialog) {
      return;
    }
    dialog = null;
    _notify();
    try {
      final app = await _operation(
        'Adopting',
        (opId) => core.adoptPath(opId: opId, path: pending.path),
      );
      _setStatus(Severity.success, 'Adopted ${app.name}');
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    await loadLibrary();
  }

  Future<void> saveArgumentsAndEnvironment() async {
    final uuid = selectedUuid;
    if (uuid == null) {
      return;
    }
    final arguments = parseArguments(argumentsInput);
    final environment = parseEnvironment(environmentInput);
    if (environment.rejected.isNotEmpty) {
      _setStatus(
        Severity.error,
        'Not a valid environment variable name: '
        '${environment.rejected.join(', ')}',
      );
      return;
    }
    try {
      await _quick(
        () => core.saveArgumentsAndEnvironment(
          uuid: uuid,
          arguments: arguments,
          environment: environment.pairs,
        ),
      );
      _setStatus(Severity.success, 'Saved arguments and environment');
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    await loadLibrary();
  }

  Future<void> applySource(String uuid) async {
    final manager = sourceManagerInput.trim();
    final config = parseConfigLines(sourceConfigInput);
    sourceError = null;
    try {
      await _quick(
        () =>
            core.setUpdateSource(uuid: uuid, manager: manager, config: config),
      );
      _setStatus(Severity.success, 'Update source saved');
    } on CoreError catch (error) {
      sourceError = error.message;
      _setStatus(Severity.error, error.message);
    }
    await loadLibrary();
  }

  Future<void> resetSource(String uuid) async {
    try {
      await _quick(() => core.unsetUpdateSource(uuid: uuid));
      _setStatus(Severity.success, 'Update source removed');
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    await loadLibrary();
  }

  AppDto? _appNamed(String uuid) {
    for (final app in library) {
      if (app.uuid == uuid) {
        return app;
      }
    }
    return null;
  }

  // Inspect and integrate --------------------------------------------------

  void setInspectPath(String path) {
    inspect.pathInput = path;
    _notify();
  }

  /// Files chosen in the picker or given on the command line. The first is
  /// inspected now and the rest queue behind it, as the original does.
  void openPaths(List<String> paths) {
    final normalised = paths
        .map(normaliseOpenTarget)
        .where((path) => path.trim().isNotEmpty)
        .toList();
    if (normalised.isEmpty) {
      return;
    }
    inspect.queued = normalised.sublist(1);
    page = AppPage.inspect;
    _notify();
    unawaited(startInspect(normalised.first));
  }

  /// A drop queues behind current work instead of replacing it.
  void dropFiles(List<String> paths) {
    final plan = planDrop(
      queued: inspect.queued,
      busy: busy != null,
      hasUnconfirmed: inspect.inspectedOk,
      incoming: paths.map(normaliseOpenTarget).toList(),
    );
    inspect.queued = plan.queue;
    page = AppPage.inspect;
    _notify();
    final start = plan.start;
    if (start != null) {
      unawaited(startInspect(start));
    }
  }

  /// A failure to open a file through the picker, shown on the Inspect page.
  void setInspectError(String message) {
    inspect.error = message;
    _notify();
  }

  void inspectFromInput() {
    final path = inspect.pathInput.trim();
    if (path.isEmpty) {
      inspect.error = 'Choose an AppImage file first';
      _notify();
      return;
    }
    unawaited(startInspect(normaliseOpenTarget(path)));
  }

  /// Forgets the inspected file, as the Inspect page's Cancel does.
  void clearInspect() {
    inspect
      ..pathInput = ''
      ..name = ''
      ..version = ''
      ..error = ''
      ..summary = const []
      ..warnings = const []
      ..inspectedOk = false;
    inspected = null;
    _notify();
  }

  /// The time the model's clock reads now (tests fix it).
  DateTime get now => _clock();

  /// Records when the last update check finished (the status bar's time).
  void markChecked(DateTime when) {
    lastChecked = when;
    _notify();
  }

  /// Cancels a task by its id (the Tasks page's Cancel).
  Future<void> cancelTaskById(String id) => _cancel(id);

  Future<void> startInspect(String path, {bool confirmUnsafe = false}) async {
    inspected = null;
    inspect
      ..pathInput = path
      ..error = ''
      ..summary = const []
      ..warnings = const []
      ..inspectedOk = false;
    _notify();
    String? opId;
    try {
      final result = await _operation('Inspecting', (id) {
        opId = id;
        return core.inspectPath(
          opId: id,
          path: path,
          confirmUnsafe: confirmUnsafe,
        );
      });
      final stopped = opId != null && _cancelledOpId == opId;
      if (stopped) {
        // The user stopped this run, so whatever came back describes part of
        // the file. It is an error, never a result (QA3-009).
        _refuseInspection(
          result.error.isEmpty ? _stoppedInspectionText : result.error,
          stopped: true,
        );
      } else if (!result.magicValid || result.error.isNotEmpty) {
        // The bridge reports a failure: its error shows, and nothing is claimed.
        _refuseInspection(
          result.error.isEmpty ? 'Not a valid AppImage' : result.error,
        );
      } else {
        inspected = result;
        inspect
          ..inspectedOk = true
          ..name = result.name
          ..version = result.version
          ..summary = inspectSummary(result)
          ..warnings = result.warnings;
      }
      if (!stopped && result.fallbackPending) {
        // The safe read failed: the state above is the safe result, with no
        // metadata. The file runs only if the user says so, for this file.
        dialog = FallbackConfirmDialog(
          fileName: p.basename(path),
          onRunAnyway: () => startInspect(path, confirmUnsafe: true),
        );
      }
    } on CoreError catch (error) {
      final stopped = opId != null && _cancelledOpId == opId;
      _refuseInspection(error.message, stopped: stopped);
    }
    _notify();
  }

  /// A run that failed or was stopped: its error shows, the file is not OK, and
  /// the result with its Integrate offer is withdrawn (QA3-009). A stopped run
  /// also replaces "Cancelling…" in the status line with its own note.
  void _refuseInspection(String message, {bool stopped = false}) {
    inspected = null;
    inspect
      ..error = message
      ..summary = const []
      ..warnings = const []
      ..inspectedOk = false;
    if (stopped) {
      _setStatus(Severity.error, message);
    }
  }

  void _takeNextQueued() {
    if (inspect.queued.isEmpty) {
      return;
    }
    final rest = [...inspect.queued];
    final next = rest.removeAt(0);
    inspect.queued = rest;
    unawaited(startInspect(next));
  }

  void integrateFromInspect() {
    final path = inspect.pathInput;
    if (path.isEmpty) {
      inspect.error = 'Choose an AppImage file first';
      _notify();
      return;
    }
    unawaited(_integrate(path, ConflictChoice.automatic, ''));
  }

  void keepBothAndIntegrate(String path) {
    dialog = null;
    _notify();
    unawaited(_integrate(path, ConflictChoice.keepBoth, ''));
  }

  void replaceAndIntegrate(String path, String uuid) {
    dialog = null;
    _notify();
    unawaited(_integrate(path, ConflictChoice.replace, uuid));
  }

  Future<void> _integrate(
    String path,
    ConflictChoice choice,
    String replaceUuid, {
    bool confirmUnsafe = false,
  }) async {
    final moveSource = settings?.moveSource ?? false;
    try {
      final outcome = await _operation(
        'Integrating',
        (opId) => core.integrateApp(
          opId: opId,
          sourcePath: path,
          conflict: choice,
          replaceUuid: replaceUuid,
          moveSource: moveSource,
          confirmUnsafe: confirmUnsafe,
        ),
      );
      if (outcome.fallbackPending) {
        // Nothing was installed. The file stays where it is unless the user
        // runs it anyway, for this file.
        dialog = FallbackConfirmDialog(
          fileName: p.basename(path),
          onRunAnyway: () =>
              _integrate(path, choice, replaceUuid, confirmUnsafe: true),
        );
        _notify();
      } else if (outcome.ok) {
        _setStatus(
          Severity.success,
          'Integrated ${outcome.app?.managedPath ?? path}',
        );
        inspect
          ..pathInput = ''
          ..error = ''
          ..summary = const []
          ..warnings = const []
          ..inspectedOk = false;
        await loadLibrary();
        _takeNextQueued();
      } else if (outcome.conflict && outcome.conflictUuid.isNotEmpty) {
        dialog = IntegrateConflictDialog(
          path: path,
          conflictName: inspect.name,
          incomingVersion: inspect.version,
          replaceUuid: outcome.conflictUuid,
          replaceLabel: outcome.conflictName,
          installedVersion: _appNamed(outcome.conflictUuid)?.version ?? '',
        );
        _notify();
      } else {
        _setStatus(Severity.error, 'Integrate failed: ${outcome.message}');
      }
    } on CoreError catch (error) {
      _setStatus(Severity.error, 'Integrate failed: ${error.message}');
    }
    _notify();
  }

  // Updates ------------------------------------------------------------------

  Future<void> checkUpdates() async {
    try {
      final scan = await _operation(
        'Checking for updates',
        (opId) => core.checkUpdates(opId: opId),
      );
      updates = scan.offers;
      lastChecked = _clock();
      _checkedUuids.addAll([for (final app in library) app.uuid]);
      checkFailures = [
        for (final failure in scan.failures)
          UpdateProblem(
            failure.name,
            failure.error,
            uuid: failure.uuid,
            timedOut: failure.timedOut,
          ),
      ];
      updateFailures = const [];
      if (checkFailures.isEmpty) {
        _setStatus(Severity.info, '${updates.length} update(s) available');
      } else {
        _setStatus(
          Severity.error,
          '${updates.length} update(s) available; '
          '${checkFailures.length} app(s) could not be checked',
        );
      }
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
  }

  Future<void> updateAll() async {
    if (updates.isEmpty) {
      _setStatus(Severity.info, 'Nothing to update');
      return;
    }
    try {
      final batch = await _operation(
        'Updating all',
        (opId) => core.applyAllUpdates(opId: opId, force: false),
      );
      final applied = batch.applied.length;
      final failed = batch.failed.length;
      final skipped = batch.skippedRunning.length;
      // Applied apps are no longer pending; the rest keep their offers.
      updates = [
        for (final offer in updates)
          if (!batch.applied.contains(offer.name)) offer,
      ];
      var text = 'Updated $applied; $failed failed';
      if (skipped > 0) {
        text += '; $skipped skipped (running)';
      }
      _setStatus(
        failed == 0 && skipped == 0 ? Severity.success : Severity.error,
        text,
      );
      updateFailures = [
        for (final item in batch.failed)
          UpdateProblem(
            item.name,
            item.error,
            uuid: item.uuid,
            timedOut: item.timedOut,
          ),
      ];
      checkFailures = [
        for (final item in batch.checkFailures)
          UpdateProblem(
            item.name,
            item.error,
            uuid: item.uuid,
            timedOut: item.timedOut,
          ),
      ];
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    await loadLibrary();
  }

  /// A running app is only updated after the user confirms that they know it
  /// will change underneath them.
  Future<void> updateOne(String uuid) async {
    UpdateOfferDto? offer;
    for (final candidate in updates) {
      if (candidate.uuid == uuid) {
        offer = candidate;
      }
    }
    if (offer?.running ?? false) {
      dialog = UpdateForceDialog(
        uuid: uuid,
        name: _appNamed(uuid)?.name ?? uuid,
      );
      _notify();
      return;
    }
    await _startUpdate(uuid, force: false);
  }

  Future<void> confirmForceUpdate() async {
    final pending = dialog;
    if (pending is! UpdateForceDialog) {
      return;
    }
    dialog = null;
    _notify();
    await _startUpdate(pending.uuid, force: true);
  }

  Future<void> _startUpdate(String uuid, {required bool force}) async {
    final name = _appNamed(uuid)?.name ?? uuid;
    try {
      final outcome = await _operation(
        'Updating',
        (opId) => core.applyUpdate(opId: opId, uuid: uuid, force: force),
      );
      if (outcome.ok) {
        // The update has landed: its offer is no longer pending, so the arrow
        // and "Update available" go until the next check finds a newer one.
        updates = [
          for (final offer in updates)
            if (offer.uuid != uuid) offer,
        ];
        _setStatus(Severity.success, 'Updated $name');
      } else if (outcome.running && !force) {
        // The cached offer can be stale: the app may have started since the
        // last check. The core refused, so ask the same question the dialog
        // asks for a running app, instead of showing the core's CLI text.
        dialog = UpdateForceDialog(uuid: uuid, name: name);
        _notify();
      } else {
        _setStatus(Severity.error, outcome.message);
      }
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    await loadLibrary();
  }

  /// The result of "Check for update" for each app checked this session.
  final Map<String, UpdateCheckDto> _checks = {};

  UpdateCheckDto? checkFor(String uuid) => _checks[uuid];

  /// "Check for update" on one app. It reports what the source offers and
  /// changes nothing installed. Applying is the separate Update action.
  Future<void> checkOne(String uuid) async {
    final app = _appNamed(uuid);
    if (app == null) {
      return;
    }
    try {
      final result = await _quick(() => core.checkOneUpdate(uuid: uuid));
      _checks[uuid] = result;
      _checkedUuids.add(uuid);
      final others = [
        for (final offer in updates)
          if (offer.uuid != uuid) offer,
      ];
      final otherProblems = [
        for (final problem in checkFailures)
          if (problem.uuid != uuid) problem,
      ];
      if (result.error.isNotEmpty) {
        updates = others;
        checkFailures = [
          ...otherProblems,
          UpdateProblem(
            app.name,
            result.error,
            uuid: uuid,
            timedOut: result.timedOut,
          ),
        ];
        _setStatus(
          Severity.error,
          '${app.name}: '
          '${checkFailureText(result.error, timedOut: result.timedOut)}',
        );
      } else {
        checkFailures = otherProblems;
        if (result.availableVersion.isEmpty) {
          updates = others;
          _setStatus(Severity.info, '${app.name} is up to date');
        } else {
          updates = [
            ...others,
            UpdateOfferDto(
              uuid: uuid,
              name: app.name,
              currentVersion: result.currentVersion,
              availableVersion: result.availableVersion,
              manager: app.updateManager,
              url: '',
              downloadSize: result.downloadSize,
              digest: '',
              reducedVerification: result.reducedVerification,
              digestAlgo: '',
              embeddedSource: app.embeddedUpdate,
              running: app.running,
            ),
          ];
          _setStatus(
            Severity.info,
            '${app.name}: ${result.availableVersion} available',
          );
        }
      }
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    _notify();
  }

  // Settings -----------------------------------------------------------------

  /// Reads the saved preferences. The text inputs are only filled on the first
  /// read, so a refresh never overwrites what the user is typing.
  Future<void> loadSettings({bool fromStart = false}) async {
    try {
      final loaded = await core.loadSettings();
      settings = loaded;
      if (fromStart) {
        managedFolderInput = loaded.managedFolder;
        maxBytesInput = maxAppimageMbFor(loaded.maxAppimageBytes).toString();
        final loadError = loaded.loadError;
        if (loadError != null) {
          status = StatusMessage(Severity.error, loadError);
        }
      }
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    _notify();
  }

  Future<bool> _save(
    SettingsPatchDto patch,
    String what, {
    void Function(String message)? onError,
  }) async {
    try {
      settings = await _savePreference(() => core.saveSettings(patch: patch));
      _notify();
      return true;
    } on CoreError catch (error) {
      onError?.call(error.message);
      _setStatus(Severity.error, 'Could not save $what: ${error.message}');
      return false;
    }
  }

  /// The unsafe extraction fallback's switch. Turning it on asks first; turning
  /// it off saves at once.
  Future<void> setUnsafeFallback(bool value) async {
    if (value) {
      dialog = const UnsafeFallbackDialog();
      _notify();
      return;
    }
    await _save(
      const SettingsPatchDto(unsafeExtractionFallback: false),
      'the unsafe extraction fallback',
    );
  }

  /// Turn on, from the warning. Only this saves the setting on.
  Future<void> confirmUnsafeFallback() async {
    if (dialog is! UnsafeFallbackDialog) {
      return;
    }
    dialog = null;
    _notify();
    await _save(
      const SettingsPatchDto(unsafeExtractionFallback: true),
      'the unsafe extraction fallback',
    );
  }

  /// Run anyway, from the per-file question. Only this runs the AppImage.
  Future<void> confirmFallback() async {
    final pending = dialog;
    if (pending is! FallbackConfirmDialog) {
      return;
    }
    dialog = null;
    _notify();
    await pending.onRunAnyway();
  }

  Future<void> setAutostart(bool enabled) async {
    // A login check needs "Check in the background" on. The control is
    // disabled then, and the core refuses the request too.
    if (enabled && !(settings?.backgroundUpdateChecks ?? false)) {
      _setStatus(
        Severity.error,
        'Turn on Check in the background before adding a login check.',
      );
      return;
    }
    try {
      await _savePreference(() => core.setAutostart(enabled: enabled));
      await loadSettings();
      _setStatus(
        Severity.success,
        enabled
            ? 'Update checks will run at login (notify only)'
            : 'Login update checks off',
      );
    } on CoreError catch (error) {
      _setStatus(Severity.error, 'Autostart failed: ${error.message}');
    }
  }

  Future<void> setBackgroundUpdateChecks(bool enabled) async {
    // Turning checks off also removes the login entry. Otherwise the login
    // check keeps contacting update endpoints after the user opted out.
    if (!enabled) {
      try {
        await _savePreference(() => core.setAutostart(enabled: false));
      } on CoreError catch (error) {
        _setStatus(
          Severity.error,
          'Cannot disable background checks: ${error.message}',
        );
        return;
      }
    }
    final saved = await _save(
      SettingsPatchDto(backgroundUpdateChecks: enabled),
      'background update checks',
    );
    if (saved) {
      await loadSettings();
      _setStatus(
        Severity.success,
        enabled
            ? 'Background update checks on (notify only)'
            : 'Background update checks off',
      );
    }
  }

  Future<void> setMoveSource(bool enabled) =>
      _save(SettingsPatchDto(moveSource: enabled), 'the copy/move preference');

  Future<void> setManageOutsideFolder(bool enabled) async {
    final saved = await _save(
      SettingsPatchDto(manageOutsideFolder: enabled),
      'outside-folder discovery',
    );
    if (saved) {
      await loadLibrary();
    }
  }

  Future<void> setTerminalOmitSuffix(bool enabled) => _save(
    SettingsPatchDto(terminalOmitSuffix: enabled),
    'the terminal-suffix preference',
  );

  Future<void> setDebugLogging(bool enabled) => _save(
    SettingsPatchDto(debugLogging: enabled),
    'the diagnostics preference',
  );

  Future<void> setAppearance(AppearanceChoice choice) =>
      _save(SettingsPatchDto(appearance: choice), 'the appearance preference');

  void setManagedFolderInput(String text) {
    managedFolderInput = text;
    _notify();
  }

  /// The chooser and the typed entry both go through [applyManagedFolder], with
  /// the same checks.
  Future<void> applyManagedFolder() async {
    final text = managedFolderInput.trim();
    if (text.isEmpty) {
      _refuseFolder('Managed folder cannot be empty');
      return;
    }
    if (!p.isAbsolute(text)) {
      _refuseFolder('Managed folder must be an absolute path');
      return;
    }
    final saved = await _save(
      SettingsPatchDto(managedFolder: text),
      'the managed folder',
      onError: (message) => managedFolderError = message,
    );
    if (saved) {
      managedFolderError = null;
      _setStatus(Severity.success, 'Managed folder updated');
      await loadLibrary();
    }
  }

  /// A path the app will not save. The reason shows inline and on the status
  /// line (QA owner decision: the typed entry).
  void _refuseFolder(String message) {
    managedFolderError = message;
    _setStatus(Severity.error, message);
  }

  void setMaxBytesInput(String text) {
    maxBytesInput = text;
    _notify();
  }

  Future<void> applyMaxBytes() async {
    final int bytes;
    try {
      bytes = parseMaxAppimageMb(maxBytesInput);
    } on FormatException catch (error) {
      _setStatus(Severity.error, error.message);
      return;
    }
    final saved = await _save(
      SettingsPatchDto(maxAppimageBytes: bytes),
      'the maximum AppImage size',
    );
    if (saved) {
      maxBytesInput = maxAppimageMbFor(bytes).toString();
      _setStatus(Severity.success, 'Maximum size updated');
    }
  }

  // Dialogs and status -------------------------------------------------------

  /// Shows a pending question, notifying the window.
  void showDialog(PendingDialog next) {
    dialog = next;
    _notify();
  }

  void dismissDialog() {
    dialog = null;
    _notify();
  }

  void dismissStatus() {
    status = null;
    _notify();
  }
}
