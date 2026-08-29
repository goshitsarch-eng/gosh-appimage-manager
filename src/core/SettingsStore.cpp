#include "SettingsStore.h"

#include <KConfig>
#include <KConfigGroup>
#include <QDir>
#include <QMutexLocker>
#include <QStandardPaths>

namespace GoshAim {

SettingsStore::SettingsStore(QObject *parent, const QString &configPath)
    : QObject(parent)
    , m_configPath(configPath)
{
    m_managedFolder = defaultManagedFolder();
    load();
}

SettingsStore::~SettingsStore()
{
    save();
}

QString SettingsStore::defaultManagedFolder() const
{
    return QDir::homePath() + QStringLiteral("/AppImages");
}

QString SettingsStore::managedFolder() const
{
    QMutexLocker locker(&m_mutex);
    return m_managedFolder.isEmpty() ? defaultManagedFolder() : m_managedFolder;
}

void SettingsStore::setManagedFolder(const QString &path)
{
    QString cleaned = QDir::cleanPath(path);
    if (cleaned.isEmpty()) {
        cleaned = defaultManagedFolder();
    }
    {
        QMutexLocker locker(&m_mutex);
        if (cleaned == m_managedFolder) {
            return;
        }
        m_managedFolder = cleaned;
        saveLocked();
    }
    Q_EMIT changed();
}

bool SettingsStore::moveSource() const
{
    QMutexLocker locker(&m_mutex);
    return m_moveSource;
}

void SettingsStore::setMoveSource(bool value)
{
    {
        QMutexLocker locker(&m_mutex);
        if (m_moveSource == value) {
            return;
        }
        m_moveSource = value;
        saveLocked();
    }
    Q_EMIT changed();
}

bool SettingsStore::manageOutsideFolder() const
{
    QMutexLocker locker(&m_mutex);
    return m_manageOutsideFolder;
}

void SettingsStore::setManageOutsideFolder(bool value)
{
    {
        QMutexLocker locker(&m_mutex);
        if (m_manageOutsideFolder == value) {
            return;
        }
        m_manageOutsideFolder = value;
        saveLocked();
    }
    Q_EMIT changed();
}

bool SettingsStore::terminalOmitSuffix() const
{
    QMutexLocker locker(&m_mutex);
    return m_terminalOmitSuffix;
}

void SettingsStore::setTerminalOmitSuffix(bool value)
{
    {
        QMutexLocker locker(&m_mutex);
        if (m_terminalOmitSuffix == value) {
            return;
        }
        m_terminalOmitSuffix = value;
        saveLocked();
    }
    Q_EMIT changed();
}

bool SettingsStore::backgroundUpdateChecks() const
{
    QMutexLocker locker(&m_mutex);
    return m_backgroundUpdateChecks;
}

void SettingsStore::setBackgroundUpdateChecks(bool value)
{
    {
        QMutexLocker locker(&m_mutex);
        if (m_backgroundUpdateChecks == value) {
            return;
        }
        m_backgroundUpdateChecks = value;
        saveLocked();
    }
    Q_EMIT changed();
}

bool SettingsStore::unsafeExtractionFallback() const
{
    QMutexLocker locker(&m_mutex);
    return m_unsafeExtractionFallback;
}

void SettingsStore::setUnsafeExtractionFallback(bool value)
{
    {
        QMutexLocker locker(&m_mutex);
        if (m_unsafeExtractionFallback == value) {
            return;
        }
        m_unsafeExtractionFallback = value;
        saveLocked();
    }
    Q_EMIT changed();
}

Appearance SettingsStore::appearance() const
{
    QMutexLocker locker(&m_mutex);
    return m_appearance;
}

QString SettingsStore::appearanceName() const
{
    QMutexLocker locker(&m_mutex);
    return GoshAim::appearanceName(m_appearance);
}

void SettingsStore::setAppearance(Appearance appearance)
{
    {
        QMutexLocker locker(&m_mutex);
        if (m_appearance == appearance) {
            return;
        }
        m_appearance = appearance;
        saveLocked();
    }
    Q_EMIT changed();
}

void SettingsStore::setAppearanceName(const QString &name)
{
    setAppearance(appearanceFromString(name));
}

void SettingsStore::setDebugLogging(bool value)
{
    {
        QMutexLocker locker(&m_mutex);
        if (m_debugLogging == value) {
            return;
        }
        m_debugLogging = value;
        saveLocked();
    }
    Q_EMIT changed();
}

bool SettingsStore::debugLogging() const
{
    QMutexLocker locker(&m_mutex);
    return m_debugLogging;
}

void SettingsStore::setMaxAppImageBytes(qint64 bytes)
{
    const qint64 clamped = qBound(kMinMaxAppImageBytes, bytes, kAbsoluteMaxAppImageBytes);
    {
        QMutexLocker locker(&m_mutex);
        if (m_maxAppImageBytes == clamped) {
            return;
        }
        m_maxAppImageBytes = clamped;
        saveLocked();
    }
    Q_EMIT changed();
}

qint64 SettingsStore::maxAppImageBytes() const
{
    QMutexLocker locker(&m_mutex);
    return m_maxAppImageBytes;
}

QString SettingsStore::applicationsDir() const
{
    return QStandardPaths::writableLocation(QStandardPaths::ApplicationsLocation);
}

QString SettingsStore::iconsDir() const
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation) + QStringLiteral("/icons/hicolor");
}

