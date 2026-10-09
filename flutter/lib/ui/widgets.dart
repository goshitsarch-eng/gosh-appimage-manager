import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';

/// The window width below which the narrow layout is used: the sidebar becomes
/// a menu button and the rows collapse (SPEC section 9). The desktop frame is
/// 1280 px wide and the narrow frame 360 px; 800 px sits between them. This
/// value is recorded in docs/flutter/ARCHITECTURE.md.
const double narrowBreakpoint = 800;

bool isNarrow(BuildContext context) =>
    MediaQuery.sizeOf(context).width < narrowBreakpoint;

/// The icons in assets/icons, copied from the mockup's own SVG markup.
class AppIcon extends StatelessWidget {
  const AppIcon(this.name, {super.key, required this.size, this.color});

  final String name;
  final double size;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return SvgPicture.asset(
      'assets/icons/$name.svg',
      width: size,
      height: size,
      theme: SvgTheme(currentColor: color ?? palette.text2),
    );
  }
}

/// The focus ring every control draws. The mockup specifies none (SPEC open
/// question 5), so the app uses a 2 px accent ring.
Border focusRingBorder(AppPalette palette) =>
    Border.all(color: palette.accent, width: 2);

enum AppButtonVariant {
  /// White face with a hairline border (`Browse…`, `Launch`).
  secondary,

  /// Accent fill with on-accent text (`Integrate`, `Update all`).
  primary,

  /// Red fill (`Update anyway`).
  destructive,

  /// No fill; ink from [AppButton.color] (`Cancel`, `Refresh`).
  text,

  /// Red text on no fill (`Delete…`).
  dangerText,
}

/// A button drawn the way the mockup draws its buttons. Enter and Space
/// activate it when it has focus.
class AppButton extends StatefulWidget {
  const AppButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.variant = AppButtonVariant.secondary,
    this.height = 32,
    this.horizontalPadding = 12,
    this.fontSize = 13,
    this.color,
    this.hint,
    this.expand = false,
    this.buttonKey,
    this.autofocus = false,
  });

  final String label;
  final VoidCallback? onPressed;
  final AppButtonVariant variant;
  final double height;
  final double horizontalPadding;
  final double fontSize;

  /// The ink of a [AppButtonVariant.text] button.
  final Color? color;

  /// A mono key hint drawn inside the button (`Ctrl O`).
  final String? hint;

  /// Stretches the button to its parent's width (`Integrate`).
  final bool expand;

  /// Lets a test find this button.
  final Key? buttonKey;

  /// Takes focus when it first appears (a dialog's safe choice).
  final bool autofocus;

  @override
  State<AppButton> createState() => _AppButtonState();
}

class _AppButtonState extends State<AppButton> {
  bool _hovered = false;
  bool _focused = false;

