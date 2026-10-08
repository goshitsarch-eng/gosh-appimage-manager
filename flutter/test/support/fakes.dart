import 'dart:async';

import 'package:gosh_appimage_flutter/core/core_api.dart';

/// Builders for the bridge's data types, and the mockup's sample data.
///
/// [mockupCore] is the library the design frames show: seven apps (Quill Notes
/// running with an update, Atlas Viewer with reduced verification, Brisk
/// Terminal up to date, Tidemark Photos updating, Cinder Chat up to date,
/// Orbit Mail with a failed check, and Ledgerline adopted from outside the
/// managed folder), three finished tasks and one running update.
AppDto fakeApp({
  String uuid = 'u1',
  String name = 'Demo',
  String version = '1.0.0',
  String managedPath = '/home/someone/AppImages/Demo.AppImage',
  bool running = false,
  bool adopted = false,
  bool externalFolder = false,
  bool reducedVerification = false,
  int sizeBytes = 4096,
  String appType = 'Type 2',
  String architecture = 'x86_64',
  String sha256 = '',
  String desktopId = 'gosh-appimage-demo.desktop',
  List<String> arguments = const [],
  List<EnvVarDto> environment = const [],
  String updateManager = '',
  List<KeyValueDto> updateConfig = const [],
  String embeddedUpdate = '',
  int integratedAt = 0,
  String integratedFolder = '',
  String iconPath = '',
}) => AppDto(
  uuid: uuid,
  name: name,
  version: version,
  comment: '',
  managedPath: managedPath,
  desktopId: desktopId,
  desktopPath: '/home/someone/.local/share/applications/$desktopId',
  iconPath: iconPath,
  sha256: sha256.isEmpty ? 'ab' * 32 : sha256,
  appType: appType,
  architecture: architecture,
  sizeBytes: sizeBytes,
  arguments: arguments,
  environment: environment,
  updateManager: updateManager,
  updateConfig: updateConfig,
  embeddedUpdate: embeddedUpdate,
  lastUpdateCheck: '',
  availableVersion: '',
  availableUrl: '',
  availableSize: 0,
  updateAvailable: false,
  digest: '',
  reducedVerification: reducedVerification,
  running: running,
  externalFolder: externalFolder,
  owned: true,
  adopted: adopted,
  website: '',
  terminal: false,
  categories: const [],
  mimeTypes: const [],
  startupWmClass: '',
  actionNames: const [],
  integratedAt: integratedAt,
  integratedFolder: integratedFolder,
);

InspectDto fakeInspect({
  bool magicValid = true,
  String error = '',
  List<String> warnings = const [],
  bool alreadyManaged = false,
  String name = 'Demo',
  String version = '1.0.0',
  String embeddedUpdate = '',
  String path = '/tmp/Demo.AppImage',
  int sizeBytes = 2048,
  String sha256 = '',
  List<String> categories = const [],
  String iconName = '',
  bool architectureSupported = true,
  bool fallbackPending = false,

  /// False gives a result with no hash, as a stopped run has (QA3-009).
  bool hashed = true,
}) => InspectDto(
  path: path,
  sizeBytes: sizeBytes,
  sha256: !hashed ? '' : (sha256.isEmpty ? 'cd' * 32 : sha256),
  appType: 'Type 2',
  architecture: 'x86_64',
  magicValid: magicValid,
  architectureSupported: architectureSupported,
  fallbackPending: fallbackPending,
  truncated: false,
  name: name,
  version: version,
  comment: '',
  iconName: iconName,
  iconFormat: '',
  iconBytes: null,
  categories: categories,
  mimeTypes: const [],
  terminal: false,
  website: '',
  startupWmClass: '',
  actionNames: const [],
  embeddedUpdate: embeddedUpdate,
  embeddedManagerHint: '',
  warnings: warnings,
  error: error,
  alreadyManaged: alreadyManaged,
  existingUuid: '',
  conflictStatus: '',
  conflictingUuid: '',
  conflictingName: '',
  needsConflictDecision: false,
  canReplace: false,
  plannedTarget: '',
  extractorUsed: '',
  usedUnsafeFallback: false,
);

