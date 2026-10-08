import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/platform/file_picker.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/detail_page.dart';
import 'package:gosh_appimage_flutter/ui/page_frame.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// The Library's columns (SPEC 7.1): the app column takes 2.3 parts of the
/// free width, the version a fixed 140 px, the status 1.7 parts and the
/// actions a fixed 132 px, with 16 px gaps and 18 px side padding.
/// The library toolbar keeps the filter, search and sort on one row from this
/// content width (the 1280 px mockup is well above it). Narrower, the filter
/// takes a row of its own so the search keeps its placeholder and at least
/// 200 px at 800 px (QA D-13).
const double _oneRowToolbarWidth = 700;

/// The side padding of a table row and its header.
const double _rowPadding = 18;

/// The desktop table's columns, fitted to the table's width. At the mockup's
/// 1280 px the grid is the mockup's own: minmax(0, 2.3fr) 140 px minmax(0,
/// 1.7fr) 132 px, with 16 px gaps. Narrower, the gaps, the version and the
/// action column shrink toward compact sizes and the status keeps 92 px (enough
/// for "Up to date"), so the name column takes the rest (QA D-13).
///
/// Below [_stackedTableWidth] the version moves under the app name instead of
/// having a column of its own (QA2-013). The status takes that room, so its
/// label fits on one line and its detail on two, and a row stays about 60 px.
class _TableColumns {
  const _TableColumns._({
    required this.stacked,
    required this.app,
    required this.gap,
    required this.version,
    required this.status,
    required this.action,
  });

  /// [width] is the table's width less its side padding.
  factory _TableColumns.fit(double width, {required bool stacked}) {
    if (stacked) {
      const gap = 12.0;
      const action = 104.0;
      final rest = (width - 2 * gap - action).clamp(0.0, double.infinity);
      final status = (rest * 0.45).clamp(150.0, double.infinity);
      return _TableColumns._(
        stacked: true,
        app: (rest - status).clamp(0.0, double.infinity),
        gap: gap,
        version: 0,
        status: status,
        action: action,
      );
    }
    // 0 at 800 px (the narrowest desktop frame), 1 at the mockup's 1280 px.
    final t = ((width - 461) / (941 - 461)).clamp(0.0, 1.0);
    final gap = 12 + 4 * t;
    final action = 104 + 28 * t;
    final version = 84 + 56 * t;
    final rest = (width - 3 * gap - action - version).clamp(
      0.0,
      double.infinity,
    );
    // The status takes the mockup's 1.7 of 4 at 1280 px, and a smaller share
    // when narrow, where it still keeps 92 px.
    final fair = rest * (0.36 + 0.065 * t);
    final status = fair < 92 ? 92.0 : fair;
    return _TableColumns._(
      stacked: false,
      app: (rest - status).clamp(0.0, double.infinity),
      gap: gap,
      version: version,
      status: status,
      action: action,
    );
  }

  /// Whether the version sits under the name, with no column of its own.
  final bool stacked;

  /// The name column: the tile, the name, the version when stacked, and the
  /// path.
  final double app;

  /// The gap between two columns.
  final double gap;

  /// The version column. Zero when the version is stacked under the name.
  final double version;

  /// The status column (its dot, status and detail).
  final double status;

  /// The action column: Launch and the row menu.
  final double action;
}

/// Below this window width the version is stacked under the app name (QA2-013).
/// At 1000 px and up the table keeps the version column of the mockup.
const double _stackedTableWidth = 1000;

String sortLabel(SortOrder order) => switch (order) {
  SortOrder.name => 'Name',
  SortOrder.version => 'Version',
  SortOrder.updatesFirst => 'Updates first',
};

String filterLabel(LibraryFilter filter) => switch (filter) {
  LibraryFilter.all => 'All',
  LibraryFilter.updates => 'Updates',
  LibraryFilter.attention => 'Needs attention',
};

/// The installed AppImages with the mockup's filter, search and sort, the
/// table, and the adoptable list. A selected app shows its Detail page.
class LibraryPage extends StatelessWidget {
  const LibraryPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final selected = model.selectedApp;
    if (selected != null) {
      return DetailPage(model: model, app: selected);
    }
    if (model.library.isEmpty && !model.loadingLibrary) {
      return _EmptyLibrary(model: model);
    }
    return isNarrow(context)
        ? _NarrowLibrary(model: model)
        : _DesktopLibrary(model: model);
  }
}

