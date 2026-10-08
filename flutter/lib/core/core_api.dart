import 'package:gosh_appimage_flutter/src/rust/api/dto.dart';
import 'package:gosh_appimage_flutter/src/rust/api/inspect.dart' as rust;
import 'package:gosh_appimage_flutter/src/rust/api/integrate.dart';
import 'package:gosh_appimage_flutter/src/rust/api/integrate.dart' as rust;
import 'package:gosh_appimage_flutter/src/rust/api/library.dart';
import 'package:gosh_appimage_flutter/src/rust/api/library.dart' as rust;
import 'package:gosh_appimage_flutter/src/rust/api/settings.dart';
import 'package:gosh_appimage_flutter/src/rust/api/settings.dart' as rust;
import 'package:gosh_appimage_flutter/src/rust/api/system.dart' as rust;
import 'package:gosh_appimage_flutter/src/rust/api/updates.dart' as rust;

export 'package:gosh_appimage_flutter/src/rust/api/common.dart'
    show CoreError, ErrorKind;
export 'package:gosh_appimage_flutter/src/rust/api/dto.dart';
export 'package:gosh_appimage_flutter/src/rust/api/library.dart'
    show LibraryDto;
export 'package:gosh_appimage_flutter/src/rust/api/integrate.dart'
    show ConflictChoice;
export 'package:gosh_appimage_flutter/src/rust/api/settings.dart'
    show AppearanceChoice, SettingsDto, SettingsPatchDto;

/// Everything the interface needs from the Rust core. Each method is one
/// bridge call; none of them holds state. The model owns the workflow.
///
/// Tests implement this interface with a fake, so widget and state tests run
/// without the native library. [BridgeCore] is the real implementation.
abstract interface class CoreApi {
  Future<LibraryDto> listLibrary();
  Future<void> launchApp({required String uuid});
  Future<void> revealApp({required String uuid});
  Future<AppDto> saveArgumentsAndEnvironment({
    required String uuid,
    required List<String> arguments,
    required List<EnvVarDto> environment,
  });
  Future<AppDto> setUpdateSource({
    required String uuid,
    required String manager,
    required List<KeyValueDto> config,
  });
  Future<AppDto> unsetUpdateSource({required String uuid});
  Future<AppDto> adoptPath({required String opId, required String path});
  Future<AppDto> refreshMetadata({required String opId, required String uuid});
  Future<OutcomeDto> removeApp({
    required String opId,
    required String uuid,
    required bool permanent,
  });
  Future<InspectDto> inspectPath({required String opId, required String path});
  Future<OutcomeDto> integrateApp({
    required String opId,
    required String sourcePath,
    required ConflictChoice conflict,
    required String replaceUuid,
    required bool moveSource,
  });
  Future<UpdateScanDto> checkUpdates({required String opId});
  Future<OutcomeDto> applyUpdate({
    required String opId,
    required String uuid,
    required bool force,
  });
  Future<BatchDto> applyAllUpdates({required String opId, required bool force});
  Future<SettingsDto> loadSettings();
  Future<SettingsDto> saveSettings({required SettingsPatchDto patch});
  Future<void> setAutostart({required bool enabled});
  Future<List<TaskDto>> listTasks();
  Future<void> clearFinishedTasks();
  Future<bool> cancelTask({required String opId});
  Future<String> appVersion();
}

/// The production [CoreApi], backed by the generated flutter_rust_bridge
/// functions. Every call runs on the bridge's worker pool.
class BridgeCore implements CoreApi {
  const BridgeCore();

  @override
  Future<LibraryDto> listLibrary() => rust.listLibrary();

  @override
  Future<void> launchApp({required String uuid}) => rust.launchApp(uuid: uuid);

  @override
  Future<void> revealApp({required String uuid}) => rust.revealApp(uuid: uuid);

  @override
  Future<AppDto> saveArgumentsAndEnvironment({
    required String uuid,
    required List<String> arguments,
    required List<EnvVarDto> environment,
  }) => rust.saveArgumentsAndEnvironment(
    uuid: uuid,
    arguments: arguments,
    environment: environment,
  );

  @override
  Future<AppDto> setUpdateSource({
    required String uuid,
    required String manager,
    required List<KeyValueDto> config,
  }) => rust.setUpdateSource(uuid: uuid, manager: manager, config: config);

  @override
  Future<AppDto> unsetUpdateSource({required String uuid}) =>
      rust.unsetUpdateSource(uuid: uuid);

  @override
  Future<AppDto> adoptPath({required String opId, required String path}) =>
      rust.adoptPath(opId: opId, path: path);

  @override
  Future<AppDto> refreshMetadata({
    required String opId,
    required String uuid,
  }) => rust.refreshMetadata(opId: opId, uuid: uuid);

  @override
  Future<OutcomeDto> removeApp({
    required String opId,
    required String uuid,
    required bool permanent,
  }) => rust.removeApp(opId: opId, uuid: uuid, permanent: permanent);

  @override
  Future<InspectDto> inspectPath({
    required String opId,
    required String path,
  }) => rust.inspectPath(opId: opId, path: path);

  @override
  Future<OutcomeDto> integrateApp({
    required String opId,
    required String sourcePath,
    required ConflictChoice conflict,
    required String replaceUuid,
    required bool moveSource,
  }) => rust.integrateApp(
    opId: opId,
    sourcePath: sourcePath,
    conflict: conflict,
    replaceUuid: replaceUuid,
    moveSource: moveSource,
  );

  @override
  Future<UpdateScanDto> checkUpdates({required String opId}) =>
      rust.checkUpdates(opId: opId);

  @override
  Future<OutcomeDto> applyUpdate({
    required String opId,
    required String uuid,
    required bool force,
  }) => rust.applyUpdate(opId: opId, uuid: uuid, force: force);

  @override
  Future<BatchDto> applyAllUpdates({
    required String opId,
    required bool force,
  }) => rust.applyAllUpdates(opId: opId, force: force);

  @override
  Future<SettingsDto> loadSettings() => rust.loadSettings();

  @override
  Future<SettingsDto> saveSettings({required SettingsPatchDto patch}) =>
      rust.saveSettings(patch: patch);

  @override
  Future<void> setAutostart({required bool enabled}) =>
      rust.setAutostart(enabled: enabled);

  @override
  Future<List<TaskDto>> listTasks() => rust.listTasks();

  @override
  Future<void> clearFinishedTasks() => rust.clearFinishedTasks();

  @override
  Future<bool> cancelTask({required String opId}) =>
      rust.cancelTask(opId: opId);

  @override
  Future<String> appVersion() => rust.appVersion();
}

/// Operation ids name a bridge operation so the task list and cancellation can
/// find it. They only need to be unique within one run of the application.
class OperationIds {
  OperationIds._();

  static int _counter = 0;

  static String next() {
    _counter += 1;
    return 'op-${DateTime.now().microsecondsSinceEpoch}-$_counter';
  }
}
