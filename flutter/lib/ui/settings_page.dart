import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

String _appearanceName(AppearanceChoice choice) => switch (choice) {
  AppearanceChoice.system => 'System',
  AppearanceChoice.light => 'Light',
  AppearanceChoice.dark => 'Dark',
};

/// Appearance, the integration folder, behaviour, update checks and the unsafe
/// fallback. Every change is written as it is made.
class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final settings = model.settings;
    final appearance = settings?.appearance ?? AppearanceChoice.system;
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text('Settings', style: CosmicType.title3),
          const SizedBox(height: 12),
          CosmicSection(
            title: 'Appearance',
            children: [
              Row(
                children: [
                  for (final choice in AppearanceChoice.values) ...[
                    CosmicButton(
                      label: _appearanceName(choice),
                      onPressed: () => model.setAppearance(choice),
                    ),
                    const SizedBox(width: 8),
                  ],
                  Flexible(
                    child: Text(
                      'Current: ${_appearanceName(appearance)}',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: 12),
          CosmicSection(
            title: 'Integration folder',
            children: [
              Row(
                children: [
                  Expanded(
                    child: CosmicTextField(
                      text: model.managedFolderInput,
                      placeholder: 'Managed folder',
                      onChanged: model.setManagedFolderInput,
                      onSubmitted: (_) => model.applyManagedFolder(),
                    ),
                  ),
                  const SizedBox(width: 8),
                  CosmicButton(
                    label: 'Apply',
                    onPressed: model.applyManagedFolder,
                  ),
                ],
              ),
              Row(
                children: [
                  Expanded(
                    child: CosmicTextField(
                      text: model.maxBytesInput,
                      placeholder: 'Max size (MB)',
                      onChanged: model.setMaxBytesInput,
                      onSubmitted: (_) => model.applyMaxBytes(),
                    ),
                  ),
                  const SizedBox(width: 8),
                  CosmicButton(label: 'Apply', onPressed: model.applyMaxBytes),
                ],
              ),
              const Text(
                'Largest AppImage to integrate or download, in megabytes '
                '(1–32768, default 8192). Oversized files are refused.',
                style: CosmicType.caption,
              ),
            ],
          ),
          const SizedBox(height: 12),
          CosmicSection(
            title: 'Behaviour',
            children: [
              CosmicSettingRow(
                label: 'Move the original into the library instead of copying',
                control: CosmicToggle(
                  value: settings?.moveSource ?? false,
                  onChanged: settings == null ? null : model.setMoveSource,
                ),
              ),
              CosmicSettingRow(
                label: 'Discover AppImages outside the managed folder',
                control: CosmicToggle(
                  value: settings?.manageOutsideFolder ?? false,
                  onChanged: settings == null
                      ? null
                      : model.setManageOutsideFolder,
                ),
              ),
              CosmicSettingRow(
                label: 'Terminal apps: drop the .AppImage suffix from the name',
                control: CosmicToggle(
                  value: settings?.terminalOmitSuffix ?? false,
                  onChanged: settings == null
                      ? null
                      : model.setTerminalOmitSuffix,
                ),
              ),
              CosmicSettingRow(
                label: 'Verbose diagnostics',
                control: CosmicToggle(
                  value: settings?.debugLogging ?? false,
                  onChanged: settings == null ? null : model.setDebugLogging,
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          CosmicSection(
            title: 'Update checks',
            children: [
              CosmicSettingRow(
                label: 'Check for updates in the background (notify only)',
                control: CosmicToggle(
                  value: settings?.backgroundUpdateChecks ?? false,
                  onChanged: settings == null
                      ? null
                      : model.setBackgroundUpdateChecks,
                ),
              ),
              CosmicSettingRow(
                label: 'Run those checks at login',
                control: CosmicToggle(
                  value: model.autostartEnabled,
                  onChanged: settings == null ? null : model.setAutostart,
                ),
              ),
              const Text(
                'Background checks only look at the update endpoints you '
                'configured, and never download or apply anything.',
                style: CosmicType.caption,
              ),
            ],
          ),
          const SizedBox(height: 12),
          CosmicSection(
            title: 'Unsafe extraction fallback',
            children: [
              CosmicSettingRow(
                label: 'Run the AppImage to read its metadata when safe extraction fails',
                control: CosmicToggle(
                  value: settings?.unsafeExtractionFallback ?? false,
                  onChanged: settings == null
                      ? null
                      : model.requestUnsafeFallback,
                ),
              ),
              const Text(
                'Off by default. This executes untrusted code, and each file '
                'still has to be confirmed individually.',
                style: CosmicType.caption,
              ),
            ],
          ),
        ],
      ),
    );
  }
}
