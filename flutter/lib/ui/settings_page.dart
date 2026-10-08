import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/page_frame.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

String appearanceLabel(AppearanceChoice choice) => switch (choice) {
  AppearanceChoice.system => 'System',
  AppearanceChoice.light => 'Light',
  AppearanceChoice.dark => 'Dark',
};

/// The Settings page (SPEC 7.7): Appearance and Integration on the left,
/// Updates and Advanced on the right. Every change is written as it is made.
class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final narrow = isNarrow(context);
    final settings = model.settings;
    final enabled = settings != null;
    final backgroundOn = settings?.backgroundUpdateChecks ?? false;
    final gutter = narrow ? 14.0 : 28.0;
    final left = [
      SettingsCard(
        title: 'Appearance',
        rows: [
          SettingsRow(
            label: 'Theme',
            help: 'Applied live and restored at startup',
            control: AppSegmented<AppearanceChoice>(
              fontSize: 12.5,
              segmentPadding: 12,
              segments: [
                for (final choice in AppearanceChoice.values)
                  AppSegment(choice, appearanceLabel(choice)),
              ],
              selected: settings?.appearance ?? AppearanceChoice.system,
              onSelected: model.setAppearance,
            ),
          ),
        ],
      ),
      SettingsCard(
        title: 'Integration',
        rows: [
          SettingsRow(
            label: 'Managed folder',
            expandControl: true,
            control: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: _ManagedFolderField(
                        model: model,
                        enabled: enabled,
                      ),
                    ),
                    const SizedBox(width: 10),
                    AppButton(
                      label: 'Change…',
                      height: 28,
                      fontSize: 12.5,
                      buttonKey: const Key('change-folder'),
                      onPressed: enabled
                          ? () => chooseManagedFolder(model)
                          : null,
                    ),
                  ],
                ),
                if (model.managedFolderError case final error?)
                  Padding(
                    padding: const EdgeInsets.only(top: 6),
                    child: Text(
                      error,
                      key: const Key('managed-folder-error'),
                      textAlign: TextAlign.end,
                      style: AppType.sans(12, color: AppScope.of(context).bad),
                    ),
                  ),
              ],
            ),
          ),
          SettingsRow(
            label: 'Maximum file size',
            help: '1 to 32768 MB',
            control: _SizeField(model: model, enabled: enabled),
          ),
          SettingsRow(
            label: 'Move originals instead of copying',
            help: 'The source goes to the Trash after a verified copy',
            control: AppToggle(
              toggleKey: const Key('toggle-move'),
              value: settings?.moveSource ?? false,
              onChanged: enabled ? model.setMoveSource : null,
            ),
            alignStart: true,
          ),
          SettingsRow(
            label: 'Discover AppImages elsewhere',
            help: 'Lists AppImages with desktop entries for adoption',
            control: AppToggle(
              toggleKey: const Key('toggle-discover'),
              value: settings?.manageOutsideFolder ?? false,
              onChanged: enabled ? model.setManageOutsideFolder : null,
            ),
            alignStart: true,
          ),
          SettingsRow(
            label: 'Drop .AppImage from terminal app names',
            help: 'Applies to the name shown in the menu',
            control: AppToggle(
              toggleKey: const Key('toggle-terminal'),
              value: settings?.terminalOmitSuffix ?? false,
              onChanged: enabled ? model.setTerminalOmitSuffix : null,
            ),
            alignStart: true,
          ),
        ],
      ),
    ];
    final right = [
      SettingsCard(
        title: 'Updates',
        rows: [
          SettingsRow(
            label: 'Check in the background',
            help: 'Notifies only. Never downloads or applies.',
            control: AppToggle(
              toggleKey: const Key('toggle-background'),
              value: settings?.backgroundUpdateChecks ?? false,
              onChanged: enabled ? model.setBackgroundUpdateChecks : null,
            ),
            alignStart: true,
          ),
          SettingsRow(
            label: 'Also check at login',
            help: 'Adds an autostart entry for the same check',
            control: AppToggle(
              toggleKey: const Key('toggle-login'),
              value: model.autostartEnabled,
              // A login check needs background checks on.
              onChanged: enabled && backgroundOn ? model.setAutostart : null,
            ),
            alignStart: true,
          ),
        ],
      ),
      SettingsCard(
        title: 'Advanced',
        rows: [
          SettingsRow(
            label: 'Verbose diagnostics',
            help: 'Per-operation lines on stderr',
            control: AppToggle(
              toggleKey: const Key('toggle-debug'),
              value: settings?.debugLogging ?? false,
              onChanged: enabled ? model.setDebugLogging : null,
            ),
            alignStart: true,
          ),
          SettingsRow(
            label: 'Unsafe extraction fallback',
            help:
                'Runs the AppImage itself to read its files when safe extraction '
                'fails. It asks before each file. Only turn this on for AppImages '
                'you trust.',
            control: AppToggle(
              toggleKey: const Key('toggle-unsafe'),
              value: settings?.unsafeExtractionFallback ?? false,
              onChanged: enabled ? model.setUnsafeFallback : null,
            ),
            alignStart: true,
          ),
        ],
      ),
    ];
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PageHeader(
            title: 'Settings',
            subtitle: 'Options are off unless noted',
          ),
          Padding(
            padding: EdgeInsets.fromLTRB(gutter, 18, gutter, 24),
            child: narrow
                ? Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      for (final card in [...left, ...right]) ...[
                        card,
                        const SizedBox(height: 16),
                      ],
                    ],
                  )
                : Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            for (final card in left) ...[
                              card,
                              const SizedBox(height: 16),
                            ],
                          ],
                        ),
                      ),
                      const SizedBox(width: 20),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            for (final card in right) ...[
                              card,
                              const SizedBox(height: 16),
                            ],
                          ],
                        ),
                      ),
                    ],
                  ),
          ),
        ],
      ),
    );
  }
}

