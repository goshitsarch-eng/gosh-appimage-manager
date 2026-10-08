import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// The modal for the model's pending question. Every destructive choice in
/// the application goes through one of these, with the same words the
/// original uses.
Widget buildDialog(AppModel model, PendingDialog dialog) {
  return switch (dialog) {
    IntegrateConflictDialog() => CosmicDialogBox(
      title: 'An AppImage with that name is already managed',
      body:
          '${dialog.conflictName} would collide with ${dialog.replaceLabel}. '
          'Keep both copies, or replace the managed installation?',
      tertiary: CosmicButton(label: 'Cancel', onPressed: model.dismissDialog),
      secondary: dialog.replaceUuid.isEmpty
          ? null
          : CosmicButton(
              label: 'Replace ${dialog.replaceLabel}',
              kind: CosmicButtonKind.destructive,
              onPressed: () =>
                  model.replaceAndIntegrate(dialog.path, dialog.replaceUuid),
            ),
      primary: CosmicButton(
        label: 'Keep both',
        kind: CosmicButtonKind.suggested,
        onPressed: () => model.keepBothAndIntegrate(dialog.path),
      ),
    ),
    RemoveDialog() => CosmicDialogBox(
      title: dialog.permanent
          ? 'Permanently delete ${dialog.name}?'
          : 'Move ${dialog.name} to Trash?',
      body: dialog.permanent
          ? '${dialog.path} will be deleted immediately. This cannot be undone, '
                'and the Gosh desktop entry and icon are removed with it.'
          : '${dialog.path} goes to Trash. The desktop entry and icon are '
                'removed only after Trash succeeds.',
      secondary: CosmicButton(label: 'Cancel', onPressed: model.dismissDialog),
      primary: CosmicButton(
        label: dialog.permanent ? 'Delete permanently' : 'Move to Trash',
        kind: CosmicButtonKind.destructive,
        onPressed: model.confirmRemove,
      ),
    ),
    UnsafeExtractDialog() => CosmicDialogBox(
      title: 'Allow running AppImages to read metadata?',
      body:
          "When safe extraction fails, this runs the AppImage's own "
          '--appimage-extract, which executes untrusted code. Each file still '
          'has to be confirmed separately, and background checks never use it.',
      secondary: CosmicButton(
        label: 'Keep off',
        onPressed: model.dismissDialog,
      ),
      primary: CosmicButton(
        label: 'Enable',
        kind: CosmicButtonKind.destructive,
        onPressed: model.confirmUnsafeFallback,
      ),
    ),
    UpdateForceDialog() => CosmicDialogBox(
      title: '${dialog.name} is running',
      body:
          'Updating replaces the file underneath a running application, which '
          'may make it behave unpredictably until it is restarted.',
      secondary: CosmicButton(label: 'Cancel', onPressed: model.dismissDialog),
      primary: CosmicButton(
        label: 'Update anyway',
        kind: CosmicButtonKind.destructive,
        onPressed: model.confirmForceUpdate,
      ),
    ),
    AdoptDialog() => CosmicDialogBox(
      title: 'Adopt this AppImage?',
      body:
          '${dialog.path} will be registered so it can be updated and removed '
          'here. Nothing on disk is changed, and its existing desktop entry is '
          'left alone.',
      secondary: CosmicButton(label: 'Cancel', onPressed: model.dismissDialog),
      primary: CosmicButton(
        label: 'Adopt',
        kind: CosmicButtonKind.suggested,
        onPressed: model.confirmAdopt,
      ),
    ),
  };
}