  /// Named after the label, so a test can tell which control has focus.
  late final FocusNode _focusNode = FocusNode(debugLabel: widget.label);

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final enabled = widget.onPressed != null;
    var filled = true;
    Color fill;
    Color ink;
    FontWeight weight;
    Border? border;
    switch (widget.variant) {
      case AppButtonVariant.secondary:
        fill = _hovered ? palette.track : palette.surface;
        ink = widget.color ?? palette.text;
        weight = FontWeight.w500;
        border = Border.all(color: palette.border);
      case AppButtonVariant.primary:
        fill = _hovered
            ? palette.accent.withValues(alpha: 0.9)
            : palette.accent;
        ink = palette.onAccent;
        weight = FontWeight.w600;
      case AppButtonVariant.destructive:
        fill = _hovered ? palette.bad.withValues(alpha: 0.9) : palette.bad;
        ink = palette.isDark ? palette.onAccent : const Color(0xFFFFFFFF);
        weight = FontWeight.w600;
      case AppButtonVariant.text:
        filled = _hovered;
        fill = palette.track;
        ink = widget.color ?? palette.text2;
        weight = FontWeight.w500;
      case AppButtonVariant.dangerText:
        filled = false;
        fill = Colors.transparent;
        ink = palette.bad;
        weight = FontWeight.w500;
    }
    // A bordered secondary button is its declared height plus the 1 px border
    // on each side, as the mockup's content-box sizing makes it.
    final face = Opacity(
      opacity: enabled ? 1 : 0.5,
      child: Container(
        height:
            widget.height +
            (widget.variant == AppButtonVariant.secondary ? 2 : 0),
        padding: EdgeInsets.symmetric(horizontal: widget.horizontalPadding),
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: filled ? fill : null,
          borderRadius: BorderRadius.circular(AppRadius.control),
          border: _focused ? focusRingBorder(palette) : border,
        ),
        child: Row(
          mainAxisSize: widget.expand ? MainAxisSize.max : MainAxisSize.min,
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Flexible(
              child: Text(
                widget.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppType.sans(
                  widget.fontSize,
                  weight: weight,
                  color: ink,
                ),
              ),
            ),
            if (widget.hint != null) ...[
              const SizedBox(width: 8),
              AppKeyHint(widget.hint!),
            ],
          ],
        ),
      ),
    );
    return FocusableActionDetector(
      enabled: enabled,
      focusNode: _focusNode,
      autofocus: widget.autofocus,
      mouseCursor: enabled
          ? SystemMouseCursors.click
          : SystemMouseCursors.basic,
      onShowHoverHighlight: (value) => setState(() => _hovered = value),
      onShowFocusHighlight: (value) => setState(() => _focused = value),
      actions: <Type, Action<Intent>>{
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (_) {
            widget.onPressed?.call();
            return null;
          },
        ),
      },
      child: Semantics(
        button: true,
        enabled: enabled,
        label: widget.label,
        child: GestureDetector(
          key: widget.buttonKey,
          behavior: HitTestBehavior.opaque,
          onTap: widget.onPressed,
          child: widget.expand ? face : IntrinsicWidth(child: face),
        ),
      ),
    );
  }
}

/// A square icon button with an optional face and border (the row menu, the
/// narrow menu, the window controls).
class AppIconButton extends StatefulWidget {
  const AppIconButton({
    super.key,
    required this.icon,
    required this.onPressed,
    required this.size,
    this.radius = AppRadius.control,
    this.background,
    this.border,
    this.tooltip,
    this.buttonKey,
  });

  final Widget icon;
  final VoidCallback? onPressed;
  final double size;
  final double radius;
  final Color? background;
  final Color? border;
  final String? tooltip;
  final Key? buttonKey;

  @override
  State<AppIconButton> createState() => _AppIconButtonState();
}

class _AppIconButtonState extends State<AppIconButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final background = _hovered && widget.background == null
        ? palette.track
        : widget.background;
    Widget button = MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        key: widget.buttonKey,
        behavior: HitTestBehavior.opaque,
        onTap: widget.onPressed,
        child: Container(
          width: widget.size,
          height: widget.size,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: background,
            borderRadius: BorderRadius.circular(widget.radius),
            border: widget.border == null
                ? null
                : Border.all(color: widget.border!),
          ),
          child: widget.icon,
        ),
      ),
    );
    if (widget.tooltip != null) {
      button = Tooltip(message: widget.tooltip!, child: button);
    }
    return Semantics(button: true, label: widget.tooltip, child: button);
  }
}

/// A field on the mockup's input chrome: a 1 px border, a surface fill and a
/// 7 px radius. Typing reports each change. A change from the model (a file
/// chosen in the picker) replaces the text without moving the caret away.
class AppTextField extends StatefulWidget {
  const AppTextField({
    super.key,
    required this.text,
    required this.onChanged,
    this.placeholder = '',
    this.onSubmitted,
    this.height = 32,
    this.mono = false,
    this.fontSize = 13,
    this.multiline = false,
    this.leading,
    this.trailing,
    this.background,
    this.textColor,
    this.radius = AppRadius.control,
    this.fieldKey,
  });

  final String text;
  final ValueChanged<String> onChanged;
  final ValueChanged<String>? onSubmitted;
  final String placeholder;
  final double height;
  final bool mono;
  final double fontSize;

  /// Grows with its content instead of staying one line.
  final bool multiline;
  final Widget? leading;
  final Widget? trailing;
  final Color? background;
  final Color? textColor;
  final double radius;

  /// Lets a test find this field.
  final Key? fieldKey;

  @override
  State<AppTextField> createState() => _AppTextFieldState();
}