/// A settings card: a 44 px heading and rows separated by hairlines.
class SettingsCard extends StatelessWidget {
  const SettingsCard({super.key, required this.title, required this.rows});

  final String title;
  final List<Widget> rows;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return AppCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          AppCardHeader(title: title),
          for (var i = 0; i < rows.length; i++)
            Container(
              decoration: BoxDecoration(
                border: i == rows.length - 1
                    ? null
                    : Border(bottom: BorderSide(color: palette.border)),
              ),
              child: rows[i],
            ),
        ],
      ),
    );
  }
}

/// One setting: its label and help on the left, its control on the right.
class SettingsRow extends StatelessWidget {
  const SettingsRow({
    super.key,
    required this.label,
    this.control,
    this.help,
    this.pill,
    this.alignStart = false,
    this.expandControl = false,
  });

  final String label;
  final String? help;

  /// The value or control on the right. A row that only states something has
  /// none.
  final Widget? control;
  final String? pill;
  final bool alignStart;

  /// The control takes three quarters of the row beside the label, so a long
  /// value inside it ends with an ellipsis instead of pushing the row wider
  /// (QA D-09).
  final bool expandControl;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final text = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Flexible(
              child: Text(
                label,
                style: AppType.sans(
                  13,
                  weight: FontWeight.w500,
                  color: palette.text,
                ),
              ),
            ),
            if (pill != null) ...[const SizedBox(width: 8), AppPill(pill!)],
          ],
        ),
        if (help != null)
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Text(
              help!,
              style: AppType.sans(12, height: 1.5, color: palette.text2),
            ),
          ),
      ],
    );
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: Row(
        crossAxisAlignment: alignStart
            ? CrossAxisAlignment.start
            : CrossAxisAlignment.center,
        children: [
          if (expandControl) Flexible(child: text) else Expanded(child: text),
          if (control != null) ...[
            const SizedBox(width: 16),
            if (expandControl) Expanded(flex: 3, child: control!) else control!,
          ],
        ],
      ),
    );
  }
}

/// The maximum file size: a number field with its unit, sharing one border.
/// Enter applies it; the core checks the range.
class _SizeField extends StatefulWidget {
  const _SizeField({required this.model, required this.enabled});

  final AppModel model;
  final bool enabled;

  @override
  State<_SizeField> createState() => _SizeFieldState();
}

