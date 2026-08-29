#include "AppImageLibrary.h"

#include "DesktopIntegration.h"
#include "ElfParser.h"
#include "ManagedRegistry.h"
#include "SafeFs.h"
#include "SettingsStore.h"

#include <QDir>
#include <QFileInfo>
#include <QHash>

namespace GoshAim {

AppImageLibrary::AppImageLibrary(SettingsStore *settings, ManagedRegistry *registry, DesktopIntegration *desktop)
    : m_settings(settings)
    , m_registry(registry)
    , m_desktop(desktop)
{
}

QVector<InstalledApp> AppImageLibrary::discover()
{
    QVector<InstalledApp> apps = m_registry->apps();
    QHash<QString, int> byPath;
    for (int i = 0; i < apps.size(); ++i) {
        byPath.insert(apps[i].managedPath, i);
        apps[i].externalFolder = !apps[i].managedPath.startsWith(m_settings->managedFolder());
    }
    const QDir dir(m_settings->applicationsDir());
    const QFileInfoList entries = dir.entryInfoList(QStringList{QStringLiteral("*.desktop")}, QDir::Files);
    for (const QFileInfo &entry : entries) {
        InstalledApp parsed = m_desktop->parseExternalDesktop(entry.absoluteFilePath());
        if (parsed.managedPath.isEmpty() || !SafeFs::isRegularFile(parsed.managedPath)) {
            continue;
        }
        const ElfInfo elf = ElfParser::parseFile(parsed.managedPath);
        if (elf.appImageType == AppImageType::Unknown) {
            continue;
        }
        parsed.type = elf.appImageType;
        parsed.architecture = elf.architecture;
        parsed.size = QFileInfo(parsed.managedPath).size();
        const bool inFolder = parsed.managedPath.startsWith(m_settings->managedFolder());
        parsed.externalFolder = !inFolder;
        if (!inFolder && !m_settings->manageOutsideFolder() && parsed.uuid.isEmpty()) {
            continue;
        }
        if (byPath.contains(parsed.managedPath)) {
            continue;
        }
        if (parsed.owned && !parsed.uuid.isEmpty() && !m_registry->byUuid(parsed.uuid).uuid.isEmpty()) {
            continue;
        }
        parsed.owned = parsed.owned && !parsed.uuid.isEmpty();
        if (!parsed.owned) {
            parsed.uuid = QStringLiteral("external:") + entry.fileName();
        }
        apps.append(parsed);
    }
    return apps;
}

bool AppImageLibrary::adopt(const InstalledApp &external, QString *error)
{
    if (external.managedPath.isEmpty()) {
        if (error) {
            *error = QStringLiteral("Nothing to adopt");
        }
        return false;
    }
    InstalledApp app = external;
    if (app.uuid.startsWith(QLatin1String("external:"))) {
        app.uuid = ManagedRegistry::newUuid();
    }
    app.owned = true;
    app.adopted = true;
    m_registry->upsert(app);
    return m_registry->save(error);
}

} // namespace GoshAim
