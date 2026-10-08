import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// Looks at one AppImage without integrating or running it, then offers to
/// integrate it when the inspection passed.
class InspectPage extends StatelessWidget {
  const InspectPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final narrow = isCondensed(context);
    final inspect = model.inspect;
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text('Inspect an AppImage', style: CosmicType.title3),
          const Text(
            'Opening a file only inspects it. Nothing is integrated or executed.',
            style: CosmicType.caption,
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: CosmicTextField(
                  text: inspect.pathInput,
                  placeholder: 'Path to .AppImage',
                  onChanged: model.setInspectPath,
                  onSubmitted: (_) => model.inspectFromInput(),
                ),
              ),
              const SizedBox(width: 8),
              CosmicButton(
                label: 'Browse…',
                onPressed: () => chooseAppImages(model),
              ),
              const SizedBox(width: 8),
              CosmicButton(
                label: 'Inspect',
                kind: CosmicButtonKind.suggested,
                onPressed: model.inspectFromInput,
              ),
            ],
          ),
          if (inspect.queued.isNotEmpty)
            Text(
              '${inspect.queued.length} more file(s) queued; each is inspected '
              'and confirmed on its own.',
              style: CosmicType.caption,
            ),
          if (model.busy != null && inspect.summary.isEmpty)
            const Text('Inspecting… this reads and hashes the file.'),
          if (inspect.error.isNotEmpty) ...[
            Text('Cannot inspect this file', style: CosmicType.heading),
            Text(inspect.error),
          ],
          for (final entry in inspect.summary)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: LabelValueRow(
                label: entry.key,
                value: entry.value,
                narrow: narrow,
              ),
            ),
          for (final warning in inspect.warnings) Text('Warning: $warning'),
          if (inspect.inspectedOk)
            Align(
              alignment: Alignment.centerLeft,
              child: CosmicButton(
                label: 'Integrate',
                kind: CosmicButtonKind.suggested,
                onPressed: model.integrateFromInspect,
              ),
            ),
        ],
      ),
    );
  }
}
