#include "AppController.h"
#include "Cli.h"

#include <KLocalizedContext>
#include <KLocalizedString>
#include <KDBusService>
#include <QApplication>
#include <QCommandLineParser>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <cstdio>
#include <memory>
#include <unistd.h>

using namespace GoshAim;

namespace {

bool isCliCommand(const QStringList &args)
{
    const QStringList commands = {QStringLiteral("--integrate"),
                                  QStringLiteral("--update"),
                                  QStringLiteral("--remove"),
                                  QStringLiteral("--remove-all"),
                                  QStringLiteral("--list-installed"),
                                  QStringLiteral("--list-updates"),
                                  QStringLiteral("--list-update-managers"),
                                  QStringLiteral("--set-update-source"),
                                  QStringLiteral("--fetch-updates"),
                                  QStringLiteral("--probe-host"),
                                  QStringLiteral("--probe-inspect")};
    for (const QString &arg : args) {
        if (commands.contains(arg)) {
            return true;
        }
    }
    return false;
}

} // namespace

int main(int argc, char *argv[])
{
    QStringList raw;
    for (int i = 1; i < argc; ++i) {
        raw.append(QString::fromLocal8Bit(argv[i]));
    }
    const bool selfTest = raw.contains(QStringLiteral("--self-test"));
    const bool probeHost = raw.contains(QStringLiteral("--probe-host"));
    const bool probeInspect = raw.contains(QStringLiteral("--probe-inspect"));
    const bool cli = isCliCommand(raw);

    if ((selfTest || cli) && qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM")) {
        qputenv("QT_QPA_PLATFORM", QByteArrayLiteral("offscreen"));
    }

    if (cli && !selfTest) {
        QCoreApplication app(argc, argv);
        KLocalizedString::setApplicationDomain("gosh-appimage-manager");
        app.setApplicationName(QStringLiteral("gosh-appimage-manager"));
        app.setApplicationVersion(QStringLiteral("0.1.0"));
        app.setOrganizationName(QStringLiteral("Gosh Apps"));
        app.setOrganizationDomain(QStringLiteral("goshapps.com"));
        AppController controller;
        if (probeHost) {
            return runHostProbe(controller);
        }
        if (probeInspect) {
            const int idx = raw.indexOf(QStringLiteral("--probe-inspect"));
            const QString path = (idx >= 0 && idx + 1 < raw.size()) ? raw.at(idx + 1) : QString();
            return runInspectProbe(controller, path);
        }
        return runCli(controller, raw, isatty(STDIN_FILENO) == 1);
    }

    QApplication app(argc, argv);
    KLocalizedString::setApplicationDomain("gosh-appimage-manager");
    app.setApplicationName(QStringLiteral("Gosh AppImage Manager"));
    app.setApplicationDisplayName(i18n("Gosh AppImage Manager"));
    app.setApplicationVersion(QStringLiteral("0.1.0"));
    app.setOrganizationName(QStringLiteral("Gosh Apps"));
    app.setOrganizationDomain(QStringLiteral("goshapps.com"));
    app.setDesktopFileName(QStringLiteral("com.goshapps.AppImageManager"));
    QApplication::setWindowIcon(QIcon::fromTheme(QStringLiteral("com.goshapps.AppImageManager")));
    QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));

    QCommandLineParser parser;
    parser.setApplicationDescription(i18n("Gosh AppImage Manager — inspect, integrate, update and remove AppImages"));
    parser.addHelpOption();
    parser.addVersionOption();
    QCommandLineOption selfTestOpt(QStringLiteral("self-test"), i18n("Initialize the application stack offscreen and exit"));
    QCommandLineOption probeHostOpt(QStringLiteral("probe-host"), i18n("Run a non-mutating host integration probe"));
    QCommandLineOption probeInspectOpt(QStringLiteral("probe-inspect"),
                                       i18n("Inspect a local file without executing it"),
                                       QStringLiteral("path"));
    parser.addOption(selfTestOpt);
    parser.addOption(probeHostOpt);
    parser.addOption(probeInspectOpt);
    parser.addPositionalArgument(QStringLiteral("files"), i18n("AppImage files to inspect"), QStringLiteral("[files...]"));
    parser.process(app);

    AppController controller;
    if (parser.isSet(probeHostOpt)) {
        return runHostProbe(controller);
    }
    if (parser.isSet(probeInspectOpt)) {
        return runInspectProbe(controller, parser.value(probeInspectOpt));
    }
    if (parser.isSet(selfTestOpt)) {
        return runSelfTest(app, controller);
    }

    std::unique_ptr<KDBusService> service = std::make_unique<KDBusService>(KDBusService::Unique);
    QObject::connect(service.get(), &KDBusService::activateRequested, &controller, [&](const QStringList &arguments, const QString &) {
        if (arguments.size() > 1) {
            controller.openAppImages(arguments.mid(1));
        }
    });

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextObject(new KLocalizedContext(&engine));
    engine.rootContext()->setContextProperty(QStringLiteral("Store"), &controller);
    engine.rootContext()->setContextProperty(QStringLiteral("Theme"), controller.theme());
    QObject::connect(
        &engine,
        &QQmlApplicationEngine::objectCreationFailed,
        &app,
        []() { QCoreApplication::exit(1); },
        Qt::QueuedConnection);
    engine.loadFromModule("com.goshapps.AppImageManager", "Main");
    if (engine.rootObjects().isEmpty()) {
        return 1;
    }
    const QStringList positional = parser.positionalArguments();
    if (!positional.isEmpty()) {
        controller.openAppImages(positional);
    }
    return app.exec();
}
