#include "core/TaskQueue.h"

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
    }
    void cancelQueued()
    {
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
    }
    void shutdownJoinSafe()
    {
        auto *queue = new TaskQueue;
        queue->enqueue(TaskKind::Inspect, QStringLiteral("x"), QStringLiteral("/tmp/z"),
                       [](TaskItem &, std::atomic<bool> *) { QThread::msleep(20); }, false);
        delete queue;
    }
};

QTEST_GUILESS_MAIN(TestTasks)
#include "test_tasks.moc"
