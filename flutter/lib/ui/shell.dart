import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/platform/window_channel.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/about_page.dart';
import 'package:gosh_appimage_flutter/ui/dialogs.dart';
import 'package:gosh_appimage_flutter/ui/inspect_page.dart';
import 'package:gosh_appimage_flutter/ui/library_page.dart';
import 'package:gosh_appimage_flutter/ui/settings_page.dart';
import 'package:gosh_appimage_flutter/ui/tasks_page.dart';
import 'package:gosh_appimage_flutter/ui/updates_page.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// The window's width in the desktop frame (SPEC section 6): the sidebar is
/// 224 px of content, 10 px padding each side and a 1 px border.
const double sidebarWidth = 245;

/// The title bar: 46 px of content and a 1 px bottom border.
const double titleBarHeight = 47;

/// The status bar: 30 px of content and a 1 px top border.
const double statusBarHeight = 31;

/// The narrow frame's title bar: 48 px and a 1 px bottom border.
const double narrowTitleHeight = 49;

String pageTitle(AppPage page) => switch (page) {
  AppPage.library => 'Library',
  AppPage.inspect => 'Inspect',
  AppPage.updates => 'Updates',
  AppPage.tasks => 'Tasks',
  AppPage.settings => 'Settings',
  AppPage.about => 'About',
};

/// The window: the title bar, the sidebar (or the narrow menu), the page, the
/// status line and the dialog that sits over everything while one is pending.
/// It also owns the keyboard shortcuts, file drops and the window title.
class ShellPage extends StatefulWidget {
  const ShellPage({super.key, required this.model});

  final AppModel model;

