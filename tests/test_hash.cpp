#include "core/SafeFs.h"
#include "ElfFixtures.h"

#include <QTemporaryDir>
#include <QtTest>
#include <atomic>
#include <thread>

using namespace GoshAim;

class TestHash : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void hashesFile()
    {
        QTemporaryDir dir;
        const QString path = TestFixt::writeFile(dir.path(), QStringLiteral("a.bin"), QByteArray("hello"));
        const HashResult result = SafeFs::sha256File(path, 1024);
        QCOMPARE(result.sha256, SafeFs::sha256Bytes(QByteArray("hello")));
        QCOMPARE(result.bytesRead, 5);
    }
    void cancelStops()
    {
        QTemporaryDir dir;
        QByteArray big(2 * 1024 * 1024, 'x');
        const QString path = TestFixt::writeFile(dir.path(), QStringLiteral("big.bin"), big);
        std::atomic<bool> cancel{true};
        const HashResult result = SafeFs::sha256File(path, 8LL * 1024 * 1024, &cancel);
        QVERIFY(result.cancelled);
        QVERIFY(result.sha256.isEmpty());
    }
    void bound()
    {
        QTemporaryDir dir;
        const QString path = TestFixt::writeFile(dir.path(), QStringLiteral("b.bin"), QByteArray(100, 'y'));
        const HashResult result = SafeFs::sha256File(path, 10);
        QVERIFY(result.truncated);
    }
    void hex()
    {
        QCOMPARE(SafeFs::hexSha256(QByteArray::fromHex("abcd")), QStringLiteral("abcd"));
    }
};

QTEST_GUILESS_MAIN(TestHash)
#include "test_hash.moc"
