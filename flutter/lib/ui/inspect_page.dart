import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/page_frame.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// Looks at one AppImage without integrating or running it (SPEC 7.4). The
/// banner says so; Integrate copies the file in only when pressed.
class InspectPage extends StatelessWidget {
  const InspectPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final narrow = isNarrow(context);
    final inspect = model.inspect;
    final hasResult = inspect.summary.isNotEmpty;
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PageHeader(
            title: 'Inspect',
            subtitle: "Read a file's metadata before you integrate it",
            actions: [
              if (!narrow)
                Container(
                  key: const Key('inspect-banner'),
                  height: 28,
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  decoration: BoxDecoration(
                    color: palette.okSoft,
                    borderRadius: BorderRadius.circular(14),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      AppIcon('banner-check', size: 12, color: palette.ok),
                      const SizedBox(width: 7),
                      Text(
                        'Nothing is executed or installed',
                        style: AppType.sans(
                          12,
                          weight: FontWeight.w500,
                          color: palette.ok,
                        ),
                      ),
                    ],
                  ),
                ),
            ],
          ),
          if (!hasResult || narrow)
            Padding(
              padding: EdgeInsets.fromLTRB(
                narrow ? 14 : 28,
                14,
                narrow ? 14 : 28,
                0,
              ),
              child: _PathRow(model: model),
            ),
          if (model.busy != null && !hasResult)
            Padding(
              padding: EdgeInsets.fromLTRB(
                narrow ? 14 : 28,
                12,
                narrow ? 14 : 28,
                0,
              ),
              child: Text(
                'Inspecting… this reads and hashes the file.',
                style: AppType.sans(13, color: palette.text2),
              ),
            ),
          if (inspect.error.isNotEmpty)
            Padding(
              padding: EdgeInsets.fromLTRB(
                narrow ? 14 : 28,
                12,
                narrow ? 14 : 28,
                0,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Cannot inspect this file',
                    style: AppType.sans(
                      13,
                      weight: FontWeight.w600,
                      color: palette.bad,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    inspect.error,
                    key: const Key('inspect-error'),
                    style: AppType.sans(13, color: palette.text2),
                  ),
                ],
              ),
            ),
          // The metadata and the Integrate card show only for a run that passed.
          // A stopped or failed run shows its error instead, and offers nothing
          // to integrate (QA3-009).
          if (hasResult && inspect.inspectedOk)
            Padding(
              padding: EdgeInsets.fromLTRB(
                narrow ? 14 : 28,
                18,
                narrow ? 14 : 28,
                24,
              ),
              child: narrow
                  ? Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        _MetadataCard(model: model),
                        const SizedBox(height: 16),
                        _IntegrateCard(model: model),
                      ],
                    )
                  : Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(flex: 155, child: _MetadataCard(model: model)),
                        const SizedBox(width: 20),
                        Expanded(
                          flex: 100,
                          child: _IntegrateCard(model: model),
                        ),
                      ],
                    ),
            ),
        ],
      ),
    );
  }
}

/// The path field with Browse… and Inspect, shown until a file is inspected.
class _PathRow extends StatelessWidget {
  const _PathRow({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: AppTextField(
            fieldKey: const Key('inspect-path'),
            text: model.inspect.pathInput,
            placeholder: 'Path to .AppImage',
            mono: true,
            fontSize: 12.5,
            onChanged: model.setInspectPath,
            onSubmitted: (_) => model.inspectFromInput(),
          ),
        ),
        const SizedBox(width: 8),
        AppButton(
          label: 'Browse…',
          hint: 'Ctrl O',
          buttonKey: const Key('inspect-browse'),
          onPressed: () => chooseAppImages(model),
        ),
        const SizedBox(width: 8),
        AppButton(
          label: 'Inspect',
          variant: AppButtonVariant.primary,
          buttonKey: const Key('inspect-run'),
          onPressed: model.inspectFromInput,
        ),
      ],
    );
  }
}