OutcomeDto fakeOutcome({
  bool ok = true,
  String message = '',
  bool conflict = false,
  String conflictUuid = '',
  String conflictName = '',
  AppDto? app,
  bool running = false,
  bool fallbackPending = false,
}) => OutcomeDto(
  ok: ok,
  partial: false,
  message: message,
  conflict: conflict,
  running: running,
  app: app,
  rolledBack: const [],
  sourceRemoved: false,
  conflictUuid: conflictUuid,
  conflictName: conflictName,
  fallbackPending: fallbackPending,
);

SettingsDto fakeSettings({
  String managedFolder = '/home/someone/AppImages',
  AppearanceChoice appearance = AppearanceChoice.system,
  bool autostartEnabled = false,
  bool backgroundUpdateChecks = false,
  bool moveSource = false,
  bool manageOutsideFolder = false,
  String? loadError,
}) => SettingsDto(
  managedFolder: managedFolder,
  moveSource: moveSource,
  manageOutsideFolder: manageOutsideFolder,
  terminalOmitSuffix: false,
  backgroundUpdateChecks: backgroundUpdateChecks,
  unsafeExtractionFallback: false,
  debugLogging: false,
  appearance: appearance,
  maxAppimageBytes: 8192 * 1024 * 1024,
  loadError: loadError,
  autostartEnabled: autostartEnabled,
);

/// A copy of [base] with a save applied, the way the core stores one. Turning
/// background checks off also removes the login entry, and a login entry can
/// exist only while background checks are on.
SettingsDto withSettings(
  SettingsDto base, {
  SettingsPatchDto? patch,
  bool? autostartEnabled,
}) {
  final background =
      patch?.backgroundUpdateChecks ?? base.backgroundUpdateChecks;
  return SettingsDto(
    managedFolder: patch?.managedFolder ?? base.managedFolder,
    moveSource: patch?.moveSource ?? base.moveSource,
    manageOutsideFolder: patch?.manageOutsideFolder ?? base.manageOutsideFolder,
    terminalOmitSuffix: patch?.terminalOmitSuffix ?? base.terminalOmitSuffix,
    backgroundUpdateChecks: background,
    unsafeExtractionFallback:
        patch?.unsafeExtractionFallback ?? base.unsafeExtractionFallback,
    debugLogging: patch?.debugLogging ?? base.debugLogging,
    appearance: patch?.appearance ?? base.appearance,
    maxAppimageBytes: patch?.maxAppimageBytes ?? base.maxAppimageBytes,
    loadError: base.loadError,
    autostartEnabled: background && (autostartEnabled ?? base.autostartEnabled),
  );
}

UpdateOfferDto fakeOffer({
  String uuid = 'u1',
  String name = 'Demo',
  String currentVersion = '1.0.0',
  String availableVersion = '1.1.0',
  String manager = 'github',
  bool running = false,
  bool reducedVerification = false,
  int downloadSize = 0,
}) => UpdateOfferDto(
  uuid: uuid,
  name: name,
  currentVersion: currentVersion,
  availableVersion: availableVersion,
  manager: manager,
  url: 'https://example.invalid/$name-$availableVersion.AppImage',
  downloadSize: downloadSize,
  digest: '',
  reducedVerification: reducedVerification,
  digestAlgo: '',
  embeddedSource: '',
  running: running,
);

/// A failed check. The default is the core's timeout text, with `timedOut` set
/// as the core sets it; the Updates page turns that into the mockup sentence.
UpdateFailureDto fakeFailure({
  String uuid = 'u1',
  String name = 'Demo',
  String error = 'Network request failed: timed out',
  bool timedOut = true,
}) => UpdateFailureDto(
  uuid: uuid,
  name: name,
  manager: 'github',
  error: error,
  timedOut: timedOut,
);

