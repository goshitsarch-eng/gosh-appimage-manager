#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "core/AppImageInspector.h"
#include "core/AppImageLibrary.h"
#include "core/DesktopIntegration.h"
#include "core/IntegrationService.h"
#include "core/ManagedRegistry.h"
#include "core/SettingsStore.h"
#include "core/UpdateService.h"
#include "core/ProcessTable.h"
#include "core/RemovalLaunch.h"

#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QtTest>

using namespace GoshAim;

class FailingRegistry : public ManagedRegistry
{
public:
    using ManagedRegistry::ManagedRegistry;
    bool failSave = false;
    bool save(QString *error = nullptr) override
    {
        if (failSave) {
            if (error) {
                *error = QStringLiteral("Forced registry save failure");
            }
            return false;
        }
        return ManagedRegistry::save(error);
    }
};

class FailingDesktop : public DesktopIntegration
{
public:
    using DesktopIntegration::DesktopIntegration;
    bool failWrite = false;
    bool failInstall = false;
    bool writeStaged(const InstalledApp &app, const QString &stagedDesktop, const QString &stagedIcon, QString *error) override
    {
        if (failWrite) {
            if (error) {
                *error = QStringLiteral("Forced desktop write failure");
            }
            return false;
        }
        return DesktopIntegration::writeStaged(app, stagedDesktop, stagedIcon, error);
    }
    bool install(const InstalledApp &app, const QString &stagedDesktop, const QString &stagedIcon, QString *error) override
    {
        if (failInstall) {
            if (error) {
                *error = QStringLiteral("Forced desktop install failure");
            }
            return false;
        }
        return DesktopIntegration::install(app, stagedDesktop, stagedIcon, error);
    }
};

class TestIntegration : public QObject
{
    Q_OBJECT
    QTemporaryDir m_home;
    SettingsStore *m_settings = nullptr;
    FailingRegistry *m_registry = nullptr;
    FakeProcessRunner m_runner;
    AppImageInspector *m_inspector = nullptr;
    FailingDesktop *m_desktop = nullptr;
    IntegrationService *m_service = nullptr;

    static QByteArray fileBytes(const QString &path)
    {
        QFile file(path);
        if (!file.open(QIODevice::ReadOnly)) {
            return {};
        }
        return file.readAll();
    }

    void setup()
    {
        QStandardPaths::setTestModeEnabled(true);
        qputenv("HOME", m_home.path().toUtf8());
        m_settings = new SettingsStore(this, m_home.path() + QStringLiteral("/cfg"));
        m_settings->setManagedFolder(m_home.path() + QStringLiteral("/AppImages"));
        m_registry = new FailingRegistry(m_settings);
        m_inspector = new AppImageInspector(&m_runner, m_settings, m_registry);
        m_desktop = new FailingDesktop(m_settings, &m_runner);
        m_service = new IntegrationService(m_settings, m_registry, m_inspector, m_desktop, &m_runner);
    }

