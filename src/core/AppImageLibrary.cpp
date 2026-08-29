#include "AppImageLibrary.h"

#include "DesktopIntegration.h"
#include "ElfParser.h"
#include "ManagedRegistry.h"
#include "SafeFs.h"
#include "SettingsStore.h"

#include <QDir>
#include <QFile>
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
    if (!m_registry || !m_desktop) {
        if (error) {
            *error = QStringLiteral("Adoption services unavailable");
        }
        return false;
    }

    ManagedRegistry::Transaction tx(m_registry);
    InstalledApp app = external;
    const QString foreignDesktop = app.desktopPath;
    const QString foreignIcon = app.iconPath;
    if (app.uuid.startsWith(QLatin1String("external:")) || app.uuid.isEmpty()) {
        app.uuid = ManagedRegistry::newUuid();
    }
    app.owned = true;
    app.adopted = true;
    app.desktopId = m_desktop->desktopFileName(app.uuid);
    app.desktopPath = m_desktop->desktopPath(app.uuid);

    QString stagedIcon;
    QStringList created;
    if (!foreignIcon.isEmpty() && QFile::exists(foreignIcon) && !foreignIcon.contains(app.uuid)) {
        app.iconPath = m_desktop->iconPathFor(app.uuid, foreignIcon);
        stagedIcon = SafeFs::siblingTemp(app.iconPath, QStringLiteral(".gosh-icon-"));
        qint64 copied = 0;
        QString copyError;
        if (!stagedIcon.isEmpty()
            && SafeFs::copyBounded(foreignIcon, stagedIcon, kMaxIconBytes, nullptr, &copied, &copyError)) {
            created.append(stagedIcon);
        } else {
            stagedIcon.clear();
            app.iconPath.clear();
        }
    } else if (foreignIcon.contains(app.uuid)) {
        app.iconPath = foreignIcon;
    } else {
        app.iconPath.clear();
    }

    const QString stagedDesktop = SafeFs::siblingTemp(app.desktopPath, QStringLiteral(".gosh-desk-"));
    if (stagedDesktop.isEmpty()) {
        for (const QString &path : created) {
            SafeFs::removeFileNoFollow(path);
        }
        if (error) {
            *error = QStringLiteral("Cannot create adoption staging file");
        }
        return false;
    }
    created.append(stagedDesktop);

    QString localError;
    const bool desktopExisted = QFile::exists(app.desktopPath);
    const bool iconExisted = !app.iconPath.isEmpty() && QFile::exists(app.iconPath);
    if (!m_desktop->writeStaged(app, stagedDesktop, stagedIcon, &localError)
        || !m_desktop->install(app, stagedDesktop, stagedIcon, &localError)) {
        for (const QString &path : created) {
            SafeFs::removeFileNoFollow(path);
        }
        if (!desktopExisted) {
            SafeFs::removeFileNoFollow(app.desktopPath);
        }
        if (!iconExisted && !app.iconPath.isEmpty() && app.iconPath != foreignIcon) {
            SafeFs::removeFileNoFollow(app.iconPath);
        }
        if (error) {
            *error = localError.isEmpty() ? QStringLiteral("Failed to create Gosh desktop for adoption") : localError;
        }
        return false;
    }
    created.removeAll(stagedDesktop);
    created.removeAll(stagedIcon);

    tx.upsert(app);
    if (!tx.save(&localError)) {
        tx.restore();
        tx.save();
        if (!desktopExisted) {
            SafeFs::removeFileNoFollow(app.desktopPath);
        }
        if (!iconExisted && !app.iconPath.isEmpty() && app.iconPath != foreignIcon) {
            SafeFs::removeFileNoFollow(app.iconPath);
        }
        for (const QString &path : created) {
            SafeFs::removeFileNoFollow(path);
        }
        if (error) {
            *error = localError.isEmpty() ? QStringLiteral("Failed to save adopted app") : localError;
        }
        return false;
    }
    for (const QString &path : created) {
        SafeFs::removeFileNoFollow(path);
    }
    Q_UNUSED(foreignDesktop);
    return true;
}

} // namespace GoshAim
