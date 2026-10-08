import 'package:file_selector/file_selector.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';

/// Shows the system file chooser for one or more files and inspects what is
/// picked. Choosing nothing changes nothing.
Future<void> chooseAppImages(AppModel model) async {
  try {
    final files = await openFiles();
    if (files.isEmpty) {
      return;
    }
    model.openPaths([for (final file in files) file.path]);
  } on Exception catch (error) {
    model.setInspectError(error.toString());
  }
}
