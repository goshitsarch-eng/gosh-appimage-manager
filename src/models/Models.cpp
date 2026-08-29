#include "Models.h"

#include <algorithm>

namespace GoshAim {

LibraryModel::LibraryModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

int LibraryModel::rowCount(const QModelIndex &parent) const
{
    if (parent.isValid()) {
        return 0;
    }
    return m_filtered.size();
}

QVariant LibraryModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_filtered.size()) {
        return {};
    }
    const InstalledApp &app = m_filtered.at(index.row());
    switch (role) {
    case UuidRole:
        return app.uuid;
    case NameRole:
        return app.name;
    case VersionRole:
        return app.version;
    case CommentRole:
        return app.comment;
    case PathRole:
        return app.managedPath;
    case DesktopIdRole:
        return app.desktopId;
    case HashRole:
        return QString::fromLatin1(app.sha256.toHex());
    case TypeRole:
        return appImageTypeName(app.type);
    case ArchRole:
        return architectureName(app.architecture);
    case SizeRole:
        return app.size;
    case UpdateAvailableRole:
        return app.updateAvailable;
    case RunningRole:
        return app.running;
    case ExternalRole:
        return app.externalFolder;
    case OwnedRole:
        return app.owned;
    case ManagerRole:
        return app.updateManager;
    case IconRole:
        return app.iconPath;
    case TerminalRole:
        return app.terminal;
    default:
        return {};
    }
}

QHash<int, QByteArray> LibraryModel::roleNames() const
{
    return {{UuidRole, "uuid"},
            {NameRole, "name"},
            {VersionRole, "version"},
            {CommentRole, "comment"},
            {PathRole, "path"},
            {DesktopIdRole, "desktopId"},
            {HashRole, "fileHash"},
            {TypeRole, "appImageType"},
            {ArchRole, "architecture"},
            {SizeRole, "size"},
            {UpdateAvailableRole, "updateAvailable"},
            {RunningRole, "running"},
            {ExternalRole, "externalFolder"},
            {OwnedRole, "owned"},
            {ManagerRole, "updateManager"},
            {IconRole, "iconPath"},
            {TerminalRole, "terminal"}};
}

void LibraryModel::setApps(const QVector<InstalledApp> &apps)
{
    m_all = apps;
    apply();
}

void LibraryModel::setFilter(const QString &filter)
{
    m_filter = filter;
    apply();
}

void LibraryModel::setSort(const QString &sort)
{
    m_sort = sort;
    apply();
}

InstalledApp LibraryModel::at(int row) const
{
    if (row < 0 || row >= m_filtered.size()) {
        return {};
    }
    return m_filtered.at(row);
}

InstalledApp LibraryModel::byUuid(const QString &uuid) const
{
    for (const InstalledApp &app : m_all) {
        if (app.uuid == uuid) {
            return app;
        }
    }
    return {};
}

void LibraryModel::apply()
{
    beginResetModel();
    m_filtered.clear();
    for (const InstalledApp &app : m_all) {
        if (m_filter.isEmpty()
            || app.name.contains(m_filter, Qt::CaseInsensitive)
            || app.managedPath.contains(m_filter, Qt::CaseInsensitive)
            || app.comment.contains(m_filter, Qt::CaseInsensitive)) {
            m_filtered.append(app);
        }
    }
    std::sort(m_filtered.begin(), m_filtered.end(), [this](const InstalledApp &a, const InstalledApp &b) {
        if (m_sort == QLatin1String("size")) {
            return a.size > b.size;
        }
        if (m_sort == QLatin1String("version")) {
            return a.version < b.version;
        }
        return a.name.toLower() < b.name.toLower();
    });
    endResetModel();
    Q_EMIT countChanged();
}

UpdatesModel::UpdatesModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

int UpdatesModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : m_offers.size();
}

QVariant UpdatesModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_offers.size()) {
        return {};
    }
    const UpdateOffer &offer = m_offers.at(index.row());
    switch (role) {
    case UuidRole:
        return offer.uuid;
    case NameRole:
        return offer.name;
    case CurrentVersionRole:
        return offer.currentVersion;
    case AvailableVersionRole:
        return offer.availableVersion;
    case ManagerRole:
        return offer.manager;
    case SizeRole:
        return offer.downloadSize;
    case RunningRole:
        return offer.running;
    case ReducedRole:
        return offer.reducedVerification;
    case UrlRole:
        return offer.url;
    default:
        return {};
    }
}

QHash<int, QByteArray> UpdatesModel::roleNames() const
{
    return {{UuidRole, "uuid"},
            {NameRole, "name"},
            {CurrentVersionRole, "currentVersion"},
            {AvailableVersionRole, "availableVersion"},
            {ManagerRole, "manager"},
            {SizeRole, "downloadSize"},
            {RunningRole, "running"},
            {ReducedRole, "reducedVerification"},
            {UrlRole, "url"}};
}

