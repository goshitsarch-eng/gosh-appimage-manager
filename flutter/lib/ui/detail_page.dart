import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// The update managers the source selector offers, with the core's ids.
const List<(String, String)> _managers = [
  ('GitHub', 'github'),
  ('GitLab', 'gitlab'),
  ('Codeberg', 'codeberg'),
  ('Forgejo', 'forgejo'),
  ('FTP', 'ftp'),
  ('Static', 'static'),
];

String managerLabel(String manager) {
  for (final (label, id) in _managers) {
    if (id == manager) {
      return label;
    }
  }
  return manager.isEmpty ? 'None' : manager;
}

/// The header keeps its three actions beside the title only from this content
/// width. Narrower, they wrap under the title, so the title is not squeezed
/// (QA3-008).
const double _actionsBesideMinWidth = 680;

/// The Remove card keeps its two buttons beside the text only from this card
/// width. Narrower, the text keeps the full width and the buttons wrap under it
/// (QA3-008).
const double _removeBesideMinWidth = 360;

/// One app's record and actions (SPEC 7.3). Its header carries the breadcrumb,
/// the app's identity and the header actions; the body holds the record, the
/// launch options, the update source and the remove card.
class DetailPage extends StatelessWidget {
  const DetailPage({super.key, required this.model, required this.app});

