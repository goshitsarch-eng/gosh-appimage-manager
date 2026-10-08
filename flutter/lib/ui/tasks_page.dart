import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/page_frame.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// The operations the core has run this session (SPEC 7.6). Running work
/// shows its versions, its stage and its bytes; finished work lists what
/// happened and when the core says it finished. Clear finished removes
/// completed entries only.
class TasksPage extends StatelessWidget {
  const TasksPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final narrow = isNarrow(context);
    final running = model.runningTasks;
    final finished = model.finishedTasks;
    final gutter = narrow ? 14.0 : 28.0;
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PageHeader(
            title: 'Tasks',
            subtitle: '${running.length} running, ${finished.length} finished',
            actions: [
              AppButton(
                label: 'Clear finished',
                buttonKey: const Key('clear-finished'),
                onPressed: finished.isEmpty ? null : model.clearFinishedTasks,
              ),
            ],
          ),
          if (model.tasks.isEmpty)
            Padding(
              padding: EdgeInsets.fromLTRB(gutter, 18, gutter, 0),
              child: Text(
                'Nothing has run yet.',
                style: AppType.sans(13, color: palette.text2),
              ),
            ),
          if (running.isNotEmpty) ...[
            _SectionLabel(text: 'Running', gutter: gutter, top: 22),
            for (final task in running)
              Padding(
                padding: EdgeInsets.fromLTRB(gutter, 0, gutter, 0),
                child: _RunningCard(model: model, task: task),
              ),
          ],
          if (finished.isNotEmpty) ...[
            _SectionLabel(text: 'Finished', gutter: gutter, top: 24),
            Padding(
              padding: EdgeInsets.fromLTRB(gutter, 0, gutter, 20),
              child: AppCard(
                child: Column(
                  children: [
                    for (var i = 0; i < finished.length; i++)
                      _FinishedRow(
                        model: model,
                        task: finished[i],
                        last: i == finished.length - 1,
                      ),
                  ],
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel({
    required this.text,
    required this.gutter,
    required this.top,
  });

  final String text;
  final double gutter;
  final double top;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    return Padding(
      padding: EdgeInsets.fromLTRB(gutter, top, gutter, 8),
      child: Text(
        text.toUpperCase(),
        style: AppType.label(color: palette.text3),
      ),
    );
  }
}

/// The stages an update passes through, in order. The number is the core's
/// `phase_index`, so the highlight follows the core, not the text.
const List<String> _phases = ['1 Download', '2 Verify', '3 Swap in'];

/// `3.1.0 → 3.2.0`, or empty when the core did not name both versions.
String _versionPair(TaskDto task) =>
    task.fromVersion.isEmpty || task.toVersion.isEmpty
    ? ''
    : '${task.fromVersion} → ${task.toVersion}';

/// Joins the non-empty words with single spaces.
String _words(Iterable<String> parts) =>
    parts.where((part) => part.isNotEmpty).join(' ');

class _RunningCard extends StatelessWidget {
  const _RunningCard({required this.model, required this.task});

  final AppModel model;
  final TaskDto task;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final target = task.target.isEmpty ? task.title : task.target;
    final initial = target.isEmpty ? '?' : target.substring(0, 1).toUpperCase();
    final active = task.phaseIndex - 1;
    final cancellable =
        task.state == TaskStateDto.running || task.state == TaskStateDto.queued;
    final versions = _versionPair(task);
    return AppCard(
      key: Key('running-${task.id}'),
      padding: const EdgeInsets.fromLTRB(18, 16, 18, 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              AppLetterTile(letter: initial, size: 34, fontSize: 14),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      _words([task.title, target]),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AppType.sans(
                        13,
                        weight: FontWeight.w600,
                        color: palette.text,
                      ),
                    ),
                    Text(
                      versions.isEmpty ? taskStateWord(task.state) : versions,
                      style: AppType.mono(12, color: palette.text2),
                    ),
                  ],
                ),
              ),
              if (cancellable)
                AppButton(
                  label: 'Cancel',
                  height: 28,
                  fontSize: 12.5,
                  buttonKey: Key('task-cancel-${task.id}'),
                  onPressed: () => model.cancelTaskById(task.id),
                ),
            ],
          ),
          const SizedBox(height: 12),
          AppProgressBar(percent: task.progress),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: Row(
                  children: [
                    for (var i = 0; i < _phases.length; i++) ...[
                      if (i > 0) const SizedBox(width: 16),
                      Text(
                        _phases[i],
                        style: AppType.sans(
                          12,
                          weight: i == active
                              ? FontWeight.w500
                              : FontWeight.w400,
                          color: i == active ? palette.text : palette.text2,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              if (task.bytesTotal > 0)
                Text(
                  byteProgressLabel(task.bytesDone, task.bytesTotal),
                  style: AppType.mono(12, color: palette.text2),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class _FinishedRow extends StatelessWidget {
  const _FinishedRow({
    required this.model,
    required this.task,
    this.last = false,
  });

  final AppModel model;
  final TaskDto task;
  final bool last;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final done = task.state == TaskStateDto.succeeded;
    // A cancelled task says so; a failed one says failed. Neither is a success.
    final cancelled = task.state == TaskStateDto.cancelled;
    final label = done
        ? _doneLabel(task)
        : _words([task.title, task.target, cancelled ? 'cancelled' : 'failed']);
    final when = model.finishedAt(task);
    return Container(
      key: Key('finished-${task.id}'),
      padding: const EdgeInsets.fromLTRB(18, 13, 18, 13),
      decoration: BoxDecoration(
        border: last ? null : Border(bottom: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          SizedBox(
            width: 20,
            child: done
                ? AppIcon('status-done', size: 16, color: palette.ok)
                : Icon(Icons.error_outline, size: 16, color: palette.bad),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AppType.sans(
                13,
                weight: FontWeight.w500,
                color: done
                    ? palette.text
                    : (cancelled ? palette.text2 : palette.bad),
              ),
            ),
          ),
          const SizedBox(width: 12),
          SizedBox(
            width: 140,
            child: Text(
              when == null
                  ? taskStateWord(task.state)
                  : taskTimeLabel(when, model.now),
              textAlign: TextAlign.right,
              style: AppType.mono(11.5, color: palette.text3),
            ),
          ),
        ],
      ),
    );
  }
}

/// What a finished task reads as: `Integrated Cinder Chat 0.14.2`,
/// `Updated Brisk Terminal 1.1.4 → 1.2.0`, `Moved Old Notes 1.0 to the Trash`,
/// or `Deleted Old Notes 1.0` for a removal that was not sent to the Trash.
String _doneLabel(TaskDto task) => switch (task.kind) {
  TaskKindDto.integrate => _words(['Integrated', task.target, task.toVersion]),
  TaskKindDto.update => _words(['Updated', task.target, _versionPair(task)]),
  TaskKindDto.remove when task.permanent => _words([
    'Deleted',
    task.target,
    task.fromVersion,
  ]),
  TaskKindDto.remove => _words([
    'Moved',
    task.target,
    task.fromVersion,
    'to the Trash',
  ]),
  TaskKindDto.inspect => _words(['Inspected', task.target]),
  TaskKindDto.refreshMetadata => _words(['Refreshed', task.target]),
  TaskKindDto.checkUpdate => _words(['Checked for updates', task.target]),
  TaskKindDto.adopt => _words(['Adopted', task.target]),
};
