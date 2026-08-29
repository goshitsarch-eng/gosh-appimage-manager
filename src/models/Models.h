#pragma once

#include "core/Types.h"

#include <QAbstractListModel>
#include <QVector>

namespace GoshAim {

class LibraryModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
public:
    enum Roles {
        UuidRole = Qt::UserRole + 1,
        NameRole,
        VersionRole,
        CommentRole,
        PathRole,
        DesktopIdRole,
        HashRole,
        TypeRole,
        ArchRole,
        SizeRole,
        UpdateAvailableRole,
        RunningRole,
        ExternalRole,
        OwnedRole,
        ManagerRole,
        IconRole,
        TerminalRole
    };
    explicit LibraryModel(QObject *parent = nullptr);
    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;
    void setApps(const QVector<InstalledApp> &apps);
    void setFilter(const QString &filter);
    void setSort(const QString &sort);
    InstalledApp at(int row) const;
    InstalledApp byUuid(const QString &uuid) const;
    QVector<InstalledApp> apps() const { return m_filtered; }

Q_SIGNALS:
    void countChanged();

private:
    void apply();
    QVector<InstalledApp> m_all;
    QVector<InstalledApp> m_filtered;
    QString m_filter;
    QString m_sort = QStringLiteral("name");
};

class UpdatesModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
public:
    enum Roles {
        UuidRole = Qt::UserRole + 1,
        NameRole,
        CurrentVersionRole,
        AvailableVersionRole,
        ManagerRole,
        SizeRole,
        RunningRole,
        ReducedRole,
        UrlRole
    };
    explicit UpdatesModel(QObject *parent = nullptr);
    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;
    void setOffers(const QVector<UpdateOffer> &offers);
    UpdateOffer at(int row) const;

Q_SIGNALS:
    void countChanged();

private:
    QVector<UpdateOffer> m_offers;
};

class TaskModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
public:
    enum Roles {
        IdRole = Qt::UserRole + 1,
        TitleRole,
        TargetRole,
        KindRole,
        StateRole,
        ProgressRole,
        StatusRole,
        ErrorRole,
        RetryableRole
    };
    explicit TaskModel(QObject *parent = nullptr);
    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;
    void setTasks(const QVector<TaskItem> &tasks);

Q_SIGNALS:
    void countChanged();

private:
    QVector<TaskItem> m_tasks;
};

class CandidateModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
public:
    enum Roles {
        PathRole = Qt::UserRole + 1,
        NameRole,
        VersionRole,
        SizeRole,
        TypeRole,
        ArchRole,
        CommentRole,
        AlreadyManagedRole,
        WarningRole,
        ErrorRole,
        IconRole,
        TerminalRole,
        UpdateSourceRole,
        PlannedTargetRole,
        CopyOutcomeRole,
        ConflictStatusRole,
        ConflictingUuidRole,
        ConflictingNameRole,
        NeedsDecisionRole,
        CanReplaceRole,
        ExistingManagedRole,
        ChosenPolicyRole,
        ChosenReplaceUuidRole
    };
    explicit CandidateModel(QObject *parent = nullptr);
    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;
    void setCandidates(const QVector<InspectionResult> &candidates);
    QVector<InspectionResult> candidates() const { return m_candidates; }

Q_SIGNALS:
    void countChanged();

private:
    QVector<InspectionResult> m_candidates;
};

} // namespace GoshAim
