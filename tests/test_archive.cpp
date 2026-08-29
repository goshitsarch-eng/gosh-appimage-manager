#include "core/DesktopParser.h"

#include <QtTest>

using namespace GoshAim;

class TestArchive : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void rejectsAbsolute()
    {
        QString error;
        QVERIFY(!ArchiveGuard::isSafeEntry(QStringLiteral("/etc/passwd"), &error));
    }
    void rejectsDotDot()
    {
        QString error;
        QVERIFY(!ArchiveGuard::isSafeEntry(QStringLiteral("foo/../../etc/passwd"), &error));
    }
    void acceptsDesktop()
    {
        QVERIFY(ArchiveGuard::isSafeEntry(QStringLiteral("demo.desktop")));
    }
    void filterExtractable()
    {
        const QStringList entries = {QStringLiteral("demo.desktop"), QStringLiteral(".DirIcon"), QStringLiteral("icon.png")};
        QString error;
        const QStringList out = ArchiveGuard::filterExtractable(entries, &error);
        QCOMPARE(out.size(), 3);
        QVERIFY(error.isEmpty());
    }
    void rejectsEscapeLink()
    {
        QString error;
        QVERIFY(!ArchiveGuard::isSafeLinkTarget(QStringLiteral("icon"), QStringLiteral("/tmp/x"), &error));
    }
    void tooManyEntries()
    {
        QStringList entries;
        for (int i = 0; i < 80; ++i) {
            entries.append(QStringLiteral("f%1.desktop").arg(i));
        }
        QString error;
        QVERIFY(ArchiveGuard::filterExtractable(entries, &error).isEmpty());
        QVERIFY(!error.isEmpty());
    }
};

QTEST_GUILESS_MAIN(TestArchive)
#include "test_archive.moc"
