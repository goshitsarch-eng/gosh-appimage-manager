# Gear Lever behavioral reference audit

Pinned upstream: `https://github.com/mijorus/gearlever`
Pinned commit: `a2917f2adafc78e0478e47d5843de9ede6c1aa3f`
Observed version: `4.6.2`
License: GNU GPL version 3 or later.
Architecture: Python, GTK 4, Libadwaita, Meson, Flatpak GNOME runtime.

This project uses Gear Lever as a feature and workflow reference only. Gosh AppImage Manager is a new native C++20/Qt 6/Kirigami implementation. Do not copy upstream Python, GTK templates, CSS, icons, screenshots, or branding.

## Behavior inventory

- Open one or multiple AppImages, including application open-with and drag/drop.
- Preview name, version, description, architecture, terminal mode, icon and desktop metadata before integration.
- Integrate into a configurable AppImages folder and the user application menu.
- Copy or move the original according to settings.
- Preserve name conflicts by keeping both or explicitly replacing.
- Discover integrated AppImages from desktop entries, including files outside the default folder when enabled.
- Launch, reveal, remove to Trash, explicitly delete, refresh metadata and edit arguments/environment variables.
- Detect running applications before update; allow an explicit force override.
- Check, download, cancel and apply updates individually or as a batch.
- Read embedded `.upd_info` update information.
- Update managers: static file, GitHub, GitLab, Codeberg, Forgejo and FTP.
- CLI: integrate, update, remove, remove-all, list-installed, list-updates, list-update-managers, set-update-source and background fetch; JSON schema version 1 for list output.
- Optional background update checks and desktop notifications.
- Metadata extraction supports AppImage Type 1/2, SquashFS and DwarFS-oriented tools, with direct AppImage execution as a blocked-by-default fallback.

## Safety lessons to improve

- Do not fall back to permanent deletion when Trash fails.
- Avoid `--filesystem=host:rw`; use portals, narrow persistent paths and argument-safe host operations.
- Do not trust MIME alone; validate regular file, size, ELF/AppImage magic and architecture.
- Never use shell command strings or interpolate untrusted values into desktop `Exec` lines.
- Treat archive paths, symlinks, icons, desktop keys, update URLs, redirects, process output and remote JSON as hostile.
- Bound all reads, extraction output, decompression, downloads, redirects, timeouts and process lifetime.
- Updates must validate to an AppImage before an atomic replacement and preserve rollback material until success.
- Manage/remove only artifacts carrying this application's ownership markers or explicitly adopted after confirmation.