String _subtitle(AppModel model) {
  final folder = shortenHome(model.managedFolderPath, homeFolder());
  final adopted = model.library.where((app) => app.adopted).length;
  final apps = model.library.length;
  final noun = apps == 1 ? 'app' : 'apps';
  return adopted == 0
      ? '$apps $noun in $folder'
      : '$apps $noun in $folder and $adopted adopted';
}

Widget _browseButton(AppModel model, {Key? key}) => AppButton(
  label: 'Browse…',
  hint: 'Ctrl O',
  buttonKey: key ?? const Key('browse'),
  onPressed: () => chooseAppImages(model),
);

class _DesktopLibrary extends StatelessWidget {
  const _DesktopLibrary({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final rows = model.visibleLibrary;
    final total = model.library.length;
    final stacked = MediaQuery.sizeOf(context).width < _stackedTableWidth;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHeader(
          title: 'Library',
          subtitle: _subtitle(model),
          actions: [_browseButton(model)],
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(28, 16, 28, 16),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final filter = AppSegmented<LibraryFilter>(
                segments: [
                  AppSegment(
                    LibraryFilter.all,
                    filterLabel(LibraryFilter.all),
                    count: '$total',
                  ),
                  AppSegment(
                    LibraryFilter.updates,
                    filterLabel(LibraryFilter.updates),
                    count: '${model.updateCount}',
                  ),
                  AppSegment(
                    LibraryFilter.attention,
                    filterLabel(LibraryFilter.attention),
                    count: '${model.attentionCount}',
                  ),
                ],
                selected: model.filter,
                onSelected: model.setFilter,
              );
              final search = _SearchField(
                model: model,
                placeholder: 'Search apps',
              );
              if (constraints.maxWidth >= _oneRowToolbarWidth) {
                return Row(
                  children: [
                    filter,
                    const SizedBox(width: 12),
                    Expanded(
                      child: Align(
                        alignment: Alignment.centerRight,
                        child: ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 342),
                          child: search,
                        ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    _SortControl(model: model),
                  ],
                );
              }
              // Narrower, the toolbar takes two rows: the filter, then the
              // search, which keeps room for its placeholder, with the sort
              // beside it.
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Align(alignment: Alignment.centerLeft, child: filter),
                  const SizedBox(height: 12),
                  Row(
                    children: [
                      Expanded(child: search),
                      const SizedBox(width: 12),
                      _SortControl(model: model),
                    ],
                  ),
                ],
              );
            },
          ),
        ),
        Expanded(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(28, 0, 28, 20),
            child: AppCard(
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final cols = _TableColumns.fit(
                    constraints.maxWidth - 2 * _rowPadding,
                    stacked: stacked,
                  );
                  return Column(
                    children: [
                      _TableHeader(
                        height: 36,
                        cells: const ['APP', 'VERSION', 'STATUS', ''],
                        cols: cols,
                        palette: palette,
                      ),
                      Expanded(
                        child: rows.isEmpty
                            ? _NoMatch(model: model)
                            : ListView(
                                key: const Key('library-table'),
                                padding: EdgeInsets.zero,
                                children: [
                                  for (final app in rows)
                                    _LibraryRow(
                                      model: model,
                                      app: app,
                                      cols: cols,
                                    ),
                                ],
                              ),
                      ),
                    ],
                  );
                },
              ),
            ),
          ),
        ),
        if (model.adoptable.isNotEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(28, 0, 28, 20),
            child: _AdoptCard(model: model),
          ),
      ],
    );
  }
}

