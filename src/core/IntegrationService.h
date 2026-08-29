#pragma once

#include "Types.h"

#include <QString>
#include <atomic>

namespace GoshAim {

class SettingsStore;
class ManagedRegistry;
class AppImageInspector;
class DesktopIntegration;
class ProcessRunner;

class IntegrationService
{
public:
    IntegrationService(SettingsStore *settings,
                       ManagedRegistry *registry,
                       AppImageInspector *inspector,
                       DesktopIntegration *desktop,
                       ProcessRunner *runner);

    QString chooseDestinationName(const InspectionResult &inspection, bool omitSuffix) const;
    IntegrateResult integrate(const IntegrateRequest &request, std::atomic<bool> *cancel = nullptr);

private:
    void rollback(const QStringList &created);
    SettingsStore *m_settings = nullptr;
    ManagedRegistry *m_registry = nullptr;
    AppImageInspector *m_inspector = nullptr;
    DesktopIntegration *m_desktop = nullptr;
    ProcessRunner *m_runner = nullptr;
};

} // namespace GoshAim
