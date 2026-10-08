import 'package:flutter/material.dart';

/// Colour, type and shape tokens of the design mockup (SPEC sections 3, 4 and
/// 6 to 10). Light and dark are separate palettes; the app follows the saved
/// appearance, or the system when that is set to System.
@immutable
class AppPalette {
  const AppPalette({
    required this.brightness,
    required this.canvas,
    required this.chrome,
    required this.sidebar,
    required this.surface,
    required this.border,
    required this.windowBorder,
    required this.text,
    required this.text2,
    required this.text3,
    required this.accent,
    required this.onAccent,
    required this.accentSoft,
    required this.ok,
    required this.okSoft,
    required this.warn,
    required this.warnText,
    required this.bad,
    required this.badSoft,
    required this.mute,
    required this.track,
    required this.toggleOff,
    required this.knob,
    required this.shade,
    required this.scrim,
    required this.tiles,
  });

  final Brightness brightness;

  /// Page background (`canvas`), title bar and status bar (`chrome`), sidebar.
  final Color canvas;
  final Color chrome;
  final Color sidebar;

  /// Cards, rows, inputs and secondary buttons (`surface`).
  final Color surface;
  final Color border;
  final Color windowBorder;

  final Color text;
  final Color text2;
  final Color text3;

  final Color accent;
  final Color onAccent;

  /// The icon tile behind the empty-state download icon, and the Inspect banner.
  final Color accentSoft;

  final Color ok;
  final Color okSoft;

  /// Reduced verification: the dot is `warn`, the notice text is `warnText`.
  final Color warn;
  final Color warnText;

  /// Check failed and destructive actions.
  final Color bad;
  final Color badSoft;
  final Color mute;

  final Color track;
  final Color toggleOff;
  final Color knob;

  /// The ink shadows are drawn in; each use sets its own alpha.
  final Color shade;

  /// The dim behind a modal dialog, which the mockup does not draw.
  final Color scrim;

  /// Letter tiles, in the order indigo, teal, amber, rose, violet, green, slate.
  final List<AppTile> tiles;

  bool get isDark => brightness == Brightness.dark;

  static const AppPalette light = AppPalette(
    brightness: Brightness.light,
    canvas: Color(0xFFF5F4F0),
    chrome: Color(0xFFF0EFEB),
    sidebar: Color(0xFFEBE9E3),
    surface: Color(0xFFFFFFFF),
    border: Color(0xFFDEDBD3),
    windowBorder: Color(0xFFD6D3CB),
    text: Color(0xFF1D1C1A),
    text2: Color(0xFF5F5B54),
    text3: Color(0xFF6F685F),
    accent: Color(0xFF3D4DB5),
    onAccent: Color(0xFFFDFDFC),
    accentSoft: Color(0xFFE3E6F8),
    ok: Color(0xFF2E7D4F),
    okSoft: Color(0xFFE2F0E7),
    warn: Color(0xFFB07A10),
    warnText: Color(0xFF7A5200),
    bad: Color(0xFFA8322D),
    badSoft: Color(0xFFF6E1E4),
    mute: Color(0xFF8F8A82),
    track: Color(0xFFE4E1D9),
    toggleOff: Color(0xFFCFCCC3),
    knob: Color(0xFFFCFCFB),
    shade: Color(0xFF1D1C1A),
    scrim: Color(0x521D1C1A),
    tiles: [
      AppTile(Color(0xFFE3E6F8), Color(0xFF3D4DB5)),
      AppTile(Color(0xFFD9ECEA), Color(0xFF1F6F6A)),
      AppTile(Color(0xFFF5E8D0), Color(0xFF8A5300)),
      AppTile(Color(0xFFF6E1E4), Color(0xFF9C3446)),
      AppTile(Color(0xFFEBE3F6), Color(0xFF6A3FA0)),
      AppTile(Color(0xFFE2EFDC), Color(0xFF3F6B2A)),
      AppTile(Color(0xFFE3E7EC), Color(0xFF44546A)),
    ],
  );

  static const AppPalette dark = AppPalette(
    brightness: Brightness.dark,
    canvas: Color(0xFF17181A),
    chrome: Color(0xFF1B1C1F),
    sidebar: Color(0xFF1E1F22),
    surface: Color(0xFF222428),
    border: Color(0xFF33343A),
    windowBorder: Color(0xFF2C2E33),
    text: Color(0xFFECEBE7),
    text2: Color(0xFFAEAAA2),
    text3: Color(0xFF8F8A82),
    accent: Color(0xFF9AA7F0),
    onAccent: Color(0xFF10132A),
    accentSoft: Color(0xFF262D52),
    ok: Color(0xFF5FC28A),
    okSoft: Color(0xFF1C3326),
    warn: Color(0xFFE7B75C),
    warnText: Color(0xFFE7B75C),
    bad: Color(0xFFF08F88),
    badSoft: Color(0xFF3D2228),
    mute: Color(0xFF8F8A82),
    track: Color(0xFF2A2C31),
    toggleOff: Color(0xFF3D3F46),
    knob: Color(0xFFE9E8E4),
    shade: Color(0xFF000000),
    scrim: Color(0x80000000),
    tiles: [
      AppTile(Color(0xFF262D52), Color(0xFF9AA7F0)),
      AppTile(Color(0xFF1D3A38), Color(0xFF7CC4BD)),
      AppTile(Color(0xFF3A2D12), Color(0xFFE0AD55)),
      AppTile(Color(0xFF3D2228), Color(0xFFEE9AAB)),
      AppTile(Color(0xFF2E2442), Color(0xFFC2A6EF)),
      AppTile(Color(0xFF1F2E1A), Color(0xFF9BD08A)),
      AppTile(Color(0xFF262C34), Color(0xFFA9B6C7)),
    ],
  );