class _NarrowLibrary extends StatelessWidget {
  const _NarrowLibrary({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final rows = model.visibleLibrary;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(14, 12, 14, 0),
          child: _SearchField(
            model: model,
            placeholder: 'Search name, version, or path',
            height: 34,
            radius: AppRadius.button,
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
          child: AppSegmented<LibraryFilter>(
            expand: true,
            fontSize: 12.5,
            segments: [
              AppSegment(LibraryFilter.all, 'All ${model.library.length}'),
              AppSegment(LibraryFilter.updates, 'Updates ${model.updateCount}'),
              AppSegment(
                LibraryFilter.attention,
                'Attention ${model.attentionCount}',
              ),
            ],
            selected: model.filter,
            onSelected: model.setFilter,
          ),
        ),
        Expanded(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 14),
            child: AppCard(
              child: rows.isEmpty
                  ? _NoMatch(model: model)
                  : ListView(
                      key: const Key('library-table'),
                      padding: EdgeInsets.zero,
                      children: [
                        for (final app in rows)
                          _NarrowRow(model: model, app: app),
                      ],
                    ),
            ),
          ),
        ),
        if (model.adoptable.isNotEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(14, 12, 14, 0),
            child: _AdoptCard(model: model),
          ),
      ],
    );
  }
}

class _NoMatch extends StatelessWidget {
  const _NoMatch({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final text = model.loadingLibrary
        ? 'Loading your library…'
        : model.search.trim().isEmpty
        ? 'No AppImage matches this filter.'
        : 'No AppImage matches “${model.search.trim()}”.';
    return Center(
      child: Text(text, style: AppType.sans(13, color: palette.text2)),
    );
  }
}

class _SearchField extends StatelessWidget {
  const _SearchField({
    required this.model,
    required this.placeholder,
    this.height = 32,
    this.radius = AppRadius.control,
  });

  final AppModel model;
  final String placeholder;
  final double height;
  final double radius;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return AppTextField(
      fieldKey: const Key('search-field'),
      text: model.search,
      placeholder: placeholder,
      height: height,
      radius: radius,
      onChanged: model.setSearch,
      leading: AppIcon('search', size: 14, color: palette.text3),
    );
  }
}

/// The sort dropdown: `Sort:` and the current order, with the chevron.
class _SortControl extends StatelessWidget {
  const _SortControl({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return PopupMenuButton<SortOrder>(
      key: const Key('sort-menu'),
      tooltip: 'Sort',
      color: palette.surface,
      position: PopupMenuPosition.under,
      onSelected: model.setSort,
      itemBuilder: (_) => [
        for (final order in SortOrder.values)
          PopupMenuItem<SortOrder>(
            value: order,
            child: Text(
              sortLabel(order),
              style: AppType.sans(13, color: palette.text),
            ),
          ),
      ],
      child: Container(
        height: 34,
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
          color: palette.surface,
          border: Border.all(color: palette.border),
          borderRadius: BorderRadius.circular(AppRadius.control),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text('Sort:', style: AppType.sans(13, color: palette.text2)),
            const SizedBox(width: 6),
            Text(
              sortLabel(model.sort),
              style: AppType.sans(
                13,
                weight: FontWeight.w500,
                color: palette.text,
              ),
            ),
            const SizedBox(width: 6),
            AppIcon('chevron-down', size: 10, color: palette.text2),
          ],
        ),
      ),
    );
  }
}

/// The grid header row: uppercase mono labels on the canvas colour.
class _TableHeader extends StatelessWidget {
  const _TableHeader({
    required this.height,
    required this.cells,
    required this.cols,
    required this.palette,
  });

  final double height;
  final List<String> cells;
  final _TableColumns cols;
  final AppPalette palette;

  @override
  Widget build(BuildContext context) {
    final style = AppType.label(color: palette.text3);
    return Container(
      height: height + 1,
      padding: const EdgeInsets.symmetric(horizontal: _rowPadding),
      decoration: BoxDecoration(
        color: palette.canvas,
        border: Border(bottom: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          SizedBox(
            width: cols.app,
            child: Text(cells[0], style: style, overflow: TextOverflow.clip),
          ),
          SizedBox(width: cols.gap),
          if (!cols.stacked) ...[
            SizedBox(
              width: cols.version,
              child: Text(cells[1], style: style, overflow: TextOverflow.clip),
            ),
            SizedBox(width: cols.gap),
          ],
          SizedBox(
            width: cols.status,
            child: Text(cells[2], style: style, overflow: TextOverflow.clip),
          ),
          SizedBox(width: cols.gap),
          SizedBox(width: cols.action),
        ],
      ),
    );
  }
}

