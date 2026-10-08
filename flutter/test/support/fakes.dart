import 'package:gosh_appimage_flutter/core/core_api.dart';

/// Builders for the bridge's data types, with values that describe one
/// ordinary installed app unless a test overrides them.
AppDto fakeApp({
  String uuid = 'u1',
  String name = 'Demo',
  String version = '1.0.0',
  String managedPath = '/home/someone/AppImages/Demo.AppImage',
  bool running = false,
  bool adopted = false,
  bool externalFolder = false,
  List<String> arguments = const [],
  List<EnvVarDto> environment = const [],
  String updateManager = '',
  String embeddedUpdate = '',
}) => AppDto(
  uuid: uuid,
  name: name,
  version: version,
  comment: '',
  managedPath: managedPath,
  desktopId: 'gosh-appimage-demo',
  desktopPath:
      '/home/someone/.local/share/applications/gosh-appimage-demo.desktop',
  iconPath: '',
  sha256: 'ab' * 32,
  appType: 'Type 2',
  architecture: 'x86-64',
  sizeBytes: 4096,
  arguments: arguments,
  environment: environment,
  updateManager: updateManager,
  updateConfig: const [],
  embeddedUpdate: embeddedUpdate,
  lastUpdateCheck: '',
  availableVersion: '',
  availableUrl: '',
  availableSize: 0,
  updateAvailable: false,
  digest: '',
  reducedVerification: false,
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
);

InspectDto fakeInspect({
  bool magicValid = true,
  String error = '',
  List<String> warnings = const [],
  bool alreadyManaged = false,
  String name = 'Demo',
  String version = '1.0.0',
  String embeddedUpdate = '',
}) => InspectDto(
  path: '/tmp/Demo.AppImage',
  sizeBytes: 2048,
  sha256: 'cd' * 32,
  appType: 'Type 2',
  architecture: 'x86-64',
  magicValid: magicValid,
  architectureSupported: true,
  truncated: false,
  name: name,
  version: version,
  comment: '',
  iconName: '',
  iconFormat: '',
  iconBytes: null,
  categories: const [],
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
}) => OutcomeDto(
  ok: ok,
  partial: false,
  message: message,
  conflict: conflict,
  running: false,
  app: app,
  rolledBack: const [],
  sourceRemoved: false,
  conflictUuid: conflictUuid,
  conflictName: conflictName,
);

SettingsDto fakeSettings({
  String managedFolder = '/home/someone/AppImages',
  AppearanceChoice appearance = AppearanceChoice.system,
  bool autostartEnabled = false,
  String? loadError,
}) => SettingsDto(
  managedFolder: managedFolder,
  moveSource: false,
  manageOutsideFolder: false,
  terminalOmitSuffix: false,
  backgroundUpdateChecks: false,
  unsafeExtractionFallback: false,
  debugLogging: false,
  appearance: appearance,
  maxAppimageBytes: 8192 * 1024 * 1024,
  loadError: loadError,
  autostartEnabled: autostartEnabled,
);

UpdateOfferDto fakeOffer({
  String uuid = 'u1',
  String name = 'Demo',
  bool running = false,
}) => UpdateOfferDto(
  uuid: uuid,
  name: name,
  currentVersion: '1.0.0',
  availableVersion: '1.1.0',
  manager: 'github',
  url: 'https://example.invalid/Demo-1.1.0.AppImage',
  downloadSize: 0,
  digest: '',
  reducedVerification: false,
  digestAlgo: '',
  embeddedSource: '',
  running: running,
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

  OutcomeDto integrateResult = fakeOutcome();
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
    _maybeFail('saveArgumentsAndEnvironment');
    return fakeApp(uuid: uuid, arguments: arguments, environment: environment);
  }

  @override
  Future<AppDto> setUpdateSource({
    required String uuid,
    required String manager,
    required List<KeyValueDto> config,
  }) async {
    _record('setUpdateSource:$uuid:$manager');
    _maybeFail('setUpdateSource');
    return fakeApp(uuid: uuid, updateManager: manager);
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
  }) async {
    _record('inspectPath:$path');
    _maybeFail('inspectPath');
    return inspectResult;
  }

  @override
  Future<OutcomeDto> integrateApp({
    required String opId,
    required String sourcePath,
    required ConflictChoice conflict,
    required String replaceUuid,
    required bool moveSource,
  }) async {
    _record('integrateApp:$sourcePath:${conflict.name}');
    _maybeFail('integrateApp');
    return integrateResult;
  }

  @override
  Future<UpdateScanDto> checkUpdates({required String opId}) async {
    _record('checkUpdates');
    _maybeFail('checkUpdates');
    return scan;
  }

  @override
  Future<OutcomeDto> applyUpdate({
    required String opId,
    required String uuid,
    required bool force,
  }) async {
    _record('applyUpdate:$uuid:${force ? 'force' : 'normal'}');
    _maybeFail('applyUpdate');
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
    return settings;
  }

  @override
  Future<void> setAutostart({required bool enabled}) async {
    _record('setAutostart:$enabled');
    _maybeFail('setAutostart');
  }

  @override
  Future<List<TaskDto>> listTasks() async {
    _record('listTasks');
    return List.of(tasks);
  }

  @override
  Future<void> clearFinishedTasks() async {
    _record('clearFinishedTasks');
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
