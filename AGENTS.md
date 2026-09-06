# Gosh AppImage Manager engineering rules

Product: Gosh AppImage Manager
Application ID: com.goshapps.AppImageManager
Executable and repository: gosh-appimage-manager
Public project identity: Gosh-Its-Arch
License: GPL-3.0-or-later

- This is an original Rust + libcosmic (COSMIC Epoch) implementation, version 3.0.0. No Qt/KF/Kirigami/CMake runtime deps.
- Gear Lever is a GPLv3 behavioral reference. Do not copy its Python/GTK source, UI assets, icon, screenshots, or branding.
- Treat every AppImage, desktop file, icon, update descriptor, URL, archive entry, and process output as untrusted.
- Never execute an AppImage to inspect metadata by default. The unsafe legacy extraction fallback must be opt-in, clearly warned, and disabled by default.
- Never construct shell command strings. Use program plus argument arrays.
- Never delete user data when Trash fails. Permanent deletion requires an explicit destructive confirmation.
- Never overwrite an existing AppImage, desktop entry, icon, configuration, or update target without verified ownership and explicit replace semantics.
- Keep mutations transactional and recoverable. Validate downloads before atomic replacement and retain rollback material until success.
- Build and test with cargo + just; ship in the freedesktop 23.08 Flatpak SDK (x86_64 + aarch64, single manifest). Real tests, package builds, packaged smoke checks, and documented evidence are required.
- No publishing or signed-channel enrollment from implementation workers.