class _AppTextFieldState extends State<AppTextField> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.text,
  );
  final FocusNode _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() => setState(() {}));
  }

  @override
  void didUpdateWidget(covariant AppTextField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.text != _controller.text && !_focus.hasFocus) {
      _controller.value = TextEditingValue(
        text: widget.text,
        selection: TextSelection.collapsed(offset: widget.text.length),
      );
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final ink = widget.textColor ?? palette.text;
    final style = widget.mono
        ? AppType.mono(widget.fontSize, color: ink)
        : AppType.sans(widget.fontSize, color: ink);
    final field = TextField(
      key: widget.fieldKey,
      controller: _controller,
      focusNode: _focus,
      minLines: widget.multiline ? 1 : null,
      maxLines: widget.multiline ? null : 1,
      onChanged: widget.onChanged,
      onSubmitted: widget.onSubmitted,
      cursorColor: palette.accent,
      style: style,
      decoration: InputDecoration.collapsed(
        hintText: widget.placeholder,
        hintStyle: style.copyWith(color: palette.text3),
      ),
    );
    return Container(
      constraints: BoxConstraints(minHeight: widget.height + 2),
      padding: const EdgeInsets.symmetric(horizontal: 10),
      alignment: Alignment.centerLeft,
      decoration: BoxDecoration(
        color: widget.background ?? palette.surface,
        borderRadius: BorderRadius.circular(widget.radius),
        border: Border.all(
          color: _focus.hasFocus ? palette.accent : palette.border,
        ),
      ),
      child: Row(
        children: [
          if (widget.leading != null) ...[
            widget.leading!,
            const SizedBox(width: 8),
          ],
          Expanded(child: field),
          if (widget.trailing != null) widget.trailing!,
        ],
      ),
    );
  }
}

/// One segment of an [AppSegmented] control.
@immutable
class AppSegment<T> {
  const AppSegment(this.value, this.label, {this.count});

  final T value;
  final String label;
  final String? count;
}

/// The mockup's segmented control: a sidebar-coloured track holding pills, the
/// selected one white with a hairline shadow.
class AppSegmented<T> extends StatelessWidget {
  const AppSegmented({
    super.key,
    required this.segments,
    required this.selected,
    required this.onSelected,
    this.fontSize = 13,
    this.segmentHeight = 26,
    this.expand = false,
    this.segmentPadding = 10,
  });

  final List<AppSegment<T>> segments;
  final T selected;
  final ValueChanged<T> onSelected;
  final double fontSize;
  final double segmentHeight;

  /// Equal-width segments that fill the track (the narrow tabs).
  final bool expand;

  /// Horizontal padding inside each segment (10 in the library, 12 in Settings).
  final double segmentPadding;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final items = [
      for (final segment in segments)
        _Segment<T>(
          segment: segment,
          selected: segment.value == selected,
          height: segmentHeight,
          fontSize: fontSize,
          expand: expand,
          padding: segmentPadding,
          onTap: () => onSelected(segment.value),
        ),
    ];
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: palette.sidebar,
        borderRadius: BorderRadius.circular(AppRadius.small),
      ),
      child: Row(
        mainAxisSize: expand ? MainAxisSize.max : MainAxisSize.min,
        children: [
          for (var i = 0; i < items.length; i++) ...[
            if (i > 0) const SizedBox(width: 2),
            if (expand) Expanded(child: items[i]) else items[i],
          ],
        ],
      ),
    );
  }
}

class _Segment<T> extends StatefulWidget {
  const _Segment({
    required this.segment,
    required this.selected,
    required this.height,
    required this.fontSize,
    required this.expand,
    required this.onTap,
    this.padding = 10,
  });

  final AppSegment<T> segment;
  final bool selected;
  final double height;
  final double fontSize;
  final bool expand;
  final VoidCallback onTap;
  final double padding;

  @override
  State<_Segment<T>> createState() => _SegmentState<T>();
}

class _SegmentState<T> extends State<_Segment<T>> {
  bool _focused = false;

