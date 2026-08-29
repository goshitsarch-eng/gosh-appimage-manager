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
#include "core/TaskQueue.h"
#include "core/UpdateService.h"
#include "core/UpdateSources.h"

#include <QDesktopServices>
#include <QFile>
#include <QFileInfo>
#include <QUrl>

namespace GoshAim {

AppController::AppController(QObject *parent,
                             ProcessRunner *runner,
                             NetworkClient *network,
                             ProcessTable *processes,
                             bool ownServices)
    : QObject(parent)
    , m_own(ownServices)
{
    m_ownRunner = runner == nullptr;
    m_ownNetwork = network == nullptr;
    m_ownProcesses = processes == nullptr;
    m_runner = runner ? runner : new QtProcessRunner();
    m_network = network ? network : new QtNetworkClient();
    m_processes = processes ? processes : new ProcProcessTable(m_runner);
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
    connect(m_tasks, &TaskQueue::tasksChanged, this, &AppController::reloadTasks);
    connect(m_tasks, &TaskQueue::finished, this, [this](const QString &, bool, const QString &) {
        refreshLibrary();
        Q_EMIT busyChanged();
    });
    connect(m_settings, &SettingsStore::changed, this, [this]() {
        m_theme->setAppearance(m_settings->appearanceName());
        refreshLibrary();
    });
    refreshLibrary();
}

AppController::~AppController()
{
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
    }
    m_libraryModel->setApps(apps);
    m_libraryModel->setFilter(m_search);
    m_libraryModel->setSort(m_sort);
}

void AppController::inspectPaths(const QStringList &paths)
{
    m_pendingInspect.clear();
    for (const QString &path : paths) {
        QString local = path;
        if (local.startsWith(QLatin1String("file:"))) {
            local = QUrl(local).toLocalFile();
        }
        InspectOptions options;
        options.maxBytes = m_settings->maxAppImageBytes();
        options.allowUnsafeExtract = false;
        m_pendingInspect.append(m_inspector->inspect(local, options));
    }
    m_candidateModel->setCandidates(m_pendingInspect);
    QStringList lines;
    for (const InspectionResult &item : m_pendingInspect) {
        lines.append(item.metadata.name + QStringLiteral(" — ") + item.identity.path);
    }
    m_inspectSummary = lines.join(QLatin1Char('\n'));
    Q_EMIT inspectSummaryChanged();
    Q_EMIT confirmInspect();
}

void AppController::confirmIntegrate(int conflictPolicy, bool moveSource)
{
    const ConflictPolicy policy = conflictPolicy == 1 ? ConflictPolicy::Replace : ConflictPolicy::KeepBoth;
    const QVector<InspectionResult> pending = m_pendingInspect;
    m_pendingInspect.clear();
    m_candidateModel->setCandidates({});
    for (const InspectionResult &item : pending) {
        if (!item.error.isEmpty() || !item.magicValid) {
            continue;
        }
        m_tasks->enqueue(
            TaskKind::Integrate,
            tr("Integrate %1").arg(item.metadata.name),
            item.identity.path,
            [this, item, policy, moveSource](TaskItem &task, std::atomic<bool> *cancel) {
                IntegrateRequest req;
                req.sourcePath = item.identity.path;
                req.conflict = policy;
                req.copyMode = moveSource ? CopyMode::Move : CopyMode::Copy;
                const IntegrateResult result = m_integration->integrate(req, cancel);
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
    m_pendingInspect.clear();
    m_candidateModel->setCandidates({});
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
    m_tasks->enqueue(
        TaskKind::Remove,
        tr("Remove %1").arg(app.name),
        app.managedPath,
        [this, uuid, permanent](TaskItem &task, std::atomic<bool> *cancel) {
            RemovalRequest req;
            req.pathOrUuid = uuid;
            req.mode = permanent ? RemovalMode::Permanent : RemovalMode::Trash;
            QString error;
            if (!m_removal->remove(req, &error, cancel)) {
                task.error = error;
            }
        },
        true);
}

void AppController::checkUpdate(const QString &uuid)
{
    const InstalledApp app = m_registry->byUuid(uuid);
    m_tasks->enqueue(
        TaskKind::CheckUpdate,
        tr("Check update for %1").arg(app.name),
        app.managedPath,
        [this, app](TaskItem &task, std::atomic<bool> *cancel) {
            const UpdateCheckResult checked = m_updates->check(app, cancel);
            if (!checked.ok) {
                task.error = checked.error;
            }
            const QVector<UpdateOffer> offers = m_updates->listUpdates(cancel);
            QMetaObject::invokeMethod(this, [this, offers]() { m_updatesModel->setOffers(offers); }, Qt::QueuedConnection);
        },
        false);
}

void AppController::updateApp(const QString &uuid, bool force)
{
    const InstalledApp app = m_registry->byUuid(uuid);
    m_tasks->enqueue(
        TaskKind::Update,
        tr("Update %1").arg(app.name),
        app.managedPath,
        [this, app, force](TaskItem &task, std::atomic<bool> *cancel) {
            const IntegrateResult result = m_updates->apply(app, force, cancel);
            if (!result.ok) {
                task.error = result.error;
                task.retryable = true;
            }
        },
        true);
}

void AppController::updateAll(bool force)
{
    for (const InstalledApp &app : m_registry->apps()) {
        if (app.owned) {
            updateApp(app.uuid, force);
        }
    }
}

void AppController::cancelTask(const QString &id)
{
    m_tasks->cancel(id);
}

void AppController::refreshMetadata(const QString &uuid)
{
    const InstalledApp app = m_registry->byUuid(uuid);
    m_tasks->enqueue(
        TaskKind::RefreshMetadata,
        tr("Refresh metadata"),
        app.managedPath,
        [this, app](TaskItem &task, std::atomic<bool> *cancel) {
            InspectOptions options;
            options.allowUnsafeExtract = false;
            const InspectionResult inspection = m_inspector->inspect(app.managedPath, options, cancel);
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
            m_registry->upsert(updated);
            m_registry->save();
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
    m_registry->upsert(app);
    m_registry->save();
    QString error;
    const QString staged = SafeFs::siblingTemp(app.desktopPath, QStringLiteral(".gosh-desk-"));
    if (m_desktop->writeStaged(app, staged, {}, &error)) {
        m_desktop->install(app, staged, {}, &error);
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
    m_registry->upsert(app);
    m_registry->save();
    QString error;
    const QString staged = SafeFs::siblingTemp(app.desktopPath, QStringLiteral(".gosh-desk-"));
    if (m_desktop->writeStaged(app, staged, {}, &error)) {
        m_desktop->install(app, staged, {}, &error);
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

} // namespace GoshAim
