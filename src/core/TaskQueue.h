#pragma once

#include "Types.h"

#include <QHash>
#include <QMutex>
#include <QObject>
#include <QThread>
#include <QWaitCondition>
#include <atomic>
#include <functional>

namespace GoshAim {

class TaskQueue : public QObject
{
    Q_OBJECT
public:
    using Job = std::function<void(TaskItem &, std::atomic<bool> *)>;

    explicit TaskQueue(QObject *parent = nullptr);
    ~TaskQueue() override;

    QString enqueue(TaskKind kind, const QString &title, const QString &target, Job job, bool mutation);
    void cancel(const QString &id);
    void cancelAll();
    void shutdown();
    void setProgress(const QString &id, int progress, const QString &statusText);
    QVector<TaskItem> tasks() const;
    TaskItem task(const QString &id) const;
    bool busy() const;

Q_SIGNALS:
    void tasksChanged();
    void taskChanged(const QString &id);
    void progressChanged(const QString &id, int progress);
    void finished(const QString &id, bool ok, const QString &error);

private:
    void workerLoop();
    QString nextId();
    struct JobItem {
        TaskItem task;
        Job job;
        bool mutation = false;
        std::atomic<bool> *cancel = nullptr;
    };
    mutable QMutex m_mutex;
    QWaitCondition m_cv;
    QVector<JobItem *> m_queue;
    QVector<TaskItem> m_history;
    QThread *m_thread = nullptr;
    std::atomic<bool> m_stop{false};
    std::atomic<bool> m_busy{false};
    std::atomic<bool> *m_currentCancel = nullptr;
    JobItem *m_currentItem = nullptr;
    QString m_currentId;
    QString m_currentTarget;
    int m_seq = 0;
    QHash<QString, qint64> m_lastProgressEmit;
};

} // namespace GoshAim