/// The metadata card: the file's identity, then its type, architecture and
/// size, then categories, icon, update source and checksum.
class _MetadataCard extends StatelessWidget {
  const _MetadataCard({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final inspect = model.inspect;
    final result = model.inspected;
    final name = inspect.name.isEmpty ? '(unknown)' : inspect.name;
    final version = inspect.version;
    final initial = name.substring(0, 1).toUpperCase();
    final tile = palette.tileFor(initial);
    final sourceLabel = result == null || result.embeddedUpdate.isEmpty
        ? '(none embedded)'
        : result.embeddedUpdate;
    final categories = result == null || result.categories.isEmpty
        ? 'None'
        : result.categories.join(', ');
    final architectureOk = result?.architectureSupported ?? false;
    return AppCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: const EdgeInsets.fromLTRB(16, 18, 16, 18),
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: palette.border)),
            ),
            child: Row(
              children: [
                Container(
                  width: 52,
                  height: 52,
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: tile.background,
                    borderRadius: BorderRadius.circular(AppRadius.tileLarge),
                  ),
                  child: Text(
                    initial,
                    style: AppType.sans(
                      22,
                      weight: FontWeight.w600,
                      color: tile.foreground,
                    ),
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        crossAxisAlignment: CrossAxisAlignment.baseline,
                        textBaseline: TextBaseline.alphabetic,
                        children: [
                          Flexible(
                            child: Text(
                              name,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: AppType.sans(
                                18,
                                weight: FontWeight.w600,
                                letterSpacing: -0.18,
                                color: palette.text,
                              ),
                            ),
                          ),
                          const SizedBox(width: 10),
                          Text(
                            version,
                            style: AppType.mono(13, color: palette.text2),
                          ),
                        ],
                      ),
                      const SizedBox(height: 3),
                      Text(
                        shortenHome(inspect.pathInput, homeFolder()),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AppType.mono(12, color: palette.text3),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          Container(
            padding: const EdgeInsets.symmetric(vertical: 12),
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: palette.border)),
            ),
            child: Row(
              children: [
                _MetaCell(
                  label: 'Type',
                  value: Text(
                    result?.appType ?? '',
                    style: AppType.sans(
                      13,
                      weight: FontWeight.w500,
                      color: palette.text,
                    ),
                  ),
                  first: true,
                ),
                _MetaCell(
                  label: 'Architecture',
                  first: true,
                  value: Text.rich(
                    TextSpan(
                      children: [
                        TextSpan(
                          text: result?.architecture ?? '',
                          style: AppType.sans(
                            13,
                            weight: FontWeight.w500,
                            color: palette.text,
                          ),
                        ),
                        if (architectureOk)
                          TextSpan(
                            text: ' · matches',
                            style: AppType.sans(
                              13,
                              weight: FontWeight.w500,
                              color: palette.ok,
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
                _MetaCell(
                  label: 'Size',
                  value: Text(
                    result == null ? '' : humanSize(result.sizeBytes),
                    style: AppType.sans(
                      13,
                      weight: FontWeight.w500,
                      color: palette.text,
                    ),
                  ),
                ),
              ],
            ),
          ),
          _FactRow(
            label: 'Categories',
            value: Text(
              categories,
              style: AppType.sans(13, color: palette.text),
            ),
          ),
          _FactRow(
            label: 'Icon',
            value: Text(
              result != null && result.iconName.isNotEmpty
                  ? 'Extracted from the AppImage'
                  : 'None',
              style: AppType.sans(13, color: palette.text),
            ),
          ),
          _FactRow(
            label: 'Update source',
            value: Text(
              sourceLabel,
              style: AppType.sans(13, color: palette.text),
            ),
          ),
          _FactRow(
            label: 'SHA-256',
            last: true,
            value: Text(
              result?.sha256 ?? '',
              style: AppType.mono(11.5, height: 1.55, color: palette.text2),
            ),
          ),
          if (inspect.warnings.isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 10, 16, 12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  for (final warning in inspect.warnings)
                    Text(
                      'Warning: $warning',
                      style: AppType.sans(12, color: palette.warnText),
                    ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _MetaCell extends StatelessWidget {
  const _MetaCell({
    required this.label,
    required this.value,
    this.first = false,
  });

  final String label;
  final Widget value;
  final bool first;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Expanded(
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        decoration: BoxDecoration(
          border: first
              ? Border(right: BorderSide(color: palette.border))
              : null,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(label, style: AppType.sans(12, color: palette.text2)),
            const SizedBox(height: 3),
            value,
          ],
        ),
      ),
    );
  }
}

class _FactRow extends StatelessWidget {
  const _FactRow({required this.label, required this.value, this.last = false});

  final String label;
  final Widget value;
  final bool last;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      decoration: BoxDecoration(
        border: last ? null : Border(bottom: BorderSide(color: palette.border)),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 120,
            child: Text(label, style: AppType.sans(13, color: palette.text2)),
          ),
          const SizedBox(width: 12),
          Expanded(child: value),
        ],
      ),
    );
  }
}

/// The Integrate card: the three steps, the Move-the-original toggle and the
/// Integrate and Cancel buttons.
class _IntegrateCard extends StatelessWidget {
  const _IntegrateCard({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final folder = shortenHome(model.managedFolderPath, homeFolder());
    final steps = [
      'Copies the file into $folder',
      'Writes a menu entry and installs the icon',
      'On a name conflict, asks to keep both or replace',
    ];
    final ready = model.inspect.inspectedOk;
    return AppCard(
      padding: const EdgeInsets.all(18),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            'Integrate into library',
            style: AppType.sans(
              15,
              weight: FontWeight.w600,
              letterSpacing: -0.15,
              color: palette.text,
            ),
          ),
          const SizedBox(height: 14),
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (var i = 0; i < steps.length; i++) ...[
                if (i > 0) const SizedBox(height: 10),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Container(
                      width: 20,
                      height: 20,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: palette.sidebar,
                        borderRadius: BorderRadius.circular(10),
                      ),
                      child: Text(
                        '${i + 1}',
                        style: AppType.mono(
                          10.5,
                          weight: FontWeight.w600,
                          color: palette.text2,
                        ),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Text(
                        steps[i],
                        style: AppType.sans(
                          13,
                          height: 1.5,
                          color: palette.text2,
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ],
          ),
          const SizedBox(height: 14),
          Container(
            padding: const EdgeInsets.only(top: 14),
            decoration: BoxDecoration(
              border: Border(top: BorderSide(color: palette.border)),
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Move the original',
                        style: AppType.sans(
                          13,
                          weight: FontWeight.w500,
                          color: palette.text,
                        ),
                      ),
                      const SizedBox(height: 2),
                      Text(
                        'After a verified copy the source goes to the Trash. It is never hard-deleted.',
                        style: AppType.sans(
                          12,
                          height: 1.5,
                          color: palette.text2,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 14),
                AppToggle(
                  toggleKey: const Key('inspect-move-toggle'),
                  value: model.settings?.moveSource ?? false,
                  onChanged: model.settings == null
                      ? null
                      : model.setMoveSource,
                ),
              ],
            ),
          ),
          const SizedBox(height: 14),
          Row(
            children: [
              Expanded(
                child: AppButton(
                  label: 'Integrate',
                  variant: AppButtonVariant.primary,
                  height: 34,
                  expand: true,
                  buttonKey: const Key('integrate'),
                  onPressed: ready ? model.integrateFromInspect : null,
                ),
              ),
              const SizedBox(width: 8),
              AppButton(
                label: 'Cancel',
                height: 34,
                horizontalPadding: 14,
                buttonKey: const Key('inspect-cancel'),
                onPressed: model.clearInspect,
              ),
            ],
          ),
        ],
      ),
    );
  }
}