  /// Named after the segment, so a test can tell which one has focus.
  late final FocusNode _focusNode = FocusNode(
    debugLabel: 'segment-${widget.segment.label}',
  );

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final segment = widget.segment;
    final selected = widget.selected;
    final ink = selected ? palette.text : palette.text2;
    final count = segment.count;
    return Semantics(
      button: true,
      selected: selected,
      label: count == null ? segment.label : '${segment.label} $count',
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
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: Container(
            height: widget.height,
            padding: EdgeInsets.symmetric(horizontal: widget.padding),
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: selected ? palette.surface : null,
              borderRadius: BorderRadius.circular(6),
              border: _focused ? focusRingBorder(palette) : null,
              boxShadow: selected
                  ? [
                      BoxShadow(
                        color: palette.shade.withValues(
                          alpha: palette.isDark ? 0.35 : 0.08,
                        ),
                        blurRadius: 2,
                        offset: const Offset(0, 1),
                      ),
                    ]
                  : null,
            ),
            child: Row(
              mainAxisSize: widget.expand ? MainAxisSize.max : MainAxisSize.min,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Flexible(
                  child: Text(
                    segment.label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: AppType.sans(
                      widget.fontSize,
                      weight: FontWeight.w500,
                      color: ink,
                    ),
                  ),
                ),
                if (count != null) ...[
                  const SizedBox(width: 6),
                  Text(
                    count,
                    style: AppType.mono(
                      11,
                      weight: FontWeight.w500,
                      color: palette.text3,
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// An on/off switch: a 34 by 20 track and a 16 px knob, accent when on. It
/// takes focus, and Space or Enter flips it.
class AppToggle extends StatefulWidget {
  const AppToggle({
    super.key,
    required this.value,
    this.onChanged,
    this.toggleKey,
  });

  final bool value;
  final ValueChanged<bool>? onChanged;
  final Key? toggleKey;

  @override
  State<AppToggle> createState() => _AppToggleState();
}

class _AppToggleState extends State<AppToggle> {
  bool _focused = false;

  /// Named after the toggle's key, so a test can tell which one has focus.
  late final FocusNode _focusNode = FocusNode(
    debugLabel: switch (widget.toggleKey) {
      ValueKey<String>(:final value) => value,
      _ => 'toggle',
    },
  );

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  void _flip() => widget.onChanged?.call(!widget.value);

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final enabled = widget.onChanged != null;
    final value = widget.value;
    return Semantics(
      toggled: value,
      enabled: enabled,
      child: FocusableActionDetector(
        enabled: enabled,
        focusNode: _focusNode,
        mouseCursor: enabled
            ? SystemMouseCursors.click
            : SystemMouseCursors.basic,
        onShowFocusHighlight: (highlight) =>
            setState(() => _focused = highlight),
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (_) {
              _flip();
              return null;
            },
          ),
        },
        child: GestureDetector(
          key: widget.toggleKey,
          behavior: HitTestBehavior.opaque,
          onTap: enabled ? _flip : null,
          child: Opacity(
            opacity: enabled ? 1 : 0.5,
            child: SizedBox(
              width: 34,
              height: 20,
              child: Stack(
                children: [
                  Positioned.fill(
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        color: value ? palette.accent : palette.toggleOff,
                        borderRadius: BorderRadius.circular(10),
                        border: _focused ? focusRingBorder(palette) : null,
                      ),
                    ),
                  ),
                  Positioned(
                    top: 2,
                    left: value ? 16 : 2,
                    width: 16,
                    height: 16,
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        color: palette.knob,
                        borderRadius: BorderRadius.circular(8),
                        boxShadow: enabled
                            ? [
                                BoxShadow(
                                  color: palette.shade.withValues(
                                    alpha: palette.isDark ? 0.4 : 0.2,
                                  ),
                                  blurRadius: 2,
                                  offset: const Offset(0, 1),
                                ),
                              ]
                            : null,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The accent count pill in the sidebar (`Updates 2`).
class AppCountPill extends StatelessWidget {
  const AppCountPill(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Container(
      constraints: const BoxConstraints(minWidth: 18),
      height: 18,
      padding: const EdgeInsets.symmetric(horizontal: 5),
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: palette.accent,
        borderRadius: BorderRadius.circular(9),
      ),
      child: Text(
        text,
        style: AppType.mono(
          10.5,
          weight: FontWeight.w600,
          color: palette.onAccent,
        ),
      ),
    );
  }
}

/// A keyboard hint: mono text in a hairline box (`Ctrl O`).
class AppKeyHint extends StatelessWidget {
  const AppKeyHint(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
      decoration: BoxDecoration(
        border: Border.all(color: palette.border),
        borderRadius: BorderRadius.circular(AppRadius.key),
      ),
      child: Text(
        text,
        style: AppType.mono(
          10.5,
          weight: FontWeight.w500,
          color: palette.text3,
        ),
      ),
    );
  }
}

/// A status dot, 6 or 7 px across.
class StatusDot extends StatelessWidget {
  const StatusDot({super.key, required this.color, this.size = 6});

  final Color color;
  final double size;

  @override
  Widget build(BuildContext context) => Container(
    width: size,
    height: size,
    decoration: BoxDecoration(
      color: color,
      borderRadius: BorderRadius.circular(size / 2),
    ),
  );
}

/// An app's tile: the app's own icon when its AppImage has one, otherwise its
/// letter tile. The icon file is untrusted. A raster icon is read only by
/// Flutter's image decoder, decoded no larger than the tile needs; an SVG is
/// drawn by flutter_svg, which parses it into vector paths and never loads
/// anything the file points to. Either is shown only once it has decoded. A
/// file that is missing, unreadable or not an image shows the letter with no
/// error (owner decision).
///
/// Many AppImages ship an SVG icon, and the image decoder cannot read one, so
/// those apps showed a letter in place of their icon.
class AppIconTile extends StatelessWidget {
  const AppIconTile({
    super.key,
    required this.letter,
    required this.iconPath,
    required this.size,
    required this.fontSize,
    this.radius = AppRadius.tile,
    this.iconBytes,
    this.iconFormat = '',
  });

  final String letter;

  /// The icon's file, from the app's record. Empty when it has none.
  final String iconPath;
  final double size;
  final double fontSize;
  final double radius;

  /// The icon's bytes, for a file that is inspected and not yet installed: the
  /// Inspect page has no file of ours to point at. [iconFormat] says which
  /// reader to use, as the extension does for [iconPath].
  final Uint8List? iconBytes;
  final String iconFormat;

  /// Whether the icon is an SVG. The core names an installed icon for its
  /// content, so the extension says which reader to use.
  bool get _isSvg => iconBytes != null
      ? iconFormat.toLowerCase() == 'svg'
      : iconPath.toLowerCase().endsWith('.svg');

  @override
  Widget build(BuildContext context) {
    final letterTile = AppLetterTile(
      letter: letter,
      size: size,
      fontSize: fontSize,
      radius: radius,
    );
    final bytes = iconBytes;
    final hasBytes = bytes != null && bytes.isNotEmpty;
    if (iconPath.isEmpty && !hasBytes) {
      return letterTile;
    }
    Widget placeholder(BuildContext context) => letterTile;
    Widget failed(BuildContext context, Object error, StackTrace? stack) =>
        letterTile;
    final Widget icon;
    if (_isSvg) {
      icon = hasBytes
          ? SvgPicture.memory(
              bytes,
              width: size,
              height: size,
              fit: BoxFit.contain,
              placeholderBuilder: placeholder,
              errorBuilder: failed,
            )
          : SvgPicture.file(
              File(iconPath),
              width: size,
              height: size,
              fit: BoxFit.contain,
              placeholderBuilder: placeholder,
              errorBuilder: failed,
            );
    } else {
      // The width alone bounds the decode and keeps the icon's shape; giving
      // both dimensions stretched a non-square icon to a square.
      Widget shown(BuildContext context, Widget child, int? frame, bool sync) =>
          frame == null && !sync ? letterTile : child;
      icon = hasBytes
          ? Image.memory(
              bytes,
              width: size,
              height: size,
              fit: BoxFit.contain,
              filterQuality: FilterQuality.medium,
              cacheWidth: (size * 2).round(),
              frameBuilder: shown,
              errorBuilder: failed,
            )
          : Image.file(
              File(iconPath),
              width: size,
              height: size,
              fit: BoxFit.contain,
              filterQuality: FilterQuality.medium,
              cacheWidth: (size * 2).round(),
              frameBuilder: shown,
              errorBuilder: failed,
            );
    }
    return ClipRRect(borderRadius: BorderRadius.circular(radius), child: icon);
  }
}

/// The letter tile that stands in for an app's icon.
class AppLetterTile extends StatelessWidget {
  const AppLetterTile({
    super.key,
    required this.letter,
    required this.size,
    required this.fontSize,
    this.radius = AppRadius.tile,
    this.weight = FontWeight.w600,
  });

  final String letter;
  final double size;
  final double fontSize;
  final double radius;
  final FontWeight weight;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final tone = palette.tileFor(letter);
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: tone.background,
        borderRadius: BorderRadius.circular(radius),
      ),
      child: Text(
        letter,
        style: AppType.sans(fontSize, weight: weight, color: tone.foreground),
      ),
    );
  }
}

/// A surface card with the mockup's hairline border and 10 px radius.
class AppCard extends StatelessWidget {
  const AppCard({
    super.key,
    required this.child,
    this.padding = EdgeInsets.zero,
    this.color,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    // The 1 px border takes space inside the card, as a CSS border does, and
    // is painted over the content so a header's fill cannot hide it.
    final radius = BorderRadius.circular(AppRadius.card);
    return Container(
      decoration: BoxDecoration(
        color: color ?? palette.surface,
        borderRadius: radius,
      ),
      foregroundDecoration: BoxDecoration(
        border: Border.all(color: palette.border),
        borderRadius: radius,
      ),
      child: ClipRRect(
        borderRadius: radius,
        child: Padding(
          padding: const EdgeInsets.all(1),
          child: Padding(padding: padding, child: child),
        ),
      ),
    );
  }
}

/// A card's 44 px heading row with a hairline underneath (`Record`,
/// `Launch options`, `Appearance`).
class AppCardHeader extends StatelessWidget {
  const AppCardHeader({super.key, required this.title, this.trailing});

  final String title;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Container(
      height: 44,
      padding: const EdgeInsets.symmetric(horizontal: 16),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(
              title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AppType.sans(
                13,
                weight: FontWeight.w600,
                color: palette.text,
              ),
            ),
          ),
          ?trailing,
        ],
      ),
    );
  }
}

/// A thin determinate bar: a track and an accent fill.
class AppProgressBar extends StatelessWidget {
  const AppProgressBar({super.key, required this.percent, this.height = 6});

  final int percent;
  final double height;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final fraction = percent.clamp(0, 100) / 100;
    return SizedBox(
      height: height,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: palette.track,
          borderRadius: BorderRadius.circular(3),
        ),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(3),
          child: Align(
            alignment: Alignment.centerLeft,
            child: FractionallySizedBox(
              widthFactor: fraction,
              heightFactor: 1,
              child: ColoredBox(color: palette.accent),
            ),
          ),
        ),
      ),
    );
  }
}

