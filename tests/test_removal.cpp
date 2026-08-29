#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "core/AppImageInspector.h"
#include "core/DesktopIntegration.h"
#include "core/IntegrationService.h"
#include "core/ManagedRegistry.h"
#include "core/ProcessTable.h"
#include "core/RemovalLaunch.h"
#include "core/SafeFs.h"
#include "core/SettingsStore.h"

#include <QDir>
#include <QFile>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QtTest>

using namespace GoshAim;

class TestRemoval : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void initTestCase() { QStandardPaths::setTestModeEnabled(true); }
    void permanentRemovesOwnedAndRegistry()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        DesktopIntegration desktop(&settings, &runner);
        FakeProcessTable processes;
        RemovalService removal(&settings, &registry, &desktop, &processes, &runner);
        AppImageInspector inspector(&runner, &settings, &registry);
        IntegrationService integration(&settings, &registry, &inspector, &desktop, &runner);
        const QString src = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = integration.integrate(req);
        QVERIFY(integrated.ok);
        RemovalRequest rem;
        rem.pathOrUuid = integrated.app.uuid;
        rem.mode = RemovalMode::Permanent;
        QString error;
        QVERIFY2(removal.remove(rem, &error), qPrintable(error));
        QVERIFY(!QFile::exists(integrated.app.managedPath));
        QVERIFY(registry.byUuid(integrated.app.uuid).uuid.isEmpty());
        QVERIFY(QFile::exists(src));
    }
    void missingExecutableReconcilesRegistry()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        DesktopIntegration desktop(&settings, &runner);
        FakeProcessTable processes;
        RemovalService removal(&settings, &registry, &desktop, &processes, &runner);
        InstalledApp app;
        app.uuid = QStringLiteral("gone");
        app.owned = true;
        app.managedPath = home.path() + QStringLiteral("/missing.AppImage");
        app.desktopPath = settings.applicationsDir() + QStringLiteral("/gosh-appimage-gone.desktop");
        registry.upsert(app);
        registry.save();
        RemovalRequest rem;
        rem.pathOrUuid = app.uuid;
        rem.mode = RemovalMode::Trash;
        QString error;
        const bool ok = removal.remove(rem, &error);
        Q_UNUSED(ok);
        QVERIFY(registry.byUuid(app.uuid).uuid.isEmpty());
    }
    void permanentRefusesRoot()
    {
        QVERIFY(SafeFs::isForbiddenPermanentTarget(QStringLiteral("/")));
        QVERIFY(SafeFs::isForbiddenPermanentTarget(QDir::homePath()));
        QVERIFY(!SafeFs::isForbiddenPermanentTarget(QDir::homePath() + QStringLiteral("/AppImages/Demo.AppImage")));
    }
    void missingExecutableReconcilesRegistryPermanent()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        DesktopIntegration desktop(&settings, &runner);
        FakeProcessTable processes;
        RemovalService removal(&settings, &registry, &desktop, &processes, &runner);
        InstalledApp app;
        app.uuid = QStringLiteral("gone-perm");
        app.owned = true;
        app.managedPath = home.path() + QStringLiteral("/missing.AppImage");
        app.desktopPath = settings.applicationsDir() + QStringLiteral("/gosh-appimage-gone-perm.desktop");
        QDir().mkpath(settings.applicationsDir());
        QFile desk(app.desktopPath);
        QVERIFY(desk.open(QIODevice::WriteOnly));
        desk.write("X-Gosh-AppImage-Manager=true\nX-Gosh-AppImage-Id=gone-perm\n");
        desk.close();
        registry.upsert(app);
        registry.save();
        RemovalRequest rem;
        rem.pathOrUuid = app.uuid;
        rem.mode = RemovalMode::Permanent;
        QString error;
        const bool ok = removal.remove(rem, &error);
        QVERIFY(ok || error.contains(QLatin1String("already gone")));
        QVERIFY(registry.byUuid(app.uuid).uuid.isEmpty());
        QVERIFY(!QFile::exists(app.desktopPath));
    }
    void refusesUnowned()
    {
        QTemporaryDir home;
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        DesktopIntegration desktop(&settings, &runner);
        FakeProcessTable processes;
        RemovalService removal(&settings, &registry, &desktop, &processes, &runner);
        RemovalRequest rem;
        rem.pathOrUuid = QStringLiteral("nope");
        QString error;
        QVERIFY(!removal.remove(rem, &error));
        QVERIFY(!error.isEmpty());
    }
};

QTEST_GUILESS_MAIN(TestRemoval)
#include "test_removal.moc"