/// One library row: tile, name, running badge and path; version and update
/// arrow; the status with its dot and detail; then Launch and the row menu.
class _LibraryRow extends StatelessWidget {
  const _LibraryRow({
    required this.model,
    required this.app,
    required this.cols,
  });

  final AppModel model;
  final AppDto app;
  final _TableColumns cols;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final offer = model.offerFor(app.uuid);
    final status = model.statusFor(app);
    // The Running badge: beside the name when the name has room, otherwise on
    // the version line under it (QA2-013 round 4).
    Widget runningLabel() => Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        StatusDot(color: palette.ok),
        const SizedBox(width: 5),
        Text(
          'Running',
          style: AppType.sans(11.5, weight: FontWeight.w500, color: palette.ok),
        ),
      ],
    );
    final runningBadge = [
      if (app.running) ...[const SizedBox(width: 8), runningLabel()],
    ];
    final versionText = Text(
      app.version.isEmpty ? 'no version' : app.version,
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: AppType.mono(12.5, color: palette.text2),
    );
    final offerText = offer == null
        ? null
        : Text(
            '→ ${offer.availableVersion}',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: AppType.mono(
              12.5,
              weight: FontWeight.w500,
              color: palette.accent,
            ),
          );
    // Stacked, the version keeps its whole width and the Running badge stays
    // beside it. The update arrow goes to the next line when it does not fit
    // beside them, so "no version" reads in full (QA3-007). Unstacked, the
    // version has its own column and shares the cell with the arrow only.
    final versionLine = cols.stacked
        ? Wrap(
            spacing: 6,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              versionText,
              if (app.running) runningLabel(),
              ?offerText,
            ],
          )
        : Row(
            children: [
              Flexible(child: versionText),
              if (offerText != null) ...[
                const SizedBox(width: 6),
                Flexible(child: offerText),
              ],
            ],
          );
    return Container(
      key: Key('row-${app.uuid}'),
      constraints: const BoxConstraints(minHeight: 59),
      alignment: Alignment.center,
      padding: const EdgeInsets.symmetric(horizontal: 18),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          SizedBox(
            width: cols.app,
            child: GestureDetector(
              key: Key('open-${app.uuid}'),
              behavior: HitTestBehavior.opaque,
              onTap: () => model.selectApp(app.uuid),
              child: Row(
                children: [
                  AppIconTile(
                    letter: _initial(app.name),
                    iconPath: app.iconPath,
                    size: 34,
                    fontSize: 14,
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (cols.stacked)
                          // Stacked: the name has its whole cell and may wrap
                          // onto a second line instead of being cut (QA2-016).
                          Text(
                            app.name,
                            style: AppType.sans(
                              13.5,
                              weight: FontWeight.w600,
                              color: palette.text,
                              height: 1.25,
                            ),
                          )
                        else
                          Row(
                            children: [
                              Flexible(
                                child: Text(
                                  app.name,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: AppType.sans(
                                    13.5,
                                    weight: FontWeight.w600,
                                    color: palette.text,
                                  ),
                                ),
                              ),
                              ...runningBadge,
                            ],
                          ),
                        if (cols.stacked) ...[
                          const SizedBox(height: 1),
                          versionLine,
                        ],
                        const SizedBox(height: 1),
                        Text(
                          shortenHome(app.managedPath, homeFolder()),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: AppType.mono(11.5, color: palette.text3),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
          SizedBox(width: cols.gap),
          if (!cols.stacked) ...[
            SizedBox(width: cols.version, child: versionLine),
            SizedBox(width: cols.gap),
          ],
          SizedBox(
            width: cols.status,
            child: _StatusCell(status: status),
          ),
          SizedBox(width: cols.gap),
          SizedBox(
            width: cols.action,
            child: Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                AppButton(
                  label: 'Launch',
                  height: 28,
                  fontSize: 12.5,
                  buttonKey: Key('launch-${app.uuid}'),
                  onPressed: () => model.launch(app.uuid),
                ),
                const SizedBox(width: 6),
                RowMenuButton(model: model, app: app),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// The status column: a dot and the status, with the detail beneath it.
class _StatusCell extends StatelessWidget {
  const _StatusCell({required this.status});

  final RowStatus status;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            StatusDot(color: palette.dot(status.tone), size: 7),
            const SizedBox(width: 7),
            Flexible(
              child: Text(
                status.label,
                style: AppType.sans(
                  13,
                  weight: FontWeight.w500,
                  color: palette.text,
                ),
              ),
            ),
          ],
        ),
        if (status.detail.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 1, left: 14),
            child: Text(
              status.detail,
              style: AppType.sans(12, color: palette.text2),
            ),
          ),
      ],
    );
  }
}

/// The narrow row (SPEC 9): name and version, one status line, and the row
/// menu. Launch and the other actions live in the menu.
class _NarrowRow extends StatelessWidget {
  const _NarrowRow({required this.model, required this.app});

  final AppModel model;
  final AppDto app;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final offer = model.offerFor(app.uuid);
    final status = model.statusFor(app);
    final label = offer != null && status.label == 'Update available'
        ? 'Update to ${offer.availableVersion}'
        : status.label;
    return Container(
      key: Key('row-${app.uuid}'),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          GestureDetector(
            key: Key('open-${app.uuid}'),
            behavior: HitTestBehavior.opaque,
            onTap: () => model.selectApp(app.uuid),
            child: AppIconTile(
              letter: _initial(app.name),
              iconPath: app.iconPath,
              size: 36,
              fontSize: 15,
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: () => model.selectApp(app.uuid),
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
                            13.5,
                            weight: FontWeight.w600,
                            color: palette.text,
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      Text(
                        app.version,
                        style: AppType.mono(11.5, color: palette.text3),
                      ),
                    ],
                  ),
                  const SizedBox(height: 2),
                  Row(
                    children: [
                      StatusDot(color: palette.dot(status.tone)),
                      const SizedBox(width: 6),
                      Flexible(
                        child: Text(
                          label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: AppType.sans(12, color: palette.text2),
                        ),
                      ),
                      if (app.running)
                        Text(
                          ' · Running',
                          style: AppType.sans(
                            12,
                            weight: FontWeight.w500,
                            color: palette.ok,
                          ),
                        ),
                    ],
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(width: 8),
          RowMenuButton(model: model, app: app),
        ],
      ),
    );
  }
}

