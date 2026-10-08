import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/state/format.dart';
import 'package:gosh_appimage_flutter/theme/cosmic_theme.dart';
import 'package:gosh_appimage_flutter/core/core_api.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// Every operation the core has run this session, newest first, with its
/// state, its target, and its progress while it runs.
class TasksPage extends StatelessWidget {
  const TasksPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final tasks = model.tasks;
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Text('Tasks', style: CosmicType.title3),
              const Spacer(),
              CosmicButton(
                label: 'Clear finished',
                onPressed: model.clearFinishedTasks,
              ),
            ],
          ),
          const SizedBox(height: 8),
          if (tasks.isEmpty) const Text('Nothing has run yet.'),
          for (final task in tasks) _TaskRow(task: task),
        ],
      ),
    );
  }
}

class _TaskRow extends StatelessWidget {
  const _TaskRow({required this.task});

  final TaskDto task;

  @override
  Widget build(BuildContext context) {
    final running =
        task.state == TaskStateDto.running ||
        task.state == TaskStateDto.cancelling;
    return Padding(
      padding: const EdgeInsets.all(8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            '${task.title} — ${taskStateWord(task.state)}',
            style: CosmicType.heading,
          ),
          if (task.target.isNotEmpty)
            Text(task.target, style: CosmicType.caption),
          if (running) ...[
            const SizedBox(height: 4),
            CosmicProgressBar(percent: task.progress),
          ],
          if (task.error.isNotEmpty) Text('Error: ${task.error}'),
        ],
      ),
    );
  }
}