TaskDto fakeTask({
  String id = 'op-1',
  TaskKindDto kind = TaskKindDto.checkUpdate,
  TaskStateDto state = TaskStateDto.succeeded,
  String title = 'Checking for updates',
  String target = '',
  int progress = 100,
  String statusText = '',
  String error = '',
  int startedAt = 0,
  int finishedAt = 0,
  String fromVersion = '',
  String toVersion = '',
  int phaseIndex = 0,
  String phase = '',
  int bytesDone = 0,
  int bytesTotal = 0,
  bool permanent = false,
}) => TaskDto(
  id: id,
  kind: kind,
  state: state,
  title: title,
  target: target,
  progress: progress,
  statusText: statusText,
  error: error,
  retryable: false,
  startedAt: startedAt,
  finishedAt: finishedAt,
  fromVersion: fromVersion,
  toVersion: toVersion,
  phaseIndex: phaseIndex,
  phase: phase,
  bytesDone: bytesDone,
  bytesTotal: bytesTotal,
  permanent: permanent,
);

/// Unix seconds for a local wall-clock time, the form the core stores.
int unixSeconds(DateTime when) => when.millisecondsSinceEpoch ~/ 1000;

UpdateCheckDto fakeCheck({
  String uuid = 'u1',
  String currentVersion = '1.0.0',
  String availableVersion = '',
  int downloadSize = 0,
  bool reducedVerification = false,
  String error = '',
  bool timedOut = false,
}) => UpdateCheckDto(
  uuid: uuid,
  currentVersion: currentVersion,
  availableVersion: availableVersion,
  downloadSize: downloadSize,
  reducedVerification: reducedVerification,
  error: error,
  timedOut: timedOut,
);

DiscoveredDto fakeDiscovered({
  String path = '/home/someone/Downloads/Found-1.0.AppImage',
  String name = 'Found',
  bool managed = false,
  bool externalDesktopEntry = false,
}) => DiscoveredDto(
  path: path,
  name: name,
  managed: managed,
  uuid: '',
  externalDesktopEntry: externalDesktopEntry,
  desktopPath: externalDesktopEntry
      ? '/home/someone/.local/share/applications/found.desktop'
      : '',
);

/// The mockup's seven sample apps, in the order the component lists them.
/// Sizes sum to the 512 MB the sidebar shows.
List<AppDto> mockupApps() => [
  fakeApp(
    uuid: 'quill',
    name: 'Quill Notes',
    version: '2.4.1',
    managedPath: '/home/someone/AppImages/Quill-Notes-x86_64.AppImage',
    running: true,
    integratedAt: unixSeconds(DateTime(2026, 9, 2, 10)),
    integratedFolder: '/home/someone/Downloads',
    sizeBytes: (84.2 * 1024 * 1024).round(),
    sha256: '3f9a6c0e2b7d41f8a95c0d6e13b7a2f4c8e90d15b6a3f27e4c1d8b9a05e6f2c3',
    desktopId: 'gosh-appimage-8f0b2d1e-4c7a-4e2b-9d15-b3a6f0c2e871.desktop',
    arguments: const ['--ozone-platform=wayland'],
    environment: const [EnvVarDto(name: 'QT_QPA_PLATFORM', value: 'wayland')],
    updateManager: 'github',
    updateConfig: const [
      KeyValueDto(key: 'repo', value: 'example-org/quill-notes'),
    ],
  ),
  fakeApp(
    uuid: 'atlas',
    name: 'Atlas Viewer',
    version: '0.9.3',
    managedPath: '/home/someone/AppImages/Atlas-Viewer-x86_64.AppImage',
    reducedVerification: true,
    sizeBytes: 52 * 1024 * 1024,
    appType: 'Type 2',
  ),
  fakeApp(
    uuid: 'brisk',
    name: 'Brisk Terminal',
    version: '1.2.0',
    managedPath: '/home/someone/AppImages/Brisk-Terminal-x86_64.AppImage',
    sizeBytes: 58 * 1024 * 1024,
  ),
  fakeApp(
    uuid: 'tidemark',
    name: 'Tidemark Photos',
    version: '3.1.0',
    managedPath: '/home/someone/AppImages/Tidemark-Photos-x86_64.AppImage',
    reducedVerification: true,
    sizeBytes: 66 * 1024 * 1024,
    updateManager: 'github',
    updateConfig: const [
      KeyValueDto(key: 'repo', value: 'example-org/tidemark'),
    ],
  ),
  fakeApp(
    uuid: 'cinder',
    name: 'Cinder Chat',
    version: '0.14.2',
    managedPath: '/home/someone/AppImages/Cinder-Chat-x86_64.AppImage',
    sizeBytes: (61.8 * 1024 * 1024).round(),
  ),
  fakeApp(
    uuid: 'orbit',
    name: 'Orbit Mail',
    version: '2.0.0',
    managedPath: '/home/someone/AppImages/Orbit-Mail-x86_64.AppImage',
    sizeBytes: 80 * 1024 * 1024,
    updateManager: 'codeberg',
    updateConfig: const [KeyValueDto(key: 'repo', value: 'example/orbit-mail')],
  ),
  fakeApp(
    uuid: 'ledger',
    name: 'Ledgerline',
    version: '5.6.1',
    managedPath: '/home/someone/Downloads/Ledgerline-5.6.1-x86_64.AppImage',
    adopted: true,
    externalFolder: true,
    sizeBytes: 110 * 1024 * 1024,
  ),
];

