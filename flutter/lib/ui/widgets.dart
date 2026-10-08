import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';

/// Below this window width the navigation rail collapses and rows stack. This
/// is the original's breakpoint: nav bar (280) + padding (8) + content (360).
const double condensedBreakpoint = 648;

bool isCondensed(BuildContext context) =>
    MediaQuery.sizeOf(context).width < condensedBreakpoint;

enum CosmicButtonKind { standard, suggested, destructive, text }

/// A button drawn the way the original draws its buttons: a 32-pixel pill for
/// standard, suggested and destructive, a plain label for text buttons. Enter
/// and Space activate it when it has focus.
class CosmicButton extends StatefulWidget {
  const CosmicButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.kind = CosmicButtonKind.standard,
  });

  final String label;
  final VoidCallback? onPressed;
  final CosmicButtonKind kind;

  @override
  State<CosmicButton> createState() => _CosmicButtonState();
}

class _CosmicButtonState extends State<CosmicButton> {
  bool _hovered = false;
  bool _pressed = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final palette = CosmicScope.of(context);
    final enabled = widget.onPressed != null;
    final isText = widget.kind == CosmicButtonKind.text;
    final (
      Color base,
      Color hover,
      Color pressed,
      Color foreground,
    ) = switch (widget.kind) {
      CosmicButtonKind.standard => (
        palette.buttonBackground,
        palette.buttonHover,
        palette.buttonPressed,
        palette.buttonText,
      ),
      CosmicButtonKind.suggested => (
        palette.accent,
        palette.accentHover,
        palette.accentPressed,
        palette.onAccent,
      ),
      CosmicButtonKind.destructive => (
        palette.destructiveButton,
        palette.destructiveHover,
        palette.destructivePressed,
        palette.onDestructive,
      ),
      CosmicButtonKind.text => (
        Colors.transparent,
        palette.navSelected,
        palette.navSelected,
        palette.accent,
      ),
    };
    final background = _pressed ? pressed : (_hovered ? hover : base);
    final ink = enabled ? foreground : foreground.withValues(alpha: 0.5);

    final face = DecoratedBox(
      decoration: BoxDecoration(
        color: enabled ? background : background.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(
          isText ? CosmicRadius.s : CosmicRadius.pill,
        ),
        border: _focused ? Border.all(color: palette.accent, width: 2) : null,
      ),
      // Sized to its label, as the original's buttons are. A Container with a
      // centred alignment would fill whatever width it was given instead.
      child: SizedBox(
        height: 32,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                widget.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: CosmicType.body.copyWith(color: ink),
              ),
            ],
          ),
        ),
      ),
    );

    return FocusableActionDetector(
      enabled: enabled,
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
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: enabled ? (_) => setState(() => _pressed = true) : null,
          onTapUp: enabled ? (_) => setState(() => _pressed = false) : null,
          onTapCancel: () => setState(() => _pressed = false),
          onTap: widget.onPressed,
          child: face,
        ),
      ),
    );
  }
}

/// An on/off switch in the accent colour, with the original's track and handle.
class CosmicToggle extends StatefulWidget {
  const CosmicToggle({super.key, required this.value, required this.onChanged});

  final bool value;
  final ValueChanged<bool>? onChanged;

  @override
  State<CosmicToggle> createState() => _CosmicToggleState();
}

class _CosmicToggleState extends State<CosmicToggle> {
  bool _focused = false;

  void _toggle() => widget.onChanged?.call(!widget.value);

