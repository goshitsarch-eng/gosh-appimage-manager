#include "Cli.h"

#include "AppController.h"
#include "core/AppImageInspector.h"
#include "core/IntegrationService.h"
#include "core/ManagedRegistry.h"
#include "core/ProcessRunner.h"
#include "core/RemovalLaunch.h"
#include "core/SettingsStore.h"
#include "core/UpdateService.h"
#include "core/UpdateSources.h"
#include "core/UpdateNotifier.h"

#include <KLocalizedContext>
#include <KLocalizedString>
#include <QApplication>
#include <QCommandLineParser>
#include <QCoreApplication>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QTextStream>
#include <QTimer>
#include <cstdio>
#include <iostream>
#include <unistd.h>

namespace GoshAim {

namespace {

QTextStream &out()
{
    static QTextStream stream(stdout);
    return stream;
}

QTextStream &err()
{
    static QTextStream stream(stderr);
    return stream;
}

bool confirm(const QString &prompt, bool yes, bool tty)
{
    if (yes) {
        return true;
    }
    if (!tty) {
        err() << QStringLiteral("Refusing destructive action without a TTY; pass --yes\n");
        return false;
    }
    err() << prompt << QStringLiteral(" [y/N] ");
    err().flush();
    char c = 0;
    if (::read(STDIN_FILENO, &c, 1) != 1) {
        return false;
    }
    return c == 'y' || c == 'Y';
}

QJsonObject appJson(const InstalledApp &app)
{
    QJsonObject obj;
    obj.insert(QStringLiteral("name"), app.name);
    obj.insert(QStringLiteral("path"), app.managedPath);
    obj.insert(QStringLiteral("desktop_id"), app.desktopId);
    obj.insert(QStringLiteral("current_version"), app.version);
    obj.insert(QStringLiteral("available_version"), app.availableVersion);
    obj.insert(QStringLiteral("download_size"), app.availableSize);
    obj.insert(QStringLiteral("manager"), app.updateManager);
    obj.insert(QStringLiteral("embedded_source"), app.embeddedUpdate);
    obj.insert(QStringLiteral("running"), app.running);
    obj.insert(QStringLiteral("uuid"), app.uuid);
    obj.insert(QStringLiteral("owned"), app.owned);
    return obj;
}

int printJson(const QString &key, const QJsonArray &items)
{
    QJsonObject root;
    root.insert(QStringLiteral("schema_version"), kJsonSchemaVersion);
    root.insert(key, items);
    const QByteArray data = QJsonDocument(root).toJson(QJsonDocument::Compact);
    fwrite(data.constData(), 1, static_cast<size_t>(data.size()), stdout);
    fputc('\n', stdout);
    return int(ExitCode::Ok);
}

QString optionPath(const QStringList &args, const QString &name)
{
    const int idx = args.indexOf(name);
    if (idx < 0) {
        return {};
    }
    static const QStringList skipAlone = {
        QStringLiteral("--yes"),
        QStringLiteral("-y"),
        QStringLiteral("--keep-both"),
        QStringLiteral("--replace"),
        QStringLiteral("--force"),
        QStringLiteral("--delete"),
        QStringLiteral("--json"),
        QStringLiteral("--all"),
        QStringLiteral("--unset"),
    };
    static const QStringList skipWithValue = {
        QStringLiteral("--replace-uuid"),
        QStringLiteral("--target"),
        QStringLiteral("--manager"),
    };
    for (int i = idx + 1; i < args.size(); ++i) {
        const QString &arg = args.at(i);
        if (skipAlone.contains(arg)) {
            continue;
        }
        if (skipWithValue.contains(arg)) {
            if (i + 1 < args.size()) {
                ++i;
            }
            continue;
        }
        if (arg.startsWith(QLatin1String("--"))) {
            continue;
        }
        return arg;
    }
    return {};
}

QString argValue(const QStringList &args, const QString &name)
{
    const QString path = optionPath(args, name);
    if (!path.isEmpty()) {
        return path;
    }
    const int idx = args.indexOf(name);
    if (idx >= 0 && idx + 1 < args.size()) {
        const QString next = args.at(idx + 1);
        if (next.startsWith(QLatin1String("--"))) {
            return {};
        }
        return next;
    }
    return {};
}

bool hasArg(const QStringList &args, const QString &name)
{
    return args.contains(name);
}

} // namespace

int runCli(AppController &controller, const QStringList &arguments, bool interactiveTty)
{
    const bool yes = hasArg(arguments, QStringLiteral("--yes")) || hasArg(arguments, QStringLiteral("-y"));
    const bool json = hasArg(arguments, QStringLiteral("--json"));
    const bool force = hasArg(arguments, QStringLiteral("--force"));
    const bool keepBoth = hasArg(arguments, QStringLiteral("--keep-both"));
    const bool replace = hasArg(arguments, QStringLiteral("--replace"));
    const bool del = hasArg(arguments, QStringLiteral("--delete"));

    if (hasArg(arguments, QStringLiteral("--list-update-managers"))) {
        for (const QString &name : UpdateSourceFactory::names()) {
            out() << name << Qt::endl;
        }
        return int(ExitCode::Ok);
    }

    if (hasArg(arguments, QStringLiteral("--list-installed"))) {
        QJsonArray items;
        for (const InstalledApp &app : controller.registry()->apps()) {
            InstalledApp copy = app;
            copy.running = controller.launchService()->isRunning(app);
            if (json) {
                items.append(appJson(copy));
            } else {
                out() << copy.name << QLatin1Char('\t') << copy.managedPath << QLatin1Char('\t') << copy.version << Qt::endl;
            }
        }
        return json ? printJson(QStringLiteral("installed"), items) : int(ExitCode::Ok);
    }

    if (hasArg(arguments, QStringLiteral("--list-updates"))) {
        const QVector<UpdateOffer> offers = controller.updates()->listUpdates(nullptr, false);
        QJsonArray items;
        for (const UpdateOffer &offer : offers) {
            if (json) {
                QJsonObject obj;
                obj.insert(QStringLiteral("name"), offer.name);
                obj.insert(QStringLiteral("path"), controller.registry()->byUuid(offer.uuid).managedPath);
                obj.insert(QStringLiteral("desktop_id"), controller.registry()->byUuid(offer.uuid).desktopId);
                obj.insert(QStringLiteral("current_version"), offer.currentVersion);
                obj.insert(QStringLiteral("available_version"), offer.availableVersion);
                obj.insert(QStringLiteral("download_size"), offer.downloadSize);
                obj.insert(QStringLiteral("manager"), offer.manager);
                obj.insert(QStringLiteral("embedded_source"), offer.embeddedSource);
                obj.insert(QStringLiteral("running"), offer.running);
                items.append(obj);
            } else {
                out() << offer.name << QLatin1Char('\t') << offer.currentVersion << QStringLiteral(" -> ") << offer.availableVersion
                      << Qt::endl;
            }
        }
        return json ? printJson(QStringLiteral("updates"), items) : int(ExitCode::Ok);
    }

    if (hasArg(arguments, QStringLiteral("--integrate"))) {
        const QString path = argValue(arguments, QStringLiteral("--integrate"));
        if (path.isEmpty()) {
            err() << QStringLiteral("Usage: --integrate <path> [--keep-both|--replace|--replace-uuid UUID|--target PATH] [--yes]\n");
            return int(ExitCode::Usage);
        }
        if (!yes && !confirm(QStringLiteral("Integrate %1?").arg(path), yes, interactiveTty)) {
            return int(ExitCode::NeedsConfirmation);
        }
        IntegrateRequest req;
        req.sourcePath = path;
        req.copyMode = controller.settings()->moveSource() ? CopyMode::Move : CopyMode::Copy;
        req.assumeYes = yes;
        if (replace) {
            req.conflict = ConflictPolicy::Replace;
            QString target = argValue(arguments, QStringLiteral("--replace-uuid"));
            if (target.isEmpty()) {
                target = argValue(arguments, QStringLiteral("--target"));
            }
            InstalledApp owned;
            if (!target.isEmpty()) {
                owned = controller.registry()->byUuid(target);
                if (owned.uuid.isEmpty()) {
                    owned = controller.registry()->byPath(target);
                }
            } else {
                owned = controller.registry()->byPath(path);
                if (owned.uuid.isEmpty()) {
                    InspectOptions options;
                    options.allowUnsafeExtract = false;
                    const InspectionResult inspected = controller.inspector()->inspect(path, options);
                    if (!inspected.existingManagedId.isEmpty()) {
                        owned = controller.registry()->byUuid(inspected.existingManagedId);
                    }
                    QVector<InstalledApp> matches;
                    for (const InstalledApp &app : controller.registry()->apps()) {
                        if (app.owned && QFileInfo(app.managedPath).fileName() == QFileInfo(path).fileName()) {
                            matches.append(app);
                        }
                    }
                    if (owned.uuid.isEmpty() && matches.size() == 1) {
                        owned = matches.first();
                    } else if (owned.uuid.isEmpty() && matches.size() > 1) {
                        err() << QStringLiteral("Replace is ambiguous; pass --replace-uuid\n");
                        return int(ExitCode::Validation);
                    }
                }
            }
            if (owned.uuid.isEmpty() || !owned.owned) {
                err() << QStringLiteral("Replace requires a specific owned managed installation\n");
                return int(ExitCode::NotIntegrated);
            }
            req.replaceUuid = owned.uuid;
        } else if (keepBoth) {
            req.conflict = ConflictPolicy::KeepBoth;
        } else {
            req.conflict = ConflictPolicy::Unspecified;
        }
        const IntegrateResult result = controller.integration()->integrate(req);
        if (!result.ok) {
            err() << result.error << Qt::endl;
            if (result.error.contains(QLatin1String("keep-both")) || result.error.contains(QLatin1String("replace"))) {
                return int(ExitCode::Validation);
            }
            return int(ExitCode::Failure);
        }
        err() << QStringLiteral("Integrated %1\n").arg(result.app.managedPath);
        return int(ExitCode::Ok);
    }

    if (hasArg(arguments, QStringLiteral("--update"))) {
        if (hasArg(arguments, QStringLiteral("--all"))) {
            if (!yes && !confirm(QStringLiteral("Update all AppImages?"), yes, interactiveTty)) {
                return int(ExitCode::NeedsConfirmation);
            }
            int failures = 0;
            int skippedRunning = 0;
            int applied = 0;
            const QVector<UpdateOffer> offers = controller.updates()->listUpdates(nullptr, false);
            for (const UpdateOffer &offer : offers) {
                const InstalledApp app = controller.registry()->byUuid(offer.uuid);
                if (!app.owned) {
                    continue;
                }
                const IntegrateResult result = controller.updates()->apply(app, force);
                if (!result.ok) {
                    err() << app.managedPath << QStringLiteral(": ") << result.error << Qt::endl;
                    if (result.error.contains(QLatin1String("running"))) {
                        ++skippedRunning;
                        continue;
                    }
                    ++failures;
                } else {
                    ++applied;
                }
            }
            if (skippedRunning > 0) {
                err() << QStringLiteral("%1 update(s) skipped because applications are running; pass --force to override\n")
                             .arg(skippedRunning);
            }
            if (failures > 0) {
                return int(ExitCode::Failure);
            }
            if (applied == 0 && skippedRunning > 0) {
                return int(ExitCode::Running);
            }
            return int(ExitCode::Ok);
        }
        const QString path = argValue(arguments, QStringLiteral("--update"));
        InstalledApp app = controller.registry()->byPath(path);
        if (app.uuid.isEmpty()) {
            app = controller.registry()->byUuid(path);
        }
        if (app.uuid.isEmpty()) {
            err() << QStringLiteral("Not integrated\n");
            return int(ExitCode::NotIntegrated);
        }
        if (!yes && !confirm(QStringLiteral("Update %1?").arg(app.managedPath), yes, interactiveTty)) {
            return int(ExitCode::NeedsConfirmation);
        }
        const IntegrateResult result = controller.updates()->apply(app, force);
        if (!result.ok) {
            err() << result.error << Qt::endl;
            if (result.error.contains(QLatin1String("running"))) {
                return int(ExitCode::Running);
            }
            return int(ExitCode::Failure);
        }
        return int(ExitCode::Ok);
    }

    if (hasArg(arguments, QStringLiteral("--remove-all"))) {
        int ownedCount = 0;
        for (const InstalledApp &app : controller.registry()->apps()) {
            if (app.owned) {
                ++ownedCount;
            }
        }
        const bool permanent = del;
        if (permanent) {
            if (!yes
                && !confirm(QStringLiteral("Permanently delete %1 owned AppImages and their Gosh desktop/icon artifacts?")
                                .arg(ownedCount),
                            yes,
                            interactiveTty)) {
                return int(ExitCode::NeedsConfirmation);
            }
        } else if (!yes
                   && !confirm(QStringLiteral("Move %1 owned AppImages to Trash?").arg(ownedCount), yes, interactiveTty)) {
            return int(ExitCode::NeedsConfirmation);
        }
        const QVector<InstalledApp> apps = controller.registry()->apps();
        for (const InstalledApp &app : apps) {
            if (!app.owned) {
                continue;
            }
            RemovalRequest req;
            req.pathOrUuid = app.uuid;
            req.mode = permanent ? RemovalMode::Permanent : RemovalMode::Trash;
            req.assumeYes = true;
            QString error;
            if (!controller.removal()->remove(req, &error)) {
                err() << error << Qt::endl;
                return int(ExitCode::Failure);
            }
        }
        return int(ExitCode::Ok);
    }

    if (hasArg(arguments, QStringLiteral("--remove"))) {
        const QString path = argValue(arguments, QStringLiteral("--remove"));
        if (path.isEmpty()) {
            return int(ExitCode::Usage);
        }
        if (!yes && !confirm(del ? QStringLiteral("Permanently delete %1?").arg(path) : QStringLiteral("Trash %1?").arg(path),
                             yes,
                             interactiveTty)) {
            return int(ExitCode::NeedsConfirmation);
        }
        RemovalRequest req;
        req.pathOrUuid = path;
        req.mode = del ? RemovalMode::Permanent : RemovalMode::Trash;
        QString error;
        if (!controller.removal()->remove(req, &error)) {
            err() << error << Qt::endl;
            return int(ExitCode::Failure);
        }
        return int(ExitCode::Ok);
    }

    if (hasArg(arguments, QStringLiteral("--set-update-source"))) {
        const QString path = argValue(arguments, QStringLiteral("--set-update-source"));
        InstalledApp app = controller.registry()->byPath(path);
        if (app.uuid.isEmpty()) {
            app = controller.registry()->byUuid(path);
        }
        if (app.uuid.isEmpty()) {
            return int(ExitCode::NotIntegrated);
        }
        if (hasArg(arguments, QStringLiteral("--unset"))) {
            QString error;
            return controller.updates()->unsetSource(app, &error) ? int(ExitCode::Ok) : int(ExitCode::Failure);
        }
        const QString manager = argValue(arguments, QStringLiteral("--manager"));
        QVariantMap config;
        for (const QString &arg : arguments) {
            if (arg.contains(QLatin1Char('=')) && !arg.startsWith(QLatin1Char('-'))) {
                config.insert(arg.section(QLatin1Char('='), 0, 0), arg.section(QLatin1Char('='), 1));
            }
        }
        QString error;
        if (!controller.updates()->setSource(app, manager, config, &error)) {
            err() << error << Qt::endl;
            return int(ExitCode::Validation);
        }
        return int(ExitCode::Ok);
    }

    if (hasArg(arguments, QStringLiteral("--fetch-updates"))) {
        const QVector<UpdateOffer> offers = controller.updates()->listUpdates(nullptr, false);
        err() << QStringLiteral("%1 update(s) available\n").arg(offers.size());
        if (!offers.isEmpty()) {
            for (const UpdateOffer &offer : offers) {
                err() << offer.name << QStringLiteral(" ") << offer.currentVersion << QStringLiteral(" -> ")
                      << offer.availableVersion << Qt::endl;
            }
            if (controller.notifier()) {
                controller.notifier()->notifyUpdatesAvailable(offers.size());
            }
        }
        return int(ExitCode::Ok);
    }

    err() << QStringLiteral("Unknown command\n");
    return int(ExitCode::Usage);
}

int runSelfTest(QApplication &app, AppController &controller)
{
    if (!controller.modelsReady()) {
        return 4;
    }
    if (!controller.inspector() || !controller.integration() || !controller.updates() || !controller.taskQueue()) {
        return 7;
    }
    if (controller.qmlActionNames().size() < 6) {
        return 8;
    }
    controller.loadSyntheticCatalog();
    if (controller.libraryModel()->rowCount() < 1) {
        return 9;
    }
    controller.refreshLibrary();
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextObject(new KLocalizedContext(&engine));
    engine.rootContext()->setContextProperty(QStringLiteral("Store"), &controller);
    engine.rootContext()->setContextProperty(QStringLiteral("Theme"), controller.theme());
    engine.loadFromModule("com.goshapps.AppImageManager", "Main");
    if (engine.rootObjects().isEmpty()) {
        return 2;
    }
    bool found = false;
    for (QObject *object : engine.rootObjects()) {
        if (object->objectName() == QLatin1String("mainWindow")) {
            found = true;
            break;
        }
    }
    if (!found) {
        return 3;
    }
    QTimer::singleShot(50, &app, &QCoreApplication::quit);
    return app.exec() == 0 ? 0 : 6;
}

int runHostProbe(AppController &controller)
{
    ProcessRequest req;
    req.program = QStringLiteral("true");
    req.host = true;
    req.timeoutMs = 5000;
    const ProcessResult result = controller.runner()->run(req);
    QTextStream stream(stdout);
    stream << QStringLiteral("host_spawn_program=") << result.program << Qt::endl;
    stream << QStringLiteral("host_spawn_exit=") << result.exitCode << Qt::endl;
    stream << QStringLiteral("in_flatpak=") << (ProcessRunner::inFlatpak() ? QStringLiteral("true") : QStringLiteral("false"))
           << Qt::endl;
    stream << QStringLiteral("managed_folder=") << controller.settings()->managedFolder() << Qt::endl;
    if (result.refused) {
        stream << QStringLiteral("HOST_PROBE_FAIL\n");
        return 1;
    }
    stream << QStringLiteral("HOST_PROBE_OK\n");
    return 0;
}

int runInspectProbe(AppController &controller, const QString &path)
{
    InspectOptions options;
    options.allowUnsafeExtract = false;
    options.confirmUnsafeExtract = false;
    const InspectionResult result = controller.inspector()->inspect(path, options);
    QJsonObject obj;
    obj.insert(QStringLiteral("schema_version"), kJsonSchemaVersion);
    obj.insert(QStringLiteral("path"), result.identity.path);
    obj.insert(QStringLiteral("size"), result.identity.size);
    obj.insert(QStringLiteral("sha256"), QString::fromLatin1(result.identity.sha256.toHex()));
    obj.insert(QStringLiteral("type"), appImageTypeName(result.type));
    obj.insert(QStringLiteral("architecture"), architectureName(result.architecture));
    obj.insert(QStringLiteral("magic_valid"), result.magicValid);
    obj.insert(QStringLiteral("name"), result.metadata.name);
    obj.insert(QStringLiteral("error"), result.error);
    obj.insert(QStringLiteral("unsafe_fallback"), result.extractionUsedUnsafeFallback);
    obj.insert(QStringLiteral("extractor"), result.extractorUsed);
    const QByteArray data = QJsonDocument(obj).toJson(QJsonDocument::Compact);
    fwrite(data.constData(), 1, static_cast<size_t>(data.size()), stdout);
    fputc('\n', stdout);
    if (result.extractionUsedUnsafeFallback) {
        fwrite("INSPECT_EXECUTED_UNSAFE\n", 1, 24, stdout);
        return 1;
    }
    fwrite("INSPECT_NO_EXECUTION\n", 1, 21, stdout);
    return result.magicValid ? 0 : 1;
}

int runAutostartProbe(AppController &controller)
{
    controller.settings()->setBackgroundUpdateChecks(true);
    QTextStream stream(stdout);
    const QString path = controller.autostartDesktopPath();
    stream << QStringLiteral("autostart_path=") << path << Qt::endl;
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        stream << QStringLiteral("AUTOSTART_MISSING") << Qt::endl;
        return 1;
    }
    const QByteArray body = file.readAll();
    stream << QString::fromUtf8(body);
    const bool flatpak = ProcessRunner::inFlatpak()
        || qEnvironmentVariable("FLATPAK_ID") == QLatin1String("com.goshapps.AppImageManager");
    const bool execOk = flatpak
        ? body.contains("flatpak run com.goshapps.AppImageManager --fetch-updates")
        : body.contains("--fetch-updates");
    if (!execOk || (flatpak && body.contains(QCoreApplication::applicationFilePath().toUtf8()) && !body.contains("flatpak run"))) {
        stream << QStringLiteral("AUTOSTART_BAD_EXEC") << Qt::endl;
        return 1;
    }
    stream << QStringLiteral("AUTOSTART_OK") << Qt::endl;
    return 0;
}

} // namespace GoshAim
