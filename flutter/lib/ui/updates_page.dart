import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/detail_page.dart';
import 'package:gosh_appimage_flutter/ui/page_frame.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// Checks every app's update source and applies updates on request (SPEC
/// 7.5). A failed check reads as unknown, never as up to date: failures are
/// rows of their own and count under "Unknown, check failed".
class UpdatesPage extends StatelessWidget {
  const UpdatesPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final narrow = isNarrow(context);
    final checked = model.lastChecked;
    final subtitle = checked == null
        ? 'Not checked yet'
        : 'Last checked today at ${clockLabel(checked)}';
    final failures = model.checkFailures;
    final total = model.library.length;
    final available = model.updateCount;
    final unknown = failures.length;
    // An app no check has covered is not checked yet: it is neither available
    // nor up to date (QA D-03).
    final notChecked = model.notCheckedCount;
    final upToDate = (total - available - unknown - notChecked).clamp(0, total);
    final offers = [
      for (final app in model.library)
        if (model.offerFor(app.uuid) != null) app,
    ];
    final failedApps = [
      for (final app in model.library)
        if (model.failureFor(app.uuid) != null) app,
    ];
    final rows = [
      ...offers,
      ...failedApps.where((app) => !offers.contains(app)),
    ];
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PageHeader(
            title: 'Updates',
            subtitle: subtitle,
            actions: [
              AppButton(
                label: 'Check now',
                buttonKey: const Key('check-now'),
                onPressed: model.checkUpdates,
              ),
              AppButton(
                label: 'Update all',
                variant: AppButtonVariant.primary,
                horizontalPadding: 14,
                buttonKey: const Key('update-all'),
                onPressed: model.updateAll,
              ),
            ],
          ),
          Padding(
            padding: EdgeInsets.fromLTRB(
              narrow ? 14 : 28,
              18,
              narrow ? 14 : 28,
              0,
            ),
            child: Row(
              children: [
                Expanded(
                  child: _SummaryCard(
                    label: 'Available',
                    value: '$available',
                    color: palette.accent,
                    keyName: 'summary-available',
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _SummaryCard(
                    label: 'Up to date',
                    value: '$upToDate',
                    color: palette.text,
                    keyName: 'summary-up-to-date',
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _SummaryCard(
                    label: 'Unknown, check failed',
                    value: '$unknown',
                    color: palette.bad,
                    keyName: 'summary-unknown',
                  ),
                ),
              ],
            ),
          ),
          Padding(
            padding: EdgeInsets.fromLTRB(
              narrow ? 14 : 28,
              16,
              narrow ? 14 : 28,
              20,
            ),
            child: AppCard(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  _UpdatesHeader(palette: palette, narrow: narrow),
                  if (model.busy != null && rows.isEmpty)
                    Padding(
                      padding: const EdgeInsets.all(18),
                      child: Text(
                        'Checking update sources…',
                        style: AppType.sans(13, color: palette.text2),
                      ),
                    ),
                  if (rows.isEmpty && model.busy == null)
                    Padding(
                      padding: const EdgeInsets.all(18),
                      child: Text(
                        total == 0
                            ? 'No AppImages are integrated yet.'
                            : notChecked > 0
                            ? 'Not checked yet. Use Check now to look for updates.'
                            : 'Everything is up to date.',
                        style: AppType.sans(13, color: palette.text2),
                      ),
                    ),
                  for (var i = 0; i < rows.length; i++)
                    _UpdateRow(
                      model: model,
                      app: rows[i],
                      narrow: narrow,
                      last: i == rows.length - 1,
                    ),
                ],
              ),
            ),
          ),
          if (model.updateFailures.isNotEmpty)
            Padding(
              padding: EdgeInsets.fromLTRB(
                narrow ? 14 : 28,
                0,
                narrow ? 14 : 28,
                20,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Some updates did not apply',
                    style: AppType.sans(
                      13,
                      weight: FontWeight.w600,
                      color: palette.bad,
                    ),
                  ),
                  for (final problem in model.updateFailures)
                    Text(
                      '${problem.name}: '
                      '${checkFailureText(problem.error, timedOut: problem.timedOut)}',
                      style: AppType.sans(12, color: palette.text2),
                    ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _SummaryCard extends StatelessWidget {
  const _SummaryCard({
    required this.label,
    required this.value,
    required this.color,
    required this.keyName,
  });

  final String label;
  final String value;
  final Color color;
  final String keyName;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return AppCard(
      key: Key(keyName),
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: AppType.sans(12, color: palette.text2)),
          const SizedBox(height: 4),
          Text(
            value,
            style: AppType.sans(
              22,
              weight: FontWeight.w600,
              letterSpacing: -0.44,
              color: color,
            ),
          ),
        ],
      ),
    );
  }
}