/// The mockup's library state: Quill Notes and Tidemark Photos have updates,
/// Orbit Mail's check failed, and Tidemark's update is running at 62%.
FakeCore mockupCore() {
  final core = FakeCore(
    library: LibraryDto(apps: mockupApps(), discovered: const []),
  );
  core.runningUuids.add('quill');
  core.scan = UpdateScanDto(
    offers: [
      fakeOffer(
        uuid: 'quill',
        name: 'Quill Notes',
        currentVersion: '2.4.1',
        availableVersion: '2.5.0',
        running: true,
      ),
      fakeOffer(
        uuid: 'tidemark',
        name: 'Tidemark Photos',
        currentVersion: '3.1.0',
        availableVersion: '3.2.0',
        reducedVerification: true,
      ),
    ],
    failures: [fakeFailure(uuid: 'orbit', name: 'Orbit Mail')],
    skipped: 0,
    checked: 7,
    cancelled: false,
  );
  // Oldest first, as the core lists them; the Tasks page shows newest first.
  // The times and versions are the ones the mockup prints.
  core.tasks.addAll([
    fakeTask(
      id: 'op-remove',
      kind: TaskKindDto.remove,
      state: TaskStateDto.succeeded,
      title: 'Removing',
      target: 'Old Notes',
      fromVersion: '1.0',
      finishedAt: unixSeconds(DateTime(2026, 10, 6, 17, 40)),
    ),
    fakeTask(
      id: 'op-brisk',
      kind: TaskKindDto.update,
      state: TaskStateDto.succeeded,
      title: 'Updating',
      target: 'Brisk Terminal',
      fromVersion: '1.1.4',
      toVersion: '1.2.0',
      finishedAt: unixSeconds(DateTime(2026, 10, 7, 8, 31)),
    ),
    fakeTask(
      id: 'op-cinder',
      kind: TaskKindDto.integrate,
      state: TaskStateDto.succeeded,
      title: 'Integrating',
      target: 'Cinder Chat',
      toVersion: '0.14.2',
      finishedAt: unixSeconds(DateTime(2026, 10, 7, 8, 52)),
    ),
    fakeTask(
      id: 'op-tidemark',
      kind: TaskKindDto.update,
      state: TaskStateDto.running,
      title: 'Updating',
      target: 'Tidemark Photos',
      progress: 62,
      fromVersion: '3.1.0',
      toVersion: '3.2.0',
      phaseIndex: 1,
      phase: 'Download',
      // 41.2 and 66.0 MiB, the mockup's byte line.
      bytesDone: 43201331,
      bytesTotal: 69206016,
      startedAt: unixSeconds(DateTime(2026, 10, 7, 9, 10)),
    ),
  ]);
  return core;
}

/// A core whose managed folder exists but cannot be read: listing fails with
/// the core's own error, which the Library shows (QA2-012).
FakeCore unreadableFolderCore(String message) => mockupCore()
  ..failures['listLibrary'] = CoreError(
    kind: ErrorKind.permission,
    message: message,
    details: '',
  );