/// The row's `…` button. Its menu holds the app's actions, so the narrow row
/// has no Launch button of its own.
class RowMenuButton extends StatelessWidget {
  const RowMenuButton({super.key, required this.model, required this.app});

  final AppModel model;
  final AppDto app;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final hasOffer = model.offerFor(app.uuid) != null;
    return PopupMenuButton<String>(
      key: Key('row-menu-${app.uuid}'),
      tooltip: 'More actions',
      color: palette.surface,
      position: PopupMenuPosition.under,
      padding: EdgeInsets.zero,
      onSelected: (action) => _run(action),
      itemBuilder: (_) => [
        _item('launch', 'Launch'),
        _item('details', 'Details'),
        _item('reveal', 'Reveal in folder'),
        if (hasOffer) _item('update', 'Update…'),
        _item('refresh', 'Refresh metadata'),
        _item('trash', 'Move to Trash'),
        _item('delete', 'Delete permanently'),
      ],
      child: Container(
        width: 28,
        height: 28,
        alignment: Alignment.center,
        child: AppIcon('row-menu', size: 14, color: palette.text2),
      ),
    );
  }

  PopupMenuItem<String> _item(String value, String label) =>
      PopupMenuItem<String>(
        value: value,
        child: Text(label, style: AppType.sans(13)),
      );

  void _run(String action) {
    switch (action) {
      case 'launch':
        model.launch(app.uuid);
      case 'details':
        model.selectApp(app.uuid);
      case 'reveal':
        model.reveal(app.uuid);
      case 'update':
        model.updateOne(app.uuid);
      case 'refresh':
        model.refreshMetadata(app.uuid);
      case 'trash':
        model.askRemove(app.uuid, permanent: false);
      case 'delete':
        model.askRemove(app.uuid, permanent: true);
    }
  }
}

