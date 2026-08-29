#include "AppController.h"
#include "Cli.h"
#include "ElfFixtures.h"
#include "FakeSeams.h"
#include "QtWarnGuard.h"
#include "core/IntegrationService.h"
#include "core/ManagedRegistry.h"
#include "core/ProcessRunner.h"
#include "core/ProcessTable.h"
#include "core/TaskQueue.h"
#include "models/Models.h"

#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QCryptographicHash>
#include <QJsonDocument>
#include <QJsonParseError>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QSignalSpy>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QThread>
#include <QVariant>
#include <QtTest>

using namespace GoshAim;

class TestController : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void initTestCase() { QStandardPaths::setTestModeEnabled(true); }
    void inspectDoesNotBlockAndCanCancel()
    {
        QtWarnGuard guard;
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QString path = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        QSignalSpy spy(&controller, &AppController::confirmInspect);
        QElapsedTimer timer;
        timer.start();
        controller.inspectPaths({path});
        QVERIFY(timer.elapsed() < 500);
        QTRY_VERIFY_WITH_TIMEOUT(spy.count() >= 1 || !controller.inspecting(), 5000);
        controller.cancelInspect();
        QVERIFY(!guard.sawLiveDestruction());
    }
    void checkAllDoesNotUseEmptyUuid()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        controller.checkAll();
        QTRY_VERIFY(controller.taskQueue()->tasks().size() >= 1);
        QCOMPARE(controller.taskQueue()->tasks().last().target, QStringLiteral("*"));
        QVERIFY(controller.taskQueue()->tasks().last().target != QString());
    }
    void localPathFromUrl()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        QCOMPARE(controller.localPathFromUrl(QStringLiteral("file:///tmp/foo")), QStringLiteral("/tmp/foo"));
        QVERIFY(controller.localPathFromUrl(QStringLiteral("https://example.com")).isEmpty());
    }
    void perCandidateReplaceDoesNotUseComboIndex()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        QDir().mkpath(home.path() + QStringLiteral("/a"));
        QDir().mkpath(home.path() + QStringLiteral("/b"));
        QDir().mkpath(home.path() + QStringLiteral("/c"));
        const QString first = TestFixt::writeFile(home.path() + QStringLiteral("/a"), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'A')));
        IntegrateRequest req;
        req.sourcePath = first;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = controller.integration()->integrate(req);
        QVERIFY2(integrated.ok, qPrintable(integrated.error));
        const QString uuid = integrated.app.uuid;
        const QString second = TestFixt::writeFile(home.path() + QStringLiteral("/b"), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'B')));
        const QString third = TestFixt::writeFile(home.path() + QStringLiteral("/c"), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'C')));
        QSignalSpy spy(&controller, &AppController::confirmInspect);
        controller.inspectPaths({second, third});
        QTRY_COMPARE(spy.count(), 1);
        QCOMPARE(controller.candidateModel()->rowCount(), 2);
        QVERIFY(controller.pendingCandidate(0).needsConflictDecision);
        QVERIFY(controller.pendingCandidate(1).needsConflictDecision);
        QVERIFY(!controller.conflictsResolved());

        controller.setCandidateConflict(0, int(ConflictPolicy::Replace), uuid);
        QCOMPARE(controller.pendingCandidate(0).chosenPolicy, ConflictPolicy::Replace);
        QCOMPARE(controller.pendingCandidate(0).chosenReplaceUuid, uuid);
        QCOMPARE(controller.pendingCandidate(0).plannedTarget, integrated.app.managedPath);
        QCOMPARE(controller.pendingCandidate(1).chosenPolicy, ConflictPolicy::Unspecified);
        QVERIFY(controller.pendingCandidate(1).chosenReplaceUuid.isEmpty());

        controller.setCandidateConflict(1, int(ConflictPolicy::KeepBoth), uuid);
        QCOMPARE(controller.pendingCandidate(1).chosenPolicy, ConflictPolicy::KeepBoth);
        QVERIFY(controller.pendingCandidate(1).chosenReplaceUuid.isEmpty());
        QVERIFY(controller.pendingCandidate(1).plannedTarget != integrated.app.managedPath);
        QVERIFY(controller.pendingCandidate(1).plannedTarget.contains(QLatin1String("-2"))
                || controller.pendingCandidate(1).plannedTarget != controller.pendingCandidate(0).plannedTarget);
        QVERIFY(controller.conflictsResolved());

        controller.confirmIntegrate(1, false, QString());
        QTRY_VERIFY([&]() {
            QFile replaced(integrated.app.managedPath);
            if (!replaced.open(QIODevice::ReadOnly)) {
                return false;
            }
            return replaced.readAll() == TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'B'));
        }());
        QVERIFY(QFile::exists(controller.settings()->managedFolder() + QStringLiteral("/Demo-2.AppImage"))
                || controller.registry()->apps().size() >= 2);
    }
    void singleCandidateReplaceIsInRange()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        QDir().mkpath(home.path() + QStringLiteral("/a"));
        QDir().mkpath(home.path() + QStringLiteral("/b"));
        const QString first = TestFixt::writeFile(home.path() + QStringLiteral("/a"), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(4, 'A')));
        IntegrateRequest req;
        req.sourcePath = first;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = controller.integration()->integrate(req);
        QVERIFY(integrated.ok);
        const QString next = TestFixt::writeFile(home.path() + QStringLiteral("/b"), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(4, 'N')));
        QSignalSpy spy(&controller, &AppController::confirmInspect);
        controller.inspectPaths({next});
        QTRY_COMPARE(spy.count(), 1);
        QCOMPARE(controller.candidateModel()->rowCount(), 1);
        controller.setCandidateConflict(0, int(ConflictPolicy::Replace), integrated.app.uuid);
        QCOMPARE(controller.pendingCandidate(0).chosenPolicy, ConflictPolicy::Replace);
        QCOMPARE(controller.pendingCandidate(0).chosenReplaceUuid, integrated.app.uuid);
        QVERIFY(controller.conflictsResolved());
        controller.confirmIntegrate(99, false, QStringLiteral("ignored"));
        QTRY_VERIFY([&]() {
            QFile replaced(integrated.app.managedPath);
            if (!replaced.open(QIODevice::ReadOnly)) {
                return false;
            }
            return replaced.readAll() == TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(4, 'N'));
        }());
    }
    void qmlDelegateUsesRequiredIndexNotComboIndex()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        QDir().mkpath(home.path() + QStringLiteral("/a"));
        QDir().mkpath(home.path() + QStringLiteral("/b"));
        const QString first = TestFixt::writeFile(home.path() + QStringLiteral("/a"), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = first;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = controller.integration()->integrate(req);
        QVERIFY(integrated.ok);
        const QString next = TestFixt::writeFile(home.path() + QStringLiteral("/b"), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(4, 'Q')));
        QSignalSpy spy(&controller, &AppController::confirmInspect);
        controller.inspectPaths({next});
        QTRY_COMPARE(spy.count(), 1);

        QQmlEngine engine;
        engine.rootContext()->setContextProperty(QStringLiteral("Store"), &controller);
        QQmlComponent component(&engine);
        component.setData(QByteArrayLiteral(R"(
            import QtQuick
            Item {
                Repeater {
                    id: repeater
                    model: Store.candidateModel
                    delegate: Item {
                        id: del
                        required property int index
                        required property bool needsDecision
                        required property string conflictingUuid
                        function chooseReplace() {
                            Store.setCandidateConflict(index, 2, conflictingUuid)
                        }
                    }
                }
                function activateReplaceOnFirst() {
                    repeater.itemAt(0).chooseReplace()
                }
            }
        )"), QUrl(QStringLiteral("qrc:/conflict-seam.qml")));
        QVERIFY2(component.errors().isEmpty(), qPrintable(component.errorString()));
        QObject *root = component.create();
        QVERIFY(root);
        QMetaObject::invokeMethod(root, "activateReplaceOnFirst");
        QCOMPARE(controller.pendingCandidate(0).chosenPolicy, ConflictPolicy::Replace);
        QCOMPARE(controller.pendingCandidate(0).chosenReplaceUuid, integrated.app.uuid);
        delete root;
    }
    void argumentTokensRoundTripThroughSave()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QString src = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = controller.integration()->integrate(req);
        QVERIFY(integrated.ok);
        controller.selectApp(integrated.app.uuid);
        controller.setArguments(integrated.app.uuid, {QStringLiteral("a b"), QStringLiteral("c")});
        QCOMPARE(controller.registry()->byUuid(integrated.app.uuid).arguments, QStringList({QStringLiteral("a b"), QStringLiteral("c")}));

        QQmlEngine engine;
        engine.rootContext()->setContextProperty(QStringLiteral("Store"), &controller);
        QQmlComponent component(&engine);
        component.setData(QByteArrayLiteral(R"(
            import QtQuick
            Item {
                id: root
                property string uuid
                property var tokens: Store.selectedDetails().arguments
                function load() {
                    tokens = Store.selectedDetails().arguments
                }
                function editSecond(text) {
                    var next = tokens.slice()
                    next[1] = text
                    tokens = next
                }
                function save() {
                    Store.setArguments(root.uuid, tokens)
                }
            }
        )"), QUrl(QStringLiteral("qrc:/args-seam.qml")));
        QVERIFY2(component.errors().isEmpty(), qPrintable(component.errorString()));
        QObject *root = component.create();
        QVERIFY(root);
        root->setProperty("uuid", integrated.app.uuid);
        QVERIFY(QMetaObject::invokeMethod(root, "load"));
        QVERIFY(QMetaObject::invokeMethod(root, "editSecond", Q_ARG(QVariant, QVariant(QStringLiteral("d e")))));
        QVERIFY(QMetaObject::invokeMethod(root, "save"));
        const QStringList saved = controller.registry()->byUuid(integrated.app.uuid).arguments;
        QCOMPARE(saved.size(), 2);
        QCOMPARE(saved.at(0), QStringLiteral("a b"));
        QCOMPARE(saved.at(1), QStringLiteral("d e"));
        delete root;
    }
    void updateAllZeroOffersEnqueuesNothing()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        InstalledApp app;
        app.uuid = QStringLiteral("owned");
        app.owned = true;
        app.name = QStringLiteral("Demo");
        app.managedPath = home.path() + QStringLiteral("/Demo.AppImage");
        controller.registry()->upsert(app);
        const int before = controller.taskQueue()->tasks().size();
        controller.updateAll(false);
        QCOMPARE(controller.taskQueue()->tasks().size(), before);
        QVERIFY(controller.updateSummary().contains(QLatin1String("No updates")));
    }
    void cancelUpdateXDoesNotCancelInspect()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        std::atomic<bool> inspectStarted{false};
        std::atomic<bool> inspectCancelled{false};
        const QString inspectId = controller.taskQueue()->enqueue(
            TaskKind::Inspect,
            QStringLiteral("inspect"),
            QStringLiteral("/tmp/inspect"),
            [&](TaskItem &, std::atomic<bool> *cancel) {
                inspectStarted.store(true);
                while (!cancel->load()) {
                    QThread::msleep(5);
                }
                inspectCancelled.store(true);
            },
            false);
        QTRY_VERIFY(inspectStarted.load());
        InstalledApp app;
        app.uuid = QStringLiteral("upd-x");
        app.owned = true;
        app.name = QStringLiteral("X");
        app.managedPath = home.path() + QStringLiteral("/X.AppImage");
        controller.registry()->upsert(app);
        controller.updateApp(QStringLiteral("upd-x"), false);
        const QString updateId = controller.updateTaskId(QStringLiteral("upd-x"));
        QVERIFY(!updateId.isEmpty());
        QVERIFY(updateId != inspectId);
        controller.cancelTask(updateId);
        QVERIFY(!inspectCancelled.load());
        QCOMPARE(controller.taskQueue()->task(inspectId).state, TaskState::Running);
        controller.taskQueue()->cancel(inspectId);
        QTRY_VERIFY(inspectCancelled.load());
    }
    void libraryBadgeFollowsCheckAndApply()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(16, 'N'));
        const QString src = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), original);
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = controller.integration()->integrate(req);
        QVERIFY2(integrated.ok, qPrintable(integrated.error));
        InstalledApp app = controller.registry()->byUuid(integrated.app.uuid);
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
        controller.refreshLibrary();
        QCOMPARE(controller.libraryModel()->data(controller.libraryModel()->index(0, 0), LibraryModel::UpdateAvailableRole).toBool(), false);
        controller.checkAll();
        QTRY_COMPARE(controller.updatesModel()->rowCount(), 1);
        QTRY_COMPARE(controller.libraryModel()->data(controller.libraryModel()->index(0, 0), LibraryModel::UpdateAvailableRole).toBool(), true);
        controller.updateApp(app.uuid, true);
        QTRY_COMPARE(controller.libraryModel()->data(controller.libraryModel()->index(0, 0), LibraryModel::UpdateAvailableRole).toBool(), false);
        QFile live(app.managedPath);
        QVERIFY(live.open(QIODevice::ReadOnly));
        QCOMPARE(live.readAll(), next);
    }
    void updateProgressVisibleBeforeFinish()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(16, 'N'));
        const QString src = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), original);
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = controller.integration()->integrate(req);
        QVERIFY(integrated.ok);
        InstalledApp app = controller.registry()->byUuid(integrated.app.uuid);
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        controller.registry()->upsert(app);
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = next;
        rule.result.contentLength = next.size();
        rule.hangOnDownload = true;
        network.rules.append(rule);
        controller.checkAll();
        QTRY_COMPARE(controller.updatesModel()->rowCount(), 1);
        controller.updateApp(app.uuid, true);
        QTRY_VERIFY(controller.updateProgress(app.uuid) > 0 && controller.updateProgress(app.uuid) < 100);
        const int mid = controller.updateProgress(app.uuid);
        QVERIFY(mid > 0 && mid < 100);
        controller.cancelTask(controller.updateTaskId(app.uuid));
        QTRY_VERIFY(controller.taskQueue()->task(controller.updateTaskId(app.uuid)).state == TaskState::Cancelled
                    || controller.taskQueue()->task(controller.updateTaskId(app.uuid)).state == TaskState::Failed);
    }
    void updateAllIgnoresInspectAndFailedEnqueue()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        std::atomic<bool> inspectStarted{false};
        std::atomic<bool> inspectHold{true};
        controller.taskQueue()->enqueue(
            TaskKind::Inspect,
            QStringLiteral("inspect"),
            QStringLiteral("/tmp/inspect-all"),
            [&](TaskItem &, std::atomic<bool> *cancel) {
                inspectStarted.store(true);
                while (inspectHold.load() && !cancel->load()) {
                    QThread::msleep(5);
                }
            },
            false);
        QTRY_VERIFY(inspectStarted.load());
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(16, 'N'));
        const QString src = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), original);
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = controller.integration()->integrate(req);
        QVERIFY(integrated.ok);
        InstalledApp app = controller.registry()->byUuid(integrated.app.uuid);
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        controller.registry()->upsert(app);
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = next;
        rule.result.contentLength = next.size();
        rule.hangOnDownload = true;
        network.rules.append(rule);
        UpdateOffer offer;
        offer.uuid = app.uuid;
        offer.name = app.name;
        controller.updatesModel()->setOffers({offer});
        controller.updateAll(true);
        inspectHold.store(false);
        QTRY_VERIFY(controller.updateProgress(app.uuid) > 0 && controller.updateProgress(app.uuid) < 100);
        QVERIFY(!controller.updateSummary().contains(QLatin1String("succeeded")));
        QVERIFY(!controller.updateSummary().contains(QLatin1String("failed")));
        controller.cancelTask(controller.updateTaskId(app.uuid));
        QTRY_VERIFY(controller.updateSummary().contains(QLatin1String("Update-all finished")));
        QVERIFY(controller.updateSummary().contains(QLatin1String("0 succeeded")));

        std::atomic<bool> mutationStarted{false};
        std::atomic<bool> mutationHold{true};
        const QString blockId = controller.taskQueue()->enqueue(
            TaskKind::Update,
            QStringLiteral("block"),
            app.managedPath,
            [&](TaskItem &, std::atomic<bool> *cancel) {
                mutationStarted.store(true);
                while (mutationHold.load() && !cancel->load()) {
                    QThread::msleep(5);
                }
            },
            true);
        QVERIFY(!blockId.isEmpty());
        QTRY_VERIFY(mutationStarted.load());
        controller.updatesModel()->setOffers({offer});
        controller.updateAll(true);
        QCOMPARE(controller.updateSummary(), QStringLiteral("No updates are currently offered"));
        mutationHold.store(false);
        QTRY_COMPARE(controller.taskQueue()->task(blockId).state, TaskState::Succeeded);
        QVERIFY(!controller.updateSummary().contains(QLatin1String("1 succeeded")));
    }
    void autostartFileIsSessionVisible()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        controller.settings()->setBackgroundUpdateChecks(true);
        const QString path = controller.autostartDesktopPath();
        QVERIFY(QFile::exists(path));
        QVERIFY(path.contains(QStringLiteral("/autostart/")));
        QFile file(path);
        QVERIFY(file.open(QIODevice::ReadOnly));
        const QByteArray body = file.readAll();
        QVERIFY(body.contains("--fetch-updates"));
        if (ProcessRunner::inFlatpak()
            || qEnvironmentVariable("FLATPAK_ID") == QLatin1String("com.goshapps.AppImageManager")) {
            QVERIFY(body.contains("flatpak run com.goshapps.AppImageManager --fetch-updates"));
        } else {
            QVERIFY(!body.contains("flatpak run"));
        }
        QCOMPARE(runAutostartProbe(controller), 0);
        controller.registry()->save();
        const QByteArray before = [&]() {
            QFile registryFile(controller.settings()->registryPath());
            if (!registryFile.open(QIODevice::ReadOnly)) {
                return QByteArray();
            }
            return registryFile.readAll();
        }();
        QCOMPARE(runCli(controller, {QStringLiteral("--fetch-updates")}, false), 0);
        QFile registryFile(controller.settings()->registryPath());
        QByteArray after;
        if (registryFile.open(QIODevice::ReadOnly)) {
            after = registryFile.readAll();
        }
        QCOMPARE(after, before);
    }
    void hideReplaceWhenConflictIsUnowned()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        QDir().mkpath(controller.settings()->managedFolder());
        TestFixt::writeFile(controller.settings()->managedFolder(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(4, 'X')));
        const QString incoming = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(4, 'Y')));
        QSignalSpy spy(&controller, &AppController::confirmInspect);
        controller.inspectPaths({incoming});
        QTRY_COMPARE(spy.count(), 1);
        QVERIFY(controller.pendingCandidate(0).needsConflictDecision);
        QVERIFY(!controller.pendingCandidate(0).canReplace);
        QVERIFY(controller.pendingCandidate(0).conflictingUuid.isEmpty());
    }
    void refreshMetadataRestoresOnSaveFailure()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QString src = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), TestFixt::makeElf64(Architecture::X86_64, 2));
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = controller.integration()->integrate(req);
        QVERIFY(integrated.ok);
        const QString name = controller.registry()->byUuid(integrated.app.uuid).name;
        QByteArray desktopBytes;
        {
            QFile desk(integrated.app.desktopPath);
            QVERIFY(desk.open(QIODevice::ReadOnly));
            desktopBytes = desk.readAll();
        }
        controller.registry()->setFailSave(true);
        controller.refreshMetadata(integrated.app.uuid);
        QTRY_VERIFY([&]() {
            const auto tasks = controller.taskQueue()->tasks();
            for (const TaskItem &task : tasks) {
                if (task.kind == TaskKind::RefreshMetadata && (task.state == TaskState::Failed || task.state == TaskState::Succeeded)) {
                    return true;
                }
            }
            return false;
        }());
        controller.registry()->setFailSave(false);
        QCOMPARE(controller.registry()->byUuid(integrated.app.uuid).name, name);
        QFile desk(integrated.app.desktopPath);
        QVERIFY(desk.open(QIODevice::ReadOnly));
        QCOMPARE(desk.readAll(), desktopBytes);
    }
    void githubUnavailableDoesNotEnqueueUpdateAll()
    {
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QByteArray payload = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'G'));
        const QByteArray digest = QCryptographicHash::hash(payload, QCryptographicHash::Sha256);
        const QString dest = TestFixt::writeFile(home.path(), QStringLiteral("Demo.AppImage"), payload);
        InstalledApp app;
        app.uuid = QStringLiteral("gh-same");
        app.owned = true;
        app.name = QStringLiteral("Demo");
        app.managedPath = dest;
        app.version = QStringLiteral("v2");
        app.sha256 = digest;
        app.size = payload.size();
        app.updateManager = QStringLiteral("github");
        app.updateConfig = {{QStringLiteral("username"), QStringLiteral("u")},
                            {QStringLiteral("repo"), QStringLiteral("r")},
                            {QStringLiteral("filename"), QStringLiteral("App.AppImage")}};
        controller.registry()->upsert(app);
        FakeNetworkClient::Rule api;
        api.hostContains = QStringLiteral("api.github.com");
        api.result.ok = true;
        api.result.status = 200;
        api.result.body = QByteArray("{\"tag_name\":\"v2\",\"assets\":[{\"name\":\"App.AppImage\",\"browser_download_url\":\"https://github.com/u/r/releases/download/v2/App.AppImage\",\"size\":")
            + QByteArray::number(payload.size()) + ",\"digest\":\"sha256:" + digest.toHex() + "\"}]}";
        network.rules.append(api);
        controller.checkAll();
        QTRY_VERIFY([&]() {
            const auto tasks = controller.taskQueue()->tasks();
            for (const TaskItem &task : tasks) {
                if (task.kind == TaskKind::CheckUpdate && (task.state == TaskState::Succeeded || task.state == TaskState::Failed)) {
                    return true;
                }
            }
            return false;
        }());
        QCOMPARE(controller.updatesModel()->rowCount(), 0);
        const int before = controller.taskQueue()->tasks().size();
        controller.updateAll(false);
        QCOMPARE(controller.taskQueue()->tasks().size(), before);
        QVERIFY(controller.updateSummary().contains(QLatin1String("No updates")));
    }
    void editsDuringHeldUpdateKeepRegistryCoherent()
    {
        QtWarnGuard guard;
        QTemporaryDir home;
        qputenv("HOME", home.path().toUtf8());
        FakeProcessRunner runner;
        FakeNetworkClient network;
        FakeProcessTable table;
        AppController controller(nullptr, &runner, &network, &table, true);
        const QByteArray original = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(8, 'O'));
        const QByteArray next = TestFixt::makeElf64(Architecture::X86_64, 2, {}, QByteArray(16, 'N'));
        const QString src = TestFixt::writeFile(home.path(), QStringLiteral("Held.AppImage"), original);
        IntegrateRequest req;
        req.sourcePath = src;
        req.conflict = ConflictPolicy::KeepBoth;
        const IntegrateResult integrated = controller.integration()->integrate(req);
        QVERIFY(integrated.ok);
        InstalledApp app = controller.registry()->byUuid(integrated.app.uuid);
        app.updateManager = QStringLiteral("static");
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        controller.registry()->upsert(app);
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.body = next;
        rule.result.contentLength = next.size();
        rule.hangOnDownload = true;
        network.rules.append(rule);
        controller.updateApp(app.uuid, true);
        QTRY_VERIFY(controller.updateProgress(app.uuid) > 0 && controller.updateProgress(app.uuid) < 100);
        controller.setArguments(app.uuid, QStringList{QStringLiteral("--held")});
        QFile json(controller.settings()->registryPath());
        QVERIFY(json.open(QIODevice::ReadOnly));
        QJsonParseError parseError;
        const QJsonDocument doc = QJsonDocument::fromJson(json.readAll(), &parseError);
        QCOMPARE(parseError.error, QJsonParseError::NoError);
        QVERIFY(doc.isObject());
        controller.cancelTask(controller.updateTaskId(app.uuid));
        QTRY_VERIFY(controller.taskQueue()->task(controller.updateTaskId(app.uuid)).state == TaskState::Cancelled
                    || controller.taskQueue()->task(controller.updateTaskId(app.uuid)).id.isEmpty()
                    || controller.taskQueue()->task(controller.updateTaskId(app.uuid)).state == TaskState::Failed);
        const InstalledApp finalApp = controller.registry()->byUuid(app.uuid);
        QVERIFY(!finalApp.uuid.isEmpty());
        QCOMPARE(finalApp.arguments, QStringList{QStringLiteral("--held")});
        QVERIFY(!guard.sawLiveDestruction());
    }
};

QTEST_GUILESS_MAIN(TestController)
#include "test_controller.moc"
