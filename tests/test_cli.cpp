#include "AppController.h"
#include "Cli.h"
#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "core/ManagedRegistry.h"
#include "core/ProcessTable.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QtTest>
#include <fcntl.h>
#include <unistd.h>

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
    void listInstalledJsonSchema()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        InstalledApp app;
        app.uuid = QStringLiteral("cli-1");
        app.name = QStringLiteral("Demo");
        app.managedPath = home.path() + QStringLiteral("/Demo.AppImage");
        app.desktopId = QStringLiteral("gosh-appimage-cli-1.desktop");
        app.version = QStringLiteral("1.0");
        app.owned = true;
        controller.registry()->upsert(app);
        int fds[2];
        QVERIFY(pipe(fds) == 0);
        const int saved = dup(STDOUT_FILENO);
        QVERIFY(saved >= 0);
        dup2(fds[1], STDOUT_FILENO);
        const int code = runCli(controller, {QStringLiteral("--list-installed"), QStringLiteral("--json")}, false);
        fflush(stdout);
        dup2(saved, STDOUT_FILENO);
        close(saved);
        close(fds[1]);
        QByteArray captured;
        char buf[4096];
        ssize_t n;
        while ((n = read(fds[0], buf, sizeof(buf))) > 0) {
            captured.append(buf, int(n));
        }
        close(fds[0]);
        QCOMPARE(code, 0);
        QJsonParseError parseError;
        const QJsonDocument doc = QJsonDocument::fromJson(captured.trimmed(), &parseError);
        QVERIFY2(doc.isObject(), captured.constData());
        const QJsonObject root = doc.object();
        QCOMPARE(root.value(QStringLiteral("schema_version")).toInt(), 1);
        QVERIFY(root.contains(QStringLiteral("installed")));
        QVERIFY(!root.contains(QStringLiteral("items")));
        const QJsonObject row = root.value(QStringLiteral("installed")).toArray().at(0).toObject();
        QVERIFY(row.value(QStringLiteral("name")).isString());
        QVERIFY(row.value(QStringLiteral("path")).isString());
        QVERIFY(row.value(QStringLiteral("desktop_id")).isString());
        QVERIFY(row.value(QStringLiteral("current_version")).isString());
        QVERIFY(row.value(QStringLiteral("available_version")).isString());
        QVERIFY(row.value(QStringLiteral("download_size")).isDouble() || row.value(QStringLiteral("download_size")).isNull());
        QVERIFY(row.value(QStringLiteral("manager")).isString());
        QVERIFY(row.value(QStringLiteral("embedded_source")).isString());
        QVERIFY(row.value(QStringLiteral("running")).isBool());
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
    void replaceOwnedSucceedsUnownedRefused()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        QCOMPARE(runCli(controller, {QStringLiteral("--integrate"), path, QStringLiteral("--yes")}, false), 0);
        const QString uuid = controller.registry()->apps().first().uuid;
        const QString next = TestFixt::writeFile(home.path(), QStringLiteral("Next.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(4, 'N')));
        QCOMPARE(runCli(controller,
                        {QStringLiteral("--integrate"), next, QStringLiteral("--replace"), QStringLiteral("--replace-uuid"), uuid, QStringLiteral("--yes")},
                        false),
                 0);
        QCOMPARE(runCli(controller,
                        {QStringLiteral("--integrate"), next, QStringLiteral("--replace"), QStringLiteral("--replace-uuid"), QStringLiteral("missing"), QStringLiteral("--yes")},
                        false),
                 int(ExitCode::NotIntegrated));
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
