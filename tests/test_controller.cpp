#include "AppController.h"
#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "QtWarnGuard.h"
#include "core/ProcessTable.h"
#include "core/TaskQueue.h"

#include <QElapsedTimer>
#include <QSignalSpy>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QtTest>

using namespace GoshAim;

class TestController : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void initTestCase() { QStandardPaths::setTestModeEnabled(true); }
    void inspectDoesNotBlockAndCanCancel()
    {
        QtWarnGuard guard;
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        QSignalSpy spy(&controller, &AppController::confirmInspect);
        QElapsedTimer timer;
        timer.start();
        controller.inspectPaths({path});
        QVERIFY(timer.elapsed() < 500);
        QTRY_VERIFY_WITH_TIMEOUT(spy.count() >= 1 || !controller.inspecting(), 5000);
        controller.cancelInspect();
        QVERIFY(!guard.sawLiveDestruction());
    }
    void checkAllDoesNotUseEmptyUuid()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        controller.checkAll();
        QTRY_VERIFY(controller.taskQueue()->tasks().size() >= 1);
        QCOMPARE(controller.taskQueue()->tasks().last().target, QStringLiteral("*"));
        QVERIFY(controller.taskQueue()->tasks().last().target != QString());
    }
    void localPathFromUrl()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        QCOMPARE(controller.localPathFromUrl(QStringLiteral("file:///tmp/foo")), QStringLiteral("/tmp/foo"));
        QVERIFY(controller.localPathFromUrl(QStringLiteral("https://example.com")).isEmpty());
    }
    void conflictReplaceUuidPassed()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        controller.setCandidateConflict(0, 2, QStringLiteral("abc"));
        QVERIFY(controller.qmlActionNames().contains(QStringLiteral("confirmIntegrate")));
    }
};

QTEST_GUILESS_MAIN(TestController)
#include "test_controller.moc"
