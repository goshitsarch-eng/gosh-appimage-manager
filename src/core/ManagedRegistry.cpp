#include "ManagedRegistry.h"

#include "SafeFs.h"
#include "SettingsStore.h"

#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QUuid>

namespace GoshAim {

ManagedRegistry::ManagedRegistry(SettingsStore *settings)
    : m_settings(settings)
{
}

QString ManagedRegistry::newUuid()
{
    return QUuid::createUuid().toString(QUuid::WithoutBraces);
}

InstalledApp ManagedRegistry::byUuid(const QString &uuid) const
{
    for (const InstalledApp &app : m_apps) {
        if (app.uuid == uuid) {
            return app;
        }
    }
    return {};
}

InstalledApp ManagedRegistry::byPath(const QString &path) const
{
    const QString canonical = QFileInfo(path).canonicalFilePath().isEmpty() ? QFileInfo(path).absoluteFilePath()
                                                                            : QFileInfo(path).canonicalFilePath();
    for (const InstalledApp &app : m_apps) {
        if (app.managedPath == path || app.managedPath == canonical || QFileInfo(app.managedPath).canonicalFilePath() == canonical) {
            return app;
        }
    }
    return {};
}

InstalledApp ManagedRegistry::byDesktopId(const QString &desktopId) const
{
    for (const InstalledApp &app : m_apps) {
        if (app.desktopId == desktopId) {
            return app;
        }
    }
    return {};
}

bool ManagedRegistry::containsPath(const QString &path) const
{
    return !byPath(path).uuid.isEmpty();
}

bool ManagedRegistry::isOwned(const InstalledApp &app) const
{
    return app.owned && !app.uuid.isEmpty();
}

void ManagedRegistry::upsert(const InstalledApp &app)
{
    for (int i = 0; i < m_apps.size(); ++i) {
        if (m_apps[i].uuid == app.uuid) {
            m_apps[i] = app;
            return;
        }
    }
    m_apps.append(app);
}

bool ManagedRegistry::removeUuid(const QString &uuid)
{
    for (int i = 0; i < m_apps.size(); ++i) {
        if (m_apps[i].uuid == uuid) {
            m_apps.removeAt(i);
            return true;
        }
    }
    return false;
}

namespace {

QJsonObject envToJson(const QVector<EnvPair> &env)
{
    QJsonObject obj;
    for (const EnvPair &pair : env) {
        obj.insert(pair.name, pair.value);
    }
    return obj;
}

QVector<EnvPair> envFromJson(const QJsonObject &obj)
{
    QVector<EnvPair> env;
    for (auto it = obj.begin(); it != obj.end(); ++it) {
        EnvPair pair;
        pair.name = it.key();
        pair.value = it.value().toString();
        env.append(pair);
    }
    return env;
}

QJsonObject appToJson(const InstalledApp &app)
{
    QJsonObject obj;
    obj.insert(QStringLiteral("uuid"), app.uuid);
    obj.insert(QStringLiteral("name"), app.name);
    obj.insert(QStringLiteral("version"), app.version);
    obj.insert(QStringLiteral("comment"), app.comment);
    obj.insert(QStringLiteral("managed_path"), app.managedPath);
    obj.insert(QStringLiteral("desktop_id"), app.desktopId);
    obj.insert(QStringLiteral("desktop_path"), app.desktopPath);
    obj.insert(QStringLiteral("icon_path"), app.iconPath);
    obj.insert(QStringLiteral("sha256"), QString::fromLatin1(app.sha256.toHex()));
    obj.insert(QStringLiteral("type"), appImageTypeName(app.type));
    obj.insert(QStringLiteral("architecture"), architectureName(app.architecture));
    obj.insert(QStringLiteral("size"), app.size);
    obj.insert(QStringLiteral("arguments"), QJsonArray::fromStringList(app.arguments));
    obj.insert(QStringLiteral("default_arguments"), QJsonArray::fromStringList(app.defaultArguments));
    obj.insert(QStringLiteral("environment"), envToJson(app.environment));
    obj.insert(QStringLiteral("update_manager"), app.updateManager);
    obj.insert(QStringLiteral("update_config"), QJsonObject::fromVariantMap(app.updateConfig));
    obj.insert(QStringLiteral("embedded_update"), app.embeddedUpdate);
    obj.insert(QStringLiteral("last_update_check"), app.lastUpdateCheck.toString(Qt::ISODate));
    obj.insert(QStringLiteral("available_version"), app.availableVersion);
    obj.insert(QStringLiteral("available_url"), app.availableUrl);
    obj.insert(QStringLiteral("available_size"), app.availableSize);
    obj.insert(QStringLiteral("update_available"), app.updateAvailable);
    obj.insert(QStringLiteral("digest"), app.digest);
    obj.insert(QStringLiteral("reduced_verification"), app.reducedVerification);
    obj.insert(QStringLiteral("external_folder"), app.externalFolder);
    obj.insert(QStringLiteral("owned"), app.owned);
    obj.insert(QStringLiteral("adopted"), app.adopted);
    obj.insert(QStringLiteral("website"), app.website);
    obj.insert(QStringLiteral("terminal"), app.terminal);
    return obj;
}

AppImageType typeFromName(const QString &name)
{
    if (name == QLatin1String("type-1")) {
        return AppImageType::Type1;
    }
    if (name == QLatin1String("type-2")) {
        return AppImageType::Type2;
    }
    if (name == QLatin1String("dwarfs")) {
        return AppImageType::Dwarfs;
    }
    return AppImageType::Unknown;
}

Architecture archFromName(const QString &name)
{
    if (name == QLatin1String("x86_64")) {
        return Architecture::X86_64;
    }
    if (name == QLatin1String("aarch64")) {
        return Architecture::AArch64;
    }
    if (name == QLatin1String("i386")) {
        return Architecture::I386;
    }
    if (name == QLatin1String("arm")) {
        return Architecture::Arm;
    }
    return Architecture::Unknown;
}

InstalledApp appFromJson(const QJsonObject &obj)
{
    InstalledApp app;
    app.uuid = obj.value(QStringLiteral("uuid")).toString();
    app.name = obj.value(QStringLiteral("name")).toString();
    app.version = obj.value(QStringLiteral("version")).toString();
    app.comment = obj.value(QStringLiteral("comment")).toString();
    app.managedPath = obj.value(QStringLiteral("managed_path")).toString();
    app.desktopId = obj.value(QStringLiteral("desktop_id")).toString();
    app.desktopPath = obj.value(QStringLiteral("desktop_path")).toString();
    app.iconPath = obj.value(QStringLiteral("icon_path")).toString();
    app.sha256 = QByteArray::fromHex(obj.value(QStringLiteral("sha256")).toString().toLatin1());
    app.type = typeFromName(obj.value(QStringLiteral("type")).toString());
    app.architecture = archFromName(obj.value(QStringLiteral("architecture")).toString());
    app.size = obj.value(QStringLiteral("size")).toInteger();
    for (const QJsonValue &v : obj.value(QStringLiteral("arguments")).toArray()) {
        app.arguments.append(v.toString());
    }
    for (const QJsonValue &v : obj.value(QStringLiteral("default_arguments")).toArray()) {
        app.defaultArguments.append(v.toString());
    }
    app.environment = envFromJson(obj.value(QStringLiteral("environment")).toObject());
    app.updateManager = obj.value(QStringLiteral("update_manager")).toString();
    app.updateConfig = obj.value(QStringLiteral("update_config")).toObject().toVariantMap();
    app.embeddedUpdate = obj.value(QStringLiteral("embedded_update")).toString();
    app.lastUpdateCheck = QDateTime::fromString(obj.value(QStringLiteral("last_update_check")).toString(), Qt::ISODate);
    app.availableVersion = obj.value(QStringLiteral("available_version")).toString();
    app.availableUrl = obj.value(QStringLiteral("available_url")).toString();
    app.availableSize = obj.value(QStringLiteral("available_size")).toInteger();
    app.updateAvailable = obj.value(QStringLiteral("update_available")).toBool();
    app.digest = obj.value(QStringLiteral("digest")).toString();
    app.reducedVerification = obj.value(QStringLiteral("reduced_verification")).toBool();
    app.externalFolder = obj.value(QStringLiteral("external_folder")).toBool();
    app.owned = obj.value(QStringLiteral("owned")).toBool(true);
    app.adopted = obj.value(QStringLiteral("adopted")).toBool();
    app.website = obj.value(QStringLiteral("website")).toString();
    app.terminal = obj.value(QStringLiteral("terminal")).toBool();
    return app;
}

} // namespace

bool ManagedRegistry::load(QString *error)
{
    m_apps.clear();
    if (!m_settings) {
        if (error) {
            *error = QStringLiteral("Settings unavailable");
        }
        return false;
    }
    const QString path = m_settings->registryPath();
    QFile file(path);
    if (!file.exists()) {
        return true;
    }
    if (!file.open(QIODevice::ReadOnly)) {
        if (error) {
            *error = QStringLiteral("Cannot read registry");
        }
        return false;
    }
    const QByteArray data = file.read(kMaxJsonBodyBytes + 1);
    if (data.size() > kMaxJsonBodyBytes) {
        if (error) {
            *error = QStringLiteral("Registry exceeds size bound");
        }
        return false;
    }
    QJsonParseError parseError;
    const QJsonDocument doc = QJsonDocument::fromJson(data, &parseError);
    if (!doc.isObject()) {
        if (error) {
            *error = QStringLiteral("Invalid registry JSON");
        }
        return false;
    }
    const QJsonObject root = doc.object();
    if (root.value(QStringLiteral("schema_version")).toInt() > kRegistrySchemaVersion) {
        if (error) {
            *error = QStringLiteral("Unsupported registry schema");
        }
        return false;
    }
    for (const QJsonValue &value : root.value(QStringLiteral("apps")).toArray()) {
        const InstalledApp app = appFromJson(value.toObject());
        if (!app.uuid.isEmpty() && !app.managedPath.isEmpty()) {
            m_apps.append(app);
        }
    }
    return true;
}

bool ManagedRegistry::save(QString *error)
{
    if (!m_settings) {
        if (error) {
            *error = QStringLiteral("Settings unavailable");
        }
        return false;
    }
    QJsonArray apps;
    for (const InstalledApp &app : m_apps) {
        apps.append(appToJson(app));
    }
    QJsonObject root;
    root.insert(QStringLiteral("schema_version"), kRegistrySchemaVersion);
    root.insert(QStringLiteral("apps"), apps);
    const QByteArray data = QJsonDocument(root).toJson(QJsonDocument::Indented);
    return SafeFs::atomicWrite(m_settings->registryPath(), data, error, 0600);
}

} // namespace GoshAim
