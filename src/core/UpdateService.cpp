#include "UpdateService.h"

#include "AppImageInspector.h"
#include "CheckStateStore.h"
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
    , m_checkState(new CheckStateStore(settings))
{
}

UpdateService::~UpdateService()
{
    delete m_checkState;
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
    const QVariantMap last = m_checkState ? m_checkState->get(app.uuid) : QVariantMap{};
    if (!last.isEmpty()) {
        copy.updateConfig.insert(QStringLiteral("_last_etag"), last.value(QStringLiteral("etag")));
        copy.updateConfig.insert(QStringLiteral("_last_modified"), last.value(QStringLiteral("last_modified")));
        copy.updateConfig.insert(QStringLiteral("_last_size"), last.value(QStringLiteral("size")));
        copy.updateConfig.insert(QStringLiteral("_last_version"), last.value(QStringLiteral("version")));
        copy.updateConfig.insert(QStringLiteral("_last_digest"), last.value(QStringLiteral("digest")));
        copy.updateConfig.insert(QStringLiteral("_last_url"), last.value(QStringLiteral("url")));
    }
    UpdateCheckResult result = source->check(copy, m_network, cancel);
    if (result.ok && m_checkState) {
        QVariantMap state;
        state.insert(QStringLiteral("etag"), result.digest);
        state.insert(QStringLiteral("last_modified"), result.version);
        state.insert(QStringLiteral("size"), result.size);
        state.insert(QStringLiteral("version"), result.version);
        state.insert(QStringLiteral("digest"), result.digest);
        state.insert(QStringLiteral("url"), result.url);
        state.insert(QStringLiteral("checked_at"), QDateTime::currentDateTimeUtc().toString(Qt::ISODate));
        m_checkState->set(app.uuid, state);
        m_checkState->save();
    }
    return result;
}

QVector<UpdateOffer> UpdateService::listUpdates(std::atomic<bool> *cancel, bool persistInstallState)
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
        if (persistInstallState) {
            app.availableVersion = checked.version;
            app.availableUrl = checked.url;
            app.availableSize = checked.size;
            app.updateAvailable = true;
            app.reducedVerification = checked.reducedVerification;
            app.lastUpdateCheck = QDateTime::currentDateTimeUtc();
            m_registry->upsert(app);
        }
    }
    if (persistInstallState) {
        m_registry->save();
    }
    return offers;
}

bool UpdateService::verifyStagedDigest(const QString &staging, const UpdateCheckResult &checked, QString *error) const
{
    if (checked.digest.trimmed().isEmpty()) {
        return true;
    }
    const QString normalized = SafeFs::normalizeDigest(checked.digest);
    const bool wantSha1 = checked.digestAlgo.compare(QLatin1String("sha1"), Qt::CaseInsensitive) == 0
        || normalized.size() == 40;
    const HashResult hashed = wantSha1 ? SafeFs::sha1File(staging, m_settings->maxAppImageBytes())
                                       : SafeFs::sha256File(staging, m_settings->maxAppImageBytes());
    if (hashed.sha256.isEmpty()) {
        if (error) {
            *error = hashed.error.isEmpty() ? QStringLiteral("Unable to hash staged download") : hashed.error;
        }
        return false;
    }
    if (!SafeFs::digestMatches(checked.digest, hashed.sha256)) {
        if (error) {
            *error = QStringLiteral("Downloaded digest does not match advertised digest");
        }
        return false;
    }
    return true;
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
    req.allowFtp = checked.manager == QLatin1String("ftp");
    const NetworkResult downloaded = m_network->fetch(req, cancel);
    if (!downloaded.ok) {
        SafeFs::removeFileNoFollow(staging);
        result.error = downloaded.error;
        return result;
    }
    QString digestError;
    if (!verifyStagedDigest(staging, checked, &digestError)) {
        SafeFs::removeFileNoFollow(staging);
        result.error = digestError;
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
    if (m_failPoint == UpdateFailPoint::AfterDownload) {
        SafeFs::removeFileNoFollow(staging);
        result.error = QStringLiteral("Forced download validation failure");
        return result;
    }

    const QString rollback = SafeFs::siblingTemp(app.managedPath, QStringLiteral(".gosh-rollback-"));
    QString copyError;
    qint64 copied = 0;
    QStringList temps{staging};
    if (QFile::exists(app.managedPath)
        && !SafeFs::copyBounded(app.managedPath, rollback, m_settings->maxAppImageBytes(), cancel, &copied, &copyError)) {
        SafeFs::removeFileNoFollow(staging);
        result.error = copyError;
        return result;
    }
    if (QFile::exists(rollback)) {
        temps.append(rollback);
    }
    QString desktopBackup;
    QString iconBackup;
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

    if (!SafeFs::chmodPath(staging, 0755, &copyError) || !SafeFs::renameOver(staging, app.managedPath, &copyError)) {
        if (QFile::exists(rollback)) {
            SafeFs::renameOver(rollback, app.managedPath);
        }
        for (const QString &temp : temps) {
            SafeFs::removeFileNoFollow(temp);
        }
        result.error = copyError;
        return result;
    }
    temps.removeAll(staging);

    auto restoreLive = [&]() {
        if (QFile::exists(rollback)) {
            SafeFs::renameOver(rollback, app.managedPath);
        }
        if (!desktopBackup.isEmpty() && QFile::exists(desktopBackup)) {
            SafeFs::renameOver(desktopBackup, app.desktopPath);
        }
        if (!iconBackup.isEmpty() && QFile::exists(iconBackup)) {
            SafeFs::renameOver(iconBackup, app.iconPath);
        }
        m_registry->restoreApps(registrySnap);
    };

    if (m_failPoint == UpdateFailPoint::AfterReplace) {
        result.error = QStringLiteral("Forced post-replace failure");
        restoreLive();
        for (const QString &temp : temps) {
            SafeFs::removeFileNoFollow(temp);
        }
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
    updated.arguments = app.arguments;
    updated.environment = app.environment;
    updated.updateManager = app.updateManager;
    updated.updateConfig = app.updateConfig;
    updated.actions = app.actions;
    const QString stagedDesktop = SafeFs::siblingTemp(updated.desktopPath, QStringLiteral(".gosh-desk-"));
    temps.append(stagedDesktop);
    if (m_failPoint == UpdateFailPoint::DesktopInstall
        || !m_desktop->writeStaged(updated, stagedDesktop, {}, &copyError)
        || !m_desktop->install(updated, stagedDesktop, {}, &copyError)) {
        result.error = m_failPoint == UpdateFailPoint::DesktopInstall
            ? QStringLiteral("Forced desktop install failure")
            : (copyError.isEmpty() ? QStringLiteral("Desktop integration failed") : copyError);
        restoreLive();
        for (const QString &temp : temps) {
            SafeFs::removeFileNoFollow(temp);
        }
        return result;
    }
    temps.removeAll(stagedDesktop);
    m_registry->upsert(updated);
    if (m_failPoint == UpdateFailPoint::RegistrySave || !m_registry->save(&copyError)) {
        result.error = m_failPoint == UpdateFailPoint::RegistrySave ? QStringLiteral("Forced registry save failure") : copyError;
        restoreLive();
        for (const QString &temp : temps) {
            SafeFs::removeFileNoFollow(temp);
        }
        return result;
    }
    for (const QString &temp : temps) {
        SafeFs::removeFileNoFollow(temp);
    }
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
    if (!source->validateConfig(config, error)) {
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
    if (m_checkState) {
        m_checkState->clear(app.uuid);
        m_checkState->save();
    }
    return m_registry->save(error);
}

} // namespace GoshAim
