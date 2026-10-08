import 'package:flutter/services.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';

/// The window's channel to the runner: its title, the header bar's colours,
/// and the header's navigation button. Off the desktop (tests, other hosts)
/// every call is a no-op.
class WindowChannel {
  WindowChannel._();

  static const MethodChannel _channel = MethodChannel('gosh/window');

  /// Called when the header's navigation button is pressed.
  static void Function()? onToggleNav;

  static Future<void> setTitle(String title) => _call('setTitle', title);

  /// Colours the header bar to the page under it, as the original's is.
  static Future<void> setChrome(CosmicPalette palette) => _call('setChrome', {
    'background': _hex(palette.windowBackground),
    'foreground': _hex(palette.windowText),
  });

  /// Answers the runner's calls. Call once, before the first page is shown.
  static void listen() {
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'toggleNav') {
        onToggleNav?.call();
      }
      return null;
    });
  }

  static Future<void> _call(String method, Object arguments) async {
    try {
      await _channel.invokeMethod<void>(method, arguments);
    } on MissingPluginException {
      // No runner to answer: nothing to do.
    } on PlatformException {
      // The runner refused the call; the window keeps what it had.
    }
  }

  static String _hex(Color color) {
    final argb = color.toARGB32().toRadixString(16).padLeft(8, '0');
    return '#${argb.substring(2)}';
  }
}
