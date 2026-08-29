#include "RemovalLaunch.h"

#include "DesktopIntegration.h"
#include "ManagedRegistry.h"
#include "ProcessRunner.h"
#include "ProcessTable.h"
#include "SafeFs.h"
#include "SettingsStore.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QStandardPaths>

namespace GoshAim {

RemovalService::RemovalService(SettingsStore *settings,
                               ManagedRegistry *registry,
                               DesktopIntegration *desktop,
                               ProcessTable *processes,
                               ProcessRunner *runner)
    : m_settings(settings)
    , m_registry(registry)
    , m_desktop(desktop)
    , m_processes(processes)
    , m_runner(runner)
{
}

InstalledApp RemovalService::resolve(const QString &pathOrUuid, QString *error) const
{
    InstalledApp app = m_registry->byUuid(pathOrUuid);
    if (app.uuid.isEmpty()) {
        app = m_registry->byPath(pathOrUuid);
    }
    if (app.uuid.isEmpty() || !app.owned) {
        if (error) {
            *error = QStringLiteral("Not an owned managed AppImage");
        }
        return {};
    }
    return app;
}

bool RemovalService::trashFile(const QString &path, QString *error)
{
    if (m_trashHook) {
        return m_trashHook(path, error);
    }
    if (QFile::moveToTrash(path)) {
        return true;
    }
    if (m_runner) {
        ProcessRequest req;
        req.program = QStringLiteral("gio");
        req.arguments = QStringList{QStringLiteral("trash"), path};
        req.host = true;
        req.timeoutMs = 10000;
        const ProcessResult result = m_runner->run(req);
        if (result.exitCode == 0 && !result.failedToStart && !result.refused) {
            return true;
        }
    }
    if (error) {
        *error = QStringLiteral("Trash failed; leaving files intact");
    }
    return false;
}

RemovalResult RemovalService::remove(const RemovalRequest &request, std::atomic<bool> *cancel)
{
    Q_UNUSED(cancel);
    RemovalResult result;
    QString resolveError;
    const InstalledApp app = resolve(request.pathOrUuid, &resolveError);
    if (app.uuid.isEmpty()) {
        result.error = resolveError;
        return result;
    }
    m_lastMode = request.mode;
    m_lastTargets.append(app.managedPath);
    if (!app.desktopPath.isEmpty() && QFile::exists(app.desktopPath)) {
        if (!m_desktop->hasOwnershipMarkers(app.desktopPath, app.uuid)) {
            result.error = QStringLiteral("Desktop file is missing ownership markers");
            return result;
        }
    }
    if (!app.iconPath.isEmpty() && QFile::exists(app.iconPath) && !app.iconPath.contains(app.uuid)) {
        result.error = QStringLiteral("Icon path is not owned by this installation");
        return result;
    }
    QString pathError;
    QString canonical = SafeFs::canonicalExisting(app.managedPath, &pathError);
    const bool missing = canonical.isEmpty();
    if (!missing && request.mode == RemovalMode::Permanent) {
        if (SafeFs::isForbiddenPermanentTarget(canonical)) {
            result.error = QStringLiteral("Refusing to permanently delete a protected path");
            return result;
        }
        QFileInfo info(app.managedPath);
        if (info.isSymLink()) {
            result.error = QStringLiteral("Refusing to follow a symlink for permanent deletion");
            return result;
        }
        if (!SafeFs::removeFileNoFollow(app.managedPath, &result.error)) {
            return result;
        }
    } else if (!missing) {
        if (!trashFile(app.managedPath, &result.error)) {
            return result;
        }
    }

    QString artifactError;
    bool artifactsOk = true;
    if (!app.desktopPath.isEmpty() && QFile::exists(app.desktopPath)) {
        artifactsOk = m_desktop->removeOwnedArtifacts(app, &artifactError);
    } else if (!app.iconPath.isEmpty() && QFile::exists(app.iconPath) && app.iconPath.contains(app.uuid)) {
        artifactsOk = m_desktop->removeOwnedArtifacts(app, &artifactError);
    } else if (!app.desktopPath.isEmpty() || !app.iconPath.isEmpty()) {
        if (!app.iconPath.isEmpty() && app.iconPath.contains(app.uuid) && QFile::exists(app.iconPath)) {
            SafeFs::removeFileNoFollow(app.iconPath);
        }
    }
    m_registry->removeUuid(app.uuid);
    QString saveError;
    const bool saved = m_registry->save(&saveError);
    if (!artifactsOk || !saved) {
        result.partial = true;
        result.error = artifactsOk ? saveError : artifactError;
        if (result.error.isEmpty()) {
            result.error = QStringLiteral("AppImage removed but owned artifacts or registry could not be fully cleaned");
        }
        return result;
    }
    result.ok = true;
    if (missing) {
        result.error = QStringLiteral("already gone");
    }
    return result;
}

bool RemovalService::remove(const RemovalRequest &request, QString *error, std::atomic<bool> *cancel)
{
    const RemovalResult result = remove(request, cancel);
    if (error) {
        *error = result.error;
    }
    return result.ok;
}

LaunchService::LaunchService(ProcessRunner *runner, ProcessTable *processes)
    : m_runner(runner)
    , m_processes(processes)
{
}

bool LaunchService::nixNeedsAppimageRun() const
{
    return QFile::exists(QStringLiteral("/etc/NIXOS"));
}

bool LaunchService::isRunning(const InstalledApp &app) const
{
    if (!m_processes) {
        return false;
    }
    const QString canonical = SafeFs::canonicalExisting(app.managedPath);
    return !m_processes->pidsForExecutable(canonical).isEmpty();
}

bool LaunchService::launch(const InstalledApp &app, QString *error)
{
    if (!m_runner) {
        if (error) {
            *error = QStringLiteral("No process runner");
        }
        return false;
    }
    if (app.managedPath.isEmpty()) {
        if (error) {
            *error = QStringLiteral("Missing managed path");
        }
        return false;
    }
    ProcessRequest req;
    if (nixNeedsAppimageRun()) {
        req.program = QStringLiteral("appimage-run");
        req.arguments = QStringList{app.managedPath} + app.arguments;
        ProcessRequest which;
        which.program = QStringLiteral("appimage-run");
        which.arguments = QStringList{QStringLiteral("--version")};
        which.host = true;
        which.timeoutMs = 3000;
        const ProcessResult probe = m_runner->run(which);
        if (probe.failedToStart || probe.refused) {
            if (error) {
                *error = QStringLiteral("appimage-run is required on NixOS but was not found");
            }
            return false;
        }
    } else {
        req.program = app.managedPath;
        req.arguments = app.arguments;
    }
    QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    bool customEnv = false;
    for (const EnvPair &pair : app.environment) {
        if (DesktopParser::isValidEnvName(pair.name) && !DesktopParser::isDangerousEnvName(pair.name)
            && DesktopParser::isValidEnvValue(pair.value)) {
            env.insert(pair.name, pair.value);
            customEnv = true;
        }
    }
    req.useEnvironment = customEnv;
    req.environment = env;
    req.host = true;
    const ProcessResult started = m_runner->startDetached(req);
    if (started.failedToStart || started.refused || started.exitCode != 0) {
        if (error) {
            *error = started.error.isEmpty() ? QStringLiteral("Failed to start") : started.error;
        }
        return false;
    }
    return true;
}

} // namespace GoshAim
