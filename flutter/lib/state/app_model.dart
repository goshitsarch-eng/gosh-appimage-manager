import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:path/path.dart' as p;

/// The six pages of the sidebar, in the order the original lists them.
enum AppPage { library, inspect, updates, tasks, settings, about }

enum SortOrder { name, version, updatesFirst }

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
    required this.replaceUuid,
    required this.replaceLabel,
  });

  final String path;
  final String conflictName;

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

class UnsafeExtractDialog extends PendingDialog {
  const UnsafeExtractDialog();
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

/// One app whose update could not be checked, or one update that did not
/// apply. Both are listed on the Updates page, never reported as "up to date".
class UpdateProblem {
  const UpdateProblem(this.name, this.error);

  final String name;
  final String error;
}

/// The Inspect page's state: the path being looked at, what it found, and
/// the files queued behind it.
class InspectState {
  String pathInput = '';
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
  });

  final CoreApi core;
  final Duration? pollInterval;
  Timer? _poll;
  bool _disposed = false;

  /// Mutations run one at a time. The core serialises them under its own
  /// lock; queuing them here keeps the bridge calls in the order the user
  /// made them and lets the status bar name the operation in progress.
  Future<void> _mutations = Future.value();

  AppPage page = AppPage.library;

  List<AppDto> library = const [];
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

  final InspectState inspect = InspectState();

  List<UpdateOfferDto> updates = const [];
  List<UpdateProblem> checkFailures = const [];
  List<UpdateProblem> updateFailures = const [];

  /// The Tasks page, newest first.
  List<TaskDto> tasks = const [];

  SettingsDto? settings;
  String? version;
  String managedFolderInput = '';
  String maxBytesInput = '';

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
        rows.sort((a, b) => a.version.compareTo(b.version));
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
    _setStatus(Severity.info, 'Cancelling…');
  }

  Future<void> _cancel(String opId) async {
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

  Future<void> loadLibrary() async {
    loadingLibrary = true;
    _notify();
    try {
      final loaded = await core.listLibrary();
      library = loaded.apps;
      discovered = loaded.discovered;
    } on CoreError catch (error) {
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
    }
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
    try {
      await _quick(
        () =>
            core.setUpdateSource(uuid: uuid, manager: manager, config: config),
      );
      _setStatus(Severity.success, 'Update source saved');
    } on CoreError catch (error) {
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

  Future<void> startInspect(String path) async {
    inspect
      ..pathInput = path
      ..error = ''
      ..summary = const []
      ..warnings = const []
      ..inspectedOk = false;
    _notify();
    try {
      final result = await _operation(
        'Inspecting',
        (opId) => core.inspectPath(opId: opId, path: path),
      );
      if (!result.magicValid) {
        inspect.error = result.error.isEmpty
            ? 'Not a valid AppImage'
            : result.error;
      } else {
        inspect
          ..inspectedOk = true
          ..summary = inspectSummary(result)
          ..warnings = result.warnings;
      }
    } on CoreError catch (error) {
      inspect.error = error.message;
    }
    _notify();
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
    String replaceUuid,
  ) async {
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
        ),
      );
      if (outcome.ok) {
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
          conflictName: inspect.pathInput,
          replaceUuid: outcome.conflictUuid,
          replaceLabel: outcome.conflictName,
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
      checkFailures = [
        for (final failure in scan.failures)
          UpdateProblem(failure.name, failure.error),
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
      var text = 'Updated $applied; $failed failed';
      if (skipped > 0) {
        text += '; $skipped skipped (running)';
      }
      _setStatus(
        failed == 0 && skipped == 0 ? Severity.success : Severity.error,
        text,
      );
      updateFailures = [
        for (final item in batch.failed) UpdateProblem(item.name, item.error),
      ];
      checkFailures = [
        for (final item in batch.checkFailures)
          UpdateProblem(item.name, item.error),
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
        _setStatus(Severity.success, 'Updated $name');
      } else {
        _setStatus(Severity.error, outcome.message);
      }
    } on CoreError catch (error) {
      _setStatus(Severity.error, error.message);
    }
    await loadLibrary();
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

  Future<bool> _save(SettingsPatchDto patch, String what) async {
    try {
      settings = await _quick(() => core.saveSettings(patch: patch));
      _notify();
      return true;
    } on CoreError catch (error) {
      _setStatus(Severity.error, 'Could not save $what: ${error.message}');
      return false;
    }
  }

  Future<void> setAutostart(bool enabled) async {
    try {
      await _quick(() => core.setAutostart(enabled: enabled));
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
        await _quick(() => core.setAutostart(enabled: false));
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

  /// Turning the unsafe fallback on needs a confirmation. Turning it off does
  /// not.
  Future<void> requestUnsafeFallback(bool enabled) async {
    if (enabled) {
      dialog = const UnsafeExtractDialog();
      _notify();
      return;
    }
    await _save(
      SettingsPatchDto(unsafeExtractionFallback: false),
      'the extraction fallback',
    );
  }

  Future<void> confirmUnsafeFallback() async {
    dialog = null;
    _notify();
    final saved = await _save(
      SettingsPatchDto(unsafeExtractionFallback: true),
      'the extraction fallback',
    );
    if (saved) {
      _setStatus(
        Severity.info,
        'Unsafe extraction fallback on; each file still needs confirming',
      );
    }
  }

  Future<void> setAppearance(AppearanceChoice choice) =>
      _save(SettingsPatchDto(appearance: choice), 'the appearance preference');

  void setManagedFolderInput(String text) {
    managedFolderInput = text;
    _notify();
  }

  Future<void> applyManagedFolder() async {
    final text = managedFolderInput.trim();
    if (text.isEmpty) {
      _setStatus(Severity.error, 'Managed folder cannot be empty');
      return;
    }
    if (!p.isAbsolute(text)) {
      _setStatus(Severity.error, 'Managed folder must be an absolute path');
      return;
    }
    final saved = await _save(
      SettingsPatchDto(managedFolder: text),
      'the managed folder',
    );
    if (saved) {
      _setStatus(Severity.success, 'Managed folder updated');
      await loadLibrary();
    }
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

  void dismissDialog() {
    dialog = null;
    _notify();
  }

  void dismissStatus() {
    status = null;
    _notify();
  }
}
