#pragma once

#include "Types.h"

#include <QMutex>
#include <QObject>
#include <QString>

class KConfig;
class KConfigGroup;

namespace GoshAim {

class SettingsStore : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QString managedFolder READ managedFolder WRITE setManagedFolder NOTIFY changed)
    Q_PROPERTY(bool moveSource READ moveSource WRITE setMoveSource NOTIFY changed)
    Q_PROPERTY(bool manageOutsideFolder READ manageOutsideFolder WRITE setManageOutsideFolder NOTIFY changed)
    Q_PROPERTY(bool terminalOmitSuffix READ terminalOmitSuffix WRITE setTerminalOmitSuffix NOTIFY changed)
    Q_PROPERTY(bool backgroundUpdateChecks READ backgroundUpdateChecks WRITE setBackgroundUpdateChecks NOTIFY changed)
    Q_PROPERTY(bool unsafeExtractionFallback READ unsafeExtractionFallback WRITE setUnsafeExtractionFallback NOTIFY changed)
    Q_PROPERTY(QString appearance READ appearanceName WRITE setAppearanceName NOTIFY changed)
    Q_PROPERTY(bool debugLogging READ debugLogging WRITE setDebugLogging NOTIFY changed)
    Q_PROPERTY(qint64 maxAppImageBytes READ maxAppImageBytes WRITE setMaxAppImageBytes NOTIFY changed)

public:
    explicit SettingsStore(QObject *parent = nullptr, const QString &configPath = {});
    ~SettingsStore() override;

    QString managedFolder() const;
    void setManagedFolder(const QString &path);
    bool moveSource() const;
    void setMoveSource(bool value);
    bool manageOutsideFolder() const;
    void setManageOutsideFolder(bool value);
    bool terminalOmitSuffix() const;
    void setTerminalOmitSuffix(bool value);
    bool backgroundUpdateChecks() const;
    void setBackgroundUpdateChecks(bool value);
    bool unsafeExtractionFallback() const;
    void setUnsafeExtractionFallback(bool value);
    Appearance appearance() const;
    QString appearanceName() const;
    void setAppearance(Appearance appearance);
    void setAppearanceName(const QString &name);
    bool debugLogging() const;
    void setDebugLogging(bool value);
    qint64 maxAppImageBytes() const;
    void setMaxAppImageBytes(qint64 bytes);

    QString applicationsDir() const;
    QString iconsDir() const;
    QString dataDir() const;
    QString cacheDir() const;
    QString registryPath() const;
    QString defaultManagedFolder() const;

    void reload();
    void save();

Q_SIGNALS:
    void changed();

private:
    void load();
    void saveLocked();
    mutable QMutex m_mutex;
    QString m_configPath;
    QString m_managedFolder;
    bool m_moveSource = false;
    bool m_manageOutsideFolder = false;
    bool m_terminalOmitSuffix = false;
    bool m_backgroundUpdateChecks = false;
    bool m_unsafeExtractionFallback = false;
    Appearance m_appearance = Appearance::System;
    bool m_debugLogging = false;
    qint64 m_maxAppImageBytes = kDefaultMaxAppImageBytes;
};

} // namespace GoshAim