class _UpdatesHeader extends StatelessWidget {
  const _UpdatesHeader({required this.palette, required this.narrow});

  final AppPalette palette;
  final bool narrow;

  @override
  Widget build(BuildContext context) {
    final style = AppType.label(color: palette.text3);
    return Container(
      height: 37,
      padding: const EdgeInsets.symmetric(horizontal: 18),
      decoration: BoxDecoration(
        color: palette.canvas,
        border: Border(bottom: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          Expanded(flex: 16, child: Text('APP', style: style)),
          if (!narrow) ...[
            const SizedBox(width: 16),
            SizedBox(width: 150, child: Text('VERSION', style: style)),
            const SizedBox(width: 16),
            Expanded(flex: 13, child: Text('STATE', style: style)),
            const SizedBox(width: 16),
            const SizedBox(width: 110),
          ],
        ],
      ),
    );
  }
}

/// One row: the app, its version change, its state, and the action that
/// applies to that state.
class _UpdateRow extends StatelessWidget {
  const _UpdateRow({
    required this.model,
    required this.app,
    required this.narrow,
    this.last = false,
  });

  final AppModel model;
  final AppDto app;
  final bool narrow;
  final bool last;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final offer = model.offerFor(app.uuid);
    final failure = model.failureFor(app.uuid);
    final task = model.updateTaskFor(app);
    final initial = app.name.isEmpty
        ? '?'
        : app.name.substring(0, 1).toUpperCase();
    final source = _sourceOf(app);
    final app0 = _AppCell(
      initial: initial,
      name: app.name,
      source: source,
      palette: palette,
    );
    final version = Row(
      children: [
        Flexible(
          child: Text(
            offer?.currentVersion ?? app.version,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: AppType.mono(12.5, color: palette.text2),
          ),
        ),
        if (offer != null) ...[
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              '→ ${offer.availableVersion}',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AppType.mono(
                12.5,
                weight: FontWeight.w500,
                color: palette.accent,
              ),
            ),
          ),
        ],
      ],
    );
    final state = _StateCell(
      model: model,
      app: app,
      offer: offer,
      failure: failure,
      task: task,
    );
    final action = _ActionCell(
      model: model,
      app: app,
      offer: offer,
      failure: failure,
      task: task,
    );
    final content = narrow
        ? Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              app0,
              const SizedBox(height: 8),
              version,
              const SizedBox(height: 8),
              state,
              const SizedBox(height: 8),
              Align(alignment: Alignment.centerRight, child: action),
            ],
          )
        : Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Expanded(flex: 16, child: app0),
              const SizedBox(width: 16),
              SizedBox(width: 150, child: version),
              const SizedBox(width: 16),
              Expanded(flex: 13, child: state),
              const SizedBox(width: 16),
              SizedBox(
                width: 110,
                child: Align(alignment: Alignment.centerRight, child: action),
              ),
            ],
          );
    return Container(
      key: Key('update-row-${app.uuid}'),
      constraints: const BoxConstraints(minHeight: 62),
      padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 10),
      decoration: BoxDecoration(
        border: last ? null : Border(bottom: BorderSide(color: palette.border)),
      ),
      child: Align(alignment: Alignment.centerLeft, child: content),
    );
  }

  String _sourceOf(AppDto app) {
    final manager = app.updateManager.isEmpty
        ? ''
        : managerLabel(app.updateManager);
    final config = [for (final pair in app.updateConfig) pair.value]
        .join(' · ');
    if (manager.isEmpty) {
      return config.isEmpty ? 'No update source' : config;
    }
    return config.isEmpty ? manager : '$manager · $config';
  }
}

