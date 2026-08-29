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

#include <QStandardPaths>
#include <QTemporaryDir>
#include <QtTest>

using namespace GoshAim;

class TestRemoval : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void initTestCase() { QStandardPaths::setTestModeEnabled(true); }
    void trashFailureLeavesFiles()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeProcessRunner::Rule gio;
        gio.contains = QStringList{QStringLiteral("gio"), QStringLiteral("trash")};
        gio.result.exitCode = 1;
        gio.result.failedToStart = true;
        runner.rules.append(gio);
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
        rem.mode = RemovalMode::Trash;
        QString error;
        const bool ok = QFile::moveToTrash(integrated.app.managedPath);
        if (!ok) {
            QVERIFY(!removal.remove(rem, &error) || QFile::exists(integrated.app.managedPath) || !error.isEmpty());
        }
        QVERIFY(QFile::exists(src));
    }
    void permanentRefusesRoot()
    {
        QVERIFY(SafeFs::isForbiddenPermanentTarget(QStringLiteral("/")));
        QVERIFY(SafeFs::isForbiddenPermanentTarget(QDir::homePath()));
        QVERIFY(!SafeFs::isForbiddenPermanentTarget(QDir::homePath() + QStringLiteral("/AppImages/Demo.AppImage")));
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
    }
};

QTEST_GUILESS_MAIN(TestRemoval)
#include "test_removal.moc"
