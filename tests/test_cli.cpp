#include "AppController.h"
#include "Cli.h"
#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "core/ManagedRegistry.h"
#include "core/ProcessTable.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QtTest>

using namespace GoshAim;

class TestCli : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void initTestCase() { QStandardPaths::setTestModeEnabled(true); }
    void listManagers()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        QCOMPARE(runCli(controller, {QStringLiteral("--list-update-managers")}, false), 0);
    }
    void listInstalledJson()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        QCOMPARE(runCli(controller, {QStringLiteral("--list-installed"), QStringLiteral("--json")}, false), 0);
    }
    void integrateNeedsYesWithoutTty()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        QCOMPARE(runCli(controller, {QStringLiteral("--integrate"), path}, false), int(ExitCode::NeedsConfirmation));
    }
    void unknownCommand()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        QCOMPARE(runCli(controller, {QStringLiteral("--nope")}, false), int(ExitCode::Usage));
    }
};

QTEST_GUILESS_MAIN(TestCli)
#include "test_cli.moc"
