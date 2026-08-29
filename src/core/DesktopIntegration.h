#pragma once

#include "DesktopParser.h"
#include "Types.h"

#include <QString>
#include <QVector>

namespace GoshAim {

class SettingsStore;
class ProcessRunner;

class DesktopIntegration
{
public:
    DesktopIntegration(SettingsStore *settings, ProcessRunner *runner);

    QString desktopFileName(const QString &uuid) const;
    QString desktopPath(const QString &uuid) const;
    QString iconPathFor(const QString &uuid, const QString &sourceIcon) const;

    QByteArray buildDesktopFile(const InstalledApp &app, const QVector<DesktopAction> &actions = {}) const;
    bool writeStaged(const InstalledApp &app, const QString &stagedDesktop, const QString &stagedIcon, QString *error);
    bool install(const InstalledApp &app, const QString &stagedDesktop, const QString &stagedIcon, QString *error);
    bool removeOwnedArtifacts(const InstalledApp &app, QString *error);
    bool refreshDatabase();
    bool hasOwnershipMarkers(const QString &desktopPath, const QString &uuid) const;
    InstalledApp parseExternalDesktop(const QString &desktopPath) const;

private:
    SettingsStore *m_settings = nullptr;
    ProcessRunner *m_runner = nullptr;
};

} // namespace GoshAim