class _AppCell extends StatelessWidget {
  const _AppCell({
    required this.initial,
    required this.name,
    required this.source,
    required this.palette,
  });

  final String initial;
  final String name;
  final String source;
  final AppPalette palette;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      AppLetterTile(letter: initial, size: 34, fontSize: 14),
      const SizedBox(width: 12),
      Expanded(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AppType.sans(
                13.5,
                weight: FontWeight.w600,
                color: palette.text,
              ),
            ),
            Text(
              source,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AppType.mono(11.5, color: palette.text3),
            ),
          ],
        ),
      ),
    ],
  );
}

/// The state column: running, updating with progress, failed, or waiting.
class _StateCell extends StatelessWidget {
  const _StateCell({
    required this.model,
    required this.app,
    required this.offer,
    required this.failure,
    required this.task,
  });

  final AppModel model;
  final AppDto app;
  final UpdateOfferDto? offer;
  final UpdateProblem? failure;
  final TaskDto? task;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final reduced = offer?.reducedVerification ?? false;
    if (failure != null) {
      return _Line(
        label: 'Check failed',
        color: palette.bad,
        dot: palette.bad,
        detail: checkFailureText(failure!.error, timedOut: failure!.timedOut),
        palette: palette,
      );
    }
    if (task != null) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(child: AppProgressBar(percent: task!.progress)),
              const SizedBox(width: 10),
              Text(
                '${task!.progress}%',
                style: AppType.mono(
                  11.5,
                  weight: FontWeight.w500,
                  color: palette.text2,
                ),
              ),
            ],
          ),
          if (reduced)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                'Reduced verification: no checksum published',
                style: AppType.sans(12, color: palette.warnText),
              ),
            ),
        ],
      );
    }
    if (app.running && offer != null) {
      return _Line(
        label: 'App is running',
        color: palette.ok,
        dot: palette.ok,
        detail: "You'll be asked to confirm",
        palette: palette,
      );
    }
    if (reduced) {
      return Text(
        'Reduced verification: no checksum published',
        style: AppType.sans(12, color: palette.warnText),
      );
    }
    return _Line(
      label: 'Update available',
      color: palette.text,
      dot: palette.accent,
      palette: palette,
    );
  }
}

class _Line extends StatelessWidget {
  const _Line({
    required this.label,
    required this.color,
    required this.dot,
    required this.palette,
    this.detail,
  });

  final String label;
  final Color color;
  final Color dot;
  final String? detail;
  final AppPalette palette;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Row(
        children: [
          StatusDot(color: dot, size: 7),
          const SizedBox(width: 7),
          Flexible(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AppType.sans(13, weight: FontWeight.w500, color: color),
            ),
          ),
        ],
      ),
      if (detail != null)
        Padding(
          padding: const EdgeInsets.only(top: 1, left: 14),
          child: Text(detail!, style: AppType.sans(12, color: palette.text2)),
        ),
    ],
  );
}

/// The action for a row: Update… for an offer, Cancel for a running update,
/// Retry for a failed check.
class _ActionCell extends StatelessWidget {
  const _ActionCell({
    required this.model,
    required this.app,
    required this.offer,
    required this.failure,
    required this.task,
  });

  final AppModel model;
  final AppDto app;
  final UpdateOfferDto? offer;
  final UpdateProblem? failure;
  final TaskDto? task;

  @override
  Widget build(BuildContext context) {
    if (task != null) {
      return AppButton(
        label: 'Cancel',
        variant: AppButtonVariant.text,
        height: 28,
        fontSize: 12.5,
        buttonKey: Key('cancel-update-${app.uuid}'),
        onPressed: () => model.cancelUpdate(app),
      );
    }
    if (failure != null) {
      return AppButton(
        label: 'Retry',
        height: 28,
        fontSize: 12.5,
        buttonKey: Key('retry-${app.uuid}'),
        onPressed: model.checkUpdates,
      );
    }
    return AppButton(
      label: 'Update…',
      height: 28,
      fontSize: 12.5,
      buttonKey: Key('update-${app.uuid}'),
      onPressed: offer == null ? null : () => model.updateOne(app.uuid),
    );
  }
}
