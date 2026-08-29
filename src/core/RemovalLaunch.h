#pragma once

#include "Types.h"

#include <atomic>

namespace GoshAim {

class SettingsStore;
class ManagedRegistry;
class DesktopIntegration;
class ProcessTable;
class ProcessRunner;

class RemovalService
{
public:
    RemovalService(SettingsStore *settings,
                   ManagedRegistry *registry,
                   DesktopIntegration *desktop,
                   ProcessTable *processes,
                   ProcessRunner *runner);

    bool trashFile(const QString &path, QString *error);
    RemovalResult remove(const RemovalRequest &request, std::atomic<bool> *cancel = nullptr);
    bool remove(const RemovalRequest &request, QString *error, std::atomic<bool> *cancel = nullptr);

private:
    InstalledApp resolve(const QString &pathOrUuid, QString *error) const;
    SettingsStore *m_settings = nullptr;
    ManagedRegistry *m_registry = nullptr;
    DesktopIntegration *m_desktop = nullptr;
    ProcessTable *m_processes = nullptr;
    ProcessRunner *m_runner = nullptr;
};

class LaunchService
{
public:
    LaunchService(ProcessRunner *runner, ProcessTable *processes);

    bool launch(const InstalledApp &app, QString *error);
    bool isRunning(const InstalledApp &app) const;
    bool nixNeedsAppimageRun() const;

private:
    ProcessRunner *m_runner = nullptr;
    ProcessTable *m_processes = nullptr;
};

} // namespace GoshAim
