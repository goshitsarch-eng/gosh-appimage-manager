#include "UpdateSources.h"

#include "Limits.h"
#include "SafeFs.h"

#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QUrlQuery>

namespace GoshAim {

namespace {

StaticFileSource g_static;
GitHubSource g_github;
GitLabSource g_gitlab;
CodebergSource g_codeberg;
ForgejoSource g_forgejo;
FtpSource g_ftp;

bool wildcardMatch(const QString &pattern, const QString &name)
{
    QRegularExpression re(QRegularExpression::wildcardToRegularExpression(pattern, QRegularExpression::UnanchoredWildcardConversion),
                          QRegularExpression::CaseInsensitiveOption);
    return re.match(name).hasMatch();
}

QString jsonString(const QJsonObject &obj, const QString &key)
{
    return obj.value(key).toString();
}

UpdateCheckResult fail(const QString &error, const QString &manager)
{
    UpdateCheckResult result;
    result.error = error;
    result.manager = manager;
    return result;
}

qint64 installedSize(const InstalledApp &app)
{
    if (app.size > 0) {
        return app.size;
    }
    if (!app.managedPath.isEmpty()) {
        const qint64 n = QFileInfo(app.managedPath).size();
        if (n > 0) {
            return n;
        }
    }
    return 0;
}

QString normalizeOneLeadingV(const QString &value)
{
    const QString trimmed = value.trimmed();
    if (trimmed.size() >= 2 && (trimmed[0] == QLatin1Char('v') || trimmed[0] == QLatin1Char('V')) && trimmed[1].isDigit()) {
        return trimmed.mid(1);
    }
    return trimmed;
}

bool versionsEquivalent(const QString &left, const QString &right)
{
    if (left.trimmed().isEmpty() || right.trimmed().isEmpty()) {
        return false;
    }
    if (left.trimmed() == right.trimmed()) {
        return true;
    }
    return normalizeOneLeadingV(left) == normalizeOneLeadingV(right);
}

bool forgeOfferAvailable(const UpdateCheckResult &result, const InstalledApp &app)
{
    const qint64 localSize = installedSize(app);
    const bool sizeSupplied = result.size > 0 && localSize > 0;
    const bool sizeDiffers = sizeSupplied && result.size != localSize;

    if (!result.digest.trimmed().isEmpty() && !app.sha256.isEmpty()) {
        if (SafeFs::digestMatches(result.digest, app.sha256)) {
            if (sizeSupplied) {
                return sizeDiffers;
            }
            return false;
        }
        return true;
    }

    const QString applied = app.updateConfig.value(QStringLiteral("_applied_version")).toString();
    if (!result.version.trimmed().isEmpty()) {
        if (!applied.isEmpty()) {
            return !versionsEquivalent(result.version, applied);
        }
        if (!app.version.isEmpty()) {
            return !versionsEquivalent(result.version, app.version);
        }
    }

    if (sizeSupplied) {
        return sizeDiffers;
    }
    return false;
}

bool pickAsset(const QJsonArray &assets,
               const QString &filename,
               const QString &nameKey,
               const QString &urlKey,
               const QString &sizeKey,
               const QString &digestKey,
               UpdateCheckResult *out)
{
    for (const QJsonValue &value : assets) {
        const QJsonObject asset = value.toObject();
        const QString name = asset.value(nameKey).toString();
        if (filename.isEmpty() || wildcardMatch(filename, name) || name.contains(QLatin1String(".AppImage"), Qt::CaseInsensitive)) {
            if (!filename.isEmpty() && !wildcardMatch(filename, name) && !name.endsWith(QLatin1String(".AppImage"), Qt::CaseInsensitive)
                && !name.endsWith(QLatin1String(".appimage"), Qt::CaseInsensitive)) {
                continue;
            }
            if (!filename.isEmpty() && !wildcardMatch(filename, name)) {
                continue;
            }
            out->url = asset.value(urlKey).toString();
            out->size = asset.value(sizeKey).toInteger(-1);
            out->digest = asset.value(digestKey).toString();
            if (out->digest.isEmpty()) {
                out->digest = asset.value(QStringLiteral("digest")).toString();
            }
            return !out->url.isEmpty();
        }
    }
    return false;
}

} // namespace

bool UpdateSource::handlesEmbedded(const QString &raw) const
{
    Q_UNUSED(raw);
    return false;
}

QVariantMap UpdateSource::configFromEmbedded(const EmbeddedUpdateInfo &info) const
{
    return info.fields;
}

bool StaticFileSource::handlesEmbedded(const QString &raw) const
{
    return raw.startsWith(QLatin1String("zsync|"));
}

QVariantMap StaticFileSource::configFromEmbedded(const EmbeddedUpdateInfo &info) const
{
    QVariantMap map;
    map.insert(QStringLiteral("url"), info.fields.value(QStringLiteral("url")));
    return map;
}

bool StaticFileSource::validateConfig(const QVariantMap &config, QString *error) const
{
    const UrlCheck check = UrlGuard::validate(config.value(QStringLiteral("url")).toString(), false, false, false);
    if (!check.ok) {
        if (error) {
            *error = check.error;
        }
        return false;
    }
    return true;
}

UpdateCheckResult StaticFileSource::check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel)
{
    QString url = app.updateConfig.value(QStringLiteral("url")).toString();
    if (url.isEmpty()) {
        url = app.embeddedUpdate.section(QLatin1Char('|'), 1);
    }
    QString error;
    QVariantMap cfg;
    cfg.insert(QStringLiteral("url"), url);
    if (!validateConfig(cfg, &error)) {
        return fail(error, name());
    }
    if (url.endsWith(QLatin1String(".zsync"), Qt::CaseInsensitive)) {
        NetworkRequest req;
        req.url = QUrl(url);
        req.maxBytes = kMaxZsyncBytes;
        const NetworkResult body = network->fetch(req, cancel);
        if (!body.ok) {
            return fail(body.error, name());
        }
        UpdateCheckResult result;
        result.ok = true;
        result.manager = name();
        result.reducedVerification = true;
        const QString text = QString::fromUtf8(body.body);
        for (const QString &line : text.split(QLatin1Char('\n'))) {
            if (line.startsWith(QLatin1String("URL: "))) {
                result.url = line.mid(5).trimmed();
            } else if (line.startsWith(QLatin1String("SHA-1: "))) {
                result.digest = line.mid(7).trimmed();
                result.digestAlgo = QStringLiteral("sha1");
            } else if (line.startsWith(QLatin1String("Length: "))) {
                result.size = line.mid(8).trimmed().toLongLong();
            } else if (line.startsWith(QLatin1String("Filename: "))) {
                result.version = line.mid(10).trimmed();
            }
        }
        result.etag = body.etag;
        result.lastModified = body.lastModified;
        if (result.url.isEmpty()) {
            result.url = url;
            result.url.chop(6);
        }
        const UrlCheck check = UrlGuard::validate(result.url);
        if (!check.ok) {
            return fail(check.error, name());
        }
        bool changed = true;
        if (!result.digest.isEmpty() && !app.managedPath.isEmpty() && QFileInfo::exists(app.managedPath)) {
            const HashResult hashed = SafeFs::sha1File(app.managedPath, kDefaultMaxAppImageBytes, cancel);
            if (!hashed.sha256.isEmpty()) {
                changed = !SafeFs::digestMatches(result.digest, hashed.sha256);
            }
        } else if (result.size > 0) {
            const qint64 local = installedSize(app);
            if (local > 0) {
                changed = result.size != local;
            } else {
                const qint64 appliedSize = app.updateConfig.value(QStringLiteral("_applied_size")).toLongLong();
                changed = appliedSize <= 0 || result.size != appliedSize;
            }
        } else if (!result.url.isEmpty()) {
            const QString appliedUrl = app.updateConfig.value(QStringLiteral("_applied_url")).toString();
            if (!appliedUrl.isEmpty()) {
                changed = result.url != appliedUrl;
            }
        }
        result.available = changed;
        return result;
    }
    NetworkRequest req;
    req.url = QUrl(url);
    req.metadataOnly = true;
    req.maxBytes = 4096;
    const NetworkResult head = network->fetch(req, cancel);
    UpdateCheckResult result;
    result.manager = name();
    result.ok = head.ok;
    result.url = url;
    result.size = head.contentLength;
    result.etag = head.etag;
    result.lastModified = head.lastModified;
    result.digest = head.digest;
    result.reducedVerification = head.digest.isEmpty();
    result.version = head.lastModified;
    if (!head.ok) {
        result.error = head.error;
        return result;
    }
    const qint64 localSize = installedSize(app);
    bool changed = false;
    if (!head.digest.isEmpty() && !app.sha256.isEmpty()) {
        changed = !SafeFs::digestMatches(head.digest, app.sha256);
    } else {
        bool compared = false;
        if (head.contentLength > 0 && localSize > 0) {
            compared = true;
            changed = head.contentLength != localSize;
        } else if (head.contentLength > 0 && localSize <= 0) {
            compared = true;
            changed = true;
        } else if (!app.version.isEmpty() && !head.lastModified.isEmpty()) {
            compared = true;
            changed = head.lastModified != app.version;
        }
        if (!compared) {
            const QString appliedEtag = app.updateConfig.value(QStringLiteral("_applied_etag")).toString();
            const qint64 appliedSize = app.updateConfig.value(QStringLiteral("_applied_size")).toLongLong();
            const QString appliedVersion = app.updateConfig.value(QStringLiteral("_applied_version")).toString();
            const QString appliedModified = app.updateConfig.value(QStringLiteral("_applied_modified")).toString();
            if (!head.etag.isEmpty() && !appliedEtag.isEmpty()) {
                changed = head.etag != appliedEtag;
            } else if (head.contentLength > 0 && appliedSize > 0) {
                changed = head.contentLength != appliedSize;
            } else if (!head.lastModified.isEmpty() && !appliedModified.isEmpty()) {
                changed = head.lastModified != appliedModified;
            } else if (!head.lastModified.isEmpty() && !appliedVersion.isEmpty()) {
                changed = head.lastModified != appliedVersion;
            } else {
                changed = false;
            }
        }
    }
    result.available = changed;
    return result;
}

