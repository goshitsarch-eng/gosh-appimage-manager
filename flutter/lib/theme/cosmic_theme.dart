import 'package:flutter/material.dart';

/// Colours from the COSMIC theme the original application renders with.
///
/// The values are the derived theme's own output for `cosmic-dark` and
/// `cosmic-light` (libcosmic's `cosmic-theme` crate, default configuration),
/// not hand-picked approximations. Translucent colours keep their alpha, so
/// Flutter blends them over the same surface iced blends them over.
///
/// Naming follows the surface a colour belongs to: `window*` is the page
/// background, `card*` is a settings card sitting on it, `primary*` is the
/// navigation rail and dialogs, and `button*`, `accent*` and `destructive*`
/// are controls.
@immutable
class CosmicPalette {
  const CosmicPalette({
    required this.brightness,
    required this.windowBackground,
    required this.windowText,
    required this.windowDivider,
    required this.cardBackground,
    required this.cardHover,
    required this.cardPressed,
    required this.cardText,
    required this.cardDivider,
    required this.primarySurface,
    required this.primaryText,
    required this.primaryDivider,
    required this.primaryInactiveText,
    required this.navSelected,
    required this.buttonBackground,
    required this.buttonHover,
    required this.buttonPressed,
    required this.buttonText,
    required this.buttonBorder,
    required this.accent,
    required this.accentHover,
    required this.accentPressed,
    required this.onAccent,
    required this.destructiveButton,
    required this.destructiveHover,
    required this.destructivePressed,
    required this.onDestructive,
    required this.success,
    required this.warning,
    required this.destructive,
    required this.toggleOff,
    required this.toggleHandle,
    required this.inputFill,
    required this.shade,
  });

  final Brightness brightness;

  /// Page background (`background.base`) and the text drawn on it.
  final Color windowBackground;
  final Color windowText;
  final Color windowDivider;

  /// Settings cards (`background.component`) and the text drawn on them.
  final Color cardBackground;
  final Color cardHover;
  final Color cardPressed;
  final Color cardText;
  final Color cardDivider;

  /// Navigation rail and dialogs (`primary`).
  final Color primarySurface;
  final Color primaryText;
  final Color primaryDivider;
  final Color primaryInactiveText;

  /// The selected navigation item (`neutral_5` at 20%).
  final Color navSelected;

  /// Standard buttons (`button`). Translucent, as in the original.
  final Color buttonBackground;
  final Color buttonHover;
  final Color buttonPressed;
  final Color buttonText;
  final Color buttonBorder;

  /// Accent: suggested buttons, toggles, progress and links.
  final Color accent;
  final Color accentHover;
  final Color accentPressed;
  final Color onAccent;

  /// Destructive buttons (`destructive_button`).
  final Color destructiveButton;
  final Color destructiveHover;
  final Color destructivePressed;
  final Color onDestructive;

  /// Status colours drawn as text.
  final Color success;
  final Color warning;
  final Color destructive;

  /// Toggle track when off and its handle (`neutral_5` and `neutral_2`).
  final Color toggleOff;
  final Color toggleHandle;

  /// Text field fill: the card colour at 25%, as in the original.
  final Color inputFill;

  /// Dialog shadow and the scrim behind a dialog.
  final Color shade;

  static const CosmicPalette dark = CosmicPalette(
    brightness: Brightness.dark,
    windowBackground: Color(0xFF1B1B1B),
    windowText: Color(0xFFE7E7E7),
    windowDivider: Color(0xFF444444),
    cardBackground: Color(0xFF2E2E2E),
    cardHover: Color(0xFF434343),
    cardPressed: Color(0xFF585858),
    cardText: Color(0xFFC0C0C0),
    cardDivider: Color(0x33C0C0C0),
    primarySurface: Color(0xFF272727),
    primaryText: Color(0xFFF8F8F8),
    primaryDivider: Color(0xFF515151),
    primaryInactiveText: Color(0xFFCACACA),
    navSelected: Color(0x33777777),
    buttonBackground: Color(0x40ABABAB),
    buttonHover: Color(0x666D6D6D),
    buttonPressed: Color(0x9E3A3A3A),
    buttonText: Color(0xFFC0C0C0),
    buttonBorder: Color(0xFFC6C6C6),
    accent: Color(0xFF94EBEB),
    accentHover: Color(0xFF8ED4D4),
    accentPressed: Color(0xFF628E8E),
    onAccent: Color(0xFF000000),
    destructiveButton: Color(0xFFFFB5B5),
    destructiveHover: Color(0xFFE4A9A9),
    destructivePressed: Color(0xFF987373),
    onDestructive: Color(0xFF000000),
    success: Color(0xFFACF7D2),
    warning: Color(0xFFFFF19E),
    destructive: Color(0xFFFFB5B5),
    toggleOff: Color(0xFF777777),
    toggleHandle: Color(0xFF303030),
    inputFill: Color(0x402E2E2E),
    shade: Color(0x66000000),
  );

