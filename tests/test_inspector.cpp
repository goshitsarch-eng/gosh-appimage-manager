#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "core/AppImageInspector.h"
#include "core/ManagedRegistry.h"
#include "core/SettingsStore.h"

#include <QStandardPaths>
#include <QTemporaryDir>
#include <QtTest>

using namespace GoshAim;

class TestInspector : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void initTestCase()
    {
        QStandardPaths::setTestModeEnabled(true);
    }
    void inspectsWithoutExecuting()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setManagedFolder(home.path() + QStringLiteral("/AppImages"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        AppImageInspector inspector(&runner, &settings, &registry);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        InspectOptions options;
        options.allowUnsafeExtract = false;
        const InspectionResult result = inspector.inspect(path, options);
        QVERIFY(result.magicValid);
        QCOMPARE(result.type, AppImageType::Type2);
        QVERIFY(!result.extractionUsedUnsafeFallback);
        for (const auto &call : runner.calls) {
            QVERIFY2(call.first != path, "candidate AppImage must not be executed as the program");
        }
        for (const auto &call : runner.detachedCalls) {
            QVERIFY(call.first != path);
        }
    }
    void unsafeFallbackRequiresConsent()
    {
        QTemporaryDir home;
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setUnsafeExtractionFallback(false);
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        AppImageInspector inspector(&runner, &settings, &registry);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        InspectOptions options;
        options.allowUnsafeExtract = true;
        options.confirmUnsafeExtract = true;
        inspector.inspect(path, options);
        QVERIFY(!runner.sawProgram(QStringLiteral("--appimage-extract")));
    }
    void parseEmbeddedGithub()
    {
        const EmbeddedUpdateInfo info = AppImageInspector::parseUpdInfo(QByteArray("gh-releases-zsync|user|repo|latest|App.AppImage.zsync"));
        QCOMPARE(info.managerHint, QStringLiteral("github"));
        QCOMPARE(info.fields.value(QStringLiteral("username")).toString(), QStringLiteral("user"));
    }
    void parseEmbeddedZsync()
    {
        const EmbeddedUpdateInfo info = AppImageInspector::parseUpdInfo(QByteArray("zsync|https://example.com/App.AppImage.zsync"));
        QCOMPARE(info.managerHint, QStringLiteral("static"));
    }
    void rejectsDirectory()
    {
        QTemporaryDir home;
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        AppImageInspector inspector(&runner, &settings, &registry);
        InspectOptions options;
        const InspectionResult result = inspector.inspect(home.path(), options);
        QVERIFY(!result.error.isEmpty());
    }
};

QTEST_GUILESS_MAIN(TestInspector)
#include "test_inspector.moc"