void UpdatesModel::setOffers(const QVector<UpdateOffer> &offers)
{
    beginResetModel();
    m_offers = offers;
    endResetModel();
    Q_EMIT countChanged();
}

UpdateOffer UpdatesModel::at(int row) const
{
    if (row < 0 || row >= m_offers.size()) {
        return {};
    }
    return m_offers.at(row);
}

TaskModel::TaskModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

int TaskModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : m_tasks.size();
}

QVariant TaskModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_tasks.size()) {
        return {};
    }
    const TaskItem &task = m_tasks.at(index.row());
    switch (role) {
    case IdRole:
        return task.id;
    case TitleRole:
        return task.title;
    case TargetRole:
        return task.target;
    case KindRole:
        return taskKindName(task.kind);
    case StateRole:
        switch (task.state) {
        case TaskState::Queued:
            return QStringLiteral("queued");
        case TaskState::Running:
            return QStringLiteral("running");
        case TaskState::Cancelling:
            return QStringLiteral("cancelling");
        case TaskState::Succeeded:
            return QStringLiteral("succeeded");
        case TaskState::Failed:
            return QStringLiteral("failed");
        case TaskState::Cancelled:
            return QStringLiteral("cancelled");
        }
        return {};
    case ProgressRole:
        return task.progress;
    case StatusRole:
        return task.statusText;
    case ErrorRole:
        return task.error;
    case RetryableRole:
        return task.retryable;
    default:
        return {};
    }
}

QHash<int, QByteArray> TaskModel::roleNames() const
{
    return {{IdRole, "id"},
            {TitleRole, "title"},
            {TargetRole, "target"},
            {KindRole, "kind"},
            {StateRole, "state"},
            {ProgressRole, "progress"},
            {StatusRole, "statusText"},
            {ErrorRole, "error"},
            {RetryableRole, "retryable"}};
}

void TaskModel::setTasks(const QVector<TaskItem> &tasks)
{
    beginResetModel();
    m_tasks = tasks;
    std::reverse(m_tasks.begin(), m_tasks.end());
    endResetModel();
    Q_EMIT countChanged();
}

CandidateModel::CandidateModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

int CandidateModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : m_candidates.size();
}

QVariant CandidateModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_candidates.size()) {
        return {};
    }
    const InspectionResult &item = m_candidates.at(index.row());
    switch (role) {
    case PathRole:
        return item.identity.path;
    case NameRole:
        return item.metadata.name;
    case VersionRole:
        return item.metadata.version;
    case SizeRole:
        return item.identity.size;
    case TypeRole:
        return appImageTypeName(item.type);
    case ArchRole:
        return architectureName(item.architecture);
    case CommentRole:
        return item.metadata.comment;
    case AlreadyManagedRole:
        return item.alreadyManaged;
    case WarningRole:
        return item.warnings.join(QStringLiteral("\n"));
    case ErrorRole:
        return item.error;
    case IconRole:
        return item.metadata.extractedIconPath;
    case TerminalRole:
        return item.metadata.terminal;
    case UpdateSourceRole:
        return item.updateInfo.raw;
    case PlannedTargetRole:
        return item.plannedTarget;
    case CopyOutcomeRole:
        return item.copyOutcome;
    case ConflictStatusRole:
        return item.conflictStatus;
    case ConflictingUuidRole:
        return item.conflictingUuid;
    case ConflictingNameRole:
        return item.conflictingName;
    case NeedsDecisionRole:
        return item.needsConflictDecision;
    case ExistingManagedRole:
        return item.existingManagedId;
    default:
        return {};
    }
}

QHash<int, QByteArray> CandidateModel::roleNames() const
{
    return {{PathRole, "path"},
            {NameRole, "name"},
            {VersionRole, "version"},
            {SizeRole, "size"},
            {TypeRole, "appImageType"},
            {ArchRole, "architecture"},
            {CommentRole, "comment"},
            {AlreadyManagedRole, "alreadyManaged"},
            {WarningRole, "warnings"},
            {ErrorRole, "error"},
            {IconRole, "iconPath"},
            {TerminalRole, "terminal"},
            {UpdateSourceRole, "updateSource"},
            {PlannedTargetRole, "plannedTarget"},
            {CopyOutcomeRole, "copyOutcome"},
            {ConflictStatusRole, "conflictStatus"},
            {ConflictingUuidRole, "conflictingUuid"},
            {ConflictingNameRole, "conflictingName"},
            {NeedsDecisionRole, "needsDecision"},
            {ExistingManagedRole, "existingManagedId"}};
}

void CandidateModel::setCandidates(const QVector<InspectionResult> &candidates)
{
    beginResetModel();
    m_candidates = candidates;
    endResetModel();
    Q_EMIT countChanged();
}

} // namespace GoshAim
