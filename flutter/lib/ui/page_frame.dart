import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// A page's heading: the title at 20 px and an optional subtitle, with the
/// page's actions on the right (SPEC 7: padding 20px 28px 0). In the narrow
/// frame the title bar carries the title, so only the actions remain.
class PageHeader extends StatelessWidget {
  const PageHeader({
    super.key,
    required this.title,
    this.subtitle,
    this.actions = const [],
  });

  final String title;
  final String? subtitle;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final narrow = isNarrow(context);
    final actionRow = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (var i = 0; i < actions.length; i++) ...[
          if (i > 0) const SizedBox(width: 8),
          actions[i],
        ],
      ],
    );
    if (narrow) {
      return Padding(
        padding: const EdgeInsets.fromLTRB(14, 12, 14, 0),
        child: actions.isEmpty
            ? const SizedBox.shrink()
            : Align(alignment: Alignment.centerRight, child: actionRow),
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(28, 20, 28, 0),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: AppType.sans(
                    20,
                    weight: FontWeight.w600,
                    letterSpacing: -0.3,
                    color: palette.text,
                  ),
                ),
                if (subtitle != null) ...[
                  const SizedBox(height: 3),
                  Text(
                    subtitle!,
                    style: AppType.sans(13, color: palette.text2),
                  ),
                ],
              ],
            ),
          ),
          if (actions.isNotEmpty) actionRow,
        ],
      ),
    );
  }
}
