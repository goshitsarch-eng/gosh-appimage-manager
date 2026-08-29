#include "UpdateService.h"

#include "AppImageInspector.h"
#include "DesktopIntegration.h"
#include "ElfParser.h"
#include "ManagedRegistry.h"
#include "ProcessTable.h"
#include "SafeFs.h"
#include "SettingsStore.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>

namespace GoshAim {

UpdateService::UpdateService(SettingsStore *settings,
                             ManagedRegistry *registry,
                             AppImageInspector *inspector,
                             DesktopIntegration *desktop,
                             NetworkClient *network,
                             ProcessTable *processes,
                             ProcessRunner *runner)
    : m_settings(settings)
    , m_registry(registry)
    , m_inspector(inspector)
    , m_desktop(desktop)
    , m_network(network)
    , m_processes(processes)
    , m_runner(runner)
{
}

UpdateCheckResult UpdateService::check(const InstalledApp &app, std::atomic<bool> *cancel)
{
    UpdateSource *source = nullptr;
    if (!app.updateManager.isEmpty()) {
        source = UpdateSourceFactory::byName(app.updateManager);
    } else if (!app.embeddedUpdate.isEmpty()) {
        for (UpdateSource *candidate : UpdateSourceFactory::all()) {
            if (candidate->handlesEmbedded(app.embeddedUpdate)) {
                source = candidate;
                break;
            }
        }
    }
    if (!source) {
        UpdateCheckResult result;
        result.error = QStringLiteral("No update method was found");
        return result;
    }
    InstalledApp copy = app;
    if (copy.updateConfig.isEmpty()) {
        EmbeddedUpdateInfo info;
        info.raw = app.embeddedUpdate;
        info.fields = AppImageInspector::parseUpdInfo(app.embeddedUpdate.toUtf8()).fields;
        copy.updateConfig = source->configFromEmbedded(info);
    }
    return source->check(copy, m_network, cancel);
}

QVector<UpdateOffer> UpdateService::listUpdates(std::atomic<bool> *cancel)
{
    QVector<UpdateOffer> offers;
    for (InstalledApp app : m_registry->apps()) {
        if (!app.owned) {
            continue;
        }
        const UpdateCheckResult checked = check(app, cancel);
        if (!checked.ok || !checked.available) {
            continue;
        }
        UpdateOffer offer;
        offer.uuid = app.uuid;
        offer.name = app.name;
        offer.currentVersion = app.version;
        offer.availableVersion = checked.version;
        offer.manager = checked.manager;
        offer.url = checked.url;
        offer.downloadSize = checked.size;
        offer.digest = checked.digest;
        offer.reducedVerification = checked.reducedVerification;
        offer.embeddedSource = app.embeddedUpdate;
        if (m_processes) {
            offer.running = !m_processes->pidsForExecutable(SafeFs::canonicalExisting(app.managedPath)).isEmpty();
        }
        offers.append(offer);
        app.availableVersion = checked.version;
        app.availableUrl = checked.url;
        app.availableSize = checked.size;
        app.updateAvailable = true;
        app.reducedVerification = checked.reducedVerification;
        app.lastUpdateCheck = QDateTime::currentDateTimeUtc();
        m_registry->upsert(app);
    }
    m_registry->save();
    return offers;
}

IntegrateResult UpdateService::apply(const InstalledApp &app, bool force, std::atomic<bool> *cancel)
{
    IntegrateResult result;
    if (!app.owned) {
        result.error = QStringLiteral("Cannot update an unowned AppImage");
        return result;
    }
    const QString canonical = SafeFs::canonicalExisting(app.managedPath);
    if (m_processes && !m_processes->pidsForExecutable(canonical).isEmpty() && !force) {
        result.error = QStringLiteral("Application is running; use --force to override");
        return result;
    }
    const UpdateCheckResult checked = check(app, cancel);
    if (!checked.ok || checked.url.isEmpty()) {
        result.error = checked.error.isEmpty() ? QStringLiteral("No update available") : checked.error;
        return result;
    }
    const QString stagingDir = m_settings->cacheDir() + QStringLiteral("/updates");
    QString mkdirError;
    if (!SafeFs::mkdir0700(stagingDir, &mkdirError)) {
        result.error = mkdirError;
        return result;
    }
    const QString staging = stagingDir + QLatin1Char('/') + app.uuid + QStringLiteral(".download");
    NetworkRequest req;
    req.url = QUrl(checked.url);
    req.destinationPath = staging;
    req.maxBytes = m_settings->maxAppImageBytes();
    const NetworkResult downloaded = m_network->fetch(req, cancel);
    if (!downloaded.ok) {
        SafeFs::removeFileNoFollow(staging);
        result.error = downloaded.error;
        return result;
    }
    InspectOptions options;
    options.maxBytes = m_settings->maxAppImageBytes();
    options.allowUnsafeExtract = false;
    const InspectionResult inspection = m_inspector->inspect(staging, options, cancel);
    if (!inspection.magicValid || !inspection.architectureSupported) {
        SafeFs::removeFileNoFollow(staging);
        result.error = inspection.error.isEmpty() ? QStringLiteral("Downloaded file is not a compatible AppImage")
                                                  : inspection.error;
        return result;
    }
    if (inspection.architecture != app.architecture && app.architecture != Architecture::Unknown) {
        SafeFs::removeFileNoFollow(staging);
        result.error = QStringLiteral("Downloaded architecture does not match the installed AppImage");
        return result;
    }
    const QString rollback = app.managedPath + QStringLiteral(".gosh-rollback");
    QString copyError;
    qint64 copied = 0;
    if (QFile::exists(app.managedPath)
        && !SafeFs::copyBounded(app.managedPath, rollback, m_settings->maxAppImageBytes(), cancel, &copied, &copyError)) {
        SafeFs::removeFileNoFollow(staging);
        result.error = copyError;
        return result;
    }
    if (!SafeFs::chmodPath(staging, 0755, &copyError) || !SafeFs::renameOver(staging, app.managedPath, &copyError)) {
        if (QFile::exists(rollback)) {
            SafeFs::renameOver(rollback, app.managedPath);
        }
        SafeFs::removeFileNoFollow(staging);
        result.error = copyError;
        return result;
    }
    InstalledApp updated = app;
    updated.sha256 = inspection.identity.sha256;
    updated.size = inspection.identity.size;
    updated.version = inspection.metadata.version.isEmpty() ? checked.version : inspection.metadata.version;
    updated.name = inspection.metadata.name.isEmpty() ? app.name : inspection.metadata.name;
    updated.comment = inspection.metadata.comment.isEmpty() ? app.comment : inspection.metadata.comment;
    updated.type = inspection.type;
    updated.architecture = inspection.architecture;
    updated.updateAvailable = false;
    updated.availableVersion.clear();
    updated.availableUrl.clear();
    const QString stagedDesktop = SafeFs::siblingTemp(updated.desktopPath, QStringLiteral(".gosh-desk-"));
    if (!m_desktop->writeStaged(updated, stagedDesktop, {}, &copyError)
        || !m_desktop->install(updated, stagedDesktop, {}, &copyError)) {
        SafeFs::renameOver(rollback, app.managedPath);
        SafeFs::removeFileNoFollow(stagedDesktop);
        result.error = copyError.isEmpty() ? QStringLiteral("Desktop integration failed") : copyError;
        return result;
    }
    m_registry->upsert(updated);
    if (!m_registry->save(&copyError)) {
        SafeFs::renameOver(rollback, app.managedPath);
        result.error = copyError;
        return result;
    }
    SafeFs::removeFileNoFollow(rollback);
    result.ok = true;
    result.app = updated;
    return result;
}

bool UpdateService::setSource(InstalledApp app, const QString &manager, const QVariantMap &config, QString *error)
{
    UpdateSource *source = UpdateSourceFactory::byName(manager);
    if (!source) {
        if (error) {
            *error = QStringLiteral("Unknown update manager");
        }
        return false;
    }
    if (!source->validateConfig(config, error) && source->name() != QLatin1String("ftp")) {
        return false;
    }
    if (source->name() == QLatin1String("ftp") && !source->validateConfig(config, error)) {
        return false;
    }
    app.updateManager = source->name();
    app.updateConfig = config;
    m_registry->upsert(app);
    return m_registry->save(error);
}

bool UpdateService::unsetSource(InstalledApp app, QString *error)
{
    app.updateManager.clear();
    app.updateConfig.clear();
    app.updateAvailable = false;
    m_registry->upsert(app);
    return m_registry->save(error);
}

} // namespace GoshAim
