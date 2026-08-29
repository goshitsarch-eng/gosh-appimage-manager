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
#include "core/SafeFs.h"

#include <QCryptographicHash>
#include <QDir>
#include <QFileInfo>
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
    void wrongDigestLeavesInstall()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        QStandardPaths::setTestModeEnabled(true);
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable processes;
        AppImageInspector inspector(&runner, &settings, &registry);
        DesktopIntegration desktop(&settings, &runner);
        UpdateService updates(&settings, &registry, &inspector, &desktop, &network, &processes, &runner);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        QDir().mkpath(settings.managedFolder());
        const QString dest = TestFixt::writeFile(settings.managedFolder(), QStringLiteral("Demo.AppImage"), original);
        InstalledApp app;
        app.uuid = QStringLiteral("digest");
        app.owned = true;
        app.name = QStringLiteral("Demo");
        app.managedPath = dest;
        app.desktopPath = settings.applicationsDir() + QStringLiteral("/gosh-appimage-digest.desktop");
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        app.arguments = QStringList{QStringLiteral("--kept")};
        registry.upsert(app);
        registry.save();
        FakeNetworkClient::Rule check;
        check.hostContains = QStringLiteral("example.com");
        check.result.ok = true;
        check.result.status = 200;
        check.result.etag = QStringLiteral("\"abc\"");
        check.result.contentLength = 12;
        check.result.body = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'N'));
        network.rules.append(check);
        network.defaultResult = check.result;
        UpdateCheckResult advertised;
        Q_UNUSED(advertised);
        // First check will set available; apply downloads body. Inject digest mismatch via static url plus digest on check result by using github-like digest on static through apply's check().
        // Static check uses etag as digest. Make apply compare against a wrong digest by putting digest in check-state after a custom source isn't easy.
        // Use GitHub manager with digest field.
        app.updateManager = QStringLiteral("github");
        app.updateConfig = {{QStringLiteral("username"), QStringLiteral("u")}, {QStringLiteral("repo"), QStringLiteral("r")}, {QStringLiteral("filename"), QStringLiteral("App.AppImage")}};
        registry.upsert(app);
        FakeNetworkClient::Rule api;
        api.hostContains = QStringLiteral("api.github.com");
        api.result.ok = true;
        api.result.status = 200;
        api.result.body = QByteArrayLiteral("{\"tag_name\":\"v9\",\"assets\":[{\"name\":\"App.AppImage\",\"browser_download_url\":\"https://github.com/u/r/releases/download/v9/App.AppImage\",\"size\":12,\"digest\":\"sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef\"}]}");
        network.rules.append(api);
        FakeNetworkClient::Rule dl;
        dl.hostContains = QStringLiteral("github.com");
        dl.result.ok = true;
        dl.result.status = 200;
        dl.result.body = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'N'));
        network.rules.append(dl);
        const IntegrateResult result = updates.apply(app, true);
        QVERIFY(!result.ok);
        QVERIFY(result.error.contains(QLatin1String("digest")));
        QFile live(dest);
        QVERIFY(live.open(QIODevice::ReadOnly));
        QCOMPARE(live.readAll(), original);
        QCOMPARE(registry.byUuid(app.uuid).arguments, QStringList{QStringLiteral("--kept")});
    }
    void matchingDigestReplaces()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        QStandardPaths::setTestModeEnabled(true);
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable processes;
        AppImageInspector inspector(&runner, &settings, &registry);
        DesktopIntegration desktop(&settings, &runner);
        UpdateService updates(&settings, &registry, &inspector, &desktop, &network, &processes, &runner);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'N'));
        QDir().mkpath(settings.managedFolder());
        const QString dest = TestFixt::writeFile(settings.managedFolder(), QStringLiteral("Demo.AppImage"), original);
        InstalledApp app;
        app.uuid = QStringLiteral("okdigest");
        app.owned = true;
        app.name = QStringLiteral("Demo");
        app.managedPath = dest;
        app.desktopPath = settings.applicationsDir() + QStringLiteral("/gosh-appimage-okdigest.desktop");
        app.architecture = Architecture::X86_64;
        app.updateManager = QStringLiteral("github");
        app.updateConfig = {{QStringLiteral("username"), QStringLiteral("u")}, {QStringLiteral("repo"), QStringLiteral("r")}, {QStringLiteral("filename"), QStringLiteral("App.AppImage")}};
        app.arguments = QStringList{QStringLiteral("--kept")};
        registry.upsert(app);
        const QByteArray digest = QCryptographicHash::hash(next, QCryptographicHash::Sha256);
        FakeNetworkClient::Rule api;
        api.hostContains = QStringLiteral("api.github.com");
        api.result.ok = true;
        api.result.status = 200;
        api.result.body = QByteArray("{\"tag_name\":\"v9\",\"assets\":[{\"name\":\"App.AppImage\",\"browser_download_url\":\"https://github.com/u/r/releases/download/v9/App.AppImage\",\"size\":12,\"digest\":\"sha256:")
            + digest.toHex() + "\"}]}";
        network.rules.append(api);
        FakeNetworkClient::Rule dl;
        dl.hostContains = QStringLiteral("github.com");
        dl.result.ok = true;
        dl.result.status = 200;
        dl.result.body = next;
        network.rules.append(dl);
        const IntegrateResult result = updates.apply(app, true);
        QVERIFY2(result.ok, qPrintable(result.error));
        QFile live(dest);
        QVERIFY(live.open(QIODevice::ReadOnly));
        QCOMPARE(live.readAll(), next);
        QCOMPARE(result.app.arguments, QStringList{QStringLiteral("--kept")});
    }
    void ftpAllowAndRefuse()
    {
        FakeNetworkClient network;
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.contentLength = 99;
        network.rules.append(rule);
        FtpSource source;
        InstalledApp app;
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("ftp://example.com/a.AppImage"));
        const UpdateCheckResult result = source.check(app, &network, nullptr);
        QVERIFY(result.ok);
        QVERIFY(!network.requests.isEmpty());
        QVERIFY(network.requests.last().allowFtp);
        NetworkRequest denied;
        denied.url = QUrl(QStringLiteral("ftp://example.com/a.AppImage"));
        denied.allowFtp = false;
        const NetworkResult blocked = network.fetch(denied);
        QVERIFY(!blocked.ok);
    }
    void staticUnchangedIsNotAvailable()
    {
        FakeNetworkClient network;
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.etag = QStringLiteral("\"same\"");
        rule.result.contentLength = 10;
        rule.result.lastModified = QStringLiteral("Wed, 01 Jan 2020 00:00:00 GMT");
        network.rules.append(rule);
        StaticFileSource source;
        InstalledApp app;
        app.size = 10;
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        const UpdateCheckResult first = source.check(app, &network, nullptr);
        QVERIFY(first.ok);
        QVERIFY(!first.available);
    }
    void updateRollbackOnDesktopFailure()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        QStandardPaths::setTestModeEnabled(true);
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable processes;
        AppImageInspector inspector(&runner, &settings, &registry);
        DesktopIntegration desktop(&settings, &runner);
        UpdateService updates(&settings, &registry, &inspector, &desktop, &network, &processes, &runner);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'N'));
        QDir().mkpath(settings.managedFolder());
        const QString dest = TestFixt::writeFile(settings.managedFolder(), QStringLiteral("Demo.AppImage"), original);
        InstalledApp app;
        app.uuid = QStringLiteral("roll");
        app.owned = true;
        app.managedPath = dest;
        app.desktopPath = settings.applicationsDir() + QStringLiteral("/gosh-appimage-roll.desktop");
        app.architecture = Architecture::X86_64;
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        registry.upsert(app);
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = next;
        rule.result.contentLength = next.size();
        rule.result.etag = QStringLiteral("\"new\"");
        network.rules.append(rule);
        updates.setFailPoint(UpdateFailPoint::DesktopInstall);
        const IntegrateResult result = updates.apply(app, true);
        QVERIFY(!result.ok);
        QFile live(dest);
        QVERIFY(live.open(QIODevice::ReadOnly));
        QCOMPARE(live.readAll(), original);
    }
    void applyRefusesUnavailable()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        QStandardPaths::setTestModeEnabled(true);
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable processes;
        AppImageInspector inspector(&runner, &settings, &registry);
        DesktopIntegration desktop(&settings, &runner);
        UpdateService updates(&settings, &registry, &inspector, &desktop, &network, &processes, &runner);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        QDir().mkpath(settings.managedFolder());
        const QString dest = TestFixt::writeFile(settings.managedFolder(), QStringLiteral("Demo.AppImage"), original);
        InstalledApp app;
        app.uuid = QStringLiteral("noavail");
        app.owned = true;
        app.managedPath = dest;
        app.size = original.size();
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        registry.upsert(app);
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.etag = QStringLiteral("\"same\"");
        rule.result.contentLength = original.size();
        rule.result.body = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'N'));
        network.rules.append(rule);
        const IntegrateResult result = updates.apply(app, true);
        QVERIFY(!result.ok);
        QVERIFY(result.error.contains(QLatin1String("No update")) || result.error.contains(QLatin1String("available")));
        QFile live(dest);
        QVERIFY(live.open(QIODevice::ReadOnly));
        QCOMPARE(live.readAll(), original);
    }
    void zsyncUnchangedIsNotAvailable()
    {
        QTemporaryDir home;
        FakeNetworkClient network;
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QString dest = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), original);
        const QByteArray sha1 = QCryptographicHash::hash(original, QCryptographicHash::Sha1);
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.pathContains = QStringLiteral(".zsync");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = QByteArrayLiteral("SHA-1: ") + sha1.toHex()
            + QByteArrayLiteral("\nLength: ") + QByteArray::number(original.size())
            + QByteArrayLiteral("\nURL: https://example.com/App.AppImage\n");
        network.rules.append(rule);
        StaticFileSource source;
        InstalledApp app;
        app.managedPath = dest;
        app.size = original.size();
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage.zsync"));
        const UpdateCheckResult result = source.check(app, &network, nullptr);
        QVERIFY(result.ok);
        QVERIFY(!result.available);
        QCOMPARE(result.digest, QString::fromLatin1(sha1.toHex()));
    }
    void zsyncChangedIsAvailable()
    {
        QTemporaryDir home;
        FakeNetworkClient network;
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QString dest = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), original);
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.pathContains = QStringLiteral(".zsync");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = QByteArrayLiteral("SHA-1: 1111111111111111111111111111111111111111\nLength: 99\nURL: https://example.com/App2.AppImage\n");
        network.rules.append(rule);
        StaticFileSource source;
        InstalledApp app;
        app.managedPath = dest;
        app.size = original.size();
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage.zsync"));
        const UpdateCheckResult result = source.check(app, &network, nullptr);
        QVERIFY(result.ok);
        QVERIFY(result.available);
        QCOMPARE(result.digest, QStringLiteral("1111111111111111111111111111111111111111"));
    }
    void ftpInvalidConfigFailsClosed()
    {
        FtpSource source;
        InstalledApp app;
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("ftp://user:pass@example.com/a.AppImage"));
        FakeNetworkClient network;
        const UpdateCheckResult result = source.check(app, &network, nullptr);
        QVERIFY(!result.ok);
        QVERIFY(network.urls.isEmpty());
    }
    void updateBackupFailureLeavesInstall()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        QStandardPaths::setTestModeEnabled(true);
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable processes;
        AppImageInspector inspector(&runner, &settings, &registry);
        DesktopIntegration desktop(&settings, &runner);
        UpdateService updates(&settings, &registry, &inspector, &desktop, &network, &processes, &runner);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'N'));
        QDir().mkpath(settings.managedFolder());
        const QString dest = TestFixt::writeFile(settings.managedFolder(), QStringLiteral("Demo.AppImage"), original);
        InstalledApp app;
        app.uuid = QStringLiteral("bak");
        app.owned = true;
        app.managedPath = dest;
        app.desktopPath = settings.applicationsDir() + QStringLiteral("/gosh-appimage-bak.desktop");
        app.architecture = Architecture::X86_64;
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        registry.upsert(app);
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = next;
        rule.result.contentLength = next.size();
        rule.result.etag = QStringLiteral("\"new\"");
        network.rules.append(rule);
        updates.setFailPoint(UpdateFailPoint::BackupCreate);
        const IntegrateResult result = updates.apply(app, true);
        QVERIFY(!result.ok);
        QFile live(dest);
        QVERIFY(live.open(QIODevice::ReadOnly));
        QCOMPARE(live.readAll(), original);
    }
    void staticCheckThenApplyReplacesOnce()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        QStandardPaths::setTestModeEnabled(true);
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable processes;
        AppImageInspector inspector(&runner, &settings, &registry);
        DesktopIntegration desktop(&settings, &runner);
        UpdateService updates(&settings, &registry, &inspector, &desktop, &network, &processes, &runner);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(16, 'N'));
        QDir().mkpath(settings.managedFolder());
        const QString dest = TestFixt::writeFile(settings.managedFolder(), QStringLiteral("Demo.AppImage"), original);
        InstalledApp app;
        app.uuid = QStringLiteral("static-check-apply");
        app.owned = true;
        app.name = QStringLiteral("Demo");
        app.managedPath = dest;
        app.desktopPath = settings.applicationsDir() + QStringLiteral("/gosh-appimage-static-check-apply.desktop");
        app.architecture = Architecture::X86_64;
        app.size = original.size();
        app.sha256 = QCryptographicHash::hash(original, QCryptographicHash::Sha256);
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        registry.upsert(app);
        registry.save();
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = next;
        rule.result.contentLength = next.size();
        rule.result.etag = QStringLiteral("\"new\"");
        network.rules.append(rule);
        const QVector<UpdateOffer> offers = updates.listUpdates();
        QCOMPARE(offers.size(), 1);
        const InstalledApp beforeApply = registry.byUuid(app.uuid);
        const IntegrateResult applied = updates.apply(beforeApply, true);
        QVERIFY2(applied.ok, qPrintable(applied.error));
        QFile live(dest);
        QVERIFY(live.open(QIODevice::ReadOnly));
        QCOMPARE(live.readAll(), next);
        live.close();
        const InstalledApp after = registry.byUuid(app.uuid);
        const UpdateCheckResult again = updates.check(after);
        QVERIFY(again.ok);
        QVERIFY(!again.available);
        const IntegrateResult refused = updates.apply(after, true);
        QVERIFY(!refused.ok);
        QFile still(dest);
        QVERIFY(still.open(QIODevice::ReadOnly));
        QCOMPARE(still.readAll(), next);
    }
    void zsyncCheckThenApplyReplacesOnce()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        QStandardPaths::setTestModeEnabled(true);
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable processes;
        AppImageInspector inspector(&runner, &settings, &registry);
        DesktopIntegration desktop(&settings, &runner);
        UpdateService updates(&settings, &registry, &inspector, &desktop, &network, &processes, &runner);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(16, 'N'));
        QDir().mkpath(settings.managedFolder());
        const QString dest = TestFixt::writeFile(settings.managedFolder(), QStringLiteral("Demo.AppImage"), original);
        const QByteArray nextSha1 = QCryptographicHash::hash(next, QCryptographicHash::Sha1);
        InstalledApp app;
        app.uuid = QStringLiteral("zsync-check-apply");
        app.owned = true;
        app.name = QStringLiteral("Demo");
        app.managedPath = dest;
        app.desktopPath = settings.applicationsDir() + QStringLiteral("/gosh-appimage-zsync-check-apply.desktop");
        app.architecture = Architecture::X86_64;
        app.size = original.size();
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage.zsync"));
        registry.upsert(app);
        FakeNetworkClient::Rule zsync;
        zsync.hostContains = QStringLiteral("example.com");
        zsync.pathContains = QStringLiteral(".zsync");
        zsync.result.ok = true;
        zsync.result.status = 200;
        zsync.result.body = QByteArrayLiteral("SHA-1: ") + nextSha1.toHex()
            + QByteArrayLiteral("\nLength: ") + QByteArray::number(next.size())
            + QByteArrayLiteral("\nURL: https://example.com/App.AppImage\n");
        network.rules.append(zsync);
        FakeNetworkClient::Rule dl;
        dl.hostContains = QStringLiteral("example.com");
        dl.pathContains = QStringLiteral("App.AppImage");
        dl.result.ok = true;
        dl.result.status = 200;
        dl.result.body = next;
        dl.result.contentLength = next.size();
        network.rules.append(dl);
        QVERIFY(updates.check(app).available);
        const QVector<UpdateOffer> offers = updates.listUpdates();
        QCOMPARE(offers.size(), 1);
        const IntegrateResult applied = updates.apply(registry.byUuid(app.uuid), true);
        QVERIFY2(applied.ok, qPrintable(applied.error));
        QFile live(dest);
        QVERIFY(live.open(QIODevice::ReadOnly));
        QCOMPARE(live.readAll(), next);
        const UpdateCheckResult again = updates.check(registry.byUuid(app.uuid));
        QVERIFY(again.ok);
        QVERIFY(!again.available);
    }
    void githubSameDigestAndTagIsUnavailable()
    {
        FakeNetworkClient network;
        const QByteArray payload = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'G'));
        const QByteArray digest = QCryptographicHash::hash(payload, QCryptographicHash::Sha256);
        FakeNetworkClient::Rule api;
        api.hostContains = QStringLiteral("api.github.com");
        api.result.ok = true;
        api.result.status = 200;
        api.result.body = QByteArray("{\"tag_name\":\"v2\",\"assets\":[{\"name\":\"App.AppImage\",\"browser_download_url\":\"https://github.com/u/r/releases/download/v2/App.AppImage\",\"size\":")
            + QByteArray::number(payload.size()) + ",\"digest\":\"sha256:" + digest.toHex() + "\"}]}";
        network.rules.append(api);
        GitHubSource source;
        InstalledApp app;
        app.version = QStringLiteral("v2");
        app.sha256 = digest;
        app.size = payload.size();
        app.updateConfig.insert(QStringLiteral("username"), QStringLiteral("u"));
        app.updateConfig.insert(QStringLiteral("repo"), QStringLiteral("r"));
        app.updateConfig.insert(QStringLiteral("filename"), QStringLiteral("App.AppImage"));
        const UpdateCheckResult same = source.check(app, &network, nullptr);
        QVERIFY(same.ok);
        QVERIFY(!same.available);
        app.version = QStringLiteral("v1");
        const UpdateCheckResult changedTag = source.check(app, &network, nullptr);
        QVERIFY(changedTag.ok);
        QVERIFY(changedTag.available);
        app.version = QStringLiteral("v2");
        app.sha256 = QByteArray(32, '\x11');
        const UpdateCheckResult changedDigest = source.check(app, &network, nullptr);
        QVERIFY(changedDigest.ok);
        QVERIFY(changedDigest.available);
        app.sha256 = digest;
        app.size = payload.size() + 9;
        const UpdateCheckResult changedSize = source.check(app, &network, nullptr);
        QVERIFY(changedSize.ok);
        QVERIFY(changedSize.available);
        app.size = payload.size();
        app.sha256 = digest;
        FakeNetworkClient emptyDigestNet;
        FakeNetworkClient::Rule empty;
        empty.hostContains = QStringLiteral("api.github.com");
        empty.result.ok = true;
        empty.result.status = 200;
        empty.result.body = QByteArray("{\"tag_name\":\"v2\",\"assets\":[{\"name\":\"App.AppImage\",\"browser_download_url\":\"https://github.com/u/r/releases/download/v2/App.AppImage\",\"size\":")
            + QByteArray::number(payload.size()) + "}]}";
        emptyDigestNet.rules.append(empty);
        const UpdateCheckResult emptyDigest = source.check(app, &emptyDigestNet, nullptr);
        QVERIFY(emptyDigest.ok);
        QVERIFY(!emptyDigest.available);
    }
};

QTEST_GUILESS_MAIN(TestUpdate)
#include "test_update.moc"