  final AppModel model;
  final AppDto app;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final narrow = isNarrow(context);
    final offer = model.offerFor(app.uuid);
    final actions = [
      AppButton(
        label: 'Reveal in folder',
        buttonKey: const Key('detail-reveal'),
        onPressed: () => model.reveal(app.uuid),
      ),
      AppButton(
        label: 'Check for update',
        buttonKey: const Key('detail-check'),
        // A check only reports what the source offers. Applying it is the
        // Update action, which asks first when the app is running.
        onPressed: () => model.checkOne(app.uuid),
      ),
      AppButton(
        label: 'Launch',
        variant: AppButtonVariant.primary,
        horizontalPadding: 16,
        buttonKey: const Key('detail-launch'),
        onPressed: () => model.launch(app.uuid),
      ),
    ];
    final statusLine = Wrap(
      spacing: 14,
      runSpacing: 4,
      children: [
        if (app.running) _StatusItem(text: 'Running', color: palette.ok),
        if (offer != null)
          _StatusItem(
            text: 'Update ${offer.availableVersion} available',
            color: palette.accent,
          ),
        Text(
          integrationLabel(app),
          style: AppType.sans(12.5, color: palette.text2),
        ),
      ],
    );
    Widget head({required bool beside}) => Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        AppIconTile(
          letter: app.name.isEmpty
              ? '?'
              : app.name.substring(0, 1).toUpperCase(),
          iconPath: app.iconPath,
          size: 52,
          fontSize: 22,
          radius: AppRadius.tileLarge,
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
                      app.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AppType.sans(
                        22,
                        weight: FontWeight.w600,
                        letterSpacing: -0.33,
                        color: palette.text,
                      ),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Text(
                    app.version,
                    style: AppType.mono(13, color: palette.text2),
                  ),
                ],
              ),
              const SizedBox(height: 6),
              statusLine,
            ],
          ),
        ),
        if (beside) ...[
          const SizedBox(width: 16),
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (var i = 0; i < actions.length; i++) ...[
                if (i > 0) const SizedBox(width: 8),
                actions[i],
              ],
            ],
          ),
        ],
      ],
    );
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: EdgeInsets.fromLTRB(
              narrow ? 14 : 28,
              16,
              narrow ? 14 : 28,
              18,
            ),
            decoration: BoxDecoration(
              color: palette.surface,
              border: Border(bottom: BorderSide(color: palette.border)),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _Breadcrumb(model: model, name: app.name),
                const SizedBox(height: 14),
                LayoutBuilder(
                  builder: (context, constraints) {
                    final beside =
                        !narrow &&
                        constraints.maxWidth >= _actionsBesideMinWidth;
                    return Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        head(beside: beside),
                        if (!beside) ...[
                          const SizedBox(height: 12),
                          Wrap(spacing: 8, runSpacing: 8, children: actions),
                        ],
                      ],
                    );
                  },
                ),
              ],
            ),
          ),
          Padding(
            padding: EdgeInsets.fromLTRB(
              narrow ? 14 : 28,
              20,
              narrow ? 14 : 28,
              20,
            ),
            child: narrow
                ? Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      _record(palette),
                      const SizedBox(height: 16),
                      ..._sideCards(palette),
                    ],
                  )
                : Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(flex: 115, child: _record(palette)),
                      const SizedBox(width: 20),
                      Expanded(
                        flex: 100,
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: _sideCards(palette),
                        ),
                      ),
                    ],
                  ),
          ),
        ],
      ),
    );
  }

  /// The Record card: path, desktop ID, checksum, type, architecture, size,
  /// update source and provenance.
  Widget _record(AppPalette palette) {
    final source = app.updateManager.isEmpty
        ? (app.embeddedUpdate.isEmpty ? 'None configured' : app.embeddedUpdate)
        : [
            managerLabel(app.updateManager),
            for (final pair in app.updateConfig) pair.value,
          ].join(' · ');
    return AppCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          AppCardHeader(
            title: 'Record',
            trailing: AppButton(
              label: 'Refresh',
              variant: AppButtonVariant.text,
              color: palette.accent,
              fontSize: 12.5,
              horizontalPadding: 0,
              buttonKey: const Key('detail-refresh'),
              onPressed: () => model.refreshMetadata(app.uuid),
            ),
          ),
          _recordRow(
            palette,
            'Path',
            Text(
              shortenHome(app.managedPath, homeFolder()),
              style: AppType.mono(12, height: 1.5, color: palette.text),
            ),
          ),
          _recordRow(
            palette,
            'Desktop ID',
            Text(
              app.desktopId,
              style: AppType.mono(12, height: 1.5, color: palette.text),
            ),
          ),
          _recordRow(
            palette,
            'SHA-256',
            Text(
              app.sha256,
              style: AppType.mono(11.5, height: 1.55, color: palette.text2),
            ),
          ),
          Container(
            padding: const EdgeInsets.symmetric(vertical: 12),
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: palette.border)),
            ),
            child: Row(
              children: [
                _Cell(label: 'Type', value: app.appType, first: true),
                _Cell(
                  label: 'Architecture',
                  value: app.architecture,
                  first: true,
                ),
                _Cell(label: 'Size', value: humanSize(app.sizeBytes)),
              ],
            ),
          ),
          _recordRow(
            palette,
            'Update source',
            Text(source, style: AppType.sans(13, color: palette.text)),
          ),
          _recordRow(
            palette,
            'Provenance',
            Text(
              provenanceLabel(app, homeFolder()),
              style: AppType.sans(13, color: palette.text),
            ),
            last: true,
          ),
        ],
      ),
    );
  }

  Widget _recordRow(
    AppPalette palette,
    String label,
    Widget value, {
    bool last = false,
  }) => Container(
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

  List<Widget> _sideCards(AppPalette palette) => [
    _launchOptions(palette),
    const SizedBox(height: 16),
    _updateSource(palette),
    const SizedBox(height: 16),
    _removeCard(palette),
  ];

  Widget _launchOptions(AppPalette palette) => AppCard(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const AppCardHeader(title: 'Launch options'),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 14, 16, 0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Arguments',
                style: AppType.sans(
                  12,
                  weight: FontWeight.w500,
                  color: palette.text2,
                ),
              ),
              const SizedBox(height: 6),
              AppTextField(
                fieldKey: const Key('arguments-field'),
                text: model.argumentsInput,
                placeholder: '--example-flag',
                mono: true,
                fontSize: 12.5,
                multiline: true,
                onChanged: model.setArgumentsInput,
              ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 14),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Environment',
                style: AppType.sans(
                  12,
                  weight: FontWeight.w500,
                  color: palette.text2,
                ),
              ),
              const SizedBox(height: 6),
              Row(
                children: [
                  Expanded(
                    child: AppTextField(
                      fieldKey: const Key('environment-field'),
                      text: model.environmentInput,
                      placeholder: 'NAME=value',
                      mono: true,
                      fontSize: 12.5,
                      multiline: true,
                      onChanged: model.setEnvironmentInput,
                    ),
                  ),
                  const SizedBox(width: 8),
                  AppButton(
                    label: 'Add',
                    color: palette.accent,
                    buttonKey: const Key('launch-options-add'),
                    onPressed: model.saveArgumentsAndEnvironment,
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    ),
  );

  Widget _updateSource(AppPalette palette) {
    final current = app.updateManager;
    // The form the field takes for the chosen manager: GitHub takes one line,
    // repo=owner/name (QA D-10).
    final chosen = model.sourceManagerInput.isEmpty
        ? current
        : model.sourceManagerInput;
    final github = chosen == 'github';
    return AppCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const AppCardHeader(title: 'Update source'),
          Padding(
            padding: const EdgeInsets.all(14),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    _SourceSelector(
                      model: model,
                      uuid: app.uuid,
                      label: managerLabel(current),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: AppTextField(
                        fieldKey: const Key('source-config-field'),
                        text: model.sourceConfigInput,
                        placeholder: github ? 'repo=owner/name' : 'key=value',
                        mono: true,
                        fontSize: 12.5,
                        onChanged: model.setSourceConfigInput,
                        onSubmitted: (_) => model.applySource(app.uuid),
                      ),
                    ),
                  ],
                ),
                if (model.sourceError case final error?)
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Text(
                      error,
                      key: const Key('source-error'),
                      style: AppType.sans(12, height: 1.5, color: palette.bad),
                    ),
                  ),
                const SizedBox(height: 8),
                Text(
                  'One key=value pair. Multi-key sources are set from the CLI. '
                  'Sources without a published checksum are marked reduced '
                  'verification.',
                  style: AppType.sans(12, height: 1.5, color: palette.text2),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _removeCard(AppPalette palette) => AppCard(
    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
    child: LayoutBuilder(
      builder: (context, constraints) {
        final description = Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Remove app',
              style: AppType.sans(
                13,
                weight: FontWeight.w600,
                color: palette.text,
              ),
            ),
            const SizedBox(height: 2),
            Text(
              'Moves to the Trash. Permanent delete asks again.',
              style: AppType.sans(12, color: palette.text2),
            ),
          ],
        );
        final trash = AppButton(
          label: 'Move to Trash',
          height: 30,
          buttonKey: const Key('detail-trash'),
          onPressed: () => model.askRemove(app.uuid, permanent: false),
        );
        final delete = AppButton(
          label: 'Delete…',
          variant: AppButtonVariant.dangerText,
          height: 30,
          buttonKey: const Key('detail-delete'),
          onPressed: () => model.askRemove(app.uuid, permanent: true),
        );
        // Beside the text the buttons need about 200 px. Narrower than
        // _removeBesideMinWidth, the text keeps the full width and the buttons
        // wrap under it (QA3-008).
        if (constraints.maxWidth >= _removeBesideMinWidth) {
          return Row(
            children: [
              Expanded(child: description),
              const SizedBox(width: 16),
              trash,
              const SizedBox(width: 8),
              delete,
            ],
          );
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            description,
            const SizedBox(height: 12),
            Wrap(spacing: 8, runSpacing: 8, children: [trash, delete]),
          ],
        );
      },
    ),
  );
}