  @override
  State<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends State<ShellPage> {
  bool _maximized = false;
  bool _menuOpen = false;
  String? _title;

  /// The window's own focus. When the control that had focus leaves the window,
  /// focus comes back here, so the next Tab starts from the top and Enter and
  /// Space reach a control again (R6-07). Named, so a test can find it.
  final FocusNode _windowFocus = FocusNode(debugLabel: 'window');

  /// Whether a dialog was showing at the last change the window saw.
  bool _dialogShowing = false;

  @override
  void initState() {
    super.initState();
    widget.model.addListener(_onModelChanged);
    HardwareKeyboard.instance.addHandler(_onKeyEvent);
    WindowChannel.listen();
    WindowChannel.onMaximizedChanged = (value) {
      if (mounted) {
        setState(() => _maximized = value);
      }
    };
    _syncTitle();
  }

  @override
  void dispose() {
    widget.model.removeListener(_onModelChanged);
    HardwareKeyboard.instance.removeHandler(_onKeyEvent);
    WindowChannel.onMaximizedChanged = null;
    _windowFocus.dispose();
    super.dispose();
  }

  /// Keeps the title in step, and hands focus back to the window when the
  /// control that had it is removed by the change: a dialog closing, or a
  /// Detail page or dialog taking its place (R6-07). A dialog that opens takes
  /// focus itself, so it is left alone.
  void _onModelChanged() {
    _syncTitle();
    if (widget.model.dialog != null) {
      _dialogShowing = true;
      return;
    }
    final closed = _dialogShowing;
    _dialogShowing = false;
    final focused = FocusManager.instance.primaryFocus;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || widget.model.dialog != null) {
        return;
      }
      // A closed dialog always returns focus to the window. Otherwise focus
      // moves only when the control that had it was removed or nothing had it.
      if (closed || focused == null || focused.enclosingScope == null) {
        _windowFocus.requestFocus();
      }
    });
  }

  void _syncTitle() {
    final title = 'Gosh AppImage Manager — ${pageTitle(widget.model.page)}';
    if (title != _title) {
      _title = title;
      WindowChannel.setTitle(title);
    }
  }

  void _selectPage(AppPage page) {
    widget.model.setPage(page);
    if (_menuOpen) {
      setState(() => _menuOpen = false);
    }
  }

  void _escape() {
    final model = widget.model;
    if (model.dialog != null) {
      model.dismissDialog();
    } else if (_menuOpen) {
      setState(() => _menuOpen = false);
    }
  }

  /// The application shortcuts. A global key handler, not a focus-tree
  /// Shortcuts widget: a Shortcuts widget only sees keys pressed while focus
  /// sits below it, and once a text field has gone there may be no such focus
  /// (QA D-08). The handler runs before the focus walk, so the shortcuts work
  /// whatever has focus.
  bool _onKeyEvent(KeyEvent event) {
    if (event is! KeyDownEvent) {
      return false;
    }
    final model = widget.model;
    final shortcuts = <(ShortcutActivator, VoidCallback)>[
      (
        const SingleActivator(LogicalKeyboardKey.keyO, control: true),
        () => chooseAppImages(model),
      ),
      (
        const SingleActivator(LogicalKeyboardKey.keyR, control: true),
        () => model.loadLibrary(),
      ),
      (
        const SingleActivator(LogicalKeyboardKey.keyF, control: true),
        () => model.checkUpdates(),
      ),
      (const SingleActivator(LogicalKeyboardKey.f5), () => model.loadLibrary()),
      (const SingleActivator(LogicalKeyboardKey.escape), _escape),
    ];
    for (final (activator, action) in shortcuts) {
      if (activator.accepts(event, HardwareKeyboard.instance)) {
        action();
        return true;
      }
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final model = widget.model;
    // Tab visits the sidebar top to bottom, then the page: the sidebar items
    // carry explicit orders (see _NavItem), and the rest follows.
    return FocusTraversalGroup(
      policy: OrderedTraversalPolicy(),
      child: Focus(
        focusNode: _windowFocus,
        autofocus: true,
        child: DropTarget(
          onDragDone: (details) =>
              model.dropFiles([for (final file in details.files) file.path]),
          child: ListenableBuilder(
            listenable: model,
            builder: (context, _) => LayoutBuilder(
              builder: (context, constraints) =>
                  _frame(context, constraints.maxWidth < narrowBreakpoint),
            ),
          ),
        ),
      ),
    );
  }

  Widget _frame(BuildContext context, bool narrow) {
    final model = widget.model;
    final palette = AppScope.of(context);
    final Widget window;
    if (narrow) {
      window = Stack(
        children: [
          Column(
            children: [
              _NarrowTitleBar(
                model: model,
                // The button opens the menu; the scrim, a page or Escape closes it.
                onMenu: () => setState(() => _menuOpen = true),
              ),
              Expanded(
                child: FocusTraversalOrder(
                  order: const NumericFocusOrder(_pageTraversalOrder),
                  child: _PageArea(model: model, narrow: true),
                ),
              ),
              _StatusBar(model: model, narrow: true),
            ],
          ),
          if (_menuOpen) ...[
            Positioned.fill(
              child: GestureDetector(
                key: const Key('menu-scrim'),
                behavior: HitTestBehavior.opaque,
                onTap: () => setState(() => _menuOpen = false),
                child: ColoredBox(color: palette.scrim),
              ),
            ),
            Positioned(
              top: 0,
              bottom: 0,
              left: 0,
              width: sidebarWidth,
              child: _Sidebar(model: model, onSelect: _selectPage),
            ),
          ],
        ],
      );
    } else {
      window = Column(
        children: [
          _TitleBar(maximized: _maximized),
          Expanded(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                SizedBox(
                  width: sidebarWidth,
                  child: _Sidebar(model: model, onSelect: _selectPage),
                ),
                Expanded(
                  child: FocusTraversalOrder(
                    order: const NumericFocusOrder(_pageTraversalOrder),
                    child: _PageArea(model: model, narrow: false),
                  ),
                ),
              ],
            ),
          ),
          _StatusBar(model: model, narrow: false),
        ],
      );
    }
    return ColoredBox(
      color: palette.canvas,
      child: DefaultTextStyle(
        style: AppType.sans(13, height: 1.4, color: palette.text),
        child: Stack(
          children: [
            Positioned.fill(child: window),
            if (!_maximized && !narrow) ..._resizeHandles(),
            if (model.dialog != null) ...[
              Positioned.fill(child: ColoredBox(color: palette.scrim)),
              Positioned.fill(
                child: Center(
                  child: Padding(
                    padding: const EdgeInsets.all(24),
                    child: buildDialog(model, model.dialog!),
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }

  /// Invisible grips along the window edges. The runner draws no frame, so
  /// these start a native resize.
  List<Widget> _resizeHandles() {
    Widget grip(
      String edge,
      MouseCursor cursor, {
      double? left,
      double? right,
      double? top,
      double? bottom,
      double? width,
      double? height,
    }) => Positioned(
      left: left,
      right: right,
      top: top,
      bottom: bottom,
      width: width,
      height: height,
      child: MouseRegion(
        cursor: cursor,
        child: GestureDetector(
          behavior: HitTestBehavior.translucent,
          onPanStart: (_) => WindowChannel.startResize(edge),
        ),
      ),
    );
    return [
      grip(
        'north',
        SystemMouseCursors.resizeUpDown,
        left: 0,
        right: 0,
        top: 0,
        height: 6,
      ),
      grip(
        'south',
        SystemMouseCursors.resizeUpDown,
        left: 0,
        right: 0,
        bottom: 0,
        height: 6,
      ),
      grip(
        'west',
        SystemMouseCursors.resizeLeftRight,
        left: 0,
        top: 0,
        bottom: 0,
        width: 6,
      ),
      grip(
        'east',
        SystemMouseCursors.resizeLeftRight,
        right: 0,
        top: 0,
        bottom: 0,
        width: 6,
      ),
    ];
  }
}

/// The desktop title bar: the logo tile and title, then the window controls.
/// Dragging the bar moves the window; a double click maximizes it.
class _TitleBar extends StatelessWidget {
  const _TitleBar({required this.maximized});

  final bool maximized;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return GestureDetector(
      key: const Key('title-bar'),
      behavior: HitTestBehavior.opaque,
      onPanStart: (_) => WindowChannel.startDrag(),
      child: Container(
        height: titleBarHeight,
        padding: const EdgeInsets.fromLTRB(16, 0, 10, 0),
        decoration: BoxDecoration(
          color: palette.chrome,
          border: Border(bottom: BorderSide(color: palette.border)),
        ),
        child: Row(
          children: [
            Container(
              width: 22,
              height: 22,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: palette.accent,
                borderRadius: BorderRadius.circular(6),
              ),
              child: Text(
                'G',
                style: AppType.mono(
                  12,
                  weight: FontWeight.w600,
                  color: palette.onAccent,
                ),
              ),
            ),
            const SizedBox(width: 12),
            Text(
              'Gosh AppImage Manager',
              style: AppType.sans(
                13.5,
                weight: FontWeight.w600,
                letterSpacing: -0.0675,
                color: palette.text,
              ),
            ),
            const Spacer(),
            const _WindowControls(),
          ],
        ),
      ),
    );
  }
}

/// Minimize, maximize and close: 26 px circles with 12 px glyphs. The close
/// control carries the mockup's highlighted face.
class _WindowControls extends StatelessWidget {
  const _WindowControls();

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    Widget control(
      String name,
      String key,
      VoidCallback onPressed,
      Color ink, {
      Color? face,
    }) => AppIconButton(
      buttonKey: Key(key),
      size: 26,
      radius: 13,
      background: face,
      onPressed: onPressed,
      tooltip: name,
      icon: AppIcon('window-$name', size: 12, color: ink),
    );
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        control(
          'minimize',
          'window-minimize',
          WindowChannel.minimize,
          palette.text2,
        ),
        const SizedBox(width: 6),
        control(
          'maximize',
          'window-maximize',
          WindowChannel.toggleMaximize,
          palette.text2,
        ),
        const SizedBox(width: 6),
        control(
          'close',
          'window-close',
          WindowChannel.close,
          palette.text,
          face: palette.track,
        ),
      ],
    );
  }
}

/// The narrow frame's title bar: the menu button, the page name, Browse…, and
/// the window controls (the mockup omits them; the app keeps them).
class _NarrowTitleBar extends StatelessWidget {
  const _NarrowTitleBar({required this.model, required this.onMenu});

  final AppModel model;
  final VoidCallback onMenu;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onPanStart: (_) => WindowChannel.startDrag(),
      child: Container(
        height: narrowTitleHeight,
        padding: const EdgeInsets.symmetric(horizontal: 14),
        decoration: BoxDecoration(
          color: palette.chrome,
          border: Border(bottom: BorderSide(color: palette.border)),
        ),
        child: Row(
          children: [
            AppIconButton(
              buttonKey: const Key('menu-button'),
              size: 30,
              radius: 6,
              background: palette.surface,
              border: palette.border,
              onPressed: onMenu,
              tooltip: 'Menu',
              icon: const _MenuBars(),
            ),
            const SizedBox(width: 12),
            Text(
              pageTitle(model.page),
              style: AppType.sans(
                15,
                weight: FontWeight.w600,
                color: palette.text,
              ),
            ),
            const Spacer(),
            if (model.page == AppPage.library) ...[
              AppButton(
                label: 'Browse…',
                height: 28,
                horizontalPadding: 10,
                fontSize: 12.5,
                buttonKey: const Key('browse-narrow'),
                onPressed: () => chooseAppImages(model),
              ),
              const SizedBox(width: 6),
            ],
            const _WindowControls(),
          ],
        ),
      ),
    );
  }
}

