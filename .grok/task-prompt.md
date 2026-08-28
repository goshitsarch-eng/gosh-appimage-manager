# Grok implementation mission: Gosh AppImage Manager

You are the primary implementation engineer for a complete production-quality application.

Working repository/worktree: `/root/projects/gosh-appimage-manager-grok`
Branch: `grok/appimage-manager`
Binding specification: `docs/implementation-brief.md`
Engineering rules: `AGENTS.md`
Behavioral audit: `docs/upstream-audit.md`
Read-only upstream reference: `/root/projects/_upstream/gearlever` pinned at `a2917f2adafc78e0478e47d5843de9ede6c1aa3f`

Implement the complete application now. Do not return merely a plan, prototype, mock UI, README-only answer, or list of future tasks.

Required process:

1. Read every binding document and inspect enough pinned Gear Lever source to understand its workflows, without copying Python/GTK source or visual assets.
2. Inspect the available KDE 6.10 SDK and Flatpak toolchain rather than guessing APIs.
3. Design a testable native C++20/Qt 6/KF6/Kirigami architecture and implement it coherently.
4. Implement production inspection, integration, ownership, desktop generation, removal, launch, update, task, model, settings and CLI paths—not fake success paths.
5. Treat AppImages and all derived/remote data as hostile. Preserve the brief's fail-closed behavior. Never execute AppImages in ordinary inspection/tests.
6. Use program plus argument arrays; never shell strings. Keep GUI I/O asynchronous and shutdown join-safe.
7. Create original Gosh artwork and complete legal/metadata/attribution files.
8. Provide a clean Flatpak manifest with pinned extraction dependencies and no `--filesystem=host:rw` shortcut.
9. Write extensive Qt tests with fake process/network seams and synthetic adversarial fixtures.
10. Build in `org.kde.Sdk//6.10`, run all tests, lint every QML file, validate desktop/metainfo, build the Flatpak, and run packaged non-mutating smoke/probe flows.
11. Iterate through every real build/test/package failure until green. Never fabricate results.
12. Self-review the complete `main..HEAD` diff for specification gaps, unsafe process/file/network behavior, lifecycle races, missing error handling, placeholders and packaging mistakes. Repair them before completion.
13. Update `README.md` and `docs/verification.md` with exact commands, outputs, supported behavior and honest limitations.
14. Run `git diff --check`, commit all intended project files in one or more coherent commits, and leave the worktree clean.

Scope constraints:

- Do not touch the user's real `~/AppImages`, desktop entries, icons or app configuration.
- Do not install, update, launch or remove a real AppImage.
- Any mutation tests must use an isolated temporary HOME and synthetic fixtures.
- Do not publish, push, create GitHub repositories, create releases or modify signed Gosh Apps channels.
- Do not use credentials or add GitHub Actions that consume paid runners.
- Do not modify `/root/projects/_upstream/gearlever`.
- Do not alter Hermes global configuration.

When finished, state:

- final commit SHA
- architecture and feature summary
- exact native and Flatpak build/test/lint/metadata/probe results
- how synthetic AppImage tests prove non-execution and transactional safety
- remaining non-blocking limitations
- confirmation that the worktree is clean

Continue autonomously through recoverable failures until the complete verified implementation is committed.