bool GitHubSource::handlesEmbedded(const QString &raw) const
{
    return raw.startsWith(QLatin1String("gh-releases-zsync|"));
}

QVariantMap GitHubSource::configFromEmbedded(const EmbeddedUpdateInfo &info) const
{
    QVariantMap map;
    map.insert(QStringLiteral("username"), info.fields.value(QStringLiteral("username")));
    map.insert(QStringLiteral("repo"), info.fields.value(QStringLiteral("repo")));
    map.insert(QStringLiteral("filename"), info.fields.value(QStringLiteral("filename")));
    map.insert(QStringLiteral("release"), info.fields.value(QStringLiteral("release")));
    return map;
}

bool GitHubSource::validateConfig(const QVariantMap &config, QString *error) const
{
    const QString user = config.value(QStringLiteral("username")).toString();
    const QString repo = config.value(QStringLiteral("repo")).toString();
    if (!UrlGuard::isSafeRepoComponent(user) || !UrlGuard::isSafeRepoComponent(repo)) {
        if (error) {
            *error = QStringLiteral("Invalid GitHub owner or repository");
        }
        return false;
    }
    return true;
}

UpdateCheckResult GitHubSource::check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel)
{
    QVariantMap cfg = app.updateConfig;
    if (cfg.isEmpty() && app.embeddedUpdate.startsWith(QLatin1String("gh-releases-zsync|"))) {
        const QStringList parts = app.embeddedUpdate.split(QLatin1Char('|'));
        if (parts.size() == 5) {
            cfg.insert(QStringLiteral("username"), parts[1]);
            cfg.insert(QStringLiteral("repo"), parts[2]);
            cfg.insert(QStringLiteral("filename"), parts[4]);
        }
    }
    QString error;
    if (!validateConfig(cfg, &error)) {
        return fail(error, name());
    }
    const QString user = cfg.value(QStringLiteral("username")).toString();
    const QString repo = cfg.value(QStringLiteral("repo")).toString();
    const QString filename = cfg.value(QStringLiteral("filename")).toString();
    NetworkRequest req;
    req.url = QUrl(QStringLiteral("https://api.github.com/repos/%1/%2/releases/latest")
                       .arg(UrlGuard::encodePathSegment(user), UrlGuard::encodePathSegment(repo)));
    req.accept = QStringLiteral("application/vnd.github+json");
    const NetworkResult body = network->fetch(req, cancel);
    if (!body.ok) {
        return fail(body.error, name());
    }
    const QJsonObject obj = QJsonDocument::fromJson(body.body).object();
    UpdateCheckResult result;
    result.manager = name();
    result.ok = true;
    result.version = jsonString(obj, QStringLiteral("tag_name"));
    if (!pickAsset(obj.value(QStringLiteral("assets")).toArray(),
                   filename,
                   QStringLiteral("name"),
                   QStringLiteral("browser_download_url"),
                   QStringLiteral("size"),
                   QStringLiteral("digest"),
                   &result)) {
        return fail(QStringLiteral("No matching GitHub asset"), name());
    }
    const UrlCheck check = UrlGuard::validate(result.url);
    if (!check.ok) {
        return fail(check.error, name());
    }
    const QString host = QUrl(result.url).host().toLower();
    if (host != QLatin1String("github.com") && !host.endsWith(QLatin1String(".githubusercontent.com"))) {
        return fail(QStringLiteral("GitHub asset host is not allowed"), name());
    }
    result.available = forgeOfferAvailable(result, app);
    result.reducedVerification = result.digest.isEmpty();
    return result;
}