/// The three 14 by 2 bars inside the narrow menu button.
class _MenuBars extends StatelessWidget {
  const _MenuBars();

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    Widget bar() => Container(
      width: 14,
      height: 2,
      decoration: BoxDecoration(
        color: palette.text,
        borderRadius: BorderRadius.circular(1),
      ),
    );
    return Column(
      mainAxisSize: MainAxisSize.min,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        bar(),
        const SizedBox(height: 4),
        bar(),
        const SizedBox(height: 4),
        bar(),
      ],
    );
  }
}

/// The sidebar: the six pages, a divider before Settings, and the managed
/// folder box at the foot. In the narrow frame it is the menu drawer.
class _Sidebar extends StatelessWidget {
  const _Sidebar({required this.model, required this.onSelect});

  final AppModel model;
  final ValueChanged<AppPage> onSelect;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final library = model.library.length;
    final updates = model.updateCount;
    final running = model.runningTasks.length;
    final home = homeFolder();
    return Container(
      decoration: BoxDecoration(
        color: palette.sidebar,
        border: Border(right: BorderSide(color: palette.border)),
      ),
      padding: const EdgeInsets.fromLTRB(10, 12, 10, 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _NavItem(
            key: const Key('nav-library'),
            page: AppPage.library,
            icon: 'nav-library',
            label: 'Library',
            selected: model.page == AppPage.library,
            trailing: Text(
              '$library',
              style: AppType.mono(11, color: palette.text3),
            ),
            onTap: () => onSelect(AppPage.library),
          ),
          const SizedBox(height: 2),
          _NavItem(
            key: const Key('nav-inspect'),
            page: AppPage.inspect,
            icon: 'nav-inspect',
            label: 'Inspect',
            selected: model.page == AppPage.inspect,
            onTap: () => onSelect(AppPage.inspect),
          ),
          const SizedBox(height: 2),
          _NavItem(
            key: const Key('nav-updates'),
            page: AppPage.updates,
            icon: 'nav-updates',
            label: 'Updates',
            selected: model.page == AppPage.updates,
            trailing: updates > 0 ? AppCountPill('$updates') : null,
            onTap: () => onSelect(AppPage.updates),
          ),
          const SizedBox(height: 2),
          _NavItem(
            key: const Key('nav-tasks'),
            page: AppPage.tasks,
            icon: 'nav-tasks',
            label: 'Tasks',
            selected: model.page == AppPage.tasks,
            trailing: running > 0
                ? Text(
                    '$running',
                    style: AppType.mono(11, color: palette.text3),
                  )
                : null,
            onTap: () => onSelect(AppPage.tasks),
          ),
          const SizedBox(height: 10),
          Container(
            height: 1,
            margin: const EdgeInsets.symmetric(horizontal: 10),
            color: palette.border,
          ),
          const SizedBox(height: 10),
          _NavItem(
            key: const Key('nav-settings'),
            page: AppPage.settings,
            icon: 'nav-settings',
            label: 'Settings',
            selected: model.page == AppPage.settings,
            onTap: () => onSelect(AppPage.settings),
          ),
          const SizedBox(height: 2),
          _NavItem(
            key: const Key('nav-about'),
            page: AppPage.about,
            icon: 'nav-about',
            label: 'About',
            selected: model.page == AppPage.about,
            onTap: () => onSelect(AppPage.about),
          ),
          const Spacer(),
          Container(
            key: const Key('managed-folder-box'),
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: palette.chrome,
              border: Border.all(color: palette.border),
              borderRadius: BorderRadius.circular(AppRadius.small),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'MANAGED FOLDER',
                  style: AppType.label(color: palette.text3),
                ),
                const SizedBox(height: 6),
                Text(
                  shortenHome(model.managedFolderPath, home),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AppType.mono(
                    12.5,
                    weight: FontWeight.w500,
                    color: palette.text,
                  ),
                ),
                const SizedBox(height: 2),
                // An unreadable folder says so, with the core's error, rather
                // than reading as empty (QA2-012).
                if (model.libraryError case final error?) ...[
                  Text(
                    'Unreadable',
                    style: AppType.sans(
                      12,
                      weight: FontWeight.w500,
                      color: palette.bad,
                    ),
                  ),
                  Text(
                    error,
                    style: AppType.sans(12, height: 1.4, color: palette.text2),
                  ),
                ] else
                  Text(
                    folderNote(library, model.totalBytes),
                    style: AppType.sans(12, color: palette.text2),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// One sidebar item: 34 px high, an icon and a label, and the white card with
/// a hairline ring when it is the current page.
class _NavItem extends StatefulWidget {
  const _NavItem({
    super.key,
    required this.page,
    required this.icon,
    required this.label,
    required this.selected,
    required this.onTap,
    this.trailing,
  });

  final AppPage page;
  final String icon;
  final String label;
  final bool selected;
  final VoidCallback onTap;
  final Widget? trailing;

  @override
  State<_NavItem> createState() => _NavItemState();
}

/// The page area follows the sidebar in Tab order (QA D-05). The sidebar items
/// take 1 to 6; controls that have no order sort after the page.
const double _pageTraversalOrder = 7;

/// The sidebar's Tab order: top to bottom.
double _traversalOrder(AppPage page) => switch (page) {
  AppPage.library => 1,
  AppPage.inspect => 2,
  AppPage.updates => 3,
  AppPage.tasks => 4,
  AppPage.settings => 5,
  AppPage.about => 6,
};

class _NavItemState extends State<_NavItem> {
  bool _hovered = false;
  bool _focused = false;

  /// Named after the page, so a test can tell which item has focus.
  late final FocusNode _focusNode = FocusNode(
    debugLabel: 'nav-${widget.page.name}',
  );

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return FocusTraversalOrder(
      order: NumericFocusOrder(_traversalOrder(widget.page)),
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: FocusableActionDetector(
          focusNode: _focusNode,
          mouseCursor: SystemMouseCursors.click,
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
            label: widget.label,
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: widget.onTap,
              child: SizedBox(
                height: 34,
                child: Stack(
                  fit: StackFit.expand,
                  children: [
                    if (widget.selected)
                      Positioned.fill(
                        child: DecoratedBox(
                          decoration: BoxDecoration(
                            color: palette.surface,
                            borderRadius: BorderRadius.circular(
                              AppRadius.control,
                            ),
                            border: Border.all(color: palette.border),
                            boxShadow: [
                              BoxShadow(
                                color: palette.shade.withValues(
                                  alpha: palette.isDark ? 0.3 : 0.06,
                                ),
                                blurRadius: 2,
                                offset: const Offset(0, 1),
                              ),
                            ],
                          ),
                        ),
                      )
                    else if (_hovered)
                      Positioned.fill(
                        child: DecoratedBox(
                          decoration: BoxDecoration(
                            color: palette.track.withValues(alpha: 0.5),
                            borderRadius: BorderRadius.circular(
                              AppRadius.control,
                            ),
                          ),
                        ),
                      ),
                    if (_focused)
                      Positioned.fill(
                        child: DecoratedBox(
                          key: ValueKey('nav-${widget.page.name}-focus-ring'),
                          decoration: BoxDecoration(
                            borderRadius: BorderRadius.circular(
                              AppRadius.control,
                            ),
                            border: focusRingBorder(palette),
                          ),
                        ),
                      ),
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 10),
                      child: Row(
                        children: [
                          AppIcon(widget.icon, size: 16, color: palette.text2),
                          const SizedBox(width: 10),
                          Expanded(
                            child: Text(
                              widget.label,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: AppType.sans(
                                13,
                                weight: FontWeight.w500,
                                color: palette.text,
                              ),
                            ),
                          ),
                          ?widget.trailing,
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The page area: the current page, and the status line under it when a task
/// is running or the last result needs reading.
class _PageArea extends StatelessWidget {
  const _PageArea({required this.model, required this.narrow});

  final AppModel model;
  final bool narrow;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final Widget page = switch (model.page) {
      AppPage.library => LibraryPage(model: model),
      AppPage.inspect => InspectPage(model: model),
      AppPage.updates => UpdatesPage(model: model),
      AppPage.tasks => TasksPage(model: model),
      AppPage.settings => SettingsPage(model: model),
      AppPage.about => AboutPage(model: model),
    };
    final showStatus = model.busy != null || model.status != null;
    return ColoredBox(
      color: palette.canvas,
      child: Padding(
        padding: EdgeInsets.only(bottom: showStatus ? 8 : 0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(child: page),
            if (showStatus)
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 28),
                child: _StatusLine(model: model),
              ),
          ],
        ),
      ),
    );
  }
}

/// The line above the status bar: what is running, or the last result. The
/// severity is spelled out as well as coloured.
class _StatusLine extends StatelessWidget {
  const _StatusLine({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final busy = model.busy;
    if (busy != null) {
      return Row(
        children: [
          Expanded(
            child: Text(
              'Working: ${busy.title}…',
              key: const Key('busy-line'),
              style: AppType.sans(12, color: palette.text2),
            ),
          ),
          const SizedBox(width: 8),
          AppButton(
            label: 'Cancel',
            variant: AppButtonVariant.text,
            fontSize: 12.5,
            onPressed: model.cancelBusy,
          ),
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
    final ink = switch (status.severity) {
      Severity.info => palette.text2,
      Severity.success => palette.ok,
      Severity.error => palette.bad,
    };
    return Row(
      children: [
        Expanded(
          child: Text(
            '$prefix: ${status.text}',
            key: const Key('status-line'),
            style: AppType.sans(12, color: ink),
          ),
        ),
        const SizedBox(width: 8),
        AppButton(
          label: 'Dismiss',
          variant: AppButtonVariant.text,
          fontSize: 12.5,
          onPressed: model.dismissStatus,
        ),
      ],
    );
  }
}

/// The status bar: the counts, and on the desktop the last-checked time. The
/// narrow bar drops the time and colours a failed check red.
class _StatusBar extends StatelessWidget {
  const _StatusBar({required this.model, required this.narrow});

  final AppModel model;
  final bool narrow;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final installed = model.library.length;
    final updates = model.updateCount;
    final failed = model.checkFailures.length;
    final counts = <Widget>[
      if (installed == 0)
        Text('0 installed', style: AppType.sans(12, color: palette.text2))
      else
        Text(
          '$installed installed',
          style: AppType.sans(12, color: palette.text2),
        ),
      if (updates > 0)
        _CountItem(
          label: '$updates updates',
          color: palette.accent,
          gap: 6,
          plain: narrow,
        ),
      if (failed > 0)
        _CountItem(
          label: '$failed check failed',
          color: palette.bad,
          gap: 6,
          plain: narrow,
          textTone: narrow ? palette.bad : null,
        ),
    ];
    final checked = model.lastChecked;
    final lastChecked = checked == null
        ? 'Not checked yet'
        : 'Last checked ${clockLabel(checked)}';
    return Container(
      key: const Key('status-bar'),
      height: statusBarHeight,
      padding: EdgeInsets.symmetric(horizontal: narrow ? 14 : 16),
      decoration: BoxDecoration(
        color: palette.chrome,
        border: Border(top: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          for (var i = 0; i < counts.length; i++) ...[
            if (i > 0) SizedBox(width: narrow ? 14 : 16),
            counts[i],
          ],
          if (!narrow) ...[
            const Spacer(),
            Text(lastChecked, style: AppType.mono(11.5, color: palette.text2)),
          ],
        ],
      ),
    );
  }
}

class _CountItem extends StatelessWidget {
  const _CountItem({
    required this.label,
    required this.color,
    required this.gap,
    this.textTone,
    this.plain = false,
  });

  final String label;
  final Color color;
  final double gap;
  final Color? textTone;

  /// Text only, with no dot (the narrow bar).
  final bool plain;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final text = Text(
      label,
      style: AppType.sans(12, color: textTone ?? palette.text2),
    );
    if (plain || textTone != null) {
      return text;
    }
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        StatusDot(color: color),
        SizedBox(width: gap),
        text,
      ],
    );
  }
}
