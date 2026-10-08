import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// The dialog buttons are 30 px high (SPEC section 10), not the header's 32.
const double _dialogButtonHeight = 30;

/// The modal for the model's pending question. Every destructive choice in
/// the application goes through one of these (SPEC section 10).
///
/// The dialog is a focus scope that loops: Tab and Shift+Tab stay inside it
/// (QA D-07), and Cancel, the safe choice, takes focus when it opens. Escape
/// closes it through the shell's shortcut handler.
Widget buildDialog(AppModel model, PendingDialog dialog) {
  return FocusScope(
    debugLabel: 'app-dialog',
    child: switch (dialog) {
      RemoveDialog() => AppDialogCard(
        title: dialog.permanent
            ? 'Permanently delete ${dialog.name}?'
            : 'Move ${dialog.name} to the Trash?',
        body: dialog.permanent
            ? '${dialog.path} will be deleted immediately. This cannot be undone, '
                  'and the Gosh desktop entry and icon are removed with it.'
            : 'The AppImage moves to the Trash and its menu entry is removed.',
        buttons: [
          AppButton(
            label: 'Cancel',
            height: _dialogButtonHeight,
            onPressed: model.dismissDialog,
            autofocus: true,
            buttonKey: const Key('dialog-cancel'),
          ),
          dialog.permanent
              ? AppButton(
                  label: 'Delete permanently',
                  variant: AppButtonVariant.destructive,
                  height: _dialogButtonHeight,
                  onPressed: model.confirmRemove,
                  buttonKey: const Key('dialog-confirm'),
                )
              : AppButton(
                  label: 'Move to Trash',
                  variant: AppButtonVariant.primary,
                  height: _dialogButtonHeight,
                  onPressed: model.confirmRemove,
                  buttonKey: const Key('dialog-confirm'),
                ),
        ],
      ),
      IntegrateConflictDialog() => AppDialogCard(
        title: '${dialog.replaceLabel} is already integrated',
        body:
            'The installed copy is ${dialog.installedVersion.isEmpty ? 'unknown' : dialog.installedVersion}. '
            'The file you are integrating is ${dialog.incomingVersion.isEmpty ? 'unknown' : dialog.incomingVersion}. '
            'Keep both adds the new copy beside the existing one. '
            'Replace swaps the installed copy out.',
        buttons: [
          AppButton(
            label: 'Cancel',
            height: _dialogButtonHeight,
            onPressed: model.dismissDialog,
            autofocus: true,
            buttonKey: const Key('dialog-cancel'),
          ),
          AppButton(
            label: 'Keep both',
            height: _dialogButtonHeight,
            onPressed: () => model.keepBothAndIntegrate(dialog.path),
            buttonKey: const Key('dialog-keep-both'),
          ),
          if (dialog.replaceUuid.isNotEmpty)
            AppButton(
              label: 'Replace',
              variant: AppButtonVariant.primary,
              height: _dialogButtonHeight,
              onPressed: () =>
                  model.replaceAndIntegrate(dialog.path, dialog.replaceUuid),
              buttonKey: const Key('dialog-replace'),
            ),
        ],
      ),
      UpdateForceDialog() => AppDialogCard(
        title: '${dialog.name} is running',
        body:
            'Updating now replaces an app that is open. Close it first, or update '
            'anyway.',
        buttons: [
          AppButton(
            label: 'Cancel',
            height: _dialogButtonHeight,
            onPressed: model.dismissDialog,
            autofocus: true,
            buttonKey: const Key('dialog-cancel'),
          ),
          AppButton(
            label: 'Update anyway',
            variant: AppButtonVariant.destructive,
            height: _dialogButtonHeight,
            onPressed: model.confirmForceUpdate,
            buttonKey: const Key('dialog-confirm'),
          ),
        ],
      ),
      FallbackConfirmDialog() => AppDialogCard(
        title: 'Run this AppImage to read it?',
        body:
            'Safe extraction could not read ${dialog.fileName}. The unsafe fallback '
            'can run the AppImage itself to read its files. Only continue if you '
            'trust this AppImage.',
        buttons: [
          AppButton(
            label: 'Cancel',
            height: _dialogButtonHeight,
            onPressed: model.dismissDialog,
            autofocus: true,
            buttonKey: const Key('dialog-cancel'),
          ),
          AppButton(
            label: 'Run anyway',
            variant: AppButtonVariant.destructive,
            height: _dialogButtonHeight,
            onPressed: model.confirmFallback,
            buttonKey: const Key('dialog-confirm'),
          ),
        ],
      ),
      UnsafeFallbackDialog() => AppDialogCard(
        title: 'Turn on the unsafe extraction fallback?',
        body:
            'When safe extraction fails, Gosh runs the AppImage itself to read its '
            'files. A running AppImage can do anything you can, so turn this on only '
            'for AppImages you trust.',
        buttons: [
          AppButton(
            label: 'Cancel',
            height: _dialogButtonHeight,
            onPressed: model.dismissDialog,
            autofocus: true,
            buttonKey: const Key('dialog-cancel'),
          ),
          AppButton(
            label: 'Turn on',
            variant: AppButtonVariant.destructive,
            height: _dialogButtonHeight,
            onPressed: model.confirmUnsafeFallback,
            buttonKey: const Key('dialog-confirm'),
          ),
        ],
      ),
      AdoptDialog() => AppDialogCard(
        title: 'Adopt this AppImage?',
        body:
            '${dialog.path} will be registered so it can be updated and removed '
            'here. Nothing on disk is changed, and its existing desktop entry is '
            'left alone.',
        buttons: [
          AppButton(
            label: 'Cancel',
            height: _dialogButtonHeight,
            onPressed: model.dismissDialog,
            autofocus: true,
            buttonKey: const Key('dialog-cancel'),
          ),
          AppButton(
            label: 'Adopt',
            variant: AppButtonVariant.primary,
            height: _dialogButtonHeight,
            onPressed: model.confirmAdopt,
            buttonKey: const Key('dialog-confirm'),
          ),
        ],
      ),
    },
  );
}

