import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// Checks every app's update source and applies updates on request. Failures
/// are listed before the offers, so "up to date" is never claimed for an app
/// that could not be checked.
class UpdatesPage extends StatelessWidget {
  const UpdatesPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final narrow = isCondensed(context);
    final controls = [
      CosmicButton(label: 'Check now', onPressed: model.checkUpdates),
      CosmicButton(
        label: 'Update all',
        kind: CosmicButtonKind.suggested,
        onPressed: model.updateAll,
      ),
    ];
    final busy = model.busy != null;
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Text('Updates', style: CosmicType.title3),
              const Spacer(),
              if (narrow)
                Column(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    for (final button in controls) ...[
                      button,
                      const SizedBox(height: 4),
                    ],
                  ],
                )
              else
                Row(
                  children: [
                    for (final button in controls) ...[
                      button,
                      const SizedBox(width: 8),
                    ],
                  ],
                ),
            ],
          ),
          const SizedBox(height: 8),
          if (busy) const Text('Checking update sources…'),
          if (model.checkFailures.isNotEmpty) ...[
            Text('Some apps could not be checked', style: CosmicType.heading),
            for (final problem in model.checkFailures)
              Text('${problem.name}: ${problem.error}'),
          ],
          if (model.updateFailures.isNotEmpty) ...[
            Text('Some updates did not apply', style: CosmicType.heading),
            for (final problem in model.updateFailures)
              Text('${problem.name}: ${problem.error}'),
          ],
          if (model.updates.isEmpty &&
              model.checkFailures.isEmpty &&
              model.updateFailures.isEmpty &&
              !busy)
            Text(
              model.library.isEmpty
                  ? 'No AppImages are integrated yet.'
                  : 'Everything is up to date.',
            ),
          for (final offer in model.updates)
            _OfferRow(model: model, offer: offer),
        ],
      ),
    );
  }
}

class _OfferRow extends StatelessWidget {
  const _OfferRow({required this.model, required this.offer});

  final AppModel model;
  final UpdateOfferDto offer;

  @override
  Widget build(BuildContext context) {
    final labels = [
      '${offer.currentVersion.isEmpty ? 'unknown' : offer.currentVersion} '
          '→ ${offer.availableVersion} (${offer.manager})',
      if (offer.running)
        'running — updating replaces the file under a live app',
      if (offer.reducedVerification)
        'reduced verification: no checksum published',
      if (offer.downloadSize > 0) humanSize(offer.downloadSize),
    ];
    return Padding(
      padding: const EdgeInsets.all(8),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(offer.name, style: CosmicType.heading),
                for (final label in labels)
                  Text(label, style: CosmicType.caption),
              ],
            ),
          ),
          const SizedBox(width: 8),
          CosmicButton(
            label: 'Update',
            kind: CosmicButtonKind.suggested,
            onPressed: () => model.updateOne(offer.uuid),
          ),
        ],
      ),
    );
  }
}
