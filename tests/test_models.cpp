#include "models/Models.h"
#include "core/UrlGuard.h"

#include <QtTest>

using namespace GoshAim;

class TestModels : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void libraryRoles()
    {
        LibraryModel model;
        InstalledApp app;
        app.uuid = QStringLiteral("u");
        app.name = QStringLiteral("Demo");
        app.managedPath = QStringLiteral("/tmp/Demo.AppImage");
        model.setApps({app});
        QCOMPARE(model.rowCount(), 1);
        QVERIFY(model.roleNames().contains(LibraryModel::UuidRole));
        QCOMPARE(model.data(model.index(0, 0), LibraryModel::NameRole).toString(), QStringLiteral("Demo"));
        model.setFilter(QStringLiteral("zzz"));
        QCOMPARE(model.rowCount(), 0);
    }
    void updatesRoles()
    {
        UpdatesModel model;
        UpdateOffer offer;
        offer.uuid = QStringLiteral("u");
        offer.name = QStringLiteral("Demo");
        model.setOffers({offer});
        QCOMPARE(model.rowCount(), 1);
        QVERIFY(model.roleNames().contains(UpdatesModel::UuidRole));
    }
    void taskRoles()
    {
        TaskModel model;
        TaskItem task;
        task.id = QStringLiteral("t1");
        task.title = QStringLiteral("Work");
        model.setTasks({task});
        QCOMPARE(model.rowCount(), 1);
        QVERIFY(model.roleNames().contains(TaskModel::IdRole));
    }
    void repoComponent()
    {
        QVERIFY(UrlGuard::isSafeRepoComponent(QStringLiteral("repo-name")));
        QVERIFY(!UrlGuard::isSafeRepoComponent(QStringLiteral("a/b")));
    }
};

QTEST_GUILESS_MAIN(TestModels)
#include "test_models.moc"
