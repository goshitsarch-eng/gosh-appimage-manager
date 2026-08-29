#pragma once

#include "Limits.h"

#include <QHostAddress>
#include <QList>
#include <QString>
#include <QUrl>

namespace GoshAim {

struct UrlCheck {
    bool ok = false;
    QString error;
    QUrl url;
    bool privateHost = false;
    bool insecure = false;
    QList<QHostAddress> resolved;
};

class HostResolver
{
public:
    virtual ~HostResolver() = default;
    virtual QList<QHostAddress> resolve(const QString &host) = 0;
};

class QtHostResolver : public HostResolver
{
public:
    QList<QHostAddress> resolve(const QString &host) override;
};

class UrlGuard
{
public:
    static UrlCheck validate(const QUrl &url, bool allowHttp = false, bool allowPrivate = false, bool allowFtp = false);
    static UrlCheck validate(const QString &url, bool allowHttp = false, bool allowPrivate = false, bool allowFtp = false);
    static UrlCheck validateResolved(const QUrl &url,
                                     HostResolver *resolver,
                                     bool allowHttp = false,
                                     bool allowPrivate = false,
                                     bool allowFtp = false);
    static bool isPrivateHost(const QString &host);
    static bool isDisallowedAddress(const QHostAddress &address);
    static bool isSafeApiHost(const QString &host, const QStringList &allowed);
    static QString encodePathSegment(const QString &segment);
    static bool isSafeRepoComponent(const QString &value);
};

} // namespace GoshAim
