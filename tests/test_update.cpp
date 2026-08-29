#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "core/AppImageInspector.h"
#include "core/DesktopIntegration.h"
#include "core/ManagedRegistry.h"
#include "core/ProcessTable.h"
#include "core/SettingsStore.h"
#include "core/UpdateService.h"
#include "core/UpdateSources.h"
#include "core/UrlGuard.h"

#include <QStandardPaths>
#include <QTemporaryDir>
#include <QtTest>

using namespace GoshAim;

class TestUpdate : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void initTestCase() { QStandardPaths::setTestModeEnabled(true); }
    void rejectsFileUrl()
    {
        QVERIFY(!UrlGuard::validate(QStringLiteral("file:///etc/passwd")).ok);
    }
    void rejectsCredentials()
    {
        QVERIFY(!UrlGuard::validate(QStringLiteral("https://user:pass@example.com/a")).ok);
    }
    void rejectsPrivate()
    {
        QVERIFY(!UrlGuard::validate(QStringLiteral("https://127.0.0.1/x")).ok);
        QVERIFY(!UrlGuard::validate(QStringLiteral("https://192.168.1.5/x")).ok);
    }
    void githubConfig()
    {
        GitHubSource source;
        QVariantMap bad;
        bad.insert(QStringLiteral("username"), QStringLiteral("../x"));
        QString error;
        QVERIFY(!source.validateConfig(bad, &error));
        QVariantMap ok;
        ok.insert(QStringLiteral("username"), QStringLiteral("user"));
        ok.insert(QStringLiteral("repo"), QStringLiteral("repo"));
        QVERIFY(source.validateConfig(ok, &error));
    }
    void ftpWarned()
    {
        FtpSource source;
        QVariantMap cfg;
        cfg.insert(QStringLiteral("url"), QStringLiteral("ftp://example.com/a.AppImage"));
        QString error;
        QVERIFY(source.validateConfig(cfg, &error));
        QVERIFY(error.contains(QLatin1String("insecure")));
    }
    void applyValidatesAndRollsBack()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable processes;
        processes.running = {1};
        processes.matchPath = home.path() + QStringLiteral("/AppImages/Demo.AppImage");
        AppImageInspector inspector(&runner, &settings, &registry);
        DesktopIntegration desktop(&settings, &runner);
        UpdateService updates(&settings, &registry, &inspector, &desktop, &network, &processes, &runner);
        InstalledApp app;
        app.uuid = QStringLiteral("abc");
        app.owned = true;
        app.managedPath = home.path() + QStringLiteral("/AppImages/Demo.AppImage");
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        QDir().mkpath(QFileInfo(app.managedPath).absolutePath());
        TestFixt::writeFile(QFileInfo(app.managedPath).absolutePath(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        const IntegrateResult blocked = updates.apply(app, false);
        QVERIFY(!blocked.ok);
        QVERIFY(blocked.error.contains(QLatin1String("running")));
        QVERIFY(QFile::exists(app.managedPath));
    }
    void githubCheckUsesApi()
    {
        FakeNetworkClient network;
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("api.github.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = QByteArrayLiteral("{\"tag_name\":\"v2\",\"assets\":[{\"name\":\"App.AppImage\",\"browser_download_url\":\"https://github.com/u/r/releases/download/v2/App.AppImage\",\"size\":12}]}");
        network.rules.append(rule);
        GitHubSource source;
        InstalledApp app;
        app.updateConfig.insert(QStringLiteral("username"), QStringLiteral("u"));
        app.updateConfig.insert(QStringLiteral("repo"), QStringLiteral("r"));
        app.updateConfig.insert(QStringLiteral("filename"), QStringLiteral("App.AppImage"));
        const UpdateCheckResult result = source.check(app, &network, nullptr);
        QVERIFY(result.ok);
        QCOMPARE(result.version, QStringLiteral("v2"));
        QVERIFY(network.urls.first().host() == QLatin1String("api.github.com"));
    }
};

QTEST_GUILESS_MAIN(TestUpdate)
#include "test_update.moc"
