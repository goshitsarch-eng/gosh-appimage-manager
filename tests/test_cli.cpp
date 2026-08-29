#include "AppController.h"
#include "Cli.h"
#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "core/ManagedRegistry.h"
#include "core/ProcessTable.h"
#include "core/UpdateNotifier.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QDir>
#include <QFile>
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
    void secondIntegrateRequiresExplicitKeepBoth()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        QCOMPARE(runCli(controller, {QStringLiteral("--integrate"), path, QStringLiteral("--yes")}, false), 0);
        const QString same = TestFixt::writeFile(home.path() + QStringLiteral("/more"), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(4, '2')));
        const int code = runCli(controller, {QStringLiteral("--integrate"), same, QStringLiteral("--yes")}, false);
        QCOMPARE(code, int(ExitCode::Validation));
        const QDir managed(controller.settings()->managedFolder());
        const QStringList leftover = managed.entryList(QStringList{QStringLiteral("*-2.AppImage")}, QDir::Files);
        QVERIFY(leftover.isEmpty());
    }
    void listUpdatesJsonSchema()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        int fds[2];
        int errfds[2];
        QVERIFY(pipe(fds) == 0);
        QVERIFY(pipe(errfds) == 0);
        const int savedOut = dup(STDOUT_FILENO);
        const int savedErr = dup(STDERR_FILENO);
        dup2(fds[1], STDOUT_FILENO);
        dup2(errfds[1], STDERR_FILENO);
        const int code = runCli(controller, {QStringLiteral("--list-updates"), QStringLiteral("--json")}, false);
        fflush(stdout);
        fflush(stderr);
        dup2(savedOut, STDOUT_FILENO);
        dup2(savedErr, STDERR_FILENO);
        close(savedOut);
        close(savedErr);
        close(fds[1]);
        close(errfds[1]);
        QByteArray captured;
        QByteArray capturedErr;
        char buf[4096];
        ssize_t n;
        while ((n = read(fds[0], buf, sizeof(buf))) > 0) {
            captured.append(buf, int(n));
        }
        while ((n = read(errfds[0], buf, sizeof(buf))) > 0) {
            capturedErr.append(buf, int(n));
        }
        close(fds[0]);
        close(errfds[0]);
        QCOMPARE(code, 0);
        QJsonParseError parseError;
        const QJsonDocument doc = QJsonDocument::fromJson(captured.trimmed(), &parseError);
        QVERIFY2(doc.isObject(), captured.constData());
        const QJsonObject root = doc.object();
        QCOMPARE(root.value(QStringLiteral("schema_version")).toInt(), 1);
        QVERIFY(root.contains(QStringLiteral("updates")));
        QVERIFY(!root.contains(QStringLiteral("items")));
        QVERIFY(root.value(QStringLiteral("updates")).isArray());
        QVERIFY(!QString::fromUtf8(captured).contains(QString::fromUtf8(capturedErr)) || capturedErr.isEmpty());
    }
    void fetchUpdatesNotifiesOnceAndDoesNotMutate()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        RecordingUpdateNotifier notifier;
        AppController controller(nullptr, &runner, &network, &table, true, &notifier);
        InstalledApp app;
        app.uuid = QStringLiteral("fetch");
        app.owned = true;
        app.name = QStringLiteral("Demo");
        app.managedPath = home.path() + QStringLiteral("/Demo.AppImage");
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        controller.registry()->upsert(app);
        controller.registry()->save();
        const QByteArray before = [&]() {
            QFile file(controller.settings()->dataDir() + QStringLiteral("/registry.json"));
            if (!file.open(QIODevice::ReadOnly)) {
                return QByteArray();
            }
            return file.readAll();
        }();
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.etag = QStringLiteral("\"new\"");
        rule.result.contentLength = 99;
        network.rules.append(rule);
        QCOMPARE(runCli(controller, {QStringLiteral("--fetch-updates")}, false), 0);
        QCOMPARE(notifier.calls, 1);
        QCOMPARE(notifier.lastCount, 1);
        QFile file(controller.settings()->dataDir() + QStringLiteral("/registry.json"));
        QByteArray after;
        if (file.open(QIODevice::ReadOnly)) {
            after = file.readAll();
        }
        QCOMPARE(after, before);
        QVERIFY(network.requests.isEmpty() || network.requests.last().destinationPath.isEmpty());
    }
    void fetchUpdatesZeroOffersDoesNotNotify()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        RecordingUpdateNotifier notifier;
        AppController controller(nullptr, &runner, &network, &table, true, &notifier);
        QCOMPARE(runCli(controller, {QStringLiteral("--fetch-updates")}, false), 0);
        QCOMPARE(notifier.calls, 0);
    }
    void updateAllAfterListUpdatesReplacesOnce()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(16, 'N'));
        QDir().mkpath(controller.settings()->managedFolder());
        const QString dest = TestFixt::writeFile(controller.settings()->managedFolder(), QStringLiteral("Demo.AppImage"), original);
        InstalledApp app;
        app.uuid = QStringLiteral("cli-upd");
        app.owned = true;
        app.name = QStringLiteral("Demo");
        app.managedPath = dest;
        app.desktopPath = controller.settings()->applicationsDir() + QStringLiteral("/gosh-appimage-cli-upd.desktop");
        app.architecture = Architecture::X86_64;
        app.size = original.size();
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        controller.registry()->upsert(app);
        controller.registry()->save();
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = next;
        rule.result.contentLength = next.size();
        network.rules.append(rule);
        QCOMPARE(runCli(controller, {QStringLiteral("--list-updates")}, false), 0);
        QCOMPARE(runCli(controller, {QStringLiteral("--update"), QStringLiteral("--all"), QStringLiteral("--yes")}, false), 0);
        QFile live(dest);
        QVERIFY(live.open(QIODevice::ReadOnly));
        QCOMPARE(live.readAll(), next);
        live.close();
        QCOMPARE(runCli(controller, {QStringLiteral("--update"), QStringLiteral("--all"), QStringLiteral("--yes")}, false), 0);
        QFile still(dest);
        QVERIFY(still.open(QIODevice::ReadOnly));
        QCOMPARE(still.readAll(), next);
    }
};

QTEST_GUILESS_MAIN(TestCli)
#include "test_cli.moc"