bool GitLabSource::validateConfig(const QVariantMap &config, QString *error) const
{
    const QString project = config.value(QStringLiteral("project")).toString();
    if (project.isEmpty() || project.contains(QLatin1String("..")) || project.contains(QChar(0))) {
        if (error) {
            *error = QStringLiteral("Invalid GitLab project");
        }
        return false;
    }
    const QString host = config.value(QStringLiteral("host"), QStringLiteral("gitlab.com")).toString().toLower();
    if (host != QLatin1String("gitlab.com") && UrlGuard::isPrivateHost(host)) {
        if (error) {
            *error = QStringLiteral("Private GitLab hosts require an explicit private-network opt-in");
        }
        return false;
    }
    if (host.contains(QLatin1Char('/')) || host.contains(QLatin1Char(':'))) {
        if (error) {
            *error = QStringLiteral("Invalid GitLab host");
        }
        return false;
    }
    return true;
}

UpdateCheckResult GitLabSource::check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel)
{
    QString error;
    if (!validateConfig(app.updateConfig, &error)) {
        return fail(error, name());
    }
    const QString host = app.updateConfig.value(QStringLiteral("host"), QStringLiteral("gitlab.com")).toString();
    const QString project = app.updateConfig.value(QStringLiteral("project")).toString();
    const QString filename = app.updateConfig.value(QStringLiteral("filename")).toString();
    NetworkRequest req;
    req.url = QUrl(QStringLiteral("https://%1/api/v4/projects/%2/releases")
                       .arg(host, QString::fromUtf8(QUrl::toPercentEncoding(project))));
    const NetworkResult body = network->fetch(req, cancel);
    if (!body.ok) {
        return fail(body.error, name());
    }
    const QJsonArray releases = QJsonDocument::fromJson(body.body).array();
    if (releases.isEmpty()) {
        return fail(QStringLiteral("No GitLab releases"), name());
    }
    const QJsonObject rel = releases.first().toObject();
    UpdateCheckResult result;
    result.manager = name();
    result.ok = true;
    result.version = jsonString(rel, QStringLiteral("tag_name"));
    const QJsonArray links = rel.value(QStringLiteral("assets")).toObject().value(QStringLiteral("links")).toArray();
    if (!pickAsset(links, filename, QStringLiteral("name"), QStringLiteral("url"), QStringLiteral("size"), QStringLiteral("checksum"), &result)) {
        for (const QJsonValue &value : links) {
            const QJsonObject link = value.toObject();
            const QString direct = link.value(QStringLiteral("direct_asset_url")).toString();
            const QString url = direct.isEmpty() ? link.value(QStringLiteral("url")).toString() : direct;
            if (url.contains(QLatin1String(".AppImage"), Qt::CaseInsensitive)) {
                result.url = url;
                result.size = link.value(QStringLiteral("size")).toInteger(-1);
                result.digest = link.value(QStringLiteral("checksum")).toString();
                break;
            }
            if (result.url.isEmpty()) {
                result.url = url;
                result.size = link.value(QStringLiteral("size")).toInteger(-1);
            }
        }
    }
    if (result.url.isEmpty() && (app.updateConfig.contains(QStringLiteral("package")) || app.updateConfig.contains(QStringLiteral("package_name")))) {
        const QString packageName = app.updateConfig.value(QStringLiteral("package"), app.updateConfig.value(QStringLiteral("package_name"))).toString();
        NetworkRequest pkgReq;
        pkgReq.url = QUrl(QStringLiteral("https://%1/api/v4/projects/%2/packages")
                              .arg(host, QString::fromUtf8(QUrl::toPercentEncoding(project))));
        const NetworkResult pkgBody = network->fetch(pkgReq, cancel);
        if (!pkgBody.ok) {
            return fail(pkgBody.error, name());
        }
        const QJsonArray packages = QJsonDocument::fromJson(pkgBody.body).array();
        int packageId = -1;
        for (const QJsonValue &value : packages) {
            const QJsonObject pkg = value.toObject();
            if (packageName.isEmpty() || pkg.value(QStringLiteral("name")).toString() == packageName) {
                packageId = pkg.value(QStringLiteral("id")).toInt(-1);
                result.version = pkg.value(QStringLiteral("version")).toString();
                break;
            }
        }
        if (packageId < 0) {
            return fail(QStringLiteral("No GitLab package matched"), name());
        }
        NetworkRequest filesReq;
        filesReq.url = QUrl(QStringLiteral("https://%1/api/v4/projects/%2/packages/%3/package_files")
                                .arg(host, QString::fromUtf8(QUrl::toPercentEncoding(project)), QString::number(packageId)));
        const NetworkResult filesBody = network->fetch(filesReq, cancel);
        if (!filesBody.ok) {
            return fail(filesBody.error, name());
        }
        const QJsonArray files = QJsonDocument::fromJson(filesBody.body).array();
        for (const QJsonValue &value : files) {
            const QJsonObject file = value.toObject();
            const QString fileName = file.value(QStringLiteral("file_name")).toString();
            if (filename.isEmpty() || wildcardMatch(filename, fileName) || fileName.endsWith(QLatin1String(".AppImage"), Qt::CaseInsensitive)) {
                result.url = QStringLiteral("https://%1/api/v4/projects/%2/packages/%3/package_files/%4/download")
                                 .arg(host,
                                      QString::fromUtf8(QUrl::toPercentEncoding(project)),
                                      QString::number(packageId),
                                      QString::number(file.value(QStringLiteral("id")).toInt()));
                result.size = file.value(QStringLiteral("size")).toInteger(-1);
                result.digest = file.value(QStringLiteral("file_sha256")).toString();
                if (result.digest.isEmpty()) {
                    result.digest = file.value(QStringLiteral("file_md5")).toString();
                }
                break;
            }
        }
    }
    const UrlCheck check = UrlGuard::validate(result.url);
    if (!check.ok) {
        return fail(check.error, name());
    }
    result.available = forgeOfferAvailable(result, app);
    result.reducedVerification = result.digest.isEmpty();
    return result;
}

