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
#include <memory>

namespace GoshAim {

class FileSink
{
public:
    virtual ~FileSink() = default;
    virtual bool open(const QString &path, QString *error) = 0;
    virtual qint64 write(const char *data, qint64 size) = 0;
    virtual bool flush() = 0;
    virtual bool sync() = 0;
    virtual void close() = 0;
    virtual QString path() const = 0;
};

struct NetworkRequest {
    QUrl url;
    QString destinationPath;
    int timeoutMs = kNetworkTimeoutMs;
    int maxRedirects = kMaxRedirects;
    qint64 maxBytes = kMaxJsonBodyBytes;
    bool allowHttp = false;
    bool allowPrivate = false;
    bool allowFtp = false;
    bool metadataOnly = false;
    QString accept;
    std::function<void(qint64 received, qint64 total)> progress;
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
    QString digest;
    qint64 bytesTransferred = 0;
    QString method;
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
    using FileSinkFactory = std::function<std::unique_ptr<FileSink>()>;

    explicit QtNetworkClient(HostResolver *resolver = nullptr);
    void setResolver(HostResolver *resolver);
    void setFileSinkFactory(FileSinkFactory factory);
    NetworkResult fetch(const NetworkRequest &request, std::atomic<bool> *cancel = nullptr) override;
    QString lastMethod() const { return m_lastMethod; }
    bool lastMetadataOnly() const { return m_lastMetadataOnly; }

private:
    HostResolver *m_resolver = nullptr;
    QtHostResolver m_defaultResolver;
    FileSinkFactory m_sinkFactory;
    QString m_lastMethod;
    bool m_lastMetadataOnly = false;
};

} // namespace GoshAim
