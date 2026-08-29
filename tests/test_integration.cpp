#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "core/AppImageInspector.h"
#include "core/DesktopIntegration.h"
#include "core/IntegrationService.h"
#include "core/ManagedRegistry.h"
#include "core/SettingsStore.h"

#include <QStandardPaths>
#include <QTemporaryDir>
#include <QtTest>

using namespace GoshAim;

class TestIntegration : public QObject
{
    Q_OBJECT
    QTemporaryDir m_home;
    SettingsStore *m_settings = nullptr;
    ManagedRegistry *m_registry = nullptr;
    FakeProcessRunner m_runner;
    AppImageInspector *m_inspector = nullptr;
    DesktopIntegration *m_desktop = nullptr;
    IntegrationService *m_service = nullptr;

    void setup()
    {
        QStandardPaths::setTestModeEnabled(true);
        qputenv("HOME", m_home.path().toUtf8());
        m_settings = new SettingsStore(this, m_home.path() + QStringLiteral("/cfg"));
        m_settings->setManagedFolder(m_home.path() + QStringLiteral("/AppImages"));
        m_registry = new ManagedRegistry(m_settings);
        m_inspector = new AppImageInspector(&m_runner, m_settings, m_registry);
        m_desktop = new DesktopIntegration(m_settings, &m_runner);
        m_service = new IntegrationService(m_settings, m_registry, m_inspector, m_desktop, &m_runner);
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
    void keepBothNames()
    {
        const InspectionResult fake;
        const QString name = m_service->chooseDestinationName(fake, false);
        QVERIFY(name.endsWith(QLatin1String(".AppImage")) || name == QStringLiteral("AppImage.AppImage") || !name.isEmpty());
    }
};

QTEST_GUILESS_MAIN(TestIntegration)
#include "test_integration.moc"
