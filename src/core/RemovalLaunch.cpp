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

bool RemovalService::remove(const RemovalRequest &request, QString *error, std::atomic<bool> *cancel)
{
    Q_UNUSED(cancel);
    const InstalledApp app = resolve(request.pathOrUuid, error);
    if (app.uuid.isEmpty()) {
        return false;
    }
    QString pathError;
    const QString canonical = SafeFs::canonicalExisting(app.managedPath, &pathError);
    if (canonical.isEmpty()) {
        if (error) {
            *error = pathError;
        }
        return false;
    }
    if (request.mode == RemovalMode::Permanent) {
        if (SafeFs::isForbiddenPermanentTarget(canonical)) {
            if (error) {
                *error = QStringLiteral("Refusing to permanently delete a protected path");
            }
            return false;
        }
        QFileInfo info(app.managedPath);
        if (info.isSymLink()) {
            if (error) {
                *error = QStringLiteral("Refusing to follow a symlink for permanent deletion");
            }
            return false;
        }
        if (!SafeFs::removeFileNoFollow(app.managedPath, error)) {
            return false;
        }
    } else {
        if (!trashFile(app.managedPath, error)) {
            return false;
        }
    }
    if (!m_desktop->removeOwnedArtifacts(app, error)) {
        return false;
    }
    m_registry->removeUuid(app.uuid);
    return m_registry->save(error);
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
