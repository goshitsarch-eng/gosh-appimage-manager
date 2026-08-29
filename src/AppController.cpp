#include "AppController.h"

#include "ThemeController.h"
#include "core/AppImageInspector.h"
#include "core/AppImageLibrary.h"
#include "core/DesktopIntegration.h"
#include "core/DesktopParser.h"
#include "core/IntegrationService.h"
#include "core/ManagedRegistry.h"
#include "core/NetworkClient.h"
#include "core/ProcessRunner.h"
#include "core/ProcessTable.h"
#include "core/RemovalLaunch.h"
#include "core/SafeFs.h"
#include "core/Limits.h"
#include "core/TaskQueue.h"
#include "core/UpdateService.h"
#include "core/UpdateSources.h"
#include "core/UpdateNotifier.h"

#include <QCoreApplication>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QLoggingCategory>
#include <QPointer>
#include <QStandardPaths>
#include <QUrl>

Q_LOGGING_CATEGORY(lcGoshAim, "gosh.aim")

namespace GoshAim {

AppController::AppController(QObject *parent,
                             ProcessRunner *runner,
                             NetworkClient *network,
                             ProcessTable *processes,
                             bool ownServices,
                             UpdateNotifier *notifier)
    : QObject(parent)
    , m_own(ownServices)
{
    m_ownRunner = runner == nullptr;
    m_ownNetwork = network == nullptr;
    m_ownProcesses = processes == nullptr;
    m_runner = runner ? runner : new QtProcessRunner();
    m_network = network ? network : new QtNetworkClient();
    m_processes = processes ? processes : new ProcProcessTable(m_runner);
    if (notifier) {
        m_notifier = notifier;
    } else if (QStandardPaths::isTestModeEnabled()) {
        m_notifier = new NullUpdateNotifier();
        m_ownNotifier = true;
    } else {
        m_notifier = new KdeUpdateNotifier();
        m_ownNotifier = true;
    }
    m_settings = new SettingsStore(this);
    m_theme = new ThemeController(this);
    m_theme->setAppearance(m_settings->appearanceName());
    m_registry = new ManagedRegistry(m_settings);
    m_registry->load();
    m_inspector = new AppImageInspector(m_runner, m_settings, m_registry);
    m_desktop = new DesktopIntegration(m_settings, m_runner);
    m_integration = new IntegrationService(m_settings, m_registry, m_inspector, m_desktop, m_runner);
    m_removal = new RemovalService(m_settings, m_registry, m_desktop, m_processes, m_runner);
    m_launch = new LaunchService(m_runner, m_processes);
    m_updates = new UpdateService(m_settings, m_registry, m_inspector, m_desktop, m_network, m_processes, m_runner);
    m_library = new AppImageLibrary(m_settings, m_registry, m_desktop);
    m_tasks = new TaskQueue(this);
    m_libraryModel = new LibraryModel(this);
    m_updatesModel = new UpdatesModel(this);
    m_taskModel = new TaskModel(this);
    m_candidateModel = new CandidateModel(this);
    m_backgroundTimer = new QTimer(this);
    m_backgroundTimer->setInterval(6 * 60 * 60 * 1000);
    connect(m_backgroundTimer, &QTimer::timeout, this, &AppController::checkAll);
    connect(m_tasks, &TaskQueue::tasksChanged, this, [this]() {
        reloadTasks();
        ++m_taskTick;
        Q_EMIT taskTickChanged();
    });
    connect(m_tasks, &TaskQueue::finished, this, [this](const QString &id, bool ok, const QString &) {
        if (ok) {
            const QString uuid = m_updateTaskIds.key(id);
            if (!uuid.isEmpty()) {
                dropOffer(uuid);
            } else {
                refreshLibrary();
            }
        } else {
            refreshLibrary();
        }
        if (m_updateAllIds.contains(id)) {
            m_updateAllIds.remove(id);
            if (ok) {
                ++m_updateAllSucceeded;
            } else {
                ++m_updateAllFailed;
            }
            if (m_updateAllIds.isEmpty()) {
                m_updateSummary = tr("Update-all finished: %1 succeeded, %2 failed")
                                      .arg(m_updateAllSucceeded)
                                      .arg(m_updateAllFailed);
                Q_EMIT updateSummaryChanged();
                setStatus(m_updateSummary);
            }
        }
        Q_EMIT busyChanged();
    });
    connect(m_settings, &SettingsStore::changed, this, [this]() {
        m_theme->setAppearance(m_settings->appearanceName());
        applyDebugLogging();
        syncBackgroundChecks();
        refreshLibrary();
    });
    applyDebugLogging();
    syncBackgroundChecks();
    refreshLibrary();
}

AppController::~AppController()
{
    ++m_inspectGeneration;
    if (m_tasks) {
        m_tasks->shutdown();
    }
    if (m_own) {
        delete m_inspector;
        delete m_desktop;
        delete m_integration;
        delete m_removal;
        delete m_launch;
        delete m_updates;
        delete m_library;
        delete m_registry;
    }
    if (m_ownRunner) {
        delete m_runner;
    }
    if (m_ownNetwork) {
        delete m_network;
    }
    if (m_ownProcesses) {
        delete m_processes;
    }
    if (m_ownNotifier) {
        delete m_notifier;
    }
}

void AppController::applyDebugLogging()
{
    const bool enabled = m_settings && m_settings->debugLogging();
    QLoggingCategory::setFilterRules(enabled ? QStringLiteral("gosh.aim=true") : QStringLiteral("gosh.aim=false"));
    qCInfo(lcGoshAim) << "debug logging" << enabled;
}

void AppController::syncBackgroundChecks()
{
    if (!m_settings || !m_backgroundTimer) {
        return;
    }
    const bool enabled = m_settings->backgroundUpdateChecks();
    if (enabled) {
        if (!m_backgroundTimer->isActive()) {
            m_backgroundTimer->start();
        }
    } else {
        m_backgroundTimer->stop();
    }
    const QString autostartDir = autostartDirectory();
    const QString desktopPath = autostartDesktopPath();
    if (enabled) {
        QDir().mkpath(autostartDir);
        QString execLine;
        if (QFile::exists(QStringLiteral("/.flatpak-info"))
            || qEnvironmentVariable("FLATPAK_ID") == QLatin1String("com.goshapps.AppImageManager")) {
            execLine = QStringLiteral("flatpak run com.goshapps.AppImageManager --fetch-updates");
        } else {
            const QString exe = QCoreApplication::applicationFilePath();
            execLine = DesktopParser::escapeExecArg(exe) + QStringLiteral(" --fetch-updates");
        }
        const QByteArray body = QByteArrayLiteral(
                                    "[Desktop Entry]\n"
                                    "Type=Application\n"
                                    "Name=Gosh AppImage Manager update checks\n"
                                    "Exec=")
            + execLine.toUtf8() + QByteArrayLiteral(
                                    "\n"
                                    "X-GNOME-Autostart-enabled=true\n"
                                    "OnlyShowIn=KDE;\n");
        QFile file(desktopPath);
        if (file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            file.write(body);
        }
    } else {
        QFile::remove(desktopPath);
    }
}

void AppController::notifyUpdates(int count)
{
    if (count <= 0) {
        return;
    }
    const QString text = tr("%1 AppImage update(s) available").arg(count);
    if (m_notifier) {
        m_notifier->notifyUpdatesAvailable(count);
    }
    setStatus(text);
}

void AppController::setSearch(const QString &value)
{
    if (m_search == value) {
        return;
    }
    m_search = value;
    m_libraryModel->setFilter(value);
    Q_EMIT searchChanged();
}

void AppController::setSort(const QString &value)
{
    if (m_sort == value) {
        return;
    }
    m_sort = value;
    m_libraryModel->setSort(value);
    Q_EMIT searchChanged();
}

bool AppController::modelsReady() const
{
    return m_libraryModel && m_updatesModel && m_taskModel && m_candidateModel
        && m_libraryModel->roleNames().contains(LibraryModel::UuidRole)
        && m_updatesModel->roleNames().contains(UpdatesModel::UuidRole)
        && m_taskModel->roleNames().contains(TaskModel::IdRole);
}

QString AppController::licenseText() const
{
    QFile file(QStringLiteral(":/gosh-appimage-manager/COPYING"));
    if (!file.open(QIODevice::ReadOnly)) {
        return QStringLiteral("GNU General Public License version 3 or later.");
    }
    return QString::fromUtf8(file.readAll());
}

bool AppController::busy() const
{
    return m_tasks && m_tasks->busy();
}

void AppController::setStatus(const QString &message)
{
    m_status = message;
    Q_EMIT statusMessageChanged();
    Q_EMIT toast(message);
}

void AppController::reloadTasks()
{
    if (m_tasks) {
        m_taskModel->setTasks(m_tasks->tasks());
    }
    Q_EMIT busyChanged();
}

void AppController::refreshLibrary()
{
    QVector<InstalledApp> apps = m_library->discover();
    for (InstalledApp &app : apps) {
        app.running = m_launch->isRunning(app);
        app.updateAvailable = m_availableUpdateUuids.contains(app.uuid);
    }
    m_libraryModel->setApps(apps);
    m_libraryModel->setFilter(m_search);
    m_libraryModel->setSort(m_sort);
}

void AppController::finishInspect(const QVector<InspectionResult> &results, int generation)
{
    if (generation != m_inspectGeneration) {
        return;
    }
    m_pendingInspect = results;
    m_candidateModel->setCandidates(m_pendingInspect);
    QStringList lines;
    for (const InspectionResult &item : m_pendingInspect) {
        lines.append(item.metadata.name + QStringLiteral(" — ") + item.identity.path);
    }
    m_inspectSummary = lines.join(QLatin1Char('\n'));
    m_inspecting = false;
    m_inspectProgress = 100;
    Q_EMIT inspectSummaryChanged();
    Q_EMIT inspectingChanged();
    Q_EMIT inspectProgressChanged();
    Q_EMIT confirmInspect();
    if (m_settings && m_settings->unsafeExtractionFallback()) {
        for (const InspectionResult &item : m_pendingInspect) {
            if (item.magicValid && item.metadata.execRaw.isEmpty() && !item.extractionUsedUnsafeFallback) {
                Q_EMIT confirmUnsafeExtract(item.identity.path);
                break;
            }
        }
    }
}

void AppController::inspectPaths(const QStringList &paths)
{
    ++m_inspectGeneration;
    const int gen = m_inspectGeneration;
    m_pendingInspect.clear();
    m_candidateModel->setCandidates({});
    m_inspecting = true;
    m_inspectProgress = 0;
    Q_EMIT inspectingChanged();
    Q_EMIT inspectProgressChanged();
    QPointer<AppController> self(this);
    m_inspectTaskId = m_tasks->enqueue(
        TaskKind::Inspect,
        tr("Inspect AppImages"),
        paths.join(QLatin1Char(',')),
        [self, paths, gen](TaskItem &task, std::atomic<bool> *cancel) {
            QVector<InspectionResult> results;
            for (int i = 0; i < paths.size(); ++i) {
                if (cancel && cancel->load()) {
                    task.error = QStringLiteral("Cancelled");
                    break;
                }
                if (!self) {
                    return;
                }
                QString local = paths.at(i);
                if (local.startsWith(QLatin1String("file:"))) {
                    local = QUrl(local).toLocalFile();
                }
                InspectOptions options;
                options.maxBytes = self->m_settings->maxAppImageBytes();
                options.allowUnsafeExtract = false;
                options.confirmUnsafeExtract = false;
                InspectionResult item = self->m_inspector->inspect(local, options, cancel);
                const CopyMode mode = self->m_settings->moveSource() ? CopyMode::Move : CopyMode::Copy;
                self->m_integration->annotatePlan(&item, mode);
                results.append(item);
                task.progress = int(double(i + 1) / double(paths.size()) * 100.0);
                QMetaObject::invokeMethod(
                    qApp,
                    [self, progress = task.progress, gen]() {
                        if (!self || gen != self->m_inspectGeneration) {
                            return;
                        }
                        self->m_inspectProgress = progress;
                        Q_EMIT self->inspectProgressChanged();
                    },
                    Qt::QueuedConnection);
            }
            QMetaObject::invokeMethod(
                qApp,
                [self, results, gen]() {
                    if (!self) {
                        return;
                    }
                    self->finishInspect(results, gen);
                },
                Qt::QueuedConnection);
        },
        false);
}

void AppController::setCandidateConflict(int row, int policy, const QString &replaceUuid)
{
    if (row < 0 || row >= m_pendingInspect.size()) {
        return;
    }
    if (policy == int(ConflictPolicy::Replace)) {
        m_pendingInspect[row].chosenPolicy = ConflictPolicy::Replace;
        m_pendingInspect[row].chosenReplaceUuid = replaceUuid.isEmpty() ? m_pendingInspect[row].conflictingUuid : replaceUuid;
    } else if (policy == int(ConflictPolicy::KeepBoth)) {
        m_pendingInspect[row].chosenPolicy = ConflictPolicy::KeepBoth;
        m_pendingInspect[row].chosenReplaceUuid.clear();
    } else {
        m_pendingInspect[row].chosenPolicy = ConflictPolicy::Unspecified;
        m_pendingInspect[row].chosenReplaceUuid.clear();
    }
    m_integration->applyConflictChoice(&m_pendingInspect[row]);
    m_candidateModel->setCandidates(m_pendingInspect);
    Q_EMIT inspectSummaryChanged();
}

void AppController::confirmIntegrate(int conflictPolicy, bool moveSource, const QString &replaceUuid)
{
    Q_UNUSED(conflictPolicy);
    Q_UNUSED(replaceUuid);
    const QVector<InspectionResult> pending = m_pendingInspect;
    m_pendingInspect.clear();
    m_candidateModel->setCandidates({});
    Q_EMIT inspectSummaryChanged();
    QPointer<AppController> self(this);
    for (const InspectionResult &item : pending) {
        if (!item.error.isEmpty() || !item.magicValid) {
            continue;
        }
        ConflictPolicy policy = item.chosenPolicy;
        QString uuid;
        if (item.needsConflictDecision) {
            if (policy != ConflictPolicy::Replace && policy != ConflictPolicy::KeepBoth) {
                continue;
            }
            if (policy == ConflictPolicy::Replace) {
                uuid = item.chosenReplaceUuid;
            }
        } else {
            policy = ConflictPolicy::KeepBoth;
        }
        m_tasks->enqueue(
            TaskKind::Integrate,
            tr("Integrate %1").arg(item.metadata.name),
            item.identity.path,
            [self, item, policy, moveSource, uuid](TaskItem &task, std::atomic<bool> *cancel) {
                if (!self) {
                    return;
                }
                IntegrateRequest req;
                req.sourcePath = item.identity.path;
                req.conflict = policy;
                req.replaceUuid = uuid;
                req.copyMode = moveSource ? CopyMode::Move : CopyMode::Copy;
                const IntegrateResult result = self->m_integration->integrate(req, cancel);
                if (!result.ok) {
                    task.error = result.error;
                    task.retryable = true;
                }
            },
            true);
    }
}

void AppController::cancelInspect()
{
    ++m_inspectGeneration;
    if (!m_inspectTaskId.isEmpty()) {
        m_tasks->cancel(m_inspectTaskId);
    }
    m_pendingInspect.clear();
    m_candidateModel->setCandidates({});
    m_inspecting = false;
    Q_EMIT inspectingChanged();
}

void AppController::confirmUnsafeExtractFor(const QString &path, bool allow)
{
    if (!allow || !m_settings || !m_settings->unsafeExtractionFallback()) {
        return;
    }
    QPointer<AppController> self(this);
    const int gen = m_inspectGeneration;
    m_tasks->enqueue(
        TaskKind::Inspect,
        tr("Unsafe extract"),
        path,
        [self, path, gen](TaskItem &task, std::atomic<bool> *cancel) {
            Q_UNUSED(task);
            if (!self) {
                return;
            }
            InspectOptions options;
            options.allowUnsafeExtract = true;
            options.confirmUnsafeExtract = true;
            InspectionResult item = self->m_inspector->inspect(path, options, cancel);
            self->m_integration->annotatePlan(&item, self->m_settings->moveSource() ? CopyMode::Move : CopyMode::Copy);
            QMetaObject::invokeMethod(
                qApp,
                [self, item, gen]() {
                    if (!self || gen != self->m_inspectGeneration) {
                        return;
                    }
                    for (int i = 0; i < self->m_pendingInspect.size(); ++i) {
                        if (self->m_pendingInspect[i].identity.path == item.identity.path) {
                            self->m_pendingInspect[i] = item;
                        }
                    }
                    self->m_candidateModel->setCandidates(self->m_pendingInspect);
                    Q_EMIT self->inspectSummaryChanged();
                },
                Qt::QueuedConnection);
        },
        false);
}

void AppController::launchApp(const QString &uuid)
{
    const InstalledApp app = m_libraryModel->byUuid(uuid);
    QString error;
    if (!m_launch->launch(app, &error)) {
        setStatus(error);
    } else {
        setStatus(tr("Launched %1").arg(app.name));
    }
}

void AppController::revealApp(const QString &uuid)
{
    const InstalledApp app = m_libraryModel->byUuid(uuid);
    const QFileInfo info(app.managedPath);
    QDesktopServices::openUrl(QUrl::fromLocalFile(info.absolutePath()));
}

void AppController::removeApp(const QString &uuid, bool permanent)
{
    const InstalledApp app = m_libraryModel->byUuid(uuid);
    QPointer<AppController> self(this);
    m_tasks->enqueue(
        TaskKind::Remove,
        tr("Remove %1").arg(app.name),
        app.managedPath,
        [self, uuid, permanent](TaskItem &task, std::atomic<bool> *cancel) {
            if (!self) {
                return;
            }
            RemovalRequest req;
            req.pathOrUuid = uuid;
            req.mode = permanent ? RemovalMode::Permanent : RemovalMode::Trash;
            QString error;
            if (!self->m_removal->remove(req, &error, cancel)) {
                task.error = error;
            }
        },
        true);
}

void AppController::checkUpdate(const QString &uuid)
{
    if (uuid.isEmpty()) {
        checkAll();
        return;
    }
    const InstalledApp app = m_registry->byUuid(uuid);
    if (app.uuid.isEmpty()) {
        setStatus(tr("No such AppImage"));
        return;
    }
    QPointer<AppController> self(this);
    m_tasks->enqueue(
        TaskKind::CheckUpdate,
        tr("Check update for %1").arg(app.name),
        app.managedPath,
        [self, app](TaskItem &task, std::atomic<bool> *cancel) {
            if (!self) {
                return;
            }
            const UpdateCheckResult checked = self->m_updates->check(app, cancel);
            if (!checked.ok) {
                task.error = checked.error;
            }
            const QVector<UpdateOffer> offers = self->m_updates->listUpdates(cancel, false);
            QMetaObject::invokeMethod(
                qApp,
                [self, offers]() {
                    if (!self) {
                        return;
                    }
                    self->syncOffers(offers);
                },
                Qt::QueuedConnection);
        },
        false);
}

void AppController::checkAll()
{
    QPointer<AppController> self(this);
    m_tasks->enqueue(
        TaskKind::CheckUpdate,
        tr("Check all updates"),
        QStringLiteral("*"),
        [self](TaskItem &task, std::atomic<bool> *cancel) {
            if (!self) {
                return;
            }
            const QVector<UpdateOffer> offers = self->m_updates->listUpdates(cancel, false);
            Q_UNUSED(task);
            QMetaObject::invokeMethod(
                qApp,
                [self, offers]() {
                    if (!self) {
                        return;
                    }
                    self->syncOffers(offers);
                    self->notifyUpdates(offers.size());
                },
                Qt::QueuedConnection);
        },
        false);
}

void AppController::updateApp(const QString &uuid, bool force)
{
    enqueueUpdate(uuid, force);
}

QString AppController::enqueueUpdate(const QString &uuid, bool force)
{
    const InstalledApp app = m_registry->byUuid(uuid);
    if (app.uuid.isEmpty()) {
        return {};
    }
    QPointer<AppController> self(this);
    const QString id = m_tasks->enqueue(
        TaskKind::Update,
        tr("Update %1").arg(app.name),
        app.managedPath,
        [self, app, force](TaskItem &task, std::atomic<bool> *cancel) {
            if (!self) {
                return;
            }
            const IntegrateResult result = self->m_updates->apply(app, force, cancel, [&](int percent, const QString &status) {
                if (!self) {
                    return;
                }
                self->m_tasks->setProgress(task.id, percent, status);
            });
            if (!result.ok) {
                task.error = result.error;
                task.retryable = true;
            }
        },
        true);
    if (!id.isEmpty()) {
        m_updateTaskIds.insert(uuid, id);
        ++m_taskTick;
        Q_EMIT taskTickChanged();
    }
    return id;
}

void AppController::updateAll(bool force)
{
    m_updateAllSucceeded = 0;
    m_updateAllFailed = 0;
    m_updateAllIds.clear();
    const int count = m_updatesModel ? m_updatesModel->rowCount() : 0;
    for (int i = 0; i < count; ++i) {
        const UpdateOffer offer = m_updatesModel->at(i);
        if (offer.uuid.isEmpty()) {
            continue;
        }
        const QString id = enqueueUpdate(offer.uuid, force);
        if (!id.isEmpty()) {
            m_updateAllIds.insert(id);
        }
    }
    if (m_updateAllIds.isEmpty()) {
        m_updateSummary = tr("No updates are currently offered");
        Q_EMIT updateSummaryChanged();
    }
}

void AppController::cancelTask(const QString &id)
{
    m_tasks->cancel(id);
}

void AppController::retryTask(const QString &id)
{
    const TaskItem item = m_tasks->task(id);
    if (!item.retryable) {
        return;
    }
    if (item.kind == TaskKind::Update) {
        const InstalledApp app = m_registry->byPath(item.target);
        if (!app.uuid.isEmpty()) {
            updateApp(app.uuid, false);
        }
    } else if (item.kind == TaskKind::Integrate) {
        inspectPaths({item.target});
    } else if (item.kind == TaskKind::Remove) {
        const InstalledApp app = m_registry->byPath(item.target);
        if (!app.uuid.isEmpty()) {
            removeApp(app.uuid, false);
        }
    }
}

void AppController::refreshMetadata(const QString &uuid)
{
    const InstalledApp app = m_registry->byUuid(uuid);
    QPointer<AppController> self(this);
    m_tasks->enqueue(
        TaskKind::RefreshMetadata,
        tr("Refresh metadata"),
        app.managedPath,
        [self, app](TaskItem &task, std::atomic<bool> *cancel) {
            if (!self) {
                return;
            }
            InspectOptions options;
            options.allowUnsafeExtract = false;
            const InspectionResult inspection = self->m_inspector->inspect(app.managedPath, options, cancel);
            if (!inspection.error.isEmpty()) {
                task.error = inspection.error;
                return;
            }
            InstalledApp updated = app;
            if (!inspection.metadata.name.isEmpty()) {
                updated.name = inspection.metadata.name;
            }
            updated.version = inspection.metadata.version;
            updated.comment = inspection.metadata.comment;
            QString error;
            if (!self->persistAppEdits(updated, &error)) {
                task.error = error.isEmpty() ? QStringLiteral("Metadata refresh failed") : error;
            }
        },
        true);
}

void AppController::setArguments(const QString &uuid, const QStringList &arguments)
{
    InstalledApp app = m_registry->byUuid(uuid);
    if (app.uuid.isEmpty() || !app.owned) {
        return;
    }
    QStringList cleaned;
    for (const QString &arg : arguments) {
        if (!arg.contains(QChar(0)) && arg.size() <= kMaxArgumentLength) {
            cleaned.append(arg);
        }
    }
    app.arguments = cleaned;
    QString error;
    if (!persistAppEdits(app, &error)) {
        setStatus(error);
        return;
    }
    refreshLibrary();
}

void AppController::setEnvironment(const QString &uuid, const QVariantMap &env)
{
    InstalledApp app = m_registry->byUuid(uuid);
    if (app.uuid.isEmpty() || !app.owned) {
        return;
    }
    QVector<EnvPair> pairs;
    for (auto it = env.begin(); it != env.end(); ++it) {
        if (DesktopParser::isValidEnvName(it.key()) && !DesktopParser::isDangerousEnvName(it.key())
            && DesktopParser::isValidEnvValue(it.value().toString())) {
            EnvPair pair;
            pair.name = it.key();
            pair.value = it.value().toString();
            pairs.append(pair);
        }
    }
    app.environment = pairs;
    QString error;
    if (!persistAppEdits(app, &error)) {
        setStatus(error);
        return;
    }
    refreshLibrary();
}

void AppController::setUpdateSource(const QString &uuid, const QString &manager, const QVariantMap &config)
{
    InstalledApp app = m_registry->byUuid(uuid);
    QString error;
    if (!m_updates->setSource(app, manager, config, &error)) {
        setStatus(error);
    }
}

void AppController::unsetUpdateSource(const QString &uuid)
{
    InstalledApp app = m_registry->byUuid(uuid);
    QString error;
    m_updates->unsetSource(app, &error);
}

void AppController::adoptApp(const QString &uuid)
{
    const InstalledApp app = m_libraryModel->byUuid(uuid);
    QString error;
    if (!m_library->adopt(app, &error)) {
        setStatus(error);
    }
    refreshLibrary();
}

void AppController::selectApp(const QString &uuid)
{
    m_selectedUuid = uuid;
    Q_EMIT selectedChanged();
}

QVariantMap AppController::selectedDetails() const
{
    const InstalledApp app = m_libraryModel->byUuid(m_selectedUuid);
    QVariantMap map;
    map.insert(QStringLiteral("uuid"), app.uuid);
    map.insert(QStringLiteral("name"), app.name);
    map.insert(QStringLiteral("version"), app.version);
    map.insert(QStringLiteral("comment"), app.comment);
    map.insert(QStringLiteral("path"), app.managedPath);
    map.insert(QStringLiteral("desktopId"), app.desktopId);
    map.insert(QStringLiteral("hash"), QString::fromLatin1(app.sha256.toHex()));
    map.insert(QStringLiteral("type"), appImageTypeName(app.type));
    map.insert(QStringLiteral("architecture"), architectureName(app.architecture));
    map.insert(QStringLiteral("size"), app.size);
    map.insert(QStringLiteral("manager"), app.updateManager);
    map.insert(QStringLiteral("embedded"), app.embeddedUpdate);
    map.insert(QStringLiteral("owned"), app.owned);
    map.insert(QStringLiteral("external"), app.externalFolder);
    map.insert(QStringLiteral("running"), app.running);
    map.insert(QStringLiteral("arguments"), app.arguments);
    QVariantMap env;
    for (const EnvPair &pair : app.environment) {
        env.insert(pair.name, pair.value);
    }
    map.insert(QStringLiteral("environment"), env);
    map.insert(QStringLiteral("updateConfig"), app.updateConfig);
    map.insert(QStringLiteral("adopted"), app.adopted);
    return map;
}

void AppController::setAppearance(const QString &name)
{
    m_settings->setAppearanceName(name);
    m_theme->setAppearance(name);
}

QString AppController::formatSize(qint64 bytes) const
{
    if (bytes < 1024) {
        return tr("%1 B").arg(bytes);
    }
    if (bytes < 1024 * 1024) {
        return tr("%1 KiB").arg(bytes / 1024);
    }
    if (bytes < 1024LL * 1024 * 1024) {
        return tr("%1 MiB").arg(bytes / (1024 * 1024));
    }
    return tr("%1 GiB").arg(bytes / (1024LL * 1024 * 1024));
}

QStringList AppController::updateManagers() const
{
    return UpdateSourceFactory::names();
}

void AppController::openAppImages(const QStringList &urls)
{
    inspectPaths(urls);
}

QString AppController::localPathFromUrl(const QString &url) const
{
    const QUrl parsed = QUrl(url);
    if (!parsed.isValid() || parsed.isEmpty()) {
        return {};
    }
    if (parsed.isLocalFile()) {
        return parsed.toLocalFile();
    }
    if (url.startsWith(QLatin1Char('/'))) {
        return url;
    }
    return {};
}

void AppController::setManagedFolderFromUrl(const QString &url)
{
    const QString local = localPathFromUrl(url);
    if (local.isEmpty()) {
        setStatus(tr("Folder picker did not return a local path. Flatpak portals may be required."));
        return;
    }
    m_settings->setManagedFolder(local);
}

QStringList AppController::qmlActionNames() const
{
    return {QStringLiteral("inspectPaths"),
            QStringLiteral("confirmIntegrate"),
            QStringLiteral("checkAll"),
            QStringLiteral("updateAll"),
            QStringLiteral("retryTask"),
            QStringLiteral("adoptApp"),
            QStringLiteral("setEnvironment"),
            QStringLiteral("setUpdateSource")};
}

void AppController::loadSyntheticCatalog()
{
    InstalledApp fake;
    fake.uuid = QStringLiteral("self-test");
    fake.name = QStringLiteral("Self Test Catalog");
    fake.version = QStringLiteral("0");
    fake.owned = true;
    fake.managedPath = QDir::tempPath() + QStringLiteral("/gosh-aim-self-test.AppImage");
    m_libraryModel->setApps({fake});
}

bool AppController::conflictsResolved() const
{
    if (m_inspecting || m_pendingInspect.isEmpty()) {
        return false;
    }
    for (const InspectionResult &item : m_pendingInspect) {
        if (!item.error.isEmpty() || !item.magicValid) {
            continue;
        }
        if (item.needsConflictDecision
            && item.chosenPolicy != ConflictPolicy::KeepBoth
            && item.chosenPolicy != ConflictPolicy::Replace) {
            return false;
        }
    }
    return true;
}

QString AppController::updateTaskId(const QString &uuid) const
{
    return m_updateTaskIds.value(uuid);
}

int AppController::updateProgress(const QString &uuid) const
{
    if (!m_tasks) {
        return 0;
    }
    return m_tasks->task(m_updateTaskIds.value(uuid)).progress;
}

QString AppController::updateStatus(const QString &uuid) const
{
    if (!m_tasks) {
        return {};
    }
    const TaskItem item = m_tasks->task(m_updateTaskIds.value(uuid));
    if (item.id.isEmpty()) {
        return {};
    }
    if (!item.error.isEmpty()) {
        return item.error;
    }
    return item.statusText;
}

QString AppController::autostartDirectory() const
{
    if (QFile::exists(QStringLiteral("/.flatpak-info"))
        || qEnvironmentVariable("FLATPAK_ID") == QLatin1String("com.goshapps.AppImageManager")) {
        return QDir::homePath() + QStringLiteral("/.config/autostart");
    }
    return QStandardPaths::writableLocation(QStandardPaths::ConfigLocation) + QStringLiteral("/autostart");
}

QString AppController::autostartDesktopPath() const
{
    return autostartDirectory() + QStringLiteral("/com.goshapps.AppImageManager-updates.desktop");
}

void AppController::syncOffers(const QVector<UpdateOffer> &offers)
{
    m_availableUpdateUuids.clear();
    for (const UpdateOffer &offer : offers) {
        if (!offer.uuid.isEmpty()) {
            m_availableUpdateUuids.insert(offer.uuid);
        }
    }
    if (m_updatesModel) {
        m_updatesModel->setOffers(offers);
    }
    refreshLibrary();
}

void AppController::dropOffer(const QString &uuid)
{
    m_availableUpdateUuids.remove(uuid);
    if (m_updatesModel) {
        QVector<UpdateOffer> remain;
        for (int i = 0; i < m_updatesModel->rowCount(); ++i) {
            const UpdateOffer offer = m_updatesModel->at(i);
            if (offer.uuid != uuid) {
                remain.append(offer);
            }
        }
        m_updatesModel->setOffers(remain);
    }
    refreshLibrary();
}

InspectionResult AppController::pendingCandidate(int row) const
{
    if (row < 0 || row >= m_pendingInspect.size()) {
        return {};
    }
    return m_pendingInspect.at(row);
}

bool AppController::persistAppEdits(InstalledApp app, QString *error)
{
    const QVector<InstalledApp> snap = m_registry->snapshot();
    QByteArray desktopBytes;
    if (!app.desktopPath.isEmpty() && QFile::exists(app.desktopPath)) {
        QFile file(app.desktopPath);
        if (file.open(QIODevice::ReadOnly)) {
            desktopBytes = file.read(kMaxDesktopFileBytes);
        }
    }
    const QString staged = SafeFs::siblingTemp(app.desktopPath, QStringLiteral(".gosh-desk-"));
    QString localError;
    if (!m_desktop->writeStaged(app, staged, {}, &localError) || !m_desktop->install(app, staged, {}, &localError)) {
        SafeFs::removeFileNoFollow(staged);
        if (!desktopBytes.isEmpty()) {
            SafeFs::atomicWrite(app.desktopPath, desktopBytes, nullptr, 0644);
        }
        m_registry->restoreApps(snap);
        if (error) {
            *error = localError.isEmpty() ? QStringLiteral("Desktop update failed") : localError;
        }
        return false;
    }
    m_registry->upsert(app);
    if (!m_registry->save(&localError)) {
        m_registry->restoreApps(snap);
        m_registry->save();
        if (!desktopBytes.isEmpty()) {
            SafeFs::atomicWrite(app.desktopPath, desktopBytes, nullptr, 0644);
        }
        if (error) {
            *error = localError.isEmpty() ? QStringLiteral("Registry save failed") : localError;
        }
        return false;
    }
    Q_EMIT selectedChanged();
    return true;
}

} // namespace GoshAim
