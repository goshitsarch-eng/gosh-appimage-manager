import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';

/// Product identity and the statements the original makes about itself. The
/// product name is not translated, as in the original.
class AboutPage extends StatelessWidget {
  const AboutPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final version = model.version;
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Gosh AppImage Manager', style: CosmicType.title3),
          Text(version == null ? 'Version' : 'Version $version'),
          const Text('Made by Gosh'),
          const Text(
            'Native COSMIC Epoch application for safely inspecting, '
            'integrating, launching, organizing, updating, and removing '
            'AppImages.',
          ),
          const Text(
            'Opening an AppImage never integrates or executes it. Updates are '
            'staged, validated, and applied atomically with rollback.',
          ),
          const Text(
            'Behavioural reference: Gear Lever by Lorenzo Paderi. This is an '
            'independent original implementation and is not endorsed by its '
            'authors.',
          ),
          const Text(
            'Licensed under the GNU General Public License, version 3 or later. '
            'This program comes with absolutely no warranty.',
          ),
          const Text('No telemetry of any kind.'),
        ],
      ),
    );
  }
}
