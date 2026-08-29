#include "TaskQueue.h"

#include <QMetaObject>

namespace GoshAim {

TaskQueue::TaskQueue(QObject *parent)
    : QObject(parent)
{
    m_thread = QThread::create([this]() { workerLoop(); });
    m_thread->setObjectName(QStringLiteral("GoshAimWorker"));
    m_thread->start();
}

TaskQueue::~TaskQueue()
{
    shutdown();
}

void TaskQueue::shutdown()
{
    const bool already = m_stop.exchange(true);
    cancelAll();
    m_cv.wakeAll();
    if (m_thread) {
        if (!m_thread->wait(30000)) {
            m_thread->wait();
        }
        if (m_thread->isRunning()) {
            QObject::connect(m_thread, &QThread::finished, m_thread, &QObject::deleteLater);
            m_thread->setParent(nullptr);
            m_thread = nullptr;
        } else {
            delete m_thread;
            m_thread = nullptr;
        }
    }
    if (already) {
        return;
    }
    QMutexLocker locker(&m_mutex);
    qDeleteAll(m_queue);
    m_queue.clear();
}

QString TaskQueue::nextId()
{
    return QStringLiteral("task-%1").arg(++m_seq);
}

QString TaskQueue::enqueue(TaskKind kind, const QString &title, const QString &target, Job job, bool mutation)
{
    QMutexLocker locker(&m_mutex);
    if (m_stop.load()) {
        return {};
    }
    if (mutation) {
        for (JobItem *item : m_queue) {
            if (item->mutation && item->task.target == target
                && (item->task.state == TaskState::Queued || item->task.state == TaskState::Running)) {
                return {};
            }
        }
        if (m_busy.load() && m_currentTarget == target) {
            return {};
        }
    }
    auto *item = new JobItem;
    item->job = std::move(job);
    item->mutation = mutation;
    item->task.id = nextId();
    item->task.kind = kind;
    item->task.title = title;
    item->task.target = target;
    item->task.state = TaskState::Queued;
    item->task.statusText = QStringLiteral("Queued");
    item->task.createdAt = QDateTime::currentDateTimeUtc();
    item->cancel = new std::atomic<bool>(false);
    m_queue.append(item);
    m_history.append(item->task);
    locker.unlock();
    m_cv.wakeOne();
    Q_EMIT tasksChanged();
    return item->task.id;
}

void TaskQueue::cancel(const QString &id)
{
    if (id.isEmpty()) {
        return;
    }
    QMutexLocker locker(&m_mutex);
    for (JobItem *item : m_queue) {
        if (item->task.id == id) {
            item->cancel->store(true);
            if (item->task.state == TaskState::Queued) {
                item->task.state = TaskState::Cancelled;
            } else {
                item->task.state = TaskState::Cancelling;
            }
        }
    }
    if (m_currentId == id && m_currentCancel) {
        m_currentCancel->store(true);
    }
    for (TaskItem &hist : m_history) {
        if (hist.id != id) {
            continue;
        }
        if (hist.state == TaskState::Queued) {
            hist.state = TaskState::Cancelled;
            hist.statusText = QStringLiteral("Cancelled");
        } else if (hist.state == TaskState::Running) {
            hist.state = TaskState::Cancelling;
            hist.statusText = QStringLiteral("Cancelling");
        }
    }
}

void TaskQueue::cancelAll()
{
    QMutexLocker locker(&m_mutex);
    for (JobItem *item : m_queue) {
        item->cancel->store(true);
    }
    if (m_currentCancel) {
        m_currentCancel->store(true);
    }
}

QVector<TaskItem> TaskQueue::tasks() const
{
    QMutexLocker locker(&m_mutex);
    return m_history;
}

TaskItem TaskQueue::task(const QString &id) const
{
    QMutexLocker locker(&m_mutex);
    for (const TaskItem &item : m_history) {
        if (item.id == id) {
            return item;
        }
    }
    return {};
}

bool TaskQueue::busy() const
{
    return m_busy.load();
}

void TaskQueue::workerLoop()
{
    while (!m_stop.load()) {
        JobItem *item = nullptr;
        {
            QMutexLocker locker(&m_mutex);
            while (m_queue.isEmpty() && !m_stop.load()) {
                m_cv.wait(&m_mutex, 200);
            }
            if (m_stop.load()) {
                break;
            }
            if (m_queue.isEmpty()) {
                continue;
            }
            item = m_queue.takeFirst();
            m_busy.store(true);
            m_currentCancel = item->cancel;
            m_currentId = item->task.id;
            m_currentTarget = item->task.target;
            item->task.state = TaskState::Running;
            item->task.statusText = QStringLiteral("Running");
            for (TaskItem &hist : m_history) {
                if (hist.id == item->task.id) {
                    hist = item->task;
                }
            }
        }
        Q_EMIT taskChanged(item->task.id);
        if (item->cancel->load()) {
            item->task.state = TaskState::Cancelled;
        } else {
            item->job(item->task, item->cancel);
            if (item->cancel->load() && item->task.state == TaskState::Running) {
                item->task.state = TaskState::Cancelled;
            } else if (item->task.state == TaskState::Running) {
                item->task.state = item->task.error.isEmpty() ? TaskState::Succeeded : TaskState::Failed;
            }
        }
        item->task.finishedAt = QDateTime::currentDateTimeUtc();
        {
            QMutexLocker locker(&m_mutex);
            for (TaskItem &hist : m_history) {
                if (hist.id == item->task.id) {
                    hist = item->task;
                }
            }
            m_busy.store(false);
            m_currentCancel = nullptr;
            m_currentId.clear();
            m_currentTarget.clear();
        }
        const bool ok = item->task.state == TaskState::Succeeded;
        const QString error = item->task.error;
        const QString id = item->task.id;
        delete item->cancel;
        delete item;
        Q_EMIT finished(id, ok, error);
        Q_EMIT tasksChanged();
        if (m_stop.load()) {
            break;
        }
    }
}

} // namespace GoshAim
