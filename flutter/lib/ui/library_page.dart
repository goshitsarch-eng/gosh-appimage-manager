import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

String _sortLabel(SortOrder order) => switch (order) {
  SortOrder.name => 'Name',
  SortOrder.version => 'Version',
  SortOrder.updatesFirst => 'Updates first',
};

/// The installed AppImages, with search, sort and the adoptable list. A
/// selected app replaces the list with its Details page, as in the original.
class LibraryPage extends StatelessWidget {
  const LibraryPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final selected = model.selectedApp;
    if (selected != null) {
      return DetailPage(model: model, app: selected);
    }
    final narrow = isCondensed(context);
    final palette = CosmicScope.of(context);
    final visible = model.visibleLibrary;
    final adoptable = model.adoptable;

    final search = CosmicTextField(
      text: model.search,
      placeholder: 'Search by name, version or path',
      pill: true,
      onChanged: model.setSearch,
      leading: Icon(Icons.search, size: 16, color: palette.windowText),
      trailing: model.search.isEmpty
          ? null
          : CosmicButton(
              label: 'Clear',
              kind: CosmicButtonKind.text,
              onPressed: () => model.setSearch(''),
            ),
    );
    final sorts = [
      for (final order in SortOrder.values)
        CosmicButton(
          label: _sortLabel(order),
          kind: order == model.sort
              ? CosmicButtonKind.suggested
              : CosmicButtonKind.standard,
          onPressed: () => model.setSort(order),
        ),
    ];

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Text('Library', style: CosmicType.title3),
              const Spacer(),
              CosmicButton(label: 'Refresh', onPressed: model.loadLibrary),
            ],
          ),
          const SizedBox(height: 8),
          // Search and sort share a row when there is room. Condensed, the
          // sort buttons would squeeze the field, so they get their own line.
          if (narrow)
            Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                search,
                const SizedBox(height: 8),
                Wrap(spacing: 8, runSpacing: 8, children: sorts),
              ],
            )
          else
            Row(
              children: [
                Expanded(child: search),
                const SizedBox(width: 8),
                for (final button in sorts) ...[
                  button,
                  const SizedBox(width: 8),
                ],
              ],
            ),
          const SizedBox(height: 8),
          if (model.library.isEmpty)
            if (model.busy != null || model.loadingLibrary)
              const Text('Loading your library…')
            else
              const Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('No AppImages yet', style: CosmicType.heading),
                  SizedBox(height: 4),
                  Text(
                    'Open one from the Inspect page. Opening a file never '
                    'integrates or executes it.',
                  ),
                ],
              ),
          if (model.library.isNotEmpty && visible.isEmpty)
            Text('No AppImage matches “${model.search.trim()}”.'),
          for (final app in visible) ...[
            _LibraryRow(model: model, app: app, narrow: narrow),
            const SizedBox(height: 8),
          ],
          if (adoptable.isNotEmpty) ...[
            const SizedBox(height: 8),
            Text('Not managed yet', style: CosmicType.title4),
            const Text(
              'Adopting registers an AppImage so it can be updated and removed '
              'here. Nothing on disk is changed.',
              style: CosmicType.caption,
            ),
            for (final found in adoptable) ...[
              _AdoptRow(model: model, found: found),
              const SizedBox(height: 8),
            ],
          ],
        ],
      ),
    );
  }
}

class _LibraryRow extends StatelessWidget {
  const _LibraryRow({
    required this.model,
    required this.app,
    required this.narrow,
  });

  final AppModel model;
  final AppDto app;
  final bool narrow;

  @override
  Widget build(BuildContext context) {
    final badges = badgesFor(app, model.updates);
    final version = app.version.isEmpty ? 'no version' : app.version;
    final actions = [
      CosmicButton(
        label: 'Launch',
        kind: CosmicButtonKind.suggested,
        onPressed: () => model.launch(app.uuid),
      ),
      CosmicButton(
        label: 'Details',
        onPressed: () => model.selectApp(app.uuid),
      ),
      CosmicButton(
        label: 'Trash',
        onPressed: () => model.askRemove(app.uuid, permanent: false),
      ),
    ];
    return Padding(
      padding: const EdgeInsets.all(8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Flexible(
                child: Text(
                  app.name,
                  style: CosmicType.heading,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 8),
              Flexible(child: Text(badges, style: CosmicType.caption)),
            ],
          ),
          const SizedBox(height: 4),
          Text('$version  ·  ${app.managedPath}', style: CosmicType.caption),
          const SizedBox(height: 4),
          if (narrow)
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                for (final button in actions) ...[
                  button,
                  const SizedBox(height: 4),
                ],
              ],
            )
          else
            Row(
              children: [
                for (final button in actions) ...[
                  button,
                  const SizedBox(width: 8),
                ],
              ],
            ),
        ],
      ),
    );
  }
}

