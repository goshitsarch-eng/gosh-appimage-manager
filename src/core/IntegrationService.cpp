#include "IntegrationService.h"

#include "AppImageInspector.h"
#include "DesktopIntegration.h"
#include "DesktopParser.h"
#include "ManagedRegistry.h"
#include "ProcessRunner.h"
#include "SafeFs.h"
#include "SettingsStore.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QUuid>

namespace GoshAim {

IntegrationService::IntegrationService(SettingsStore *settings,
                                       ManagedRegistry *registry,
                                       AppImageInspector *inspector,
                                       DesktopIntegration *desktop,
                                       ProcessRunner *runner)
    : m_settings(settings)
    , m_registry(registry)
    , m_inspector(inspector)
    , m_desktop(desktop)
    , m_runner(runner)
{
}

QString IntegrationService::preferredDestinationName(const InspectionResult &inspection, bool omitSuffix) const
{
    QString base = DesktopParser::sanitizeFileBase(inspection.metadata.name);
    if (base.isEmpty()) {
        base = DesktopParser::sanitizeFileBase(QFileInfo(inspection.identity.path).completeBaseName());
    }
    if (base.isEmpty()) {
        base = QStringLiteral("AppImage");
    }
    const bool suffix = !(omitSuffix && inspection.metadata.terminal);
    return suffix ? base + QStringLiteral(".AppImage") : base;
}

QString IntegrationService::chooseDestinationName(const InspectionResult &inspection, bool omitSuffix) const
{
    const QString ext = (omitSuffix && inspection.metadata.terminal) ? QString() : QStringLiteral(".AppImage");
    QString base = DesktopParser::sanitizeFileBase(inspection.metadata.name);
    if (base.isEmpty()) {
        base = DesktopParser::sanitizeFileBase(QFileInfo(inspection.identity.path).completeBaseName());
    }
    if (base.isEmpty()) {
        base = QStringLiteral("AppImage");
    }
    QString candidate = base + ext;
    const QString dir = m_settings->managedFolder();
    int n = 2;
    while (QFileInfo::exists(dir + QLatin1Char('/') + candidate)) {
        candidate = base + QLatin1Char('-') + QString::number(n++) + ext;
        if (n > 1000) {
            candidate = base + QLatin1Char('-') + QUuid::createUuid().toString(QUuid::WithoutBraces).left(8) + ext;
            break;
        }
    }
    return candidate;
}

void IntegrationService::annotatePlan(InspectionResult *inspection, CopyMode copyMode) const
{
    if (!inspection) {
        return;
    }
    inspection->copyOutcome = copyMode == CopyMode::Move ? QStringLiteral("move") : QStringLiteral("copy");
    const QString destName = preferredDestinationName(*inspection, m_settings && m_settings->terminalOmitSuffix());
    const QString destDir = m_settings ? m_settings->managedFolder() : QString();
    inspection->plannedTarget = destDir + QLatin1Char('/') + destName;
    inspection->conflictStatus = QStringLiteral("none");
    inspection->needsConflictDecision = false;
    if (inspection->alreadyManaged && !inspection->existingManagedId.isEmpty()) {
        const InstalledApp existing = m_registry->byUuid(inspection->existingManagedId);
        inspection->conflictingUuid = existing.uuid;
        inspection->conflictingPath = existing.managedPath;
        inspection->conflictingName = existing.name;
        inspection->conflictStatus = QStringLiteral("already-managed");
        inspection->needsConflictDecision = existing.owned;
        if (existing.owned) {
            inspection->plannedTarget = existing.managedPath;
        }
        return;
    }
    if (QFileInfo::exists(inspection->plannedTarget)) {
        const InstalledApp byPath = m_registry->byPath(inspection->plannedTarget);
        inspection->conflictingUuid = byPath.uuid;
        inspection->conflictingPath = inspection->plannedTarget;
        inspection->conflictingName = byPath.name.isEmpty() ? QFileInfo(inspection->plannedTarget).fileName() : byPath.name;
        inspection->conflictStatus = byPath.owned ? QStringLiteral("owned-dest") : QStringLiteral("dest-exists");
        inspection->needsConflictDecision = byPath.owned;
    }
}

void IntegrationService::rollbackTemps(const QStringList &created)
{
    for (int i = created.size() - 1; i >= 0; --i) {
        SafeFs::removeFileNoFollow(created.at(i));
    }
}

IntegrateResult IntegrationService::integrate(const IntegrateRequest &request, std::atomic<bool> *cancel)
{
    IntegrateResult result;
    InspectOptions options;
    options.maxBytes = m_settings->maxAppImageBytes();
    options.allowUnsafeExtract = false;
    options.confirmUnsafeExtract = false;
    const InspectionResult inspection = m_inspector->inspect(request.sourcePath, options, cancel);
    if (!inspection.error.isEmpty() || !inspection.magicValid) {
        result.error = inspection.error.isEmpty() ? QStringLiteral("Not a valid AppImage") : inspection.error;
        return result;
    }
    if (!inspection.architectureSupported) {
        result.error = QStringLiteral("Unsupported architecture");
        return result;
    }

    const QString destDir = m_settings->managedFolder();
    QString mkdirError;
    if (!SafeFs::mkdir0700(destDir, &mkdirError)) {
        result.error = mkdirError;
        return result;
    }

    InstalledApp replacing;
    const QString preferredName = preferredDestinationName(inspection, m_settings->terminalOmitSuffix());
    QString destPath = destDir + QLatin1Char('/') + preferredName;
    QString rollbackCopy;
    QString desktopBackup;
    QString iconBackup;
    const bool destExisted = QFileInfo::exists(destPath);

    if (request.conflict == ConflictPolicy::Replace) {
        replacing = m_registry->byUuid(request.replaceUuid);
        if (replacing.uuid.isEmpty() || !replacing.owned) {
            result.error = QStringLiteral("Replace requires a specific owned managed installation");
            return result;
        }
        destPath = replacing.managedPath;
        rollbackCopy = SafeFs::siblingTemp(destPath, QStringLiteral(".gosh-rollback-"));
        QString copyError;
        qint64 copied = 0;
        if (QFile::exists(destPath)
            && !SafeFs::copyBounded(destPath, rollbackCopy, m_settings->maxAppImageBytes(), cancel, &copied, &copyError)) {
            result.error = copyError;
            SafeFs::removeFileNoFollow(rollbackCopy);
            return result;
        }
        if (!QFile::exists(destPath)) {
            rollbackCopy.clear();
        }
    } else if (request.conflict == ConflictPolicy::Unspecified && (destExisted || inspection.alreadyManaged)) {
        result.error = QStringLiteral("Name conflict requires keep-both or replace");
        return result;
    } else {
        destPath = destDir + QLatin1Char('/') + chooseDestinationName(inspection, m_settings->terminalOmitSuffix());
    }

    const QString staging = SafeFs::siblingTemp(destPath, QStringLiteral(".gosh-stage-"));
    QStringList temps;
    temps.append(staging);
    if (!rollbackCopy.isEmpty()) {
        temps.append(rollbackCopy);
    }

    QString copyError;
    qint64 copied = 0;
    if (!SafeFs::copyBounded(inspection.identity.path, staging, m_settings->maxAppImageBytes(), cancel, &copied, &copyError)) {
        result.error = copyError;
        rollbackTemps(temps);
        return result;
    }
    if (!SafeFs::chmodPath(staging, 0755, &copyError)) {
        result.error = copyError;
        rollbackTemps(temps);
        return result;
    }
    const HashResult hash = SafeFs::sha256File(staging, m_settings->maxAppImageBytes(), cancel);
    if (hash.sha256.isEmpty() || (!inspection.identity.sha256.isEmpty() && hash.sha256 != inspection.identity.sha256)) {
        result.error = QStringLiteral("Staged copy hash mismatch");
        rollbackTemps(temps);
        return result;
    }
    if (QFileInfo(staging).size() != inspection.identity.size) {
        result.error = QStringLiteral("Staged copy size mismatch");
        rollbackTemps(temps);
        return result;
    }
    if (m_failPoint == IntegrateFailPoint::AfterStage) {
        result.error = QStringLiteral("Forced staging failure");
        rollbackTemps(temps);
        return result;
    }

    InstalledApp app;
    app.uuid = replacing.uuid.isEmpty() ? ManagedRegistry::newUuid() : replacing.uuid;
    app.name = inspection.metadata.name;
    app.version = inspection.metadata.version;
    app.comment = inspection.metadata.comment;
    app.managedPath = destPath;
    app.desktopId = m_desktop->desktopFileName(app.uuid);
    app.desktopPath = m_desktop->desktopPath(app.uuid);
    app.iconPath = m_desktop->iconPathFor(app.uuid, inspection.metadata.extractedIconPath);
    app.sha256 = hash.sha256;
    app.type = inspection.type;
    app.architecture = inspection.architecture;
    app.size = inspection.identity.size;
    app.arguments = inspection.metadata.execArguments;
    app.defaultArguments = inspection.metadata.execArguments;
    app.actions = inspection.metadata.actions;
    app.embeddedUpdate = inspection.updateInfo.raw;
    if (!inspection.updateInfo.managerHint.isEmpty()) {
        app.updateManager = inspection.updateInfo.managerHint;
        app.updateConfig = inspection.updateInfo.fields;
    }
    if (!replacing.uuid.isEmpty()) {
        app.arguments = replacing.arguments;
        app.environment = replacing.environment;
        app.updateManager = replacing.updateManager.isEmpty() ? app.updateManager : replacing.updateManager;
        app.updateConfig = replacing.updateConfig.isEmpty() ? app.updateConfig : replacing.updateConfig;
        app.actions = replacing.actions.isEmpty() ? app.actions : replacing.actions;
    }
    app.owned = true;
    app.terminal = inspection.metadata.terminal;
    EnvPair desktopIntegration;
    desktopIntegration.name = QStringLiteral("DESKTOPINTEGRATION");
    desktopIntegration.value = QStringLiteral("1");
    bool hasDi = false;
    for (const EnvPair &pair : app.environment) {
        if (pair.name == QLatin1String("DESKTOPINTEGRATION")) {
            hasDi = true;
        }
    }
    if (!hasDi) {
        app.environment.append(desktopIntegration);
    }

    const QString stagedDesktop = SafeFs::siblingTemp(app.desktopPath, QStringLiteral(".gosh-desk-"));
    QString stagedIcon;
    temps.append(stagedDesktop);
    if (!inspection.metadata.extractedIconPath.isEmpty() && QFile::exists(inspection.metadata.extractedIconPath)) {
        stagedIcon = SafeFs::siblingTemp(app.iconPath, QStringLiteral(".gosh-icon-"));
        qint64 iconCopied = 0;
        if (SafeFs::copyBounded(inspection.metadata.extractedIconPath, stagedIcon, kMaxIconBytes, cancel, &iconCopied, &copyError)) {
            temps.append(stagedIcon);
            app.iconPath = m_desktop->iconPathFor(app.uuid, inspection.metadata.extractedIconPath);
        } else {
            stagedIcon.clear();
            app.iconPath.clear();
        }
    } else {
        app.iconPath.clear();
    }
    if (m_failPoint == IntegrateFailPoint::DesktopWrite
        || !m_desktop->writeStaged(app, stagedDesktop, stagedIcon, &copyError)) {
        result.error = m_failPoint == IntegrateFailPoint::DesktopWrite ? QStringLiteral("Forced desktop write failure") : copyError;
        rollbackTemps(temps);
        return result;
    }

    if (QFile::exists(app.desktopPath)) {
        desktopBackup = SafeFs::siblingTemp(app.desktopPath, QStringLiteral(".gosh-desk-bak-"));
        qint64 n = 0;
        if (SafeFs::copyBounded(app.desktopPath, desktopBackup, kMaxDesktopFileBytes, cancel, &n, &copyError)) {
            temps.append(desktopBackup);
        } else {
            desktopBackup.clear();
        }
    }
    if (!app.iconPath.isEmpty() && QFile::exists(app.iconPath)) {
        iconBackup = SafeFs::siblingTemp(app.iconPath, QStringLiteral(".gosh-icon-bak-"));
        qint64 n = 0;
        if (SafeFs::copyBounded(app.iconPath, iconBackup, kMaxIconBytes, cancel, &n, &copyError)) {
            temps.append(iconBackup);
        } else {
            iconBackup.clear();
        }
    }

    const QVector<InstalledApp> registrySnap = m_registry->snapshot();
    const bool destWasLive = QFile::exists(destPath);

    if (!SafeFs::renameOver(staging, destPath, &copyError)) {
        result.error = copyError;
        rollbackTemps(temps);
        return result;
    }
    temps.removeAll(staging);

    auto restoreLive = [&]() {
        if (!rollbackCopy.isEmpty() && QFile::exists(rollbackCopy)) {
            SafeFs::renameOver(rollbackCopy, destPath);
        } else if (!destWasLive) {
            SafeFs::removeFileNoFollow(destPath);
        }
        if (!desktopBackup.isEmpty() && QFile::exists(desktopBackup)) {
            SafeFs::renameOver(desktopBackup, app.desktopPath);
        }
        if (!iconBackup.isEmpty() && QFile::exists(iconBackup)) {
            SafeFs::renameOver(iconBackup, replacing.iconPath.isEmpty() ? app.iconPath : replacing.iconPath);
        }
        m_registry->restoreApps(registrySnap);
    };

    if (m_failPoint == IntegrateFailPoint::DesktopInstall
        || !m_desktop->install(app, stagedDesktop, stagedIcon, &copyError)) {
        result.error = m_failPoint == IntegrateFailPoint::DesktopInstall
            ? QStringLiteral("Forced desktop install failure")
            : (copyError.isEmpty() ? QStringLiteral("Desktop integration failed") : copyError);
        restoreLive();
        rollbackTemps(temps);
        return result;
    }
    temps.removeAll(stagedDesktop);
    temps.removeAll(stagedIcon);

    m_registry->upsert(app);
    if (m_failPoint == IntegrateFailPoint::RegistrySave || !m_registry->save(&copyError)) {
        result.error = m_failPoint == IntegrateFailPoint::RegistrySave ? QStringLiteral("Forced registry save failure") : copyError;
        restoreLive();
        if (!rollbackCopy.isEmpty() && !QFile::exists(destPath) && QFile::exists(rollbackCopy)) {
            SafeFs::renameOver(rollbackCopy, destPath);
        }
        rollbackTemps(temps);
        return result;
    }

    rollbackTemps(temps);

    if (request.copyMode == CopyMode::Move) {
        const QString sourceCanonical = SafeFs::canonicalExisting(request.sourcePath);
        if (sourceCanonical != SafeFs::canonicalExisting(destPath)) {
            QString moveError;
            const bool deleted = m_failPoint != IntegrateFailPoint::SourceDelete
                && SafeFs::removeFileNoFollow(request.sourcePath, &moveError);
            if (!deleted) {
                result.ok = false;
                result.partial = true;
                result.app = app;
                result.error = m_failPoint == IntegrateFailPoint::SourceDelete
                    ? QStringLiteral("Source deletion failed")
                    : (moveError.isEmpty() ? QStringLiteral("Source deletion failed") : moveError);
                return result;
            }
            result.sourceRemoved = true;
        }
    }

    result.ok = true;
    result.app = app;
    return result;
}

} // namespace GoshAim
