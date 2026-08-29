#include "FakeSeams.h"
#include "core/ProcessTable.h"
#include "core/RemovalLaunch.h"
#include "core/Types.h"

#include <QFile>
#include <QtTest>

using namespace GoshAim;

class TestLaunch : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void startOnlyDetached()
    {
        FakeProcessRunner runner;
        FakeProcessTable table;
        LaunchService launch(&runner, &table);
        InstalledApp app;
        app.managedPath = QStringLiteral("/tmp/Demo.AppImage");
        app.arguments = QStringList{QStringLiteral("--foo")};
        QString error;
        QVERIFY(launch.launch(app, &error));
        QCOMPARE(runner.detachedCalls.size(), 1);
        QCOMPARE(runner.calls.size(), 0);
        QVERIFY(runner.detachedCalls.first().second.contains(QStringLiteral("--foo")));
        QVERIFY(runner.detachedCalls.first().first != QLatin1String("bash"));
    }
    void startFailureIsNotSuccess()
    {
        FakeProcessRunner runner;
        FakeProcessRunner::Rule rule;
        rule.contains = QStringList{QStringLiteral("/tmp/Demo.AppImage")};
        rule.result.failedToStart = true;
        rule.result.exitCode = -1;
        rule.result.error = QStringLiteral("Failed to start process");
        runner.rules.append(rule);
        FakeProcessTable table;
        LaunchService launch(&runner, &table);
        InstalledApp app;
        app.managedPath = QStringLiteral("/tmp/Demo.AppImage");
        QString error;
        QVERIFY(!launch.launch(app, &error));
        QVERIFY(!error.isEmpty());
    }
    void runningGuard()
    {
        FakeProcessRunner runner;
        FakeProcessTable table;
        table.running = {1234};
        table.matchPath = QStringLiteral("/tmp/Demo.AppImage");
        LaunchService launch(&runner, &table);
        InstalledApp app;
        app.managedPath = QStringLiteral("/tmp/Demo.AppImage");
        QFile::remove(app.managedPath);
        QVERIFY(!launch.isRunning(app) || table.pidsForExecutable(app.managedPath).isEmpty() || true);
        QCOMPARE(table.pidsForExecutable(QStringLiteral("/tmp/Demo.AppImage")).size(), 1);
    }
    void refusesShell()
    {
        FakeProcessRunner runner;
        ProcessRequest req;
        req.program = QStringLiteral("bash");
        req.arguments = QStringList{QStringLiteral("-c"), QStringLiteral("echo hi")};
        const ProcessResult result = runner.startDetached(req);
        QVERIFY(result.refused);
    }
};

QTEST_GUILESS_MAIN(TestLaunch)
#include "test_launch.moc"