QString SettingsStore::dataDir() const
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation) + QStringLiteral("/gosh-appimage-manager");
}

QString SettingsStore::cacheDir() const
{
    return QStandardPaths::writableLocation(QStandardPaths::CacheLocation);
}

QString SettingsStore::registryPath() const
{
    return dataDir() + QStringLiteral("/registry.json");
}

void SettingsStore::reload()
{
    {
        QMutexLocker locker(&m_mutex);
        load();
    }
    Q_EMIT changed();
}

void SettingsStore::load()
{
    KConfig config(m_configPath.isEmpty() ? QStringLiteral("gosh-appimagemanager") : m_configPath,
                   m_configPath.isEmpty() ? KConfig::SimpleConfig : KConfig::SimpleConfig);
    KConfigGroup group(&config, QStringLiteral("General"));
    m_managedFolder = group.readEntry("ManagedFolder", defaultManagedFolder());
    m_moveSource = group.readEntry("MoveSource", false);
    m_manageOutsideFolder = group.readEntry("ManageOutsideFolder", false);
    m_terminalOmitSuffix = group.readEntry("TerminalOmitSuffix", false);
    m_backgroundUpdateChecks = group.readEntry("BackgroundUpdateChecks", false);
    m_unsafeExtractionFallback = group.readEntry("UnsafeExtractionFallback", false);
    m_appearance = appearanceFromString(group.readEntry("Appearance", QStringLiteral("system")));
    m_debugLogging = group.readEntry("DebugLogging", false);
    m_maxAppImageBytes = group.readEntry("MaxAppImageBytes", kDefaultMaxAppImageBytes);
    m_maxAppImageBytes = qBound(kMinMaxAppImageBytes, m_maxAppImageBytes, kAbsoluteMaxAppImageBytes);
}

void SettingsStore::save()
{
    QMutexLocker locker(&m_mutex);
    saveLocked();
}

void SettingsStore::saveLocked()
{
    KConfig config(m_configPath.isEmpty() ? QStringLiteral("gosh-appimagemanager") : m_configPath,
                   KConfig::SimpleConfig);
    KConfigGroup group(&config, QStringLiteral("General"));
    group.writeEntry("ManagedFolder", m_managedFolder);
    group.writeEntry("MoveSource", m_moveSource);
    group.writeEntry("ManageOutsideFolder", m_manageOutsideFolder);
    group.writeEntry("TerminalOmitSuffix", m_terminalOmitSuffix);
    group.writeEntry("BackgroundUpdateChecks", m_backgroundUpdateChecks);
    group.writeEntry("UnsafeExtractionFallback", m_unsafeExtractionFallback);
    group.writeEntry("Appearance", GoshAim::appearanceName(m_appearance));
    group.writeEntry("DebugLogging", m_debugLogging);
    group.writeEntry("MaxAppImageBytes", m_maxAppImageBytes);
    config.sync();
}

} // namespace GoshAim
