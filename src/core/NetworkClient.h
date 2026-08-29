#pragma once

#include "Limits.h"
#include "UrlGuard.h"

#include <QByteArray>
#include <QHash>
#include <QNetworkReply>
#include <QString>
#include <QUrl>
#include <atomic>
#include <functional>

namespace GoshAim {

struct NetworkRequest {
    QUrl url;
    QString destinationPath;
    int timeoutMs = kNetworkTimeoutMs;
    int maxRedirects = kMaxRedirects;
    qint64 maxBytes = kMaxJsonBodyBytes;
    bool allowHttp = false;
    bool allowPrivate = false;
    bool allowFtp = false;
    QString accept;
};

struct NetworkResult {
    bool ok = false;
    int status = 0;
    QByteArray body;
    QString savedPath;
    QString error;
    QString etag;
    QString lastModified;
    qint64 contentLength = -1;
    QUrl finalUrl;
    bool cancelled = false;
    bool truncated = false;
    bool redirected = false;
};

class NetworkClient
{
public:
    virtual ~NetworkClient() = default;
    virtual NetworkResult fetch(const NetworkRequest &request, std::atomic<bool> *cancel = nullptr) = 0;
};

class QtNetworkClient : public NetworkClient
{
public:
    NetworkResult fetch(const NetworkRequest &request, std::atomic<bool> *cancel = nullptr) override;
};

} // namespace GoshAim