bool CodebergSource::validateConfig(const QVariantMap &config, QString *error) const
{
    const QString user = config.value(QStringLiteral("username")).toString();
    const QString repo = config.value(QStringLiteral("repo")).toString();
    if (!UrlGuard::isSafeRepoComponent(user) || !UrlGuard::isSafeRepoComponent(repo)) {
        if (error) {
            *error = QStringLiteral("Invalid Codeberg owner or repository");
        }
        return false;
    }
    return true;
}

UpdateCheckResult CodebergSource::check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel)
{
    QString error;
    if (!validateConfig(app.updateConfig, &error)) {
        return fail(error, name());
    }
    const QString user = app.updateConfig.value(QStringLiteral("username")).toString();
    const QString repo = app.updateConfig.value(QStringLiteral("repo")).toString();
    const QString filename = app.updateConfig.value(QStringLiteral("filename")).toString();
    NetworkRequest req;
    req.url = QUrl(QStringLiteral("https://codeberg.org/api/v1/repos/%1/%2/releases")
                       .arg(UrlGuard::encodePathSegment(user), UrlGuard::encodePathSegment(repo)));
    const NetworkResult body = network->fetch(req, cancel);
    if (!body.ok) {
        return fail(body.error, name());
    }
    const QJsonArray releases = QJsonDocument::fromJson(body.body).array();
    if (releases.isEmpty()) {
        return fail(QStringLiteral("No Codeberg releases"), name());
    }
    const QJsonObject rel = releases.first().toObject();
    UpdateCheckResult result;
    result.manager = name();
    result.ok = true;
    result.version = jsonString(rel, QStringLiteral("tag_name"));
    pickAsset(rel.value(QStringLiteral("assets")).toArray(),
              filename,
              QStringLiteral("name"),
              QStringLiteral("browser_download_url"),
              QStringLiteral("size"),
              QStringLiteral("digest"),
              &result);
    const UrlCheck check = UrlGuard::validate(result.url);
    if (!check.ok) {
        return fail(check.error, name());
    }
    const QString host = QUrl(result.url).host().toLower();
    if (host != QLatin1String("codeberg.org")) {
        return fail(QStringLiteral("Codeberg asset host is not allowed"), name());
    }
    result.available = forgeOfferAvailable(result, app);
    result.reducedVerification = result.digest.isEmpty();
    return result;
}