  static const CosmicPalette light = CosmicPalette(
    brightness: Brightness.light,
    windowBackground: Color(0xFFDDDDDD),
    windowText: Color(0xFF161616),
    windowDivider: Color(0xFFB5B5B5),
    cardBackground: Color(0xFFFCFCFC),
    cardHover: Color(0xFFE2E2E2),
    cardPressed: Color(0xFFC9C9C9),
    cardText: Color(0xFF2C2C2C),
    cardDivider: Color(0x332C2C2C),
    primarySurface: Color(0xFFF1F1F1),
    primaryText: Color(0xFF252525),
    primaryDivider: Color(0xFFC8C8C8),
    primaryInactiveText: Color(0xFF191919),
    navSelected: Color(0x33777777),
    buttonBackground: Color(0x40474747),
    buttonHover: Color(0x663B3B3B),
    buttonPressed: Color(0x9E717171),
    buttonText: Color(0xFF2C2C2C),
    buttonBorder: Color(0xFF303030),
    accent: Color(0xFF00496D),
    accentHover: Color(0xFF18526F),
    accentPressed: Color(0xFF63889A),
    onAccent: Color(0xFFFFFFFF),
    destructiveButton: Color(0xFFA0252B),
    destructiveHover: Color(0xFF98353A),
    destructivePressed: Color(0xFFB37679),
    onDestructive: Color(0xFFFFFFFF),
    success: Color(0xFF3B6E43),
    warning: Color(0xFF966800),
    destructive: Color(0xFFA0252B),
    toggleOff: Color(0xFF777777),
    toggleHandle: Color(0xFFC6C6C6),
    inputFill: Color(0x40FCFCFC),
    shade: Color(0x66000000),
  );
}

/// Provides the palette to the widget tree. Pages read it through [of].
class CosmicScope extends InheritedWidget {
  const CosmicScope({super.key, required this.palette, required super.child});

  final CosmicPalette palette;

  static CosmicPalette of(BuildContext context) {
    final scope = context.dependOnInheritedWidgetOfExactType<CosmicScope>();
    assert(scope != null, 'No CosmicScope above this context');
    return scope?.palette ?? CosmicPalette.dark;
  }

  @override
  bool updateShouldNotify(CosmicScope oldWidget) =>
      oldWidget.palette != palette;
}

/// The original's type scale, with its line heights. Colour is deliberately
/// absent: text takes the colour of the surface it sits on, as in iced, so a
/// caption in a card and one on the page each get the right ink.
class CosmicType {
  CosmicType._();

  static const String family = 'FiraSans';

  static const TextStyle title1 = TextStyle(
    fontFamily: family,
    fontSize: 32,
    height: 44 / 32,
    fontWeight: FontWeight.w600,
  );
  static const TextStyle title2 = TextStyle(
    fontFamily: family,
    fontSize: 28,
    height: 36 / 28,
    fontWeight: FontWeight.w400,
  );
  static const TextStyle title3 = TextStyle(
    fontFamily: family,
    fontSize: 24,
    height: 32 / 24,
    fontWeight: FontWeight.w400,
  );
  static const TextStyle title4 = TextStyle(
    fontFamily: family,
    fontSize: 20,
    height: 28 / 20,
    fontWeight: FontWeight.w400,
  );
  static const TextStyle heading = TextStyle(
    fontFamily: family,
    fontSize: 14,
    height: 20 / 14,
    fontWeight: FontWeight.w600,
  );
  static const TextStyle captionHeading = TextStyle(
    fontFamily: family,
    fontSize: 10,
    height: 14 / 10,
    fontWeight: FontWeight.w600,
  );
  static const TextStyle body = TextStyle(
    fontFamily: family,
    fontSize: 14,
    height: 20 / 14,
    fontWeight: FontWeight.w400,
  );
  static const TextStyle caption = TextStyle(
    fontFamily: family,
    fontSize: 10,
    height: 14 / 10,
    fontWeight: FontWeight.w400,
  );
}

/// Corner radii, in logical pixels, as the COSMIC corner scale defines them.
class CosmicRadius {
  CosmicRadius._();

  static const double xs = 4;
  static const double s = 8;
  static const double m = 16;

  /// Fully rounded: standard buttons are pills.
  static const double pill = 160;
}

/// The Material theme the stock widgets (scroll views, tooltips, focus and
/// text selection) pick up. Visible styling comes from [CosmicPalette].
ThemeData cosmicThemeData(CosmicPalette palette) {
  final scheme = ColorScheme(
    brightness: palette.brightness,
    primary: palette.accent,
    onPrimary: palette.onAccent,
    secondary: palette.accent,
    onSecondary: palette.onAccent,
    error: palette.destructiveButton,
    onError: palette.onDestructive,
    surface: palette.cardBackground,
    onSurface: palette.cardText,
    surfaceContainerHighest: palette.primarySurface,
    outline: palette.windowDivider,
    outlineVariant: palette.windowDivider,
  );
  final text = TextTheme(
    headlineMedium: CosmicType.title3.copyWith(color: palette.windowText),
    titleLarge: CosmicType.title4.copyWith(color: palette.windowText),
    titleMedium: CosmicType.heading.copyWith(color: palette.windowText),
    bodyLarge: CosmicType.body.copyWith(color: palette.windowText),
    bodyMedium: CosmicType.body.copyWith(color: palette.windowText),
    bodySmall: CosmicType.caption.copyWith(color: palette.windowText),
    labelLarge: CosmicType.body.copyWith(color: palette.windowText),
  );
  return ThemeData(
    useMaterial3: true,
    brightness: palette.brightness,
    colorScheme: scheme,
    scaffoldBackgroundColor: palette.windowBackground,
    canvasColor: palette.windowBackground,
    dividerColor: palette.windowDivider,
    textTheme: text,
    primaryTextTheme: text,
    focusColor: palette.accent.withValues(alpha: 0.3),
    hoverColor: palette.cardHover.withValues(alpha: 0.5),
    splashFactory: NoSplash.splashFactory,
    highlightColor: Colors.transparent,
    dialogTheme: DialogThemeData(
      backgroundColor: palette.primarySurface,
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(CosmicRadius.m),
        side: BorderSide(color: palette.primaryDivider),
      ),
    ),
    tooltipTheme: TooltipThemeData(
      decoration: BoxDecoration(
        color: palette.cardBackground,
        borderRadius: BorderRadius.circular(CosmicRadius.s),
        border: Border.all(color: palette.windowDivider),
      ),
      textStyle: CosmicType.caption.copyWith(color: palette.cardText),
    ),
  );
}