  /// The status dot colour for a tone (SPEC 3.5, the `C` constants).
  Color dot(StatusTone tone) => switch (tone) {
    StatusTone.accent => accent,
    StatusTone.ok => ok,
    StatusTone.warn => warn,
    StatusTone.bad => bad,
    StatusTone.mute => mute,
  };

  /// The letter tile for an app's initial (SPEC 3.5, the `T` constants). The
  /// sample apps use fixed tones; any other letter takes one by its code.
  AppTile tileFor(String initial) {
    final code = initial.isEmpty ? 0 : initial.toUpperCase().codeUnitAt(0);
    final index = switch (String.fromCharCode(code)) {
      'Q' => 0,
      'T' => 1,
      'A' => 2,
      'O' => 3,
      'C' => 4,
      'L' => 5,
      'B' => 6,
      _ => code % tiles.length,
    };
    return tiles[index];
  }
}

/// The status tone a dot, text or pill takes (SPEC 3.1, `accent`, `ok`, `warn`,
/// `bad`, `mute`).
enum StatusTone { accent, ok, warn, bad, mute }

/// A letter tile's background and initial colour.
@immutable
class AppTile {
  const AppTile(this.background, this.foreground);

  final Color background;
  final Color foreground;
}

/// Provides the palette to the widget tree.
class AppScope extends InheritedWidget {
  const AppScope({super.key, required this.palette, required super.child});

  final AppPalette palette;

  static AppPalette of(BuildContext context) {
    final scope = context.dependOnInheritedWidgetOfExactType<AppScope>();
    return scope?.palette ?? AppPalette.light;
  }

  @override
  bool updateShouldNotify(AppScope oldWidget) => oldWidget.palette != palette;
}

/// Text roles. The mockup sets Onest for interface text and IBM Plex Mono for
/// paths, versions, counts and uppercase column labels. Colour is left to the
/// caller so each surface sets its own ink.
class AppType {
  AppType._();

  static const String sansFamily = 'Onest';
  static const String monoFamily = 'IBM Plex Mono';

  static TextStyle sans(
    double size, {
    FontWeight weight = FontWeight.w400,
    double? height,
    double? letterSpacing,
    Color? color,
  }) => TextStyle(
    fontFamily: sansFamily,
    fontSize: size,
    fontWeight: weight,
    // The window's line height is 1.4 (SPEC 4): interface text takes it
    // unless a role sets its own.
    height: height ?? 1.4,
    letterSpacing: letterSpacing,
    color: color,
  );

  static TextStyle mono(
    double size, {
    FontWeight weight = FontWeight.w400,
    double? height,
    double? letterSpacing,
    Color? color,
  }) => TextStyle(
    fontFamily: monoFamily,
    fontSize: size,
    fontWeight: weight,
    height: height,
    letterSpacing: letterSpacing,
    color: color,
  );

  /// Uppercase mono column and section labels (10.5px, 600, +0.63px).
  static TextStyle label({Color? color}) =>
      mono(10.5, weight: FontWeight.w600, letterSpacing: 0.63, color: color);
}

/// Corner radii the mockup uses.
class AppRadius {
  AppRadius._();

  static const double key = 4;
  static const double button = 6;
  static const double control = 7;
  static const double small = 8;
  static const double card = 10;
  static const double dialog = 14;
  static const double tile = 9;
  static const double tileLarge = 13;
  static const double pill = 9;
}

/// The Material theme the stock widgets (text fields, dialogs, tooltips) pick
/// up. Visible styling comes from [AppPalette].
ThemeData appThemeData(AppPalette palette) {
  final scheme = ColorScheme(
    brightness: palette.brightness,
    primary: palette.accent,
    onPrimary: palette.onAccent,
    secondary: palette.accent,
    onSecondary: palette.onAccent,
    error: palette.bad,
    onError: palette.onAccent,
    surface: palette.surface,
    onSurface: palette.text,
    outline: palette.border,
    outlineVariant: palette.border,
  );
  final base = AppType.sans(13, height: 1.4, color: palette.text);
  return ThemeData(
    useMaterial3: true,
    brightness: palette.brightness,
    colorScheme: scheme,
    scaffoldBackgroundColor: palette.canvas,
    canvasColor: palette.canvas,
    dividerColor: palette.border,
    textTheme: TextTheme(
      bodyLarge: base,
      bodyMedium: base,
      bodySmall: base,
      labelLarge: base,
    ),
    focusColor: palette.accent.withValues(alpha: 0.3),
    hoverColor: Colors.transparent,
    splashFactory: NoSplash.splashFactory,
    highlightColor: Colors.transparent,
    tooltipTheme: TooltipThemeData(
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(AppRadius.control),
        border: Border.all(color: palette.border),
      ),
      textStyle: AppType.sans(12, color: palette.text),
    ),
  );
}
