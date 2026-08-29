#include "core/DesktopParser.h"

#include <QtTest>

using namespace GoshAim;

class TestDesktop : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void parseNameAndExec()
    {
        const QByteArray data = QByteArrayLiteral("[Desktop Entry]\nName=Demo App\nComment=Hi\nExec=/tmp/Demo.AppImage --foo %F\nTerminal=true\nCategories=Utility;\n");
        const ParsedDesktop parsed = DesktopParser::parse(data);
        QVERIFY(parsed.ok);
        QCOMPARE(parsed.metadata.name, QStringLiteral("Demo App"));
        QCOMPARE(parsed.metadata.execArguments, QStringList() << QStringLiteral("--foo") << QStringLiteral("%F"));
        QVERIFY(parsed.metadata.terminal);
    }
    void rejectsNul()
    {
        QByteArray data = QByteArrayLiteral("[Desktop Entry]\nName=Bad\n");
        data.append('\0');
        QVERIFY(!DesktopParser::parse(data).ok);
    }
    void sanitizesIcon()
    {
        QVERIFY(DesktopParser::sanitizeIconName(QStringLiteral("https://evil")).isEmpty());
        QVERIFY(DesktopParser::sanitizeIconName(QStringLiteral("../x")).isEmpty());
        QCOMPARE(DesktopParser::sanitizeIconName(QStringLiteral("demo")), QStringLiteral("demo"));
    }
    void envValidation()
    {
        QVERIFY(DesktopParser::isValidEnvName(QStringLiteral("FOO_BAR")));
        QVERIFY(!DesktopParser::isValidEnvName(QStringLiteral("1BAD")));
        QVERIFY(DesktopParser::isDangerousEnvName(QStringLiteral("LD_PRELOAD")));
        QVERIFY(!DesktopParser::isValidEnvValue(QStringLiteral("a\nb")));
    }
    void execLineUsesEnvProgram()
    {
        EnvPair pair{QStringLiteral("FOO"), QStringLiteral("bar baz")};
        const QString line = DesktopParser::buildExecLine(QStringLiteral("/tmp/App.AppImage"), {QStringLiteral("--x")}, {pair});
        QVERIFY(line.startsWith(QLatin1String("env FOO=")));
        QVERIFY(line.contains(QLatin1String("/tmp/App.AppImage")));
        QVERIFY(!line.contains(QLatin1Char(';')));
    }
    void rewriteDropsUnknownFieldCodes()
    {
        const QStringList rewritten = DesktopParser::rewriteArguments({QStringLiteral("/bin/app"), QStringLiteral("%i"), QStringLiteral("--ok")});
        QCOMPARE(rewritten, QStringList{QStringLiteral("--ok")});
    }
    void fileBase()
    {
        QCOMPARE(DesktopParser::sanitizeFileBase(QStringLiteral("My App!")), QStringLiteral("My-App"));
    }
};

QTEST_GUILESS_MAIN(TestDesktop)
#include "test_desktop.moc"
