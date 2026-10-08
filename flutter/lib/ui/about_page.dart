import 'package:flutter/material.dart';
import 'package:gosh_appimage_flutter/state/app_model.dart';
import 'package:gosh_appimage_flutter/theme/app_theme.dart';
import 'package:gosh_appimage_flutter/ui/page_frame.dart';
import 'package:gosh_appimage_flutter/ui/settings_page.dart';
import 'package:gosh_appimage_flutter/ui/widgets.dart';

/// The application ID and homepage. Cargo.toml and the AppStream metainfo
/// (data/com.goshapps.AppImageManager.metainfo.xml) state the same two values.
const String _appId = 'com.goshapps.AppImageManager';
const String _homepage = 'https://goshapps.com';

/// The content width at which the cards sit in two columns: two columns of
/// about 340 px, the widest row's need, and the 20 px gap between them.
const double _twoColumnsMinWidth = 700;

/// Product identity and the statements the application makes about itself.
/// The mockup has no About frame, so the page uses the Settings layout: the
/// same header, cards, rows and type roles (SPEC 7.7). The wording is the
/// original About text; only the toolkit reference changed.
class AboutPage extends StatelessWidget {
  const AboutPage({super.key, required this.model});

  final AppModel model;

  @override
  Widget build(BuildContext context) {
    final palette = AppScope.of(context);
    final narrow = isNarrow(context);
    final gutter = narrow ? 14.0 : 28.0;
    final version = model.version;
    final valueStyle = AppType.mono(12, color: palette.text2);
    final versionValue = version == null
        ? null
        : Text(version, style: valueStyle);
    final identity = SettingsCard(
      title: 'Gosh AppImage Manager',
      rows: [
        SettingsRow(label: 'Version', control: versionValue),
        SettingsRow(
          label: 'Application ID',
          control: Text(_appId, style: valueStyle),
        ),
        SettingsRow(
          label: 'Homepage',
          control: Text(_homepage, style: valueStyle),
        ),
        const SettingsRow(label: 'Made by', control: Text('Gosh')),
        const SettingsRow(
          label: 'Purpose',
          help:
              'Native application for safely inspecting, integrating, '
              'launching, organizing, updating, and removing AppImages.',
        ),
      ],
    );
    final about = SettingsCard(
      title: 'About this application',
      rows: const [
        SettingsRow(
          label: 'Safety',
          help:
              'Opening an AppImage never integrates or executes it. Updates are '
              'staged, validated, and applied atomically with rollback.',
        ),
        SettingsRow(label: 'Telemetry', help: 'No telemetry of any kind.'),
      ],
    );
    final credits = SettingsCard(
      title: 'Credits',
      rows: const [
        SettingsRow(
          label: 'Behavioural reference',
          help:
              'Gear Lever by Lorenzo Paderi. This is an independent original '
              'implementation and is not endorsed by its authors.',
        ),
      ],
    );
    final license = SettingsCard(
      title: 'License',
      rows: const [
        SettingsRow(
          label: 'License',
          help:
              'Licensed under the GNU General Public License, version 3 or '
              'later. This program comes with absolutely no warranty.',
        ),
      ],
    );
    final cards = [identity, about, credits, license];
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PageHeader(
            title: 'About',
            subtitle: 'Gosh AppImage Manager ${version ?? ''}'.trim(),
          ),
          LayoutBuilder(
            builder: (context, constraints) {
              // Two columns need room for the widest row, the application ID
              // with its label. Narrower than that, the cards stack in one
              // column instead of overflowing (R6-08).
              final columnsFit =
                  constraints.maxWidth - 2 * gutter >= _twoColumnsMinWidth;
              final stacked = narrow || !columnsFit;
              return Padding(
                padding: EdgeInsets.fromLTRB(gutter, 18, gutter, 24),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    stacked
                        ? Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              for (final card in cards) ...[
                                card,
                                const SizedBox(height: 16),
                              ],
                            ],
                          )
                        : Row(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Expanded(
                                child: Column(
                                  crossAxisAlignment:
                                      CrossAxisAlignment.stretch,
                                  children: [
                                    identity,
                                    const SizedBox(height: 16),
                                    about,
                                  ],
                                ),
                              ),
                              const SizedBox(width: 20),
                              Expanded(
                                child: Column(
                                  crossAxisAlignment:
                                      CrossAxisAlignment.stretch,
                                  children: [
                                    credits,
                                    const SizedBox(height: 16),
                                    license,
                                  ],
                                ),
                              ),
                            ],
                          ),
                    // The footer line takes the Settings help role.
                    const SizedBox(height: 16),
                    Text(
                      'Licensed under GPL-3.0-or-later.',
                      style: AppType.sans(
                        12,
                        height: 1.5,
                        color: palette.text2,
                      ),
                    ),
                  ],
                ),
              );
            },
          ),
        ],
      ),
    );
  }
}