bool ForgejoSource::validateConfig(const QVariantMap &config, QString *error) const
{
    const QString host = config.value(QStringLiteral("host")).toString().toLower();
    const QString user = config.value(QStringLiteral("username")).toString();
    const QString repo = config.value(QStringLiteral("repo")).toString();
    if (host.isEmpty() || host.contains(QLatin1Char('/')) || UrlGuard::isPrivateHost(host)) {
        if (error) {
            *error = QStringLiteral("Forgejo host must be a public HTTPS hostname");
        }
        return false;
    }
    if (!UrlGuard::isSafeRepoComponent(user) || !UrlGuard::isSafeRepoComponent(repo)) {
        if (error) {
            *error = QStringLiteral("Invalid Forgejo owner or repository");
        }
        return false;
    }
    return true;
}

UpdateCheckResult ForgejoSource::check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel)
{
    QString error;
    if (!validateConfig(app.updateConfig, &error)) {
        return fail(error, name());
    }
    const QString host = app.updateConfig.value(QStringLiteral("host")).toString();
    const QString user = app.updateConfig.value(QStringLiteral("username")).toString();
    const QString repo = app.updateConfig.value(QStringLiteral("repo")).toString();
    const QString filename = app.updateConfig.value(QStringLiteral("filename")).toString();
    NetworkRequest req;
    req.url = QUrl(QStringLiteral("https://%1/api/v1/repos/%2/%3/releases")
                       .arg(host, UrlGuard::encodePathSegment(user), UrlGuard::encodePathSegment(repo)));
    const NetworkResult body = network->fetch(req, cancel);
    if (!body.ok) {
        return fail(body.error, name());
    }
    const QJsonArray releases = QJsonDocument::fromJson(body.body).array();
    if (releases.isEmpty()) {
        return fail(QStringLiteral("No Forgejo releases"), name());
    }
    const QJsonObject rel = releases.first().toObject();
    UpdateCheckResult result;
    result.manager = name();
    result.ok = true;
    result.version = jsonString(rel, QStringLiteral("tag_name"));
    pickAsset(rel.value(QStringLiteral("assets")).toArray(),
              filename,
              QStringLiteral("name"),
              QStringLiteral("browser_download_url"),
              QStringLiteral("size"),
              QStringLiteral("digest"),
              &result);
    const UrlCheck check = UrlGuard::validate(result.url);
    if (!check.ok) {
        return fail(check.error, name());
    }
    if (QUrl(result.url).host().toLower() != host.toLower()) {
        return fail(QStringLiteral("Forgejo asset host mismatch"), name());
    }
    result.available = forgeOfferAvailable(result, app);
    result.reducedVerification = result.digest.isEmpty();
    return result;
}