class _AdoptRow extends StatelessWidget {
  const _AdoptRow({required this.model, required this.found});

  final AppModel model;
  final DiscoveredDto found;

  @override
  Widget build(BuildContext context) {
    final origin = found.externalDesktopEntry
        ? 'outside the managed folder · ${found.desktopPath}'
        : 'in the managed folder';
    return Padding(
      padding: const EdgeInsets.all(8),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(found.name, style: CosmicType.heading),
                Text(found.path, style: CosmicType.caption),
                Text(origin, style: CosmicType.caption),
              ],
            ),
          ),
          const SizedBox(width: 8),
          CosmicButton(
            label: 'Adopt',
            kind: CosmicButtonKind.suggested,
            onPressed: () => model.askAdopt(found.path),
          ),
        ],
      ),
    );
  }
}

/// One app's facts and the actions that apply to it. Its sections are cards,
/// as in the original; the arguments and source fields take several lines.
class DetailPage extends StatelessWidget {
  const DetailPage({super.key, required this.model, required this.app});

  final AppModel model;
  final AppDto app;

  @override
  Widget build(BuildContext context) {
    final narrow = isCondensed(context);
    final facts = appFacts(app);
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              CosmicButton(label: '← Library', onPressed: model.closeDetail),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  app.name,
                  style: CosmicType.title3,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          CosmicSection(
            title: 'Details',
            children: [
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  for (final fact in facts) ...[
                    LabelValueRow(
                      label: fact.key,
                      value: fact.value,
                      narrow: narrow,
                    ),
                    const SizedBox(height: 2),
                  ],
                ],
              ),
            ],
          ),
          const SizedBox(height: 12),
          CosmicSection(
            title: 'Actions',
            children: [
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  CosmicButton(
                    label: 'Launch',
                    kind: CosmicButtonKind.suggested,
                    onPressed: () => model.launch(app.uuid),
                  ),
                  const SizedBox(height: 6),
                  CosmicButton(
                    label: 'Reveal in file manager',
                    onPressed: () => model.reveal(app.uuid),
                  ),
                  const SizedBox(height: 6),
                  CosmicButton(
                    label: 'Check and update now',
                    onPressed: () => model.updateOne(app.uuid),
                  ),
                  const SizedBox(height: 6),
                  CosmicButton(
                    label: 'Refresh metadata',
                    onPressed: () => model.refreshMetadata(app.uuid),
                  ),
                  const SizedBox(height: 6),
                  CosmicButton(
                    label: 'Move to Trash',
                    onPressed: () =>
                        model.askRemove(app.uuid, permanent: false),
                  ),
                  const SizedBox(height: 6),
                  CosmicButton(
                    label: 'Delete permanently',
                    kind: CosmicButtonKind.destructive,
                    onPressed: () => model.askRemove(app.uuid, permanent: true),
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: 12),
          CosmicSection(
            title: 'Command arguments',
            children: [
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Text(
                    'One argument per line. These are passed as separate '
                    'arguments, never as a shell command.',
                    style: CosmicType.caption,
                  ),
                  const SizedBox(height: 4),
                  CosmicTextField(
                    text: model.argumentsInput,
                    placeholder: '--example-flag',
                    onChanged: model.setArgumentsInput,
                    maxLines: null,
                  ),
                ],
              ),
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Text(
                    'Environment variables, one NAME=value per line.',
                    style: CosmicType.caption,
                  ),
                  const SizedBox(height: 4),
                  CosmicTextField(
                    text: model.environmentInput,
                    placeholder: 'NAME=value',
                    onChanged: model.setEnvironmentInput,
                    maxLines: null,
                  ),
                  const SizedBox(height: 4),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: CosmicButton(
                      label: 'Save',
                      kind: CosmicButtonKind.suggested,
                      onPressed: model.saveArgumentsAndEnvironment,
                    ),
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: 12),
          CosmicSection(
            title: 'Update source',
            children: [
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  CosmicTextField(
                    text: model.sourceManagerInput,
                    placeholder: 'Manager (static, github, gitlab, codeberg, forgejo, ftp)',
                    onChanged: model.setSourceManagerInput,
                  ),
                  const SizedBox(height: 4),
                  CosmicTextField(
                    text: model.sourceConfigInput,
                    placeholder: 'key=value',
                    onChanged: model.setSourceConfigInput,
                    maxLines: null,
                  ),
                  const SizedBox(height: 4),
                  Row(
                    children: [
                      CosmicButton(
                        label: 'Apply',
                        kind: CosmicButtonKind.suggested,
                        onPressed: () => model.applySource(app.uuid),
                      ),
                      const SizedBox(width: 8),
                      CosmicButton(
                        label: 'Reset',
                        onPressed: () => model.resetSource(app.uuid),
                      ),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ],
      ),
    );
  }
}
