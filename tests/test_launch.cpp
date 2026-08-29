#include "FakeSeams.h"
#include "core/ProcessTable.h"
#include "core/RemovalLaunch.h"
#include "core/Types.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
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
        const QString path = QDir::tempPath() + QStringLiteral("/gosh-aim-running.AppImage");
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly));
        file.write("x");
        file.close();
        table.running = {1234};
        table.matchPath = QFileInfo(path).canonicalFilePath();
        LaunchService launch(&runner, &table);
        InstalledApp app;
        app.managedPath = path;
        QVERIFY(launch.isRunning(app));
        QCOMPARE(table.pidsForExecutable(table.matchPath).size(), 1);
        QFile::remove(path);
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