/// Discovered AppImages that are not registered yet. Adopting one registers
/// it; nothing on disk changes (the adopt dialog says so).
class _AdoptCard extends StatelessWidget {
  const _AdoptCard({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return AppCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const AppCardHeader(title: 'Not managed yet'),
          for (final found in model.adoptable)
            Container(
              key: Key('adopt-${found.path}'),
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
              decoration: BoxDecoration(
                border: Border(bottom: BorderSide(color: palette.border)),
              ),
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          found.name,
                          style: AppType.sans(13, weight: FontWeight.w600),
                        ),
                        Text(
                          found.path,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: AppType.mono(11.5, color: palette.text3),
                        ),
                        Text(
                          found.externalDesktopEntry
                              ? 'Outside the managed folder · ${found.desktopPath}'
                              : 'In the managed folder',
                          style: AppType.sans(12, color: palette.text2),
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 12),
                  AppButton(
                    label: 'Adopt',
                    variant: AppButtonVariant.primary,
                    height: 28,
                    fontSize: 12.5,
                    buttonKey: Key('adopt-button-${found.path}'),
                    onPressed: () => model.askAdopt(found.path),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

/// The empty library (SPEC 7.2): a dashed drop card with Browse… and Open
/// Inspect. Nothing is installed until Integrate is pressed.
class _EmptyLibrary extends StatelessWidget {
  const _EmptyLibrary({required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final narrow = isNarrow(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHeader(
          title: 'Library',
          subtitle: 'No apps yet',
          actions: [_browseButton(model, key: const Key('browse-empty'))],
        ),
        Expanded(
          child: Center(
            child: SingleChildScrollView(
              padding: EdgeInsets.fromLTRB(28, 24, 28, narrow ? 24 : 48),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 555),
                child: DashedBorder(
                  color: palette.toggleOff,
                  radius: AppRadius.dialog,
                  child: Container(
                    key: const Key('empty-card'),
                    // A 480 px content box, 36 px side padding and the 1.5 px
                    // dashed border on each side (SPEC 7.2).
                    padding: narrow
                        ? const EdgeInsets.symmetric(
                            horizontal: 20,
                            vertical: 40,
                          )
                        : const EdgeInsets.symmetric(
                            horizontal: 37.5,
                            vertical: 41.5,
                          ),
                    decoration: BoxDecoration(
                      color: palette.surface,
                      borderRadius: BorderRadius.circular(AppRadius.dialog),
                    ),
                    child: Column(
                      children: [
                        Container(
                          width: 48,
                          height: 48,
                          margin: const EdgeInsets.only(bottom: 16),
                          alignment: Alignment.center,
                          decoration: BoxDecoration(
                            color: palette.accentSoft,
                            borderRadius: BorderRadius.circular(12),
                          ),
                          child: AppIcon(
                            'empty-state-download',
                            size: 22,
                            color: palette.accent,
                          ),
                        ),
                        Text(
                          'Drop an AppImage here',
                          textAlign: TextAlign.center,
                          style: AppType.sans(
                            17,
                            weight: FontWeight.w600,
                            letterSpacing: -0.17,
                            color: palette.text,
                          ),
                        ),
                        const SizedBox(height: 6),
                        Text(
                          'Or open one from your file manager. You can inspect '
                          'it first. Nothing is installed until you press '
                          'Integrate.',
                          textAlign: TextAlign.center,
                          style: AppType.sans(
                            13,
                            height: 1.55,
                            color: palette.text2,
                          ),
                        ),
                        const SizedBox(height: 20),
                        Row(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            AppButton(
                              label: 'Browse…',
                              variant: AppButtonVariant.primary,
                              horizontalPadding: 14,
                              buttonKey: const Key('browse-drop'),
                              onPressed: () => chooseAppImages(model),
                            ),
                            const SizedBox(width: 8),
                            AppButton(
                              label: 'Open Inspect',
                              horizontalPadding: 14,
                              buttonKey: const Key('open-inspect'),
                              onPressed: () => model.setPage(AppPage.inspect),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
        if (model.adoptable.isNotEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(28, 0, 28, 20),
            child: _AdoptCard(model: model),
          ),
      ],
    );
  }
}

String _initial(String name) {
  final trimmed = name.trim();
  return trimmed.isEmpty ? '?' : trimmed.substring(0, 1).toUpperCase();
}
