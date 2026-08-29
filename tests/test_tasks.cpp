#include "core/TaskQueue.h"
#include "QtWarnGuard.h"

#include <QSignalSpy>
#include <QThread>
#include <QtTest>
#include <atomic>

using namespace GoshAim;

class TestTasks : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void serializesMutations()
    {
        QtWarnGuard guard;
        TaskQueue queue;
        std::atomic<int> ran{0};
        const QString a = queue.enqueue(TaskKind::Integrate, QStringLiteral("a"), QStringLiteral("/tmp/x"),
                                        [&](TaskItem &, std::atomic<bool> *) { ran.fetch_add(1); }, true);
        const QString b = queue.enqueue(TaskKind::Integrate, QStringLiteral("b"), QStringLiteral("/tmp/x"),
                                        [&](TaskItem &, std::atomic<bool> *) { ran.fetch_add(1); }, true);
        QVERIFY(!a.isEmpty());
        QVERIFY(b.isEmpty());
        QTRY_VERIFY(ran.load() >= 1);
        queue.shutdown();
        QVERIFY(!guard.sawLiveDestruction());
    }
    void cancelQueued()
    {
        QtWarnGuard guard;
        TaskQueue queue;
        std::atomic<bool> started{false};
        queue.enqueue(TaskKind::Inspect, QStringLiteral("hang"), QStringLiteral("/tmp/y"),
                      [&](TaskItem &, std::atomic<bool> *cancel) {
                          started.store(true);
                          while (!cancel->load()) {
                              QThread::msleep(5);
                          }
                      },
                      false);
        QTRY_VERIFY(started.load());
        queue.cancelAll();
        queue.shutdown();
        QVERIFY(!guard.sawLiveDestruction());
    }
    void shutdownJoinSafe()
    {
        QtWarnGuard guard;
        auto *queue = new TaskQueue;
        queue->enqueue(TaskKind::Inspect, QStringLiteral("x"), QStringLiteral("/tmp/z"),
                       [](TaskItem &, std::atomic<bool> *) { QThread::msleep(20); }, false);
        delete queue;
        QVERIFY(!guard.sawLiveDestruction());
    }
    void cancelQueuedDoesNotCancelRunning()
    {
        QtWarnGuard guard;
        TaskQueue queue;
        std::atomic<bool> aStarted{false};
        std::atomic<bool> aFinished{false};
        std::atomic<bool> proceed{false};
        const QString a = queue.enqueue(TaskKind::Inspect, QStringLiteral("A"), QStringLiteral("/tmp/a"),
                                        [&](TaskItem &task, std::atomic<bool> *cancel) {
                                            aStarted.store(true);
                                            while (!proceed.load() && !cancel->load()) {
                                                QThread::msleep(5);
                                            }
                                            if (!cancel->load()) {
                                                task.statusText = QStringLiteral("done");
                                            }
                                            aFinished.store(true);
                                        },
                                        false);
        QTRY_VERIFY(aStarted.load());
        const QString b = queue.enqueue(TaskKind::Inspect, QStringLiteral("B"), QStringLiteral("/tmp/b"),
                                        [&](TaskItem &, std::atomic<bool> *) {}, false);
        QVERIFY(!b.isEmpty());
        queue.cancel(b);
        QVERIFY(aStarted.load());
        QCOMPARE(queue.task(a).state, TaskState::Running);
        proceed.store(true);
        QTRY_VERIFY(aFinished.load());
        QTRY_COMPARE(queue.task(a).state, TaskState::Succeeded);
        QTRY_COMPARE(queue.task(b).state, TaskState::Cancelled);
        queue.shutdown();
        QVERIFY(!guard.sawLiveDestruction());
    }
    void publishesProgressBeforeFinish()
    {
        QtWarnGuard guard;
        TaskQueue queue;
        std::atomic<bool> started{false};
        std::atomic<bool> hold{true};
        QString id;
        id = queue.enqueue(TaskKind::Update, QStringLiteral("upd"), QStringLiteral("/tmp/p"),
                           [&](TaskItem &task, std::atomic<bool> *) {
                               started.store(true);
                               queue.setProgress(task.id, 40, QStringLiteral("Downloading"));
                               while (hold.load()) {
                                   QThread::msleep(5);
                               }
                           },
                           true);
        QTRY_VERIFY(started.load());
        QTRY_COMPARE(queue.task(id).progress, 40);
        QVERIFY(queue.task(id).progress > 0 && queue.task(id).progress < 100);
        QCOMPARE(queue.task(id).statusText, QStringLiteral("Downloading"));
        hold.store(false);
        QTRY_COMPARE(queue.task(id).state, TaskState::Succeeded);
        queue.shutdown();
        QVERIFY(!guard.sawLiveDestruction());
    }
};

QTEST_GUILESS_MAIN(TestTasks)
#include "test_tasks.moc"
