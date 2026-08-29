#pragma once

#include "Types.h"

#include <QMutex>
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

    QVector<InstalledApp> apps() const;
    InstalledApp byUuid(const QString &uuid) const;
    InstalledApp byPath(const QString &path) const;
    InstalledApp byDesktopId(const QString &desktopId) const;
    bool containsPath(const QString &path) const;
    bool isOwned(const InstalledApp &app) const;

    void upsert(const InstalledApp &app);
    bool removeUuid(const QString &uuid);
    void restoreApps(const QVector<InstalledApp> &apps);
    QVector<InstalledApp> snapshot() const;
    void setFailSave(bool fail);

    class Transaction
    {
    public:
        explicit Transaction(ManagedRegistry *registry);
        ~Transaction();
        Transaction(const Transaction &) = delete;
        Transaction &operator=(const Transaction &) = delete;

        QVector<InstalledApp> snapshot() const;
        InstalledApp byUuid(const QString &uuid) const;
        void upsert(const InstalledApp &app);
        bool removeUuid(const QString &uuid);
        void restore();
        bool save(QString *error = nullptr);

    private:
        ManagedRegistry *m_registry = nullptr;
        QVector<InstalledApp> m_snapshot;
    };

    static QString newUuid();

private:
    friend class Transaction;
    QVector<InstalledApp> appsUnlocked() const;
    InstalledApp byUuidUnlocked(const QString &uuid) const;
    void upsertUnlocked(const InstalledApp &app);
    bool saveUnlocked(QString *error);

    SettingsStore *m_settings = nullptr;
    QVector<InstalledApp> m_apps;
    bool m_failSave = false;
    mutable QRecursiveMutex m_mutex;
};

} // namespace GoshAim
