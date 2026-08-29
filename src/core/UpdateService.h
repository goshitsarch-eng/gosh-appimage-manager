#pragma once

#include "Types.h"
#include "UpdateSources.h"

#include <atomic>

namespace GoshAim {

class SettingsStore;
class ManagedRegistry;
class AppImageInspector;
class DesktopIntegration;
class IntegrationService;
class NetworkClient;
class ProcessTable;
class ProcessRunner;

class UpdateService
{
public:
    UpdateService(SettingsStore *settings,
                  ManagedRegistry *registry,
                  AppImageInspector *inspector,
                  DesktopIntegration *desktop,
                  NetworkClient *network,
                  ProcessTable *processes,
                  ProcessRunner *runner);

    UpdateCheckResult check(const InstalledApp &app, std::atomic<bool> *cancel = nullptr);
    QVector<UpdateOffer> listUpdates(std::atomic<bool> *cancel = nullptr);
    IntegrateResult apply(const InstalledApp &app, bool force, std::atomic<bool> *cancel = nullptr);
    bool setSource(InstalledApp app, const QString &manager, const QVariantMap &config, QString *error);
    bool unsetSource(InstalledApp app, QString *error);

private:
    SettingsStore *m_settings = nullptr;
    ManagedRegistry *m_registry = nullptr;
    AppImageInspector *m_inspector = nullptr;
    DesktopIntegration *m_desktop = nullptr;
    NetworkClient *m_network = nullptr;
    ProcessTable *m_processes = nullptr;
    ProcessRunner *m_runner = nullptr;
};

} // namespace GoshAim