/// A confirmation card: 460 px wide, a 17 px title, 16 px body text, and the
/// buttons on the right with the last one as the primary.
class AppDialogCard extends StatelessWidget {
  const AppDialogCard({
    super.key,
    required this.title,
    required this.body,
    required this.buttons,
  });

  final String title;
  final String body;
  final List<Widget> buttons;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return RepaintBoundary(
      child: Container(
        width: 460,
        padding: const EdgeInsets.all(24),
        decoration: BoxDecoration(
          color: palette.surface,
          border: Border.all(color: palette.border),
          borderRadius: BorderRadius.circular(AppRadius.dialog),
          boxShadow: [
            BoxShadow(
              color: palette.shade.withValues(
                alpha: palette.isDark ? 0.3 : 0.06,
              ),
              blurRadius: 2,
              offset: const Offset(0, 1),
            ),
            BoxShadow(
              color: palette.shade.withValues(
                alpha: palette.isDark ? 0.5 : 0.22,
              ),
              blurRadius: 48,
              spreadRadius: -12,
              offset: const Offset(0, 24),
            ),
          ],
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              title,
              style: AppType.sans(
                17,
                weight: FontWeight.w600,
                letterSpacing: -0.17,
                // SPEC 10 gives the title a "normal" line height, not the window 1.4.
                // 1.22 is the value that puts its glyphs on the mockup rows.
                height: 1.22,
                color: palette.text,
              ),
            ),
            const SizedBox(height: 12),
            Text(
              body,
              style: AppType.sans(16, height: 1.55, color: palette.text2),
            ),
            const SizedBox(height: 12),
            Padding(
              padding: const EdgeInsets.only(top: 6),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  for (var i = 0; i < buttons.length; i++) ...[
                    if (i > 0) const SizedBox(width: 8),
                    buttons[i],
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
