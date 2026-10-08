import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/src/rust/api/settings.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/shell.dart';

/// The application root. The saved appearance picks light, dark, or the
/// system's choice; the palette follows whichever is in effect.
class GoshApp extends StatelessWidget {
  const GoshApp({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: model,
      builder: (context, _) {
        final mode = switch (model.settings?.appearance ??
            AppearanceChoice.system) {
          AppearanceChoice.light => ThemeMode.light,
          AppearanceChoice.dark => ThemeMode.dark,
          AppearanceChoice.system => ThemeMode.system,
        };
        return MaterialApp(
          title: 'Gosh AppImage Manager',
          debugShowCheckedModeBanner: false,
          themeMode: mode,
          theme: appThemeData(AppPalette.light),
          darkTheme: appThemeData(AppPalette.dark),
          builder: (context, child) {
            final dark = Theme.of(context).brightness == Brightness.dark;
            return AppScope(
              palette: dark ? AppPalette.dark : AppPalette.light,
              child: child!,
            );
          },
          // TextField needs a Material ancestor. The shell paints its own
          // background, so the Material is transparent.
          home: Material(
            type: MaterialType.transparency,
            child: ShellPage(model: model),
          ),
        );
      },
    );
  }
}
