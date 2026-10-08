import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/src/rust/api/app.dart';
import 'package:gosh_appimage_flutter/src/rust/frb_generated.dart';

/// Bridge spike: one screen that calls each kind of bridge operation once.
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  runApp(const SpikeApp());
}

class SpikeApp extends StatelessWidget {
  const SpikeApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Gosh AppImage Manager (bridge spike)',
      theme: ThemeData(colorSchemeSeed: Colors.teal, useMaterial3: true),
      home: const SpikePage(),
    );
  }
}

class SpikePage extends StatefulWidget {
  const SpikePage({super.key});

  @override
  State<SpikePage> createState() => _SpikePageState();
}

class _SpikePageState extends State<SpikePage> {
  final TextEditingController _path = TextEditingController(
    text: Platform.environment['GOSH_INSPECT_PATH'] ?? '',
  );
  String _inspectResult = 'Not run yet.';
  String _tickResult = 'Not run yet.';
  String _panicResult = 'Not run yet.';
  StreamSubscription<int>? _ticks;

  @override
  void initState() {
    super.initState();
    if (_path.text.isNotEmpty) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _inspect());
    }
  }

  @override
  void dispose() {
    _ticks?.cancel();
    _path.dispose();
    super.dispose();
  }

  Future<void> _inspect() async {
    try {
      final summary = await inspectPath(path: _path.text);
      setState(() => _inspectResult = _format(summary));
    } on CoreError catch (error) {
      setState(
        () => _inspectResult = 'Error (${error.kind.name}): ${error.message}',
      );
    }
  }

  void _count() {
    _ticks?.cancel();
    final values = <int>[];
    setState(() => _tickResult = 'Counting...');
    _ticks = countTicks(total: 5).listen(
      (value) => setState(() {
        values.add(value);
        _tickResult = 'Received: ${values.join(', ')}';
      }),
      onDone: () => setState(() => _tickResult = '$_tickResult  (done)'),
    );
  }

  Future<void> _panic() async {
    try {
      final value = await panicForContractTest();
      setState(() => _panicResult = 'Unexpected success: $value');
    } on CoreError catch (error) {
      setState(
        () => _panicResult = 'Mapped to ${error.kind.name}: ${error.message}',
      );
    }
  }

  String _format(InspectSummary s) {
    return [
      'path: ${s.path}',
      'size: ${s.sizeBytes} bytes',
      'sha256: ${s.sha256}',
      'type: ${s.appType}',
      'architecture: ${s.architecture}',
      'magic valid: ${s.magicValid}',
      'architecture supported: ${s.architectureSupported}',
      'name: ${s.name.isEmpty ? '(none)' : s.name}',
      'warnings: ${s.warnings.isEmpty ? '(none)' : s.warnings.join('; ')}',
    ].join('\n');
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text('Bridge spike, core ${bridgeVersion()}')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: ListView(
          children: [
            TextField(
              controller: _path,
              decoration: const InputDecoration(
                labelText: 'AppImage path',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              children: [
                FilledButton(onPressed: _inspect, child: const Text('Inspect')),
                OutlinedButton(
                  onPressed: _count,
                  child: const Text('Count to 5'),
                ),
                OutlinedButton(
                  onPressed: _panic,
                  child: const Text('Trigger panic'),
                ),
              ],
            ),
            const SizedBox(height: 16),
            SelectableText('Inspect:\n$_inspectResult'),
            const SizedBox(height: 12),
            SelectableText('Stream:\n$_tickResult'),
            const SizedBox(height: 12),
            SelectableText('Panic containment:\n$_panicResult'),
          ],
        ),
      ),
    );
  }
}
