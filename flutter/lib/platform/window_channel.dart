import 'package:flutter/services.dart';

/// The window's channel to the Linux runner. The runner draws no title bar:
/// the Flutter title bar carries the mockup's logo, title and window controls,
/// and calls these to act on the native window. Off the desktop (tests, other
/// hosts) every call is a no-op.
class WindowChannel {
  WindowChannel._();

  static const MethodChannel _channel = MethodChannel('gosh/window');

  /// Told by the runner whether the window is maximized, so the maximize
  /// control shows the right state.
  static void Function(bool maximized)? onMaximizedChanged;

  static Future<void> setTitle(String title) => _call('setTitle', title);

  static Future<void> minimize() => _call('minimize');

  static Future<void> toggleMaximize() => _call('toggleMaximize');

  static Future<void> close() => _call('close');

  /// Starts an interactive move, as a press on the title bar does.
  static Future<void> startDrag() => _call('startDrag');

  /// Starts an interactive resize from one edge or corner: one of
  /// `north`, `south`, `east`, `west`, `north-east`, `north-west`,
  /// `south-east` or `south-west`.
  static Future<void> startResize(String edge) => _call('startResize', edge);

  /// Answers the runner's calls. Call once, before the first page is shown.
  static void listen() {
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'maximizedChanged') {
        onMaximizedChanged?.call(call.arguments == true);
      }
      return null;
    });
  }

  static Future<void> _call(String method, [Object? arguments]) async {
    try {
      await _channel.invokeMethod<void>(method, arguments);
    } on MissingPluginException {
      // No runner to answer: nothing to do.
    } on PlatformException {
      // The runner refused the call; the window keeps what it had.
    }
  }
}
