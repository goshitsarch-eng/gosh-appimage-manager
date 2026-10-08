import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/platform/window_channel.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';
import 'package:gosh_appimage_flutter/ui/about_page.dart';
import 'package:gosh_appimage_flutter/ui/dialogs.dart';
import 'package:gosh_appimage_flutter/ui/inspect_page.dart';
import 'package:gosh_appimage_flutter/ui/library_page.dart';
import 'package:gosh_appimage_flutter/ui/settings_page.dart';
import 'package:gosh_appimage_flutter/ui/tasks_page.dart';
import 'package:gosh_appimage_flutter/ui/updates_page.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// The navigation rail's width. The original's breakpoint is built from it.
const double navWidth = 280;

/// The gap between the window edge and the rail, and between the rail and the
/// page.
const double _railMargin = 8;

String pageTitle(AppPage page) => switch (page) {
  AppPage.library => 'Library',
  AppPage.inspect => 'Inspect',
  AppPage.updates => 'Updates',
  AppPage.tasks => 'Tasks',
  AppPage.settings => 'Settings',
  AppPage.about => 'About',
};

/// The window: the navigation rail, the page, the status line, and the dialog
/// that sits over everything while one is pending. It also owns the keyboard
/// shortcuts, file drops and the window title.
class ShellPage extends StatefulWidget {
  const ShellPage({super.key, required this.model});

  final AppModel model;

  @override
  State<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends State<ShellPage> {
  /// Set by the header's button. Null means the default for the current width
  /// applies: the rail is shown when there is room, and is a drawer when not.
  bool? _navOverride;
  bool _navShown = true;
  bool _lastCondensed = false;
  Brightness? _chromeBrightness;
  String? _title;

  @override
  void initState() {
    super.initState();
    widget.model.addListener(_syncTitle);
    WindowChannel.listen();
    WindowChannel.onToggleNav = _toggleNav;
    _syncTitle();
  }

  @override
  void dispose() {
    widget.model.removeListener(_syncTitle);
    WindowChannel.onToggleNav = null;
    super.dispose();
  }

  void _syncTitle() {
    final title = 'Gosh AppImage Manager — ${pageTitle(widget.model.page)}';
    if (title != _title) {
      _title = title;
      WindowChannel.setTitle(title);
    }
  }

  void _toggleNav() => setState(() => _navOverride = !_navShown);

  void _selectPage(AppPage page) {
    widget.model.setPage(page);
    // A drawer closes once a page is chosen, as the original's does.
    if (_lastCondensed) {
      setState(() => _navOverride = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final model = widget.model;
    return CallbackShortcuts(
      bindings: <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.keyO, control: true): () =>
            chooseAppImages(model),
        const SingleActivator(LogicalKeyboardKey.keyR, control: true): () =>
            model.loadLibrary(),
        const SingleActivator(LogicalKeyboardKey.keyF, control: true): () =>
            model.checkUpdates(),
        const SingleActivator(LogicalKeyboardKey.f5): () => model.loadLibrary(),
        const SingleActivator(LogicalKeyboardKey.escape): () =>
            model.dismissDialog(),
      },
      child: Focus(
        autofocus: true,
        child: DropTarget(
          onDragDone: (details) =>
              model.dropFiles([for (final file in details.files) file.path]),
          child: ListenableBuilder(
            listenable: model,
            builder: (context, _) => LayoutBuilder(
              builder: (context, constraints) =>
                  _frame(context, constraints.maxWidth < condensedBreakpoint),
            ),
          ),
        ),
      ),
    );
  }

  Widget _frame(BuildContext context, bool condensed) {
    final model = widget.model;
    final palette = CosmicScope.of(context);
    if (condensed != _lastCondensed) {
      // Crossing the breakpoint re-applies the default for the new width.
      _lastCondensed = condensed;
      _navOverride = null;
    }
    _navShown = _navOverride ?? !condensed;
    if (_chromeBrightness != palette.brightness) {
      _chromeBrightness = palette.brightness;
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => WindowChannel.setChrome(palette),
      );
    }
    final rail = NavRail(selected: model.page, onSelect: _selectPage);
    final content = _Content(model: model);
    // The rail floats 8 pixels in from the window edges, as the original's
    // does, so the page starts 16 pixels in from the rail's own edge.
    final body = condensed
        ? Stack(
            children: [
              content,
              if (_navShown)
                Positioned(
                  top: _railMargin,
                  bottom: _railMargin,
                  left: _railMargin,
                  width: navWidth,
                  child: rail,
                ),
            ],
          )
        : Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (_navShown)
                Padding(
                  padding: const EdgeInsets.all(_railMargin),
                  child: SizedBox(width: navWidth, child: rail),
                ),
              Expanded(child: content),
            ],
          );
    final dialog = model.dialog;
    return Stack(
      children: [
        body,
        if (dialog != null)
          Positioned.fill(
            child: Stack(
              children: [
                Positioned.fill(child: ColoredBox(color: palette.shade)),
                buildDialog(model, dialog),
              ],
            ),
          ),
      ],
    );
  }
}

