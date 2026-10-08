import 'package:file_selector/file_selector.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';

/// The system choosers. The app calls them through these fields so widget
/// tests can answer in place of the desktop portal.
class FilePickers {
  FilePickers._();

  /// Chooses one or more files to inspect. An empty list means the user
  /// cancelled.
  static Future<List<String>> Function() openAppImages = _openAppImages;

  /// Chooses the managed folder. Null means the user cancelled.
  static Future<String?> Function() openFolder = _openFolder;

  static Future<List<String>> _openAppImages() async {
    final files = await openFiles();
    return [for (final file in files) file.path];
  }

  static Future<String?> _openFolder() => getDirectoryPath();
}

/// Shows the file chooser and inspects what is picked. Choosing nothing
/// changes nothing.
Future<void> chooseAppImages(AppModel model) async {
  try {
    final paths = await FilePickers.openAppImages();
    if (paths.isEmpty) {
      return;
    }
    model.openPaths(paths);
  } on Exception catch (error) {
    model.setInspectError(error.toString());
  }
}

/// Shows the folder chooser for the managed folder. The chosen folder goes
/// through the same checks as a typed one. Choosing nothing changes nothing.
Future<void> chooseManagedFolder(AppModel model) async {
  try {
    final path = await FilePickers.openFolder();
    if (path == null || path.isEmpty) {
      return;
    }
    await model.chooseManagedFolder(path);
  } on Exception catch (error) {
    model.setStatusError(error.toString());
  }
}
