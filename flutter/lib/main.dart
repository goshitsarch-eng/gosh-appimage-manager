import 'package:flutter/widgets.dart';
import 'package:gosh_appimage_flutter/app.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/src/rust/frb_generated.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';

/// Starts the window first and then loads, so the shell appears at once. Any
/// files given on the command line are opened in Inspect once the core is up.
Future<void> main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  final model = AppModel(core: const BridgeCore());
  runApp(GoshApp(model: model));
  await model.start(args);
}
