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
    void largeTreeSkipsUnrelatedAndKeepsRootDesktop()
    {
        QVector<ArchiveEntry> entries;
        for (int i = 0; i < 200; ++i) {
            ArchiveEntry file;
            file.path = QStringLiteral("usr/lib/file%1").arg(i);
            file.size = 10;
            entries.append(file);
        }
        ArchiveEntry link;
        link.path = QStringLiteral("usr/bin/foo");
        link.kind = ArchiveEntryKind::Symlink;
        link.linkTarget = QStringLiteral("../lib/foo");
        entries.append(link);
        ArchiveEntry desktop;
        desktop.path = QStringLiteral("demo.desktop");
        desktop.size = 80;
        entries.append(desktop);
        ArchiveEntry icon;
        icon.path = QStringLiteral(".DirIcon");
        icon.size = 20;
        entries.append(icon);
        QString error;
        const QStringList out = ArchiveGuard::filterExtractable(entries, &error);
        QVERIFY(error.isEmpty());
        QVERIFY(out.contains(QStringLiteral("demo.desktop")));
        QVERIFY(out.contains(QStringLiteral(".DirIcon")));
        QVERIFY(!out.contains(QStringLiteral("usr/bin/foo")));
        QVERIFY(!out.contains(QStringLiteral("usr/lib/file0")));
    }
    void unsafeWantedSymlinkIsRejected()
    {
        QVector<ArchiveEntry> entries;
        ArchiveEntry link;
        link.path = QStringLiteral("demo.desktop");
        link.kind = ArchiveEntryKind::Symlink;
        link.linkTarget = QStringLiteral("../etc/passwd");
        entries.append(link);
        QString error;
        QVERIFY(ArchiveGuard::filterExtractable(entries, &error).isEmpty());
        QVERIFY(!error.isEmpty());
    }
    void parse7zRejectsAbsolute()
    {
        QString error;
        const QVector<ArchiveEntry> entries = ArchiveGuard::parse7zList(QByteArray("Path = /etc/passwd\nSize = 1\n"), &error);
        QVERIFY(ArchiveGuard::filterExtractable(entries, &error).isEmpty());
    }
    void parseDwarfsDeviceRejected()
    {
        QString error;
        const QVector<ArchiveEntry> entries = ArchiveGuard::parseDwarfsList(QByteArray("crw-rw-rw- 1 0 0 1, 3 /dev/null\n"), &error);
        QVERIFY(ArchiveGuard::filterExtractable(entries, &error).isEmpty());
    }
};

QTEST_GUILESS_MAIN(TestArchive)
#include "test_archive.moc"