class _SizeFieldState extends State<_SizeField> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.model.maxBytesInput,
  );
  final FocusNode _focus = FocusNode(debugLabel: 'max-size');

  @override
  void initState() {
    super.initState();
    _focus.addListener(_onFocusChanged);
  }

  /// Leaving the field applies what was typed, as Enter does, so a typed value
  /// is not dropped (QA D-17). A bad value is reported by the model on the
  /// status line, and the text stays for correction.
  void _onFocusChanged() {
    if (_focus.hasFocus) {
      return;
    }
    final saved = widget.model.settings?.maxAppimageBytes;
    if (saved == null) {
      return;
    }
    if (widget.model.maxBytesInput != maxAppimageMbFor(saved).toString()) {
      widget.model.applyMaxBytes();
    }
  }

  @override
  void didUpdateWidget(covariant _SizeField oldWidget) {
    super.didUpdateWidget(oldWidget);
    final text = widget.model.maxBytesInput;
    if (text != _controller.text) {
      _controller.value = TextEditingValue(
        text: text,
        selection: TextSelection.collapsed(offset: text.length),
      );
    }
  }

  @override
  void dispose() {
    _focus.removeListener(_onFocusChanged);
    _focus.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Opacity(
      opacity: widget.enabled ? 1 : 0.5,
      child: Container(
        height: 28,
        decoration: BoxDecoration(
          border: Border.all(color: palette.border),
          borderRadius: BorderRadius.circular(AppRadius.control),
        ),
        clipBehavior: Clip.antiAlias,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox(
              width: 49,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 10),
                child: Center(
                  child: TextField(
                    key: const Key('max-size-field'),
                    controller: _controller,
                    focusNode: _focus,
                    enabled: widget.enabled,
                    maxLines: 1,
                    textAlignVertical: TextAlignVertical.center,
                    onChanged: widget.model.setMaxBytesInput,
                    onSubmitted: (_) => widget.model.applyMaxBytes(),
                    style: AppType.mono(12, color: palette.text),
                    decoration: const InputDecoration.collapsed(hintText: ''),
                  ),
                ),
              ),
            ),
            Container(
              height: 28,
              padding: const EdgeInsets.symmetric(horizontal: 8),
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: palette.canvas,
                border: Border(left: BorderSide(color: palette.border)),
              ),
              child: Text('MB', style: AppType.sans(12, color: palette.text2)),
            ),
          ],
        ),
      ),
    );
  }
}

/// The managed folder's path, typed. Enter saves it, and so does leaving the
/// field with a changed path. The model checks it, as it checks a chosen one.
class _ManagedFolderField extends StatefulWidget {
  const _ManagedFolderField({required this.model, required this.enabled});

  final AppModel model;
  final bool enabled;

  @override
  State<_ManagedFolderField> createState() => _ManagedFolderFieldState();
}

class _ManagedFolderFieldState extends State<_ManagedFolderField> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.model.managedFolderPath,
  );

  /// The path as last saved, to see when the core has changed it.
  late String _saved = widget.model.managedFolderPath;
  final FocusNode _focus = FocusNode(debugLabel: 'managed-folder');

  @override
  void initState() {
    super.initState();
    _focus.addListener(_onFocusChanged);
  }

  @override
  void didUpdateWidget(covariant _ManagedFolderField oldWidget) {
    super.didUpdateWidget(oldWidget);
    final saved = widget.model.managedFolderPath;
    if (saved != _saved) {
      _saved = saved;
      _controller.value = TextEditingValue(
        text: saved,
        selection: TextSelection.collapsed(offset: saved.length),
      );
    }
  }

  void _onFocusChanged() {
    setState(() {});
    if (!_focus.hasFocus && _controller.text.trim() != _saved) {
      _apply();
    }
  }

  void _apply() {
    widget.model.setManagedFolderInput(_controller.text);
    widget.model.applyManagedFolder();
  }

  @override
  void dispose() {
    _focus.removeListener(_onFocusChanged);
    _focus.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Opacity(
      opacity: widget.enabled ? 1 : 0.5,
      child: Container(
        height: 28,
        padding: const EdgeInsets.symmetric(horizontal: 8),
        alignment: Alignment.centerLeft,
        decoration: BoxDecoration(
          border: Border.all(
            color: _focus.hasFocus ? palette.accent : palette.border,
          ),
          borderRadius: BorderRadius.circular(AppRadius.control),
        ),
        child: TextField(
          key: const Key('managed-folder-field'),
          controller: _controller,
          focusNode: _focus,
          enabled: widget.enabled,
          maxLines: 1,
          textAlignVertical: TextAlignVertical.center,
          onSubmitted: (_) => _apply(),
          style: AppType.mono(12, color: palette.text),
          decoration: const InputDecoration.collapsed(hintText: ''),
        ),
      ),
    );
  }
}
