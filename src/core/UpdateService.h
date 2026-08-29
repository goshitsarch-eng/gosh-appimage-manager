#pragma once

#include "Types.h"
#include "UpdateSources.h"

#include <atomic>
#include <functional>

namespace GoshAim {

class SettingsStore;
class ManagedRegistry;
class AppImageInspector;
class DesktopIntegration;
class IntegrationService;
class NetworkClient;
class ProcessTable;
class ProcessRunner;
class CheckStateStore;

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
    ~UpdateService();

    using ProgressFn = std::function<void(int percent, const QString &status)>;

    UpdateCheckResult check(const InstalledApp &app, std::atomic<bool> *cancel = nullptr);
    QVector<UpdateOffer> listUpdates(std::atomic<bool> *cancel = nullptr, bool persistInstallState = false);
    IntegrateResult apply(const InstalledApp &app, bool force, std::atomic<bool> *cancel = nullptr, const ProgressFn &progress = {});
    bool setSource(InstalledApp app, const QString &manager, const QVariantMap &config, QString *error);
    bool unsetSource(InstalledApp app, QString *error);
    void setFailPoint(UpdateFailPoint point) { m_failPoint = point; }

private:
    bool verifyStagedDigest(const QString &staging, const UpdateCheckResult &checked, QString *error) const;
    SettingsStore *m_settings = nullptr;
    ManagedRegistry *m_registry = nullptr;
    AppImageInspector *m_inspector = nullptr;
    DesktopIntegration *m_desktop = nullptr;
    NetworkClient *m_network = nullptr;
    ProcessTable *m_processes = nullptr;
    ProcessRunner *m_runner = nullptr;
    CheckStateStore *m_checkState = nullptr;
    UpdateFailPoint m_failPoint = UpdateFailPoint::None;
};

} // namespace GoshAim
