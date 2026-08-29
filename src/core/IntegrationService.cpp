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

QString IntegrationService::chooseDestinationName(const InspectionResult &inspection, bool omitSuffix) const
{
    QString base = DesktopParser::sanitizeFileBase(inspection.metadata.name);
    if (base.isEmpty()) {
        base = DesktopParser::sanitizeFileBase(QFileInfo(inspection.identity.path).completeBaseName());
    }
    const bool suffix = !(omitSuffix && inspection.metadata.terminal);
    const QString ext = suffix ? QStringLiteral(".AppImage") : QString();
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

void IntegrationService::rollback(const QStringList &created)
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
    QString destName = chooseDestinationName(inspection, m_settings->terminalOmitSuffix());
    QString destPath = destDir + QLatin1Char('/') + destName;
    QString rollbackCopy;

    if (request.conflict == ConflictPolicy::Replace) {
        replacing = m_registry->byUuid(request.replaceUuid);
        if (replacing.uuid.isEmpty() || !replacing.owned) {
            result.error = QStringLiteral("Replace requires a specific owned managed installation");
            return result;
        }
        destPath = replacing.managedPath;
        destName = QFileInfo(destPath).fileName();
        rollbackCopy = destPath + QStringLiteral(".gosh-rollback");
        QString copyError;
        qint64 copied = 0;
        if (QFile::exists(destPath)
            && !SafeFs::copyBounded(destPath, rollbackCopy, m_settings->maxAppImageBytes(), cancel, &copied, &copyError)) {
            result.error = copyError;
            return result;
        }
    } else if (request.conflict == ConflictPolicy::Unspecified && QFileInfo::exists(destPath)) {
        result.error = QStringLiteral("Name conflict requires keep-both or replace");
        return result;
    }

    const QString staging = SafeFs::siblingTemp(destPath, QStringLiteral(".gosh-stage-"));
    QStringList created;
    created.append(staging);
    if (!rollbackCopy.isEmpty()) {
        created.append(rollbackCopy);
    }

    QString copyError;
    qint64 copied = 0;
    if (!SafeFs::copyBounded(inspection.identity.path, staging, m_settings->maxAppImageBytes(), cancel, &copied, &copyError)) {
        result.error = copyError;
        rollback(created);
        return result;
    }
    if (!SafeFs::chmodPath(staging, 0755, &copyError)) {
        result.error = copyError;
        rollback(created);
        return result;
    }
    const HashResult hash = SafeFs::sha256File(staging, m_settings->maxAppImageBytes(), cancel);
    if (hash.sha256.isEmpty() || (!inspection.identity.sha256.isEmpty() && hash.sha256 != inspection.identity.sha256)) {
        result.error = QStringLiteral("Staged copy hash mismatch");
        rollback(created);
        return result;
    }
    if (QFileInfo(staging).size() != inspection.identity.size) {
        result.error = QStringLiteral("Staged copy size mismatch");
        rollback(created);
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
    created.append(stagedDesktop);
    if (!inspection.metadata.extractedIconPath.isEmpty() && QFile::exists(inspection.metadata.extractedIconPath)) {
        stagedIcon = SafeFs::siblingTemp(app.iconPath, QStringLiteral(".gosh-icon-"));
        qint64 iconCopied = 0;
        if (SafeFs::copyBounded(inspection.metadata.extractedIconPath, stagedIcon, kMaxIconBytes, cancel, &iconCopied, &copyError)) {
            created.append(stagedIcon);
            app.iconPath = m_desktop->iconPathFor(app.uuid, inspection.metadata.extractedIconPath);
        } else {
            stagedIcon.clear();
            app.iconPath.clear();
        }
    } else {
        app.iconPath.clear();
    }
    if (!m_desktop->writeStaged(app, stagedDesktop, stagedIcon, &copyError)) {
        result.error = copyError;
        rollback(created);
        return result;
    }

    if (!SafeFs::renameOver(staging, destPath, &copyError)) {
        result.error = copyError;
        rollback(created);
        return result;
    }
    created.removeAll(staging);
    created.append(destPath);

    if (!m_desktop->install(app, stagedDesktop, stagedIcon, &copyError)) {
        result.error = copyError;
        if (!rollbackCopy.isEmpty() && QFile::exists(rollbackCopy)) {
            SafeFs::renameOver(rollbackCopy, destPath);
        } else {
            SafeFs::removeFileNoFollow(destPath);
        }
        rollback(created);
        return result;
    }
    created.removeAll(stagedDesktop);
    created.removeAll(stagedIcon);

    m_registry->upsert(app);
    if (!m_registry->save(&copyError)) {
        result.error = copyError;
        if (!rollbackCopy.isEmpty() && QFile::exists(rollbackCopy)) {
            SafeFs::renameOver(rollbackCopy, destPath);
        }
        m_desktop->removeOwnedArtifacts(app, nullptr);
        rollback(created);
        return result;
    }

    if (!rollbackCopy.isEmpty()) {
        SafeFs::removeFileNoFollow(rollbackCopy);
    }

    if (request.copyMode == CopyMode::Move) {
        const QString sourceCanonical = SafeFs::canonicalExisting(request.sourcePath);
        if (sourceCanonical != SafeFs::canonicalExisting(destPath)) {
            SafeFs::removeFileNoFollow(request.sourcePath);
        }
    }

    result.ok = true;
    result.app = app;
    return result;
}

} // namespace GoshAim
