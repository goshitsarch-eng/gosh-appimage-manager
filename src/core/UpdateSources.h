#pragma once

#include "NetworkClient.h"
#include "Types.h"
#include "UrlGuard.h"

#include <QString>
#include <QStringList>
#include <QVariantMap>
#include <QVector>
#include <atomic>

namespace GoshAim {

struct UpdateCheckResult {
    bool ok = false;
    bool available = false;
    QString version;
    QString url;
    qint64 size = -1;
    QString digest;
    QString digestAlgo;
    bool reducedVerification = false;
    QString error;
    QString manager;
};

class UpdateSource
{
public:
    virtual ~UpdateSource() = default;
    virtual QString name() const = 0;
    virtual QString label() const = 0;
    virtual bool handlesEmbedded(const QString &raw) const;
    virtual bool validateConfig(const QVariantMap &config, QString *error) const = 0;
    virtual UpdateCheckResult check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel) = 0;
    virtual QVariantMap configFromEmbedded(const EmbeddedUpdateInfo &info) const;
};

class StaticFileSource : public UpdateSource
{
public:
    QString name() const override { return QStringLiteral("static"); }
    QString label() const override { return QStringLiteral("Static HTTPS file"); }
    bool handlesEmbedded(const QString &raw) const override;
    bool validateConfig(const QVariantMap &config, QString *error) const override;
    UpdateCheckResult check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel) override;
    QVariantMap configFromEmbedded(const EmbeddedUpdateInfo &info) const override;
};

class GitHubSource : public UpdateSource
{
public:
    QString name() const override { return QStringLiteral("github"); }
    QString label() const override { return QStringLiteral("GitHub releases"); }
    bool handlesEmbedded(const QString &raw) const override;
    bool validateConfig(const QVariantMap &config, QString *error) const override;
    UpdateCheckResult check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel) override;
    QVariantMap configFromEmbedded(const EmbeddedUpdateInfo &info) const override;
};

class GitLabSource : public UpdateSource
{
public:
    QString name() const override { return QStringLiteral("gitlab"); }
    QString label() const override { return QStringLiteral("GitLab releases"); }
    bool validateConfig(const QVariantMap &config, QString *error) const override;
    UpdateCheckResult check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel) override;
};

class CodebergSource : public UpdateSource
{
public:
    QString name() const override { return QStringLiteral("codeberg"); }
    QString label() const override { return QStringLiteral("Codeberg"); }
    bool validateConfig(const QVariantMap &config, QString *error) const override;
    UpdateCheckResult check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel) override;
};

class ForgejoSource : public UpdateSource
{
public:
    QString name() const override { return QStringLiteral("forgejo"); }
    QString label() const override { return QStringLiteral("Forgejo"); }
    bool validateConfig(const QVariantMap &config, QString *error) const override;
    UpdateCheckResult check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel) override;
};

class FtpSource : public UpdateSource
{
public:
    QString name() const override { return QStringLiteral("ftp"); }
    QString label() const override { return QStringLiteral("FTP (legacy, insecure)"); }
    bool validateConfig(const QVariantMap &config, QString *error) const override;
    UpdateCheckResult check(const InstalledApp &app, NetworkClient *network, std::atomic<bool> *cancel) override;
};

class UpdateSourceFactory
{
public:
    static QVector<UpdateSource *> all();
    static UpdateSource *byName(const QString &name);
    static QStringList names();
};

} // namespace GoshAim