/// The left-hand rail: one item per page, the selected one filled.
class NavRail extends StatelessWidget {
  const NavRail({super.key, required this.selected, required this.onSelect});

  final AppPage selected;
  final ValueChanged<AppPage> onSelect;

  @override
  Widget build(BuildContext context) {
    final palette = CosmicScope.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.primarySurface,
        borderRadius: BorderRadius.circular(CosmicRadius.s),
      ),
      child: Padding(
        padding: const EdgeInsets.all(8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final page in AppPage.values) ...[
              if (page != AppPage.values.first) const SizedBox(height: 8),
              _NavItem(
                label: pageTitle(page),
                selected: page == selected,
                onTap: () => onSelect(page),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _NavItem extends StatefulWidget {
  const _NavItem({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  State<_NavItem> createState() => _NavItemState();
}

class _NavItemState extends State<_NavItem> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final palette = CosmicScope.of(context);
    final fill = widget.selected
        ? palette.navSelected
        : (_hovered ? palette.navSelected.withValues(alpha: 0.5) : null);
    // The selected label is drawn in the accent, as the original's is.
    final ink = widget.selected ? palette.accent : palette.primaryInactiveText;
    return FocusableActionDetector(
      mouseCursor: SystemMouseCursors.click,
      onShowHoverHighlight: (value) => setState(() => _hovered = value),
      onShowFocusHighlight: (value) => setState(() => _focused = value),
      actions: <Type, Action<Intent>>{
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (_) {
            widget.onTap();
            return null;
          },
        ),
      },
      child: Semantics(
        button: true,
        selected: widget.selected,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: Container(
            height: 32,
            alignment: Alignment.centerLeft,
            padding: const EdgeInsets.symmetric(horizontal: 16),
            decoration: BoxDecoration(
              color: fill,
              borderRadius: BorderRadius.circular(CosmicRadius.m),
              border: _focused
                  ? Border.all(color: palette.accent, width: 2)
                  : null,
            ),
            child: Text(
              widget.label,
              maxLines: 1,
              style: CosmicType.body.copyWith(color: ink),
            ),
          ),
        ),
      ),
    );
  }
}

/// The page, its padding, and the status line under it. The original's frame
/// is a 16-pixel margin with 8 pixels between the page and the status line.
class _Content extends StatelessWidget {
  const _Content({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = CosmicScope.of(context);
    final showStatus = model.busy != null || model.status != null;
    return ColoredBox(
      color: palette.windowBackground,
      child: DefaultTextStyle(
        style: CosmicType.body.copyWith(color: palette.windowText),
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Expanded(child: _pageFor(model)),
              if (showStatus) ...[
                const SizedBox(height: 8),
                _StatusLine(model: model),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _pageFor(AppModel model) => switch (model.page) {
    AppPage.library => LibraryPage(model: model),
    AppPage.inspect => InspectPage(model: model),
    AppPage.updates => UpdatesPage(model: model),
    AppPage.tasks => TasksPage(model: model),
    AppPage.settings => SettingsPage(model: model),
    AppPage.about => AboutPage(model: model),
  };
}

/// The line under the page: what is running, or the last result. Severity is
/// spelled out as well as coloured, so it is never conveyed by colour alone.
class _StatusLine extends StatelessWidget {
  const _StatusLine({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final busy = model.busy;
    if (busy != null) {
      return Row(
        children: [
          Expanded(child: Text('Working: ${busy.title}…')),
          const SizedBox(width: 8),
          CosmicButton(label: 'Cancel', onPressed: model.cancelBusy),
        ],
      );
    }
    final status = model.status;
    if (status == null) {
      return const SizedBox.shrink();
    }
    final prefix = switch (status.severity) {
      Severity.info => 'Note',
      Severity.success => 'Done',
      Severity.error => 'Error',
    };
    return Row(
      children: [
        Expanded(child: Text('$prefix: ${status.text}')),
        const SizedBox(width: 8),
        CosmicButton(
          label: 'Dismiss',
          kind: CosmicButtonKind.text,
          onPressed: model.dismissStatus,
        ),
      ],
    );
  }
}