    bool noOrphans() const
    {
        const QDir dir(m_settings->managedFolder());
        const QFileInfoList entries = dir.entryInfoList(QDir::Files);
        for (const QFileInfo &info : entries) {
            if (info.fileName().contains(QLatin1String(".gosh-"))) {
                return false;
            }
        }
        return true;
    }

private Q_SLOTS:
    void initTestCase() { setup(); }
    void copiesAndOwns()
    {
        const QString src = TestFixt::writeFile(m_home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult result = m_service->integrate(req);
        QVERIFY2(result.ok, qPrintable(result.error));
        QVERIFY(QFile::exists(result.app.managedPath));
        QVERIFY(QFile::exists(src));
        QVERIFY(result.app.owned);
        QVERIFY(QFile::exists(result.app.desktopPath));
        QFile desktop(result.app.desktopPath);
        QVERIFY(desktop.open(QIODevice::ReadOnly));
        const QByteArray text = desktop.readAll();
        QVERIFY(text.contains("X-Gosh-AppImage-Manager=true"));
        QVERIFY(text.contains("TryExec="));
        QVERIFY(noOrphans());
    }
    void rollbackOnCopyCancel()
    {
        const QString src = TestFixt::writeFile(m_home.path(), QStringLiteral("Other.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        std::atomic<bool> cancel{true};
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult result = m_service->integrate(req, &cancel);
        QVERIFY(!result.ok);
        QVERIFY(QFile::exists(src));
        QVERIFY(noOrphans());
    }
    void replaceRequiresOwned()
    {
        IntegrateRequest req;
        req.sourcePath = TestFixt::writeFile(m_home.path(), QStringLiteral("R.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        req.conflict = ConflictPolicy::Replace;
        req.replaceUuid = QStringLiteral("missing");
        const IntegrateResult result = m_service->integrate(req);
        QVERIFY(!result.ok);
    }
    void replaceSuccessPreservesSettings()
    {
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, QByteArray(), QByteArray(32, 'A'));
        const QString src1 = TestFixt::writeFile(m_home.path(), QStringLiteral("Orig.AppImage"), original);
        IntegrateRequest req;
        req.sourcePath = src1;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult first = m_service->integrate(req);
        QVERIFY(first.ok);
        InstalledApp app = first.app;
        app.arguments = QStringList{QStringLiteral("keep me")};
        EnvPair env;
        env.name = QStringLiteral("FOO");
        env.value = QStringLiteral("bar");
        app.environment.append(env);
        app.updateManager = QStringLiteral("static");
        m_registry->upsert(app);
        m_registry->save();
        const QByteArray replacement = TestFixt::makeElf64(Architecture::X86_64, 2, QByteArray(), QByteArray(48, 'B'));
        const QString src2 = TestFixt::writeFile(m_home.path(), QStringLiteral("New.AppImage"), replacement);
        IntegrateRequest replace;
        replace.sourcePath = src2;
        replace.conflict = ConflictPolicy::Replace;
        replace.replaceUuid = app.uuid;
        const IntegrateResult result = m_service->integrate(replace);
        QVERIFY2(result.ok, qPrintable(result.error));
        QCOMPARE(fileBytes(result.app.managedPath), replacement);
        QCOMPARE(m_registry->byUuid(app.uuid).arguments, QStringList{QStringLiteral("keep me")});
        QVERIFY(QFile::exists(src2));
        QVERIFY(noOrphans());
    }
    void replaceDesktopInstallFailureRestoresBytes()
    {
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, QByteArray(), QByteArray(16, 'C'));
        const QString src1 = TestFixt::writeFile(m_home.path(), QStringLiteral("Live.AppImage"), original);
        IntegrateRequest req;
        req.sourcePath = src1;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult first = m_service->integrate(req);
        QVERIFY(first.ok);
        const QString dest = first.app.managedPath;
        const QByteArray before = fileBytes(dest);
        const QString desktopBefore = first.app.desktopPath;
        QFile desk(desktopBefore);
        QVERIFY(desk.open(QIODevice::ReadOnly));
        const QByteArray desktopBytes = desk.readAll();
        desk.close();
        m_desktop->failInstall = true;
        const QByteArray replacement = TestFixt::makeElf64(Architecture::X86_64, 2, QByteArray(), QByteArray(24, 'D'));
        const QString src2 = TestFixt::writeFile(m_home.path(), QStringLiteral("FailInstall.AppImage"), replacement);
        IntegrateRequest replace;
        replace.sourcePath = src2;
        replace.conflict = ConflictPolicy::Replace;
        replace.replaceUuid = first.app.uuid;
        const IntegrateResult result = m_service->integrate(replace);
        m_desktop->failInstall = false;
        QVERIFY(!result.ok);
        QCOMPARE(fileBytes(dest), before);
        QFile desk2(desktopBefore);
        QVERIFY(desk2.open(QIODevice::ReadOnly));
        QCOMPARE(desk2.readAll(), desktopBytes);
        QVERIFY(QFile::exists(src2));
        QVERIFY(noOrphans());
    }
    void replaceRegistrySaveFailureRestores()
    {
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, QByteArray(), QByteArray(16, 'E'));
        const QString src1 = TestFixt::writeFile(m_home.path(), QStringLiteral("Reg.AppImage"), original);
        IntegrateRequest req;
        req.sourcePath = src1;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult first = m_service->integrate(req);
        QVERIFY(first.ok);
        const QByteArray before = fileBytes(first.app.managedPath);
        m_registry->failSave = true;
        const QByteArray replacement = TestFixt::makeElf64(Architecture::X86_64, 2, QByteArray(), QByteArray(24, 'F'));
        const QString src2 = TestFixt::writeFile(m_home.path(), QStringLiteral("FailReg.AppImage"), replacement);
        IntegrateRequest replace;
        replace.sourcePath = src2;
        replace.conflict = ConflictPolicy::Replace;
        replace.replaceUuid = first.app.uuid;
        const IntegrateResult result = m_service->integrate(replace);
        m_registry->failSave = false;
        QVERIFY(!result.ok);
        QCOMPARE(fileBytes(first.app.managedPath), before);
        QCOMPARE(m_registry->byUuid(first.app.uuid).sha256, first.app.sha256);
        QVERIFY(QFile::exists(src2));
        QVERIFY(noOrphans());
    }
    void desktopWriteFailureLeavesSource()
    {
        m_desktop->failWrite = true;
        const QString src = TestFixt::writeFile(m_home.path(), QStringLiteral("WriteFail.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult result = m_service->integrate(req);
        m_desktop->failWrite = false;
        QVERIFY(!result.ok);
        QVERIFY(QFile::exists(src));
        QVERIFY(noOrphans());
    }
    void moveSourceFailureIsReported()
    {
        const QString src = TestFixt::writeFile(m_home.path(), QStringLiteral("MoveFail.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        req.copyMode = CopyMode::Move;
        m_service->setFailPoint(IntegrateFailPoint::SourceDelete);
        const IntegrateResult result = m_service->integrate(req);
        m_service->setFailPoint(IntegrateFailPoint::None);
        QVERIFY(!result.ok);
        QVERIFY(result.partial);
        QVERIFY(QFile::exists(src));
        QVERIFY(QFile::exists(result.app.managedPath));
    }
    void unspecifiedConflictRequiresDecision()
    {
        const QString src = TestFixt::writeFile(m_home.path(), QStringLiteral("Conflict.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest first;
        first.sourcePath = src;
        first.conflict = ConflictPolicy::KeepBoth;
        QVERIFY(m_service->integrate(first).ok);
        IntegrateRequest second;
        second.sourcePath = src;
        second.conflict = ConflictPolicy::Unspecified;
        const IntegrateResult result = m_service->integrate(second);
        QVERIFY(!result.ok);
        QVERIFY(result.error.contains(QLatin1String("keep-both")));
    }
    void keepBothNames()
    {
        InspectionResult fake;
        fake.metadata.name = QStringLiteral("Demo");
        const QString name = m_service->chooseDestinationName(fake, false);
        QVERIFY(name.endsWith(QLatin1String(".AppImage")));
    }
    bool noGoshTempsAnywhere() const
    {
        const QStringList roots = {m_settings->managedFolder(), m_settings->applicationsDir(), m_settings->dataDir()};
        for (const QString &root : roots) {
            QDir dir(root);
            const QFileInfoList entries = dir.entryInfoList(QDir::Files | QDir::NoDotAndDotDot);
            for (const QFileInfo &info : entries) {
                if (info.fileName().contains(QLatin1String(".gosh-"))) {
                    return false;
                }
            }
        }
        return true;
    }
    void newIntegrationRegistrySaveFailureCleansArtifacts()
    {
        m_registry->failSave = true;
        const QString src = TestFixt::writeFile(m_home.path(), QStringLiteral("NewFail.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult result = m_service->integrate(req);
        m_registry->failSave = false;
        QVERIFY(!result.ok);
        QVERIFY(!result.app.managedPath.isEmpty());
        QVERIFY(!QFile::exists(result.app.managedPath));
        QVERIFY(result.app.desktopPath.isEmpty() || !QFile::exists(result.app.desktopPath));
        QVERIFY(result.app.iconPath.isEmpty() || !QFile::exists(result.app.iconPath));
        QVERIFY(m_registry->apps().isEmpty() || m_registry->byUuid(result.app.uuid).uuid.isEmpty());
        QVERIFY(noGoshTempsAnywhere());
        QVERIFY(QFile::exists(src));
    }
    void newIntegrationPartialDesktopInstallCleans()
    {
        class PartialDesktop : public FailingDesktop
        {
        public:
            using FailingDesktop::FailingDesktop;
            bool install(const InstalledApp &app, const QString &stagedDesktop, const QString &stagedIcon, QString *error) override
            {
                Q_UNUSED(stagedDesktop);
                if (!stagedIcon.isEmpty() && QFile::exists(stagedIcon)) {
                    QDir().mkpath(QFileInfo(app.iconPath).absolutePath());
                    QFile::rename(stagedIcon, app.iconPath);
                }
                if (error) {
                    *error = QStringLiteral("Forced partial icon install");
                }
                return false;
            }
        };
        PartialDesktop partial(m_settings, &m_runner);
        IntegrationService service(m_settings, m_registry, m_inspector, &partial, &m_runner);
        const QString iconSrc = TestFixt::writeFile(m_home.path(), QStringLiteral("icon.png"), QByteArray(16, 'I'));
        Q_UNUSED(iconSrc);
        const QString src = TestFixt::writeFile(m_home.path(), QStringLiteral("Partial.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult result = service.integrate(req);
        QVERIFY(!result.ok);
        QVERIFY(result.app.managedPath.isEmpty() || !QFile::exists(result.app.managedPath));
        QVERIFY(result.app.desktopPath.isEmpty() || !QFile::exists(result.app.desktopPath));
        QVERIFY(result.app.iconPath.isEmpty() || !QFile::exists(result.app.iconPath));
        QVERIFY(noGoshTempsAnywhere());
    }
    void backupCreationFailureIsFailClosed()
    {
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(16, 'Z'));
        const QString src1 = TestFixt::writeFile(m_home.path(), QStringLiteral("Bak.AppImage"), original);
        IntegrateRequest req;
        req.sourcePath = src1;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult first = m_service->integrate(req);
        QVERIFY(first.ok);
        const QByteArray before = fileBytes(first.app.managedPath);
        const QByteArray desktopBefore = fileBytes(first.app.desktopPath);
        m_service->setFailPoint(IntegrateFailPoint::BackupCreate);
        const QByteArray replacement = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(24, 'Y'));
        const QString src2 = TestFixt::writeFile(m_home.path(), QStringLiteral("BakNew.AppImage"), replacement);
        IntegrateRequest replace;
        replace.sourcePath = src2;
        replace.conflict = ConflictPolicy::Replace;
        replace.replaceUuid = first.app.uuid;
        const IntegrateResult result = m_service->integrate(replace);
        m_service->setFailPoint(IntegrateFailPoint::None);
        QVERIFY(!result.ok);
        QVERIFY(result.error.contains(QLatin1String("backup")));
        QCOMPARE(fileBytes(first.app.managedPath), before);
        QCOMPARE(fileBytes(first.app.desktopPath), desktopBefore);
        QVERIFY(QFile::exists(src2));
        QVERIFY(noOrphans());
    }
    void keepBothRefusesUnownedDestRace()
    {
        const QString src = TestFixt::writeFile(m_home.path(), QStringLiteral("Race.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        QByteArray planted = QByteArrayLiteral("unowned-dest");
        QString plantedPath;
        m_service->setBeforeCommitHook([&](const QString &destPath) {
            plantedPath = destPath;
            QFile file(destPath);
            QVERIFY(file.open(QIODevice::WriteOnly | QIODevice::Truncate));
            file.write(planted);
        });
        const IntegrateResult result = m_service->integrate(req);
        m_service->setBeforeCommitHook({});
        QVERIFY(!result.ok);
        QVERIFY(QFile::exists(src));
        QCOMPARE(fileBytes(plantedPath), planted);
        QVERIFY(noOrphans());
    }
    void adoptLeavesForeignDesktopUntouched()
    {
        const QByteArray payload = TestFixt::makeElf64(Architecture::X86_64, 2);
        const QString appPath = TestFixt::writeFile(m_home.path(), QStringLiteral("Foreign.AppImage"), payload);
        const QString foreignDesktop = m_home.path() + QStringLiteral("/foreign.desktop");
        const QByteArray foreignBytes = QByteArrayLiteral("[Desktop Entry]\nName=Gear Lever App\nExec=")
            + appPath.toUtf8() + "\nTryExec=" + appPath.toUtf8() + "\n";
        QVERIFY(QFile(foreignDesktop).open(QIODevice::WriteOnly));
        {
            QFile file(foreignDesktop);
            QVERIFY(file.open(QIODevice::WriteOnly | QIODevice::Truncate));
            file.write(foreignBytes);
        }
        InstalledApp external;
        external.uuid = QStringLiteral("external:foreign.desktop");
        external.name = QStringLiteral("Gear Lever App");
        external.managedPath = appPath;
        external.desktopPath = foreignDesktop;
        external.owned = false;
        AppImageLibrary library(m_settings, m_registry, m_desktop);
        QString error;
        QVERIFY2(library.adopt(external, &error), qPrintable(error));
        QCOMPARE(fileBytes(foreignDesktop), foreignBytes);
        const InstalledApp adopted = m_registry->apps().last();
        QVERIFY(adopted.owned);
        QVERIFY(adopted.adopted);
        QVERIFY(adopted.desktopPath.contains(QLatin1String("gosh-appimage-")));
        QVERIFY(adopted.desktopPath != foreignDesktop);
        QFile gosh(adopted.desktopPath);
        QVERIFY(gosh.open(QIODevice::ReadOnly));
        const QByteArray goshBytes = gosh.readAll();
        QVERIFY(goshBytes.contains("X-Gosh-AppImage-Manager=true"));
        QVERIFY(goshBytes.contains(adopted.uuid.toUtf8()));

        FakeNetworkClient network;
        FakeProcessTable processes;
        UpdateService updates(m_settings, m_registry, m_inspector, m_desktop, &network, &processes, &m_runner);
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'N'));
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = next;
        rule.result.contentLength = next.size() + 1;
        network.rules.append(rule);
        InstalledApp toUpdate = adopted;
        toUpdate.updateManager = QStringLiteral("static");
        toUpdate.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        toUpdate.size = payload.size();
        m_registry->upsert(toUpdate);
        updates.apply(toUpdate, true);
        QCOMPARE(fileBytes(foreignDesktop), foreignBytes);

        RemovalService removal(m_settings, m_registry, m_desktop, &processes, &m_runner);
        removal.setTrashHook([](const QString &, QString *) { return true; });
        RemovalRequest req;
        req.pathOrUuid = adopted.uuid;
        req.mode = RemovalMode::Trash;
        QVERIFY(removal.remove(req, &error));
        QCOMPARE(fileBytes(foreignDesktop), foreignBytes);
        QVERIFY(!QFile::exists(adopted.desktopPath));
    }
};

QTEST_GUILESS_MAIN(TestIntegration)
#include "test_integration.moc"
