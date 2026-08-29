#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "core/AppImageInspector.h"
#include "core/ManagedRegistry.h"
#include "core/SettingsStore.h"

#include <QDir>
#include <QFile>
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
    void unsquashfsRejectsAbsoluteAndNeverExtractsIt()
    {
        QTemporaryDir home;
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeProcessRunner::Rule list;
        list.contains = QStringList{QStringLiteral("unsquashfs")};
        list.result.exitCode = 0;
        list.result.standardOutput = QByteArray("squashfs-root/demo.desktop\n/etc/passwd\n");
        runner.rules.append(list);
        AppImageInspector inspector(&runner, &settings, &registry);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        InspectOptions options;
        inspector.inspect(path, options);
        for (const auto &call : runner.calls) {
            QVERIFY(!call.second.contains(QStringLiteral("/etc/passwd")));
            QVERIFY(call.first != path);
        }
        for (const auto &call : runner.detachedCalls) {
            QVERIFY(call.first != path);
        }
    }
    void sevenZipListingUnsafeNeverExtracted()
    {
        QTemporaryDir home;
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeProcessRunner::Rule unsquash;
        unsquash.contains = QStringList{QStringLiteral("unsquashfs")};
        unsquash.result.failedToStart = true;
        unsquash.result.exitCode = -1;
        runner.rules.append(unsquash);
        FakeProcessRunner::Rule list;
        list.contains = QStringList{QStringLiteral("7zz"), QStringLiteral("l")};
        list.result.exitCode = 0;
        list.result.standardOutput = QByteArray("Path = ../etc/passwd\nSize = 12\nAttributes = A\n");
        runner.rules.append(list);
        AppImageInspector inspector(&runner, &settings, &registry);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("T1.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 1));
        InspectOptions options;
        inspector.inspect(path, options);
        for (const auto &call : runner.calls) {
            if (call.second.contains(QStringLiteral("x"))) {
                QVERIFY(!call.second.contains(QStringLiteral("../etc/passwd")));
            }
            QVERIFY(call.first != path);
        }
    }
    void dwarfsOversizedListingFailsClosed()
    {
        QTemporaryDir home;
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeProcessRunner::Rule unsquash;
        unsquash.contains = QStringList{QStringLiteral("unsquashfs")};
        unsquash.result.failedToStart = true;
        runner.rules.append(unsquash);
        FakeProcessRunner::Rule seven;
        seven.contains = QStringList{QStringLiteral("7zz")};
        seven.result.failedToStart = true;
        runner.rules.append(seven);
        QByteArray listing;
        for (int i = 0; i < 80; ++i) {
            listing += QByteArray("f") + QByteArray::number(i) + ".desktop\n";
        }
        FakeProcessRunner::Rule dwarfs;
        dwarfs.contains = QStringList{QStringLiteral("dwarfsck")};
        dwarfs.result.exitCode = 0;
        dwarfs.result.standardOutput = listing;
        runner.rules.append(dwarfs);
        AppImageInspector inspector(&runner, &settings, &registry);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("D.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray("DWARFS\0\0", 8)));
        InspectOptions options;
        inspector.inspect(path, options);
        QVERIFY(!runner.sawProgram(QStringLiteral("dwarfsextract")));
        for (const auto &call : runner.calls) {
            QVERIFY(call.first != path);
        }
    }
    void expandedSizeBoundFailsClosed()
    {
        QTemporaryDir home;
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        ManagedRegistry registry(&settings);
        FakeProcessRunner runner;
        FakeProcessRunner::Rule list;
        list.contains = QStringList{QStringLiteral("unsquashfs")};
        list.result.exitCode = 0;
        list.result.standardOutput = QByteArray("-rw-r--r-- user/group 99999999 2020-01-01 00:00 squashfs-root/demo.desktop\n");
        runner.rules.append(list);
        AppImageInspector inspector(&runner, &settings, &registry);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Big.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        InspectOptions options;
        inspector.inspect(path, options);
        for (const auto &call : runner.calls) {
            QVERIFY(!call.second.contains(QStringLiteral("-e")));
        }
    }
    void unsafeExtractStillVerifiesTree()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        settings.setUnsafeExtractionFallback(true);
        ManagedRegistry registry(&settings);
        class PlantSymlinkRunner : public FakeProcessRunner
        {
        public:
            ProcessResult run(const ProcessRequest &request, std::atomic<bool> *cancel = nullptr) override
            {
                ProcessResult result = FakeProcessRunner::run(request, cancel);
                if (request.arguments.contains(QStringLiteral("--appimage-extract"))) {
                    const QString tree = request.workingDirectory + QStringLiteral("/squashfs-root");
                    QDir().mkpath(tree);
                    QFile::link(QStringLiteral("/etc/passwd"), tree + QStringLiteral("/evil"));
                }
                return result;
            }
        };
        PlantSymlinkRunner runner;
        FakeProcessRunner::Rule unsquash;
        unsquash.contains = QStringList{QStringLiteral("unsquashfs")};
        unsquash.result.failedToStart = true;
        runner.rules.append(unsquash);
        FakeProcessRunner::Rule seven;
        seven.contains = QStringList{QStringLiteral("7zz")};
        seven.result.failedToStart = true;
        runner.rules.append(seven);
        AppImageInspector inspector(&runner, &settings, &registry);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        InspectOptions options;
        options.allowUnsafeExtract = true;
        options.confirmUnsafeExtract = true;
        const InspectionResult result = inspector.inspect(path, options);
        QVERIFY(!result.extractionUsedUnsafeFallback);
        bool sawExtract = false;
        for (const auto &call : runner.calls) {
            if (call.second.contains(QStringLiteral("--appimage-extract"))) {
                sawExtract = true;
            }
        }
        QVERIFY(sawExtract);
        QVERIFY(!result.warnings.isEmpty());
    }
    void largeListingIngestsDesktopName()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        SettingsStore settings(nullptr, home.path() + QStringLiteral("/cfg"));
        ManagedRegistry registry(&settings);
        class ExtractDesktopRunner : public FakeProcessRunner
        {
        public:
            ProcessResult run(const ProcessRequest &request, std::atomic<bool> *cancel = nullptr) override
            {
                ProcessResult result = FakeProcessRunner::run(request, cancel);
                const int destIdx = request.arguments.indexOf(QStringLiteral("-d"));
                if (request.arguments.contains(QStringLiteral("-e")) && destIdx >= 0 && destIdx + 1 < request.arguments.size()) {
                    const QString dest = request.arguments.at(destIdx + 1);
                    QDir().mkpath(dest);
                    QFile desktop(dest + QStringLiteral("/demo.desktop"));
                    if (desktop.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                        desktop.write("[Desktop Entry]\nName=Demo App\nExec=demo\nType=Application\n");
                    }
                }
                return result;
            }
        };
        ExtractDesktopRunner runner;
        QByteArray listing = QByteArrayLiteral("-rw-r--r-- user/group 80 2020-01-01 00:00 squashfs-root/demo.desktop\n");
        listing += QByteArrayLiteral("lrwxrwxrwx user/group 0 2020-01-01 00:00 squashfs-root/usr/bin/foo -> ../lib/foo\n");
        for (int i = 0; i < 200; ++i) {
            listing += QByteArrayLiteral("-rw-r--r-- user/group 10 2020-01-01 00:00 squashfs-root/usr/lib/file")
                + QByteArray::number(i) + "\n";
        }
        FakeProcessRunner::Rule list;
        list.contains = QStringList{QStringLiteral("unsquashfs")};
        list.result.exitCode = 0;
        list.result.standardOutput = listing;
        runner.rules.append(list);
        AppImageInspector inspector(&runner, &settings, &registry);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        InspectOptions options;
        const InspectionResult result = inspector.inspect(path, options);
        QVERIFY(result.magicValid);
        QCOMPARE(result.metadata.name, QStringLiteral("Demo App"));
        bool sawExtract = false;
        for (const auto &call : runner.calls) {
            if (call.second.contains(QStringLiteral("-e"))) {
                sawExtract = true;
                QVERIFY(call.second.contains(QStringLiteral("demo.desktop")));
                QVERIFY(!call.second.contains(QStringLiteral("usr/bin/foo")));
                QVERIFY(!call.second.contains(QStringLiteral("usr/lib/file0")));
            }
        }
        QVERIFY(sawExtract);
    }
};

QTEST_GUILESS_MAIN(TestInspector)
#include "test_inspector.moc"
