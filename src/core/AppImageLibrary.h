#pragma once

#include "Types.h"

#include <QVector>

namespace GoshAim {

class SettingsStore;
class ManagedRegistry;
class DesktopIntegration;

class AppImageLibrary
{
public:
    AppImageLibrary(SettingsStore *settings, ManagedRegistry *registry, DesktopIntegration *desktop);

    QVector<InstalledApp> discover();
    bool adopt(const InstalledApp &external, QString *error);

private:
    SettingsStore *m_settings = nullptr;
    ManagedRegistry *m_registry = nullptr;
    DesktopIntegration *m_desktop = nullptr;
};

} // namespace GoshAim
