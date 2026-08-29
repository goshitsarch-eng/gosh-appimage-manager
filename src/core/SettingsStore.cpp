#include "SettingsStore.h"

#include <KConfig>
#include <KConfigGroup>
#include <QDir>
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
    return m_managedFolder.isEmpty() ? defaultManagedFolder() : m_managedFolder;
}

void SettingsStore::setManagedFolder(const QString &path)
{
    QString cleaned = QDir::cleanPath(path);
    if (cleaned.isEmpty()) {
        cleaned = defaultManagedFolder();
    }
    if (cleaned == m_managedFolder) {
        return;
    }
    m_managedFolder = cleaned;
    save();
    Q_EMIT changed();
}

void SettingsStore::setMoveSource(bool value)
{
    if (m_moveSource == value) {
        return;
    }
    m_moveSource = value;
    save();
    Q_EMIT changed();
}

void SettingsStore::setManageOutsideFolder(bool value)
{
    if (m_manageOutsideFolder == value) {
        return;
    }
    m_manageOutsideFolder = value;
    save();
    Q_EMIT changed();
}

void SettingsStore::setTerminalOmitSuffix(bool value)
{
    if (m_terminalOmitSuffix == value) {
        return;
    }
    m_terminalOmitSuffix = value;
    save();
    Q_EMIT changed();
}

void SettingsStore::setBackgroundUpdateChecks(bool value)
{
    if (m_backgroundUpdateChecks == value) {
        return;
    }
    m_backgroundUpdateChecks = value;
    save();
    Q_EMIT changed();
}

void SettingsStore::setUnsafeExtractionFallback(bool value)
{
    if (m_unsafeExtractionFallback == value) {
        return;
    }
    m_unsafeExtractionFallback = value;
    save();
    Q_EMIT changed();
}

QString SettingsStore::appearanceName() const
{
    return GoshAim::appearanceName(m_appearance);
}

void SettingsStore::setAppearance(Appearance appearance)
{
    if (m_appearance == appearance) {
        return;
    }
    m_appearance = appearance;
    save();
    Q_EMIT changed();
}

void SettingsStore::setAppearanceName(const QString &name)
{
    setAppearance(appearanceFromString(name));
}

void SettingsStore::setDebugLogging(bool value)
{
    if (m_debugLogging == value) {
        return;
    }
    m_debugLogging = value;
    save();
    Q_EMIT changed();
}

void SettingsStore::setMaxAppImageBytes(qint64 bytes)
{
    const qint64 clamped = qBound(kMinMaxAppImageBytes, bytes, kAbsoluteMaxAppImageBytes);
    if (m_maxAppImageBytes == clamped) {
        return;
    }
    m_maxAppImageBytes = clamped;
    save();
    Q_EMIT changed();
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
    load();
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
    KConfig config(m_configPath.isEmpty() ? QStringLiteral("gosh-appimagemanager") : m_configPath,
                   KConfig::SimpleConfig);
    KConfigGroup group(&config, QStringLiteral("General"));
    group.writeEntry("ManagedFolder", m_managedFolder);
    group.writeEntry("MoveSource", m_moveSource);
    group.writeEntry("ManageOutsideFolder", m_manageOutsideFolder);
    group.writeEntry("TerminalOmitSuffix", m_terminalOmitSuffix);
    group.writeEntry("BackgroundUpdateChecks", m_backgroundUpdateChecks);
    group.writeEntry("UnsafeExtractionFallback", m_unsafeExtractionFallback);
    group.writeEntry("Appearance", appearanceName());
    group.writeEntry("DebugLogging", m_debugLogging);
    group.writeEntry("MaxAppImageBytes", m_maxAppImageBytes);
    config.sync();
}

} // namespace GoshAim
