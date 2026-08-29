#include "core/ElfParser.h"
#include "ElfFixtures.h"

#include <QtTest>

using namespace GoshAim;

class TestElf : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void type2X64()
    {
        const QByteArray data = TestFixt::makeElf64(Architecture::X86_64, 2);
        const ElfInfo info = ElfParser::parse(data);
        QCOMPARE(info.appImageType, AppImageType::Type2);
        QCOMPARE(info.architecture, Architecture::X86_64);
        QVERIFY(info.valid);
    }
    void type1Aarch64()
    {
        const ElfInfo info = ElfParser::parse(TestFixt::makeElf64(Architecture::AArch64, 1));
        QCOMPARE(info.appImageType, AppImageType::Type1);
        QCOMPARE(info.architecture, Architecture::AArch64);
    }
    void unknownArch()
    {
        QByteArray data = TestFixt::makeElf64(Architecture::X86_64, 2);
        data[18] = 0x00;
        data[19] = 0x00;
        const ElfInfo info = ElfParser::parse(data);
        QCOMPARE(info.architecture, Architecture::Unknown);
        QVERIFY(!ElfParser::architectureSupported(info.architecture));
    }
    void truncated()
    {
        const ElfInfo info = ElfParser::parse(QByteArray("\x7f""ELF", 4));
        QVERIFY(info.truncated);
    }
    void notElf()
    {
        const ElfInfo info = ElfParser::parse(QByteArray("not an elf file at all"));
        QVERIFY(!info.valid);
        QVERIFY(!info.error.isEmpty());
    }
    void missingMagic()
    {
        QByteArray data = TestFixt::makeElf64(Architecture::X86_64, 2);
        data[8] = 0;
        data[9] = 0;
        data[10] = 0;
        const ElfInfo info = ElfParser::parse(data);
        QCOMPARE(info.appImageType, AppImageType::Unknown);
    }
    void updInfoSection()
    {
        const QByteArray upd("gh-releases-zsync|user|repo|latest|App.AppImage.zsync");
        const ElfInfo info = ElfParser::parse(TestFixt::makeElf64(Architecture::X86_64, 2, upd));
        QVERIFY(QString::fromUtf8(info.updInfo).startsWith(QLatin1String("gh-releases-zsync|")));
    }
    void i386Reported()
    {
        const ElfInfo info = ElfParser::parse(TestFixt::makeElf64(Architecture::I386, 2));
        QCOMPARE(info.architecture, Architecture::I386);
        QVERIFY(!ElfParser::architectureSupported(info.architecture));
    }
};

QTEST_GUILESS_MAIN(TestElf)
#include "test_elf.moc"