/// A core double. Every call is recorded in [calls]; a method named in
/// [failures] throws its error instead of answering.
class FakeCore implements CoreApi {
  FakeCore({LibraryDto? library, SettingsDto? settings})
    : library = library ?? LibraryDto(apps: const [], discovered: const []),
      settings = settings ?? fakeSettings();

  LibraryDto library;
  SettingsDto settings;
  final List<String> calls = [];
  final Map<String, CoreError> failures = {};
  final List<TaskDto> tasks = [];

  /// Calls named here wait until their completer completes, so a test can keep
  /// a bridge operation open while it acts on the window (round 6).
  final Map<String, Completer<void>> holds = {};

  Future<void> _hold(String method) async {
    final gate = holds[method];
    if (gate != null) {
      await gate.future;
    }
  }

  OutcomeDto integrateResult = fakeOutcome();

  /// The answers to a request made with confirmUnsafe true (round 4).
  InspectDto inspectUnsafeResult = fakeInspect();
  OutcomeDto integrateUnsafeResult = fakeOutcome();
  OutcomeDto removeResult = fakeOutcome();
  OutcomeDto updateResult = fakeOutcome();
  InspectDto inspectResult = fakeInspect();
  UpdateScanDto scan = UpdateScanDto(
    offers: const [],
    failures: const [],
    skipped: 0,
    checked: 0,
    cancelled: false,
  );
  BatchDto batch = BatchDto(
    applied: const [],
    failed: const [],
    skippedRunning: const [],
    checkFailures: const [],
    cancelled: false,
  );

  SettingsPatchDto? lastPatch;

  /// Apps whose program is running. The core refuses to update one of them
  /// unless the update is forced, whatever the cached offer says.
  final Set<String> runningUuids = {};

  /// What "Check for update" answers for an app. An app without an entry is
  /// up to date.
  final Map<String, UpdateCheckDto> checkResults = {};

  /// The arguments and the replace target of the latest such calls.
  List<String>? lastArguments;
  String? lastReplaceUuid;

  void _record(String call) => calls.add(call);

  void _maybeFail(String method) {
    final error = failures[method];
    if (error != null) {
      throw error;
    }
  }

  @override
  Future<LibraryDto> listLibrary() async {
    _record('listLibrary');
    _maybeFail('listLibrary');
    return library;
  }

  @override
  Future<void> launchApp({required String uuid}) async {
    _record('launchApp:$uuid');
    _maybeFail('launchApp');
  }

  @override
  Future<void> revealApp({required String uuid}) async {
    _record('revealApp:$uuid');
    _maybeFail('revealApp');
  }

  @override
  Future<AppDto> saveArgumentsAndEnvironment({
    required String uuid,
    required List<String> arguments,
    required List<EnvVarDto> environment,
  }) async {
    _record('saveArgumentsAndEnvironment:$uuid');
    lastArguments = arguments;
    _maybeFail('saveArgumentsAndEnvironment');
    return fakeApp(uuid: uuid, arguments: arguments, environment: environment);
  }

  @override
  Future<AppDto> setUpdateSource({
    required String uuid,
    required String manager,
    required List<KeyValueDto> config,
  }) async {
    _record(
      'setUpdateSource:$uuid:$manager:${config.map((p) => '${p.key}=${p.value}').join(',')}',
    );
    _maybeFail('setUpdateSource');
    return fakeApp(uuid: uuid, updateManager: manager, updateConfig: config);
  }

  @override
  Future<AppDto> unsetUpdateSource({required String uuid}) async {
    _record('unsetUpdateSource:$uuid');
    _maybeFail('unsetUpdateSource');
    return fakeApp(uuid: uuid);
  }

  @override
  Future<AppDto> adoptPath({required String opId, required String path}) async {
    _record('adoptPath:$path');
    _maybeFail('adoptPath');
    return fakeApp(uuid: 'adopted', name: 'Adopted');
  }

  @override
  Future<AppDto> refreshMetadata({
    required String opId,
    required String uuid,
  }) async {
    _record('refreshMetadata:$uuid');
    _maybeFail('refreshMetadata');
    return fakeApp(uuid: uuid);
  }

