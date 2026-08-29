#pragma once

#include "Types.h"

#include <QHash>
#include <QString>
#include <QVector>

namespace GoshAim {

class SettingsStore;

class ManagedRegistry
{
public:
    explicit ManagedRegistry(SettingsStore *settings);
    virtual ~ManagedRegistry() = default;

    bool load(QString *error = nullptr);
    virtual bool save(QString *error = nullptr);

    QVector<InstalledApp> apps() const { return m_apps; }
    InstalledApp byUuid(const QString &uuid) const;
    InstalledApp byPath(const QString &path) const;
    InstalledApp byDesktopId(const QString &desktopId) const;
    bool containsPath(const QString &path) const;
    bool isOwned(const InstalledApp &app) const;

    void upsert(const InstalledApp &app);
    bool removeUuid(const QString &uuid);
    void restoreApps(const QVector<InstalledApp> &apps);
    QVector<InstalledApp> snapshot() const { return m_apps; }

    static QString newUuid();

private:
    SettingsStore *m_settings = nullptr;
    QVector<InstalledApp> m_apps;
};

} // namespace GoshAim
