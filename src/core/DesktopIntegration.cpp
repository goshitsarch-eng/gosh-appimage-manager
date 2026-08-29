#include "DesktopIntegration.h"

#include "DesktopParser.h"
#include "ProcessRunner.h"
#include "SafeFs.h"
#include "SettingsStore.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QTextStream>

namespace GoshAim {

DesktopIntegration::DesktopIntegration(SettingsStore *settings, ProcessRunner *runner)
    : m_settings(settings)
    , m_runner(runner)
{
}

QString DesktopIntegration::desktopFileName(const QString &uuid) const
{
    return QStringLiteral("%1%2.desktop").arg(QLatin1String(kDesktopPrefix), uuid);
}

QString DesktopIntegration::desktopPath(const QString &uuid) const
{
    return m_settings->applicationsDir() + QLatin1Char('/') + desktopFileName(uuid);
}

QString DesktopIntegration::iconPathFor(const QString &uuid, const QString &sourceIcon) const
{
    QString ext = QFileInfo(sourceIcon).suffix().toLower();
    if (ext != QLatin1String("png") && ext != QLatin1String("svg")) {
        ext = QStringLiteral("png");
    }
    return m_settings->dataDir() + QStringLiteral("/icons/") + uuid + QLatin1Char('.') + ext;
}

QByteArray DesktopIntegration::buildDesktopFile(const InstalledApp &app, const QVector<DesktopAction> &actions) const
{
    QString text;
    QTextStream out(&text);
    out << QStringLiteral("[Desktop Entry]\n");
    out << QStringLiteral("Type=Application\n");
    out << QStringLiteral("Name=") << DesktopParser::escapeDesktopValue(app.name) << QLatin1Char('\n');
    if (!app.comment.isEmpty()) {
        out << QStringLiteral("Comment=") << DesktopParser::escapeDesktopValue(app.comment) << QLatin1Char('\n');
    }
    if (!app.version.isEmpty()) {
        out << QStringLiteral("X-AppImage-Version=") << DesktopParser::escapeDesktopValue(app.version) << QLatin1Char('\n');
    }
    out << QStringLiteral("Exec=") << DesktopParser::buildExecLine(app.managedPath, app.arguments, app.environment) << QLatin1Char('\n');
    out << QStringLiteral("TryExec=") << DesktopParser::escapeExecArg(app.managedPath) << QLatin1Char('\n');
    if (!app.iconPath.isEmpty()) {
        out << QStringLiteral("Icon=") << app.iconPath << QLatin1Char('\n');
    }
    out << QStringLiteral("Terminal=") << (app.terminal ? QStringLiteral("true") : QStringLiteral("false")) << QLatin1Char('\n');
    out << QStringLiteral("Categories=Utility;\n");
    out << QStringLiteral("StartupNotify=true\n");
    out << QLatin1String(kOwnershipKey) << QStringLiteral("=true\n");
    out << QLatin1String(kOwnershipUuidKey) << QLatin1Char('=') << app.uuid << QLatin1Char('\n');
    out << QLatin1String(kOwnershipPathKey) << QLatin1Char('=') << app.managedPath << QLatin1Char('\n');
    QStringList actionIds;
    for (const DesktopAction &action : actions) {
        if (!action.id.isEmpty()) {
            actionIds.append(action.id);
        }
    }
    if (!actionIds.isEmpty()) {
        out << QStringLiteral("Actions=") << actionIds.join(QLatin1Char(';')) << QStringLiteral(";\n");
        for (const DesktopAction &action : actions) {
            out << QStringLiteral("\n[Desktop Action ") << action.id << QStringLiteral("]\n");
            out << QStringLiteral("Name=") << DesktopParser::escapeDesktopValue(action.name) << QLatin1Char('\n');
            out << QStringLiteral("Exec=") << DesktopParser::buildExecLine(app.managedPath, action.arguments, {}) << QLatin1Char('\n');
        }
    }
    return text.toUtf8();
}

bool DesktopIntegration::writeStaged(const InstalledApp &app, const QString &stagedDesktop, const QString &stagedIcon, QString *error)
{
    Q_UNUSED(stagedIcon);
    const QByteArray data = buildDesktopFile(app, app.actions);
    return SafeFs::atomicWrite(stagedDesktop, data, error, 0644);
}

bool DesktopIntegration::install(const InstalledApp &app, const QString &stagedDesktop, const QString &stagedIcon, QString *error)
{
    if (!SafeFs::mkdir0700(QFileInfo(app.desktopPath).absolutePath(), error)) {
        return false;
    }
    if (!stagedIcon.isEmpty() && QFile::exists(stagedIcon)) {
        if (!SafeFs::mkdir0700(QFileInfo(app.iconPath).absolutePath(), error)) {
            return false;
        }
        if (!SafeFs::renameOver(stagedIcon, app.iconPath, error)) {
            return false;
        }
    }
    if (!SafeFs::renameOver(stagedDesktop, app.desktopPath, error)) {
        return false;
    }
    refreshDatabase();
    return true;
}

bool DesktopIntegration::removeOwnedArtifacts(const InstalledApp &app, QString *error)
{
    if (!app.owned) {
        if (error) {
            *error = QStringLiteral("Refusing to remove artifacts that are not owned");
        }
        return false;
    }
    if (!app.desktopPath.isEmpty()) {
        if (!hasOwnershipMarkers(app.desktopPath, app.uuid)) {
            if (error) {
                *error = QStringLiteral("Desktop file is missing ownership markers");
            }
            return false;
        }
        if (!SafeFs::removeFileNoFollow(app.desktopPath, error)) {
            return false;
        }
    }
    if (!app.iconPath.isEmpty() && app.iconPath.contains(app.uuid)) {
        SafeFs::removeFileNoFollow(app.iconPath);
    }
    refreshDatabase();
    return true;
}

bool DesktopIntegration::refreshDatabase()
{
    if (!m_runner || !m_settings) {
        return false;
    }
    ProcessRequest req;
    req.program = QStringLiteral("update-desktop-database");
    req.arguments = QStringList{m_settings->applicationsDir(), QStringLiteral("-q")};
    req.host = true;
    req.timeoutMs = 10000;
    m_runner->run(req);
    return true;
}

bool DesktopIntegration::hasOwnershipMarkers(const QString &desktopPath, const QString &uuid) const
{
    QFile file(desktopPath);
    if (!file.open(QIODevice::ReadOnly)) {
        return false;
    }
    const QByteArray data = file.read(kMaxDesktopFileBytes);
    const QString text = QString::fromUtf8(data);
    return text.contains(QLatin1String(kOwnershipKey) + QStringLiteral("=true"))
        && text.contains(QLatin1String(kOwnershipUuidKey) + QLatin1Char('=') + uuid);
}

InstalledApp DesktopIntegration::parseExternalDesktop(const QString &desktopPath) const
{
    InstalledApp app;
    QFile file(desktopPath);
    if (!file.open(QIODevice::ReadOnly)) {
        return app;
    }
    const ParsedDesktop parsed = DesktopParser::parse(file.read(kMaxDesktopFileBytes));
    if (!parsed.ok) {
        return app;
    }
    app.name = parsed.metadata.name;
    app.comment = parsed.metadata.comment;
    app.version = parsed.metadata.version;
    app.iconPath = parsed.metadata.iconName;
    app.terminal = parsed.metadata.terminal;
    app.arguments = parsed.metadata.execArguments;
    app.desktopPath = desktopPath;
    app.desktopId = QFileInfo(desktopPath).fileName();
    app.managedPath = parsed.metadata.tryExec;
    app.owned = QString::fromUtf8(file.readAll()).contains(QLatin1String(kOwnershipKey));
    QFile rewind(desktopPath);
    if (rewind.open(QIODevice::ReadOnly)) {
        const QString text = QString::fromUtf8(rewind.readAll());
        app.owned = text.contains(QLatin1String(kOwnershipKey) + QStringLiteral("=true"));
        const QString marker = QLatin1String(kOwnershipUuidKey) + QLatin1Char('=');
        const int idx = text.indexOf(marker);
        if (idx >= 0) {
            app.uuid = text.mid(idx + marker.size()).section(QLatin1Char('\n'), 0, 0).trimmed();
        }
    }
    return app;
}

} // namespace GoshAim