  @override
  Future<OutcomeDto> removeApp({
    required String opId,
    required String uuid,
    required bool permanent,
  }) async {
    _record('removeApp:$uuid:${permanent ? 'permanent' : 'trash'}');
    _maybeFail('removeApp');
    return removeResult;
  }

  @override
  Future<InspectDto> inspectPath({
    required String opId,
    required String path,
    bool confirmUnsafe = false,
  }) async {
    _record(
      confirmUnsafe ? 'inspectPath:$path:confirmUnsafe' : 'inspectPath:$path',
    );
    _maybeFail('inspectPath');
    await _hold('inspectPath');
    return confirmUnsafe ? inspectUnsafeResult : inspectResult;
  }

  @override
  Future<OutcomeDto> integrateApp({
    required String opId,
    required String sourcePath,
    required ConflictChoice conflict,
    required String replaceUuid,
    required bool moveSource,
    bool confirmUnsafe = false,
  }) async {
    _record(
      confirmUnsafe
          ? 'integrateApp:$sourcePath:${conflict.name}:confirmUnsafe'
          : 'integrateApp:$sourcePath:${conflict.name}',
    );
    lastReplaceUuid = replaceUuid;
    _maybeFail('integrateApp');
    return confirmUnsafe ? integrateUnsafeResult : integrateResult;
  }

  @override
  Future<UpdateScanDto> checkUpdates({required String opId}) async {
    _record('checkUpdates');
    _maybeFail('checkUpdates');
    await _hold('checkUpdates');
    return scan;
  }

  @override
  Future<UpdateCheckDto> checkOneUpdate({required String uuid}) async {
    _record('checkOneUpdate:$uuid');
    _maybeFail('checkOneUpdate');
    return checkResults[uuid] ?? fakeCheck(uuid: uuid);
  }

  @override
  Future<OutcomeDto> applyUpdate({
    required String opId,
    required String uuid,
    required bool force,
  }) async {
    _record('applyUpdate:$uuid:${force ? 'force' : 'normal'}');
    _maybeFail('applyUpdate');
    if (!force && runningUuids.contains(uuid)) {
      // The core's answer for a running app, whatever the cached offer says.
      return fakeOutcome(
        ok: false,
        running: true,
        message: 'Application is running; use --force to override',
      );
    }
    return updateResult;
  }

  @override
  Future<BatchDto> applyAllUpdates({
    required String opId,
    required bool force,
  }) async {
    _record('applyAllUpdates:${force ? 'force' : 'normal'}');
    _maybeFail('applyAllUpdates');
    return batch;
  }

  @override
  Future<SettingsDto> loadSettings() async {
    _record('loadSettings');
    _maybeFail('loadSettings');
    return settings;
  }

  @override
  Future<SettingsDto> saveSettings({required SettingsPatchDto patch}) async {
    _record('saveSettings');
    _maybeFail('saveSettings');
    lastPatch = patch;
    settings = withSettings(settings, patch: patch);
    return settings;
  }

  @override
  Future<void> setAutostart({required bool enabled}) async {
    _record('setAutostart:$enabled');
    _maybeFail('setAutostart');
    // The core refuses a login check while background checks are off.
    if (enabled && !settings.backgroundUpdateChecks) {
      throw const CoreError(
        kind: ErrorKind.validation,
        message: 'Turn on Check in the background before adding a login check.',
        details: '',
      );
    }
    settings = withSettings(settings, autostartEnabled: enabled);
  }

  @override
  Future<List<TaskDto>> listTasks() async {
    _record('listTasks');
    return List.of(tasks);
  }

  @override
  Future<void> clearFinishedTasks() async {
    _record('clearFinishedTasks');
    tasks.removeWhere(
      (task) =>
          task.state == TaskStateDto.succeeded ||
          task.state == TaskStateDto.failed ||
          task.state == TaskStateDto.cancelled,
    );
  }

  @override
  Future<bool> cancelTask({required String opId}) async {
    _record('cancelTask:$opId');
    return true;
  }

  @override
  Future<String> appVersion() async {
    _record('appVersion');
    return '3.0.0';
  }
}

CoreError coreError(String message, {ErrorKind kind = ErrorKind.failure}) =>
    CoreError(kind: kind, message: message, details: '');