/// `Library › Quill Notes`: the first part returns to the list.
class _Breadcrumb extends StatelessWidget {
  const _Breadcrumb({required this.model, required this.name});

  final AppModel model;
  final String name;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Row(
      children: [
        GestureDetector(
          key: const Key('breadcrumb-library'),
          behavior: HitTestBehavior.opaque,
          onTap: model.closeDetail,
          child: Text(
            'Library',
            style: AppType.sans(12.5, color: palette.text2),
          ),
        ),
        const SizedBox(width: 6),
        AppIcon('breadcrumb-chevron', size: 10, color: palette.text2),
        const SizedBox(width: 6),
        Text(
          name,
          style: AppType.sans(
            12.5,
            weight: FontWeight.w500,
            color: palette.text,
          ),
        ),
      ],
    );
  }
}

class _StatusItem extends StatelessWidget {
  const _StatusItem({required this.text, required this.color});

  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      StatusDot(color: color),
      const SizedBox(width: 6),
      Text(
        text,
        style: AppType.sans(12.5, weight: FontWeight.w500, color: color),
      ),
    ],
  );
}

class _Cell extends StatelessWidget {
  const _Cell({required this.label, required this.value, this.first = false});

  final String label;
  final String value;
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
            Text(
              value,
              style: AppType.sans(
                13,
                weight: FontWeight.w500,
                color: palette.text,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The source selector: the manager's name and a chevron. Choosing one saves
/// it with the key=value text beside it; `None` removes the source.
class _SourceSelector extends StatelessWidget {
  const _SourceSelector({
    required this.model,
    required this.uuid,
    required this.label,
  });

  final AppModel model;
  final String uuid;
  final String label;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return PopupMenuButton<String>(
      key: const Key('source-selector'),
      tooltip: 'Update manager',
      color: palette.surface,
      position: PopupMenuPosition.under,
      onSelected: (manager) {
        if (manager.isEmpty) {
          model.resetSource(uuid);
        } else {
          model.setSourceManagerInput(manager);
          model.applySource(uuid);
        }
      },
      itemBuilder: (_) => [
        PopupMenuItem<String>(
          value: '',
          child: Text('None', style: AppType.sans(13)),
        ),
        for (final (name, id) in _managers)
          PopupMenuItem<String>(
            value: id,
            child: Text(name, style: AppType.sans(13)),
          ),
      ],
      child: Container(
        height: 32,
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
          border: Border.all(color: palette.border),
          borderRadius: BorderRadius.circular(AppRadius.control),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              label,
              style: AppType.sans(
                13,
                weight: FontWeight.w500,
                color: palette.text,
              ),
            ),
            const SizedBox(width: 8),
            AppIcon('select-chevron', size: 10, color: palette.text2),
          ],
        ),
      ),
    );
  }
}
