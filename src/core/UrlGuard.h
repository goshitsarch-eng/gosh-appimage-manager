#pragma once

#include "Limits.h"

#include <QString>
#include <QUrl>

namespace GoshAim {

struct UrlCheck {
    bool ok = false;
    QString error;
    QUrl url;
    bool privateHost = false;
    bool insecure = false;
};

class UrlGuard
{
public:
    static UrlCheck validate(const QUrl &url, bool allowHttp = false, bool allowPrivate = false, bool allowFtp = false);
    static UrlCheck validate(const QString &url, bool allowHttp = false, bool allowPrivate = false, bool allowFtp = false);
    static bool isPrivateHost(const QString &host);
    static bool isSafeApiHost(const QString &host, const QStringList &allowed);
    static QString encodePathSegment(const QString &segment);
    static bool isSafeRepoComponent(const QString &value);
};

} // namespace GoshAim