bool FtpSource::validateConfig(const QVariantMap &config, QString *error) const
{
    const UrlCheck check = UrlGuard::validate(config.value(QStringLiteral("url")).toString(), false, false, true);
    if (!check.ok) {
        if (error) {
            *error = check.error;
        }
        return false;
    }
    if (error) {
        *error = QStringLiteral("FTP is a legacy insecure transport");
    }
    return true;
}

UpdateCheckResult FtpSource::check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel)
{
    QString error;
    if (!validateConfig(app.updateConfig, &error)) {
        return fail(error.isEmpty() ? QStringLiteral("Invalid FTP URL") : error, name());
    }
    const QString url = app.updateConfig.value(QStringLiteral("url")).toString();
    NetworkRequest req;
    req.url = QUrl(url);
    req.allowFtp = true;
    req.metadataOnly = true;
    req.maxBytes = 4096;
    const NetworkResult head = network->fetch(req, cancel);
    UpdateCheckResult result;
    result.manager = name();
    result.url = url;
    result.ok = head.ok;
    result.size = head.contentLength;
    result.etag = head.etag;
    result.lastModified = head.lastModified;
    result.version = head.lastModified;
    result.reducedVerification = true;
    result.error = QStringLiteral("FTP is a legacy insecure transport");
    if (!head.ok) {
        result.error = head.error.isEmpty() ? result.error : head.error;
        return result;
    }
    const qint64 localSize = installedSize(app);
    if (head.contentLength > 0 && localSize > 0) {
        result.available = head.contentLength != localSize;
    } else if (head.contentLength > 0 && localSize <= 0) {
        result.available = true;
    } else if (!head.lastModified.isEmpty() && !app.version.isEmpty()) {
        result.available = head.lastModified != app.version;
    } else {
        const QString appliedEtag = app.updateConfig.value(QStringLiteral("_applied_etag")).toString();
        const qint64 appliedSize = app.updateConfig.value(QStringLiteral("_applied_size")).toLongLong();
        const QString appliedVersion = app.updateConfig.value(QStringLiteral("_applied_version")).toString();
        if (!head.etag.isEmpty() && !appliedEtag.isEmpty()) {
            result.available = head.etag != appliedEtag;
        } else if (head.contentLength > 0 && appliedSize > 0) {
            result.available = head.contentLength != appliedSize;
        } else if (!head.lastModified.isEmpty() && !appliedVersion.isEmpty()) {
            result.available = head.lastModified != appliedVersion;
        } else {
            result.available = false;
        }
    }
    return result;
}

QVector<UpdateSource *> UpdateSourceFactory::all()
{
    return {&g_static, &g_github, &g_gitlab, &g_codeberg, &g_forgejo, &g_ftp};
}

UpdateSource *UpdateSourceFactory::byName(const QString &name)
{
    const QString lower = name.toLower();
    for (UpdateSource *source : all()) {
        if (source->name() == lower || source->name() + QStringLiteral("updater") == lower) {
            return source;
        }
    }
    if (lower.contains(QLatin1String("github"))) {
        return &g_github;
    }
    if (lower.contains(QLatin1String("gitlab"))) {
        return &g_gitlab;
    }
    if (lower.contains(QLatin1String("codeberg"))) {
        return &g_codeberg;
    }
    if (lower.contains(QLatin1String("forgejo"))) {
        return &g_forgejo;
    }
    if (lower.contains(QLatin1String("static")) || lower.contains(QLatin1String("file"))) {
        return &g_static;
    }
    if (lower.contains(QLatin1String("ftp"))) {
        return &g_ftp;
    }
    return nullptr;
}

QStringList UpdateSourceFactory::names()
{
    QStringList names;
    for (UpdateSource *source : all()) {
        names.append(source->name());
    }
    return names;
}

} // namespace GoshAim