/// A rounded rectangle with a dashed outline (the empty-state drop card).
class DashedBorder extends StatelessWidget {
  const DashedBorder({
    super.key,
    required this.child,
    required this.color,
    required this.radius,
    this.width = 1.5,
  });

  final Widget child;
  final Color color;
  final double radius;
  final double width;

  @override
  Widget build(BuildContext context) => CustomPaint(
    foregroundPainter: _DashedPainter(color, radius, width),
    child: child,
  );
}

class _DashedPainter extends CustomPainter {
  const _DashedPainter(this.color, this.radius, this.width);

  final Color color;
  final double radius;
  final double width;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = width;
    final rect = Rect.fromLTWH(
      width / 2,
      width / 2,
      size.width - width,
      size.height - width,
    );
    final path = Path()
      ..addRRect(RRect.fromRectAndRadius(rect, Radius.circular(radius)));
    const dash = 5.0;
    const gap = 4.0;
    for (final metric in path.computeMetrics()) {
      var distance = 0.0;
      while (distance < metric.length) {
        final end = (distance + dash).clamp(0.0, metric.length);
        canvas.drawPath(metric.extractPath(distance, end), paint);
        distance += dash + gap;
      }
    }
  }

  @override
  bool shouldRepaint(_DashedPainter oldDelegate) =>
      oldDelegate.color != color ||
      oldDelegate.radius != radius ||
      oldDelegate.width != width;
}

/// A small grey label such as the `Unavailable` pill.
class AppPill extends StatelessWidget {
  const AppPill(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Container(
      height: 18,
      padding: const EdgeInsets.symmetric(horizontal: 6),
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: palette.sidebar,
        borderRadius: BorderRadius.circular(AppRadius.key),
      ),
      child: Text(
        text,
        style: AppType.sans(11, weight: FontWeight.w500, color: palette.text2),
      ),
    );
  }
}