  @override
  Widget build(BuildContext context) {
    final palette = CosmicScope.of(context);
    final enabled = widget.onChanged != null;
    return FocusableActionDetector(
      enabled: enabled,
      mouseCursor: enabled
          ? SystemMouseCursors.click
          : SystemMouseCursors.basic,
      onShowFocusHighlight: (value) => setState(() => _focused = value),
      actions: <Type, Action<Intent>>{
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (_) {
            _toggle();
            return null;
          },
        ),
      },
      child: Semantics(
        toggled: widget.value,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: enabled ? _toggle : null,
          child: Opacity(
            opacity: enabled ? 1 : 0.5,
            child: Container(
              width: 48,
              height: 24,
              decoration: BoxDecoration(
                color: widget.value ? palette.accent : palette.toggleOff,
                borderRadius: BorderRadius.circular(12),
                border: _focused
                    ? Border.all(color: palette.accent, width: 2)
                    : null,
              ),
              child: AnimatedAlign(
                duration: const Duration(milliseconds: 150),
                alignment: widget.value
                    ? Alignment.centerRight
                    : Alignment.centerLeft,
                child: Padding(
                  padding: const EdgeInsets.all(2),
                  child: Container(
                    width: 20,
                    height: 20,
                    decoration: BoxDecoration(
                      color: palette.toggleHandle,
                      shape: BoxShape.circle,
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// A text input that follows its text from the model. Typing reports each
/// change; a change made elsewhere (for example a file opened from the
/// picker) replaces the field's text without moving the user's caret away.
class CosmicTextField extends StatefulWidget {
  const CosmicTextField({
    super.key,
    required this.text,
    required this.onChanged,
    this.placeholder = '',
    this.onSubmitted,
    this.maxLines = 1,
    this.pill = false,
    this.leading,
    this.trailing,
  });

  final String text;
  final ValueChanged<String> onChanged;
  final String placeholder;
  final ValueChanged<String>? onSubmitted;

  /// Null means the field grows with its content.
  final int? maxLines;

  /// A fully rounded field, as the search field is in the original.
  final bool pill;
  final Widget? leading;
  final Widget? trailing;

  @override
  State<CosmicTextField> createState() => _CosmicTextFieldState();
}

class _CosmicTextFieldState extends State<CosmicTextField> {
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
  void didUpdateWidget(covariant CosmicTextField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.text != _controller.text) {
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
    final palette = CosmicScope.of(context);
    final ink = DefaultTextStyle.of(context).style.color ?? palette.windowText;
    final field = TextField(
      controller: _controller,
      focusNode: _focus,
      minLines: widget.maxLines == null ? 1 : null,
      maxLines: widget.maxLines,
      onChanged: widget.onChanged,
      onSubmitted: widget.onSubmitted,
      cursorColor: palette.accent,
      style: CosmicType.body.copyWith(color: ink),
      decoration: InputDecoration.collapsed(
        hintText: widget.placeholder,
        hintStyle: CosmicType.body.copyWith(color: ink.withValues(alpha: 0.5)),
      ),
    );
    return Container(
      constraints: const BoxConstraints(minHeight: 32),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      decoration: BoxDecoration(
        color: palette.inputFill,
        borderRadius: BorderRadius.circular(widget.pill ? 16 : CosmicRadius.s),
        border: Border.all(
          color: _focus.hasFocus ? palette.accent : palette.windowDivider,
          width: _focus.hasFocus ? 2 : 1,
        ),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
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

/// A rounded card of rows, under an optional heading, as the original's
/// settings sections are. Children are separated by hairline dividers.
class CosmicSection extends StatelessWidget {
  const CosmicSection({super.key, this.title, required this.children});

  final String? title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final palette = CosmicScope.of(context);
    final rows = <Widget>[];
    for (var i = 0; i < children.length; i++) {
      if (i > 0) {
        rows.add(Divider(height: 1, thickness: 1, color: palette.cardDivider));
      }
      rows.add(
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: children[i],
        ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (title != null) ...[
          Text(
            title!,
            style: CosmicType.heading.copyWith(color: palette.windowText),
          ),
          const SizedBox(height: 8),
        ],
        DecoratedBox(
          decoration: BoxDecoration(
            color: palette.cardBackground,
            borderRadius: BorderRadius.circular(CosmicRadius.s),
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(CosmicRadius.s),
            child: DefaultTextStyle(
              style: CosmicType.body.copyWith(color: palette.cardText),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: rows,
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// One row of a settings card: a label on the left and a control on the right.
class CosmicSettingRow extends StatelessWidget {
  const CosmicSettingRow({
    super.key,
    required this.label,
    required this.control,
  });

  final String label;
  final Widget control;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(child: Text(label, style: CosmicType.body)),
        const SizedBox(width: 12),
        control,
      ],
    );
  }
}

/// A caption-sized label beside a body-sized value. Stacked when narrow.
class LabelValueRow extends StatelessWidget {
  const LabelValueRow({
    super.key,
    required this.label,
    required this.value,
    required this.narrow,
  });

  final String label;
  final String value;
  final bool narrow;

  @override
  Widget build(BuildContext context) {
    final caption = Text(label, style: CosmicType.caption);
    final body = Text(value, style: CosmicType.body);
    if (narrow) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [caption, const SizedBox(height: 2), body],
      );
    }
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(flex: 2, child: caption),
        const SizedBox(width: 8),
        Expanded(flex: 5, child: body),
      ],
    );
  }
}

/// A thin determinate bar in the accent colour.
class CosmicProgressBar extends StatelessWidget {
  const CosmicProgressBar({super.key, required this.percent});

  final int percent;

  @override
  Widget build(BuildContext context) {
    final palette = CosmicScope.of(context);
    final fraction = (percent.clamp(0, 100)) / 100;
    return SizedBox(
      height: 4,
      child: Stack(
        children: [
          DecoratedBox(
            decoration: BoxDecoration(
              color: palette.navSelected,
              borderRadius: BorderRadius.circular(2),
            ),
            child: const SizedBox.expand(),
          ),
          FractionallySizedBox(
            widthFactor: fraction,
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: palette.accent,
                borderRadius: BorderRadius.circular(2),
              ),
              child: const SizedBox.expand(),
            ),
          ),
        ],
      ),
    );
  }
}

/// The body of a dialog: a title, a sentence or two, and the buttons. The
/// primary action is the rightmost button, as in the original.
class CosmicDialogBox extends StatelessWidget {
  const CosmicDialogBox({
    super.key,
    required this.title,
    required this.body,
    required this.primary,
    this.secondary,
    this.tertiary,
  });

  final String title;
  final String body;
  final Widget primary;
  final Widget? secondary;
  final Widget? tertiary;

  @override
  Widget build(BuildContext context) {
    final palette = CosmicScope.of(context);
    return Center(
      child: Container(
        constraints: const BoxConstraints(maxWidth: 480),
        margin: const EdgeInsets.all(24),
        padding: const EdgeInsets.all(24),
        decoration: BoxDecoration(
          color: palette.primarySurface,
          border: Border.all(color: palette.primaryDivider),
          borderRadius: BorderRadius.circular(CosmicRadius.m),
          boxShadow: [
            BoxShadow(
              color: palette.shade,
              blurRadius: 16,
              offset: const Offset(0, 4),
            ),
          ],
        ),
        child: DefaultTextStyle(
          style: CosmicType.body.copyWith(color: palette.primaryText),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                title,
                style: CosmicType.title3.copyWith(color: palette.primaryText),
              ),
              const SizedBox(height: 16),
              Text(body),
              const SizedBox(height: 24),
              Row(
                children: [
                  ?tertiary,
                  const Spacer(),
                  if (secondary case final button?) ...[
                    button,
                    const SizedBox(width: 8),
                  ],
                  primary,
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
