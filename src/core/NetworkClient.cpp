#include "NetworkClient.h"

#include <QAbstractSocket>
#include <QEventLoop>
#include <QFile>
#include <QHostAddress>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QTimer>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

namespace GoshAim {

namespace {

class QtFileSink final : public FileSink
{
public:
    bool open(const QString &path, QString *error) override
    {
        m_path = path;
        const int fd = ::open(path.toLocal8Bit().constData(),
                              O_WRONLY | O_CREAT | O_TRUNC | O_NOFOLLOW | O_CLOEXEC,
                              0600);
        if (fd < 0) {
            if (error) {
                *error = QStringLiteral("Cannot open download destination");
            }
            return false;
        }
        if (!m_file.open(fd, QIODevice::WriteOnly, QFileDevice::AutoCloseHandle)) {
            ::close(fd);
            if (error) {
                *error = QStringLiteral("Cannot open download destination");
            }
            return false;
        }
        ::fchmod(m_file.handle(), 0600);
        return true;
    }

    qint64 write(const char *data, qint64 size) override
    {
        qint64 off = 0;
        while (off < size) {
            const qint64 n = m_file.write(data + off, size - off);
            if (n <= 0) {
                return -1;
            }
            off += n;
        }
        return off;
    }

    bool flush() override
    {
        return m_file.flush();
    }

    bool sync() override
    {
        const int fd = m_file.handle();
        if (fd < 0) {
            return false;
        }
        return ::fsync(fd) == 0;
    }

    void close() override
    {
        if (m_file.isOpen()) {
            m_file.close();
        }
    }

    QString path() const override
    {
        return m_path;
    }

private:
    QFile m_file;
    QString m_path;
};

bool isDowngrade(const QUrl &from, const QUrl &to)
{
    return from.scheme().toLower() == QLatin1String("https") && to.scheme().toLower() != QLatin1String("https");
}

QHostAddress pickPinnedAddress(const QList<QHostAddress> &addresses, bool allowPrivate)
{
    for (const QHostAddress &address : addresses) {
        if (address.isNull()) {
            continue;
        }
        if (!UrlGuard::isDisallowedAddress(address) || allowPrivate) {
            return address;
        }
    }
    return {};
}

QUrl pinToAddress(const QUrl &url, const QHostAddress &address)
{
    QUrl pinned = url;
    if (address.protocol() == QAbstractSocket::IPv6Protocol) {
        pinned.setHost(address.toString());
    } else {
        pinned.setHost(QHostAddress(address.toIPv4Address()).toString());
    }
    return pinned;
}

QByteArray hostHeaderValue(const QUrl &url)
{
    const QString host = url.host();
    const int port = url.port();
    const QString scheme = url.scheme().toLower();
    const int implied = scheme == QLatin1String("https") ? 443 : (scheme == QLatin1String("http") ? 80 : -1);
    if (port == -1 || port == implied) {
        return host.toUtf8();
    }
    return (host + QLatin1Char(':') + QString::number(port)).toUtf8();
}

bool schemeSupportsHead(const QUrl &url)
{
    const QString scheme = url.scheme().toLower();
    return scheme == QLatin1String("http") || scheme == QLatin1String("https");
}

} // namespace

QtNetworkClient::QtNetworkClient(HostResolver *resolver)
    : m_resolver(resolver)
{
}

void QtNetworkClient::setResolver(HostResolver *resolver)
{
    m_resolver = resolver;
}

void QtNetworkClient::setFileSinkFactory(FileSinkFactory factory)
{
    m_sinkFactory = std::move(factory);
}

NetworkResult QtNetworkClient::fetch(const NetworkRequest &request, std::atomic<bool> *cancel)
{
    NetworkResult result;
    HostResolver *resolver = m_resolver ? m_resolver : &m_defaultResolver;
    UrlCheck check = UrlGuard::validateResolved(request.url, resolver, request.allowHttp, request.allowPrivate, request.allowFtp);
    if (!check.ok) {
        result.error = check.error;
        return result;
    }

    QNetworkAccessManager manager;
    QUrl logical = request.url;
    int redirects = 0;
    bool usedHead = false;
    while (true) {
        if (cancel && cancel->load()) {
            result.cancelled = true;
            result.error = QStringLiteral("Cancelled");
            if (!request.destinationPath.isEmpty()) {
                QFile::remove(request.destinationPath);
            }
            return result;
        }
        UrlCheck hop = UrlGuard::validateResolved(logical, resolver, request.allowHttp, request.allowPrivate, request.allowFtp);
        if (!hop.ok) {
            result.error = hop.error;
            if (!request.destinationPath.isEmpty()) {
                QFile::remove(request.destinationPath);
            }
            return result;
        }
        const QHostAddress pinned = pickPinnedAddress(hop.resolved, request.allowPrivate);
        if (pinned.isNull()) {
            result.error = QStringLiteral("Resolved address is private, loopback, link-local, multicast or unspecified");
            if (!request.destinationPath.isEmpty()) {
                QFile::remove(request.destinationPath);
            }
            return result;
        }

        const QUrl connectUrl = pinToAddress(logical, pinned);
        QNetworkRequest req(connectUrl);
        req.setAttribute(QNetworkRequest::RedirectPolicyAttribute, QNetworkRequest::ManualRedirectPolicy);
        req.setMaximumRedirectsAllowed(0);
        req.setTransferTimeout(request.timeoutMs);
        req.setRawHeader("Host", hostHeaderValue(logical));
        req.setPeerVerifyName(logical.host());
        if (!request.accept.isEmpty()) {
            req.setRawHeader("Accept", request.accept.toUtf8());
        }
        req.setHeader(QNetworkRequest::UserAgentHeader, QStringLiteral("GoshAppImageManager/0.1"));

        const bool wantHead = request.metadataOnly && schemeSupportsHead(logical) && !usedHead;
        m_lastMetadataOnly = request.metadataOnly;
        m_lastMethod = wantHead ? QStringLiteral("HEAD") : QStringLiteral("GET");
        result.method = m_lastMethod;

        QNetworkReply *reply = wantHead ? manager.head(req) : manager.get(req);
        QEventLoop loop;
        QTimer timer;
        timer.setSingleShot(true);
        QObject::connect(reply, &QNetworkReply::finished, &loop, &QEventLoop::quit);
        QObject::connect(&timer, &QTimer::timeout, &loop, [&]() {
            reply->abort();
            loop.quit();
        });

        QByteArray held;
        qint64 written = 0;
        bool truncated = false;
        bool peerReady = logical.scheme().toLower() != QLatin1String("https");
        bool streaming = false;
        bool isRedirect = false;
        bool metadataAbort = false;
        bool writeFailed = false;
        QString writeError;
        std::unique_ptr<FileSink> sink;
        bool destOpen = false;

        auto abortWrite = [&]() {
            if (destOpen && sink) {
                sink->close();
                QFile::remove(request.destinationPath);
                destOpen = false;
            }
            held.clear();
        };

        auto writeChunk = [&](const QByteArray &chunk) -> bool {
            if (chunk.isEmpty()) {
                return true;
            }
            if (destOpen && sink) {
                if (sink->write(chunk.constData(), chunk.size()) != chunk.size()) {
                    writeFailed = true;
                    writeError = QStringLiteral("Short write to download destination");
                    return false;
                }
            } else {
                result.body += chunk;
            }
            return true;
        };

        auto beginStreaming = [&]() -> bool {
            if (streaming || isRedirect) {
                return true;
            }
            if (!peerReady && logical.scheme().toLower() == QLatin1String("https")) {
                return true;
            }
            streaming = true;
            if (request.metadataOnly) {
                if (!schemeSupportsHead(logical) || usedHead) {
                    metadataAbort = true;
                    reply->abort();
                }
                held.clear();
                return true;
            }
            if (!request.destinationPath.isEmpty() && !destOpen) {
                sink = m_sinkFactory ? m_sinkFactory() : std::make_unique<QtFileSink>();
                if (!sink || !sink->open(request.destinationPath, &result.error)) {
                    if (result.error.isEmpty()) {
                        result.error = QStringLiteral("Cannot open download destination");
                    }
                    reply->abort();
                    return false;
                }
                destOpen = true;
            }
            if (!held.isEmpty()) {
                written += held.size();
                if (written > request.maxBytes) {
                    truncated = true;
                    reply->abort();
                    return false;
                }
                if (!writeChunk(held)) {
                    reply->abort();
                    return false;
                }
                held.clear();
            }
            return true;
        };

        QObject::connect(reply, &QNetworkReply::encrypted, &loop, [&]() {
            peerReady = true;
            if (!isRedirect) {
                beginStreaming();
            }
        });
        QObject::connect(reply, &QNetworkReply::metaDataChanged, &loop, [&]() {
            const QVariant redir = reply->attribute(QNetworkRequest::RedirectionTargetAttribute);
            if (redir.isValid()) {
                isRedirect = true;
                held.clear();
                return;
            }
            beginStreaming();
        });
        QObject::connect(reply, &QNetworkReply::readyRead, &loop, [&]() {
            if (cancel && cancel->load()) {
                reply->abort();
                return;
            }
            const QByteArray chunk = reply->readAll();
            if (request.metadataOnly) {
                return;
            }
            if (!peerReady || isRedirect || !streaming) {
                held += chunk;
                if (held.size() > request.maxBytes) {
                    truncated = true;
                    reply->abort();
                }
                return;
            }
            written += chunk.size();
            if (written > request.maxBytes) {
                truncated = true;
                reply->abort();
                return;
            }
            if (!writeChunk(chunk)) {
                reply->abort();
                return;
            }
            if (request.progress) {
                const qint64 total = reply->header(QNetworkRequest::ContentLengthHeader).toLongLong();
                request.progress(written, total > 0 ? total : -1);
            }
        });
        timer.start(request.timeoutMs);
        loop.exec();
        if (cancel && cancel->load()) {
            result.cancelled = true;
            result.error = QStringLiteral("Cancelled");
            abortWrite();
            reply->deleteLater();
            return result;
        }
        result.status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
        result.etag = QString::fromUtf8(reply->rawHeader("ETag"));
        result.lastModified = QString::fromUtf8(reply->rawHeader("Last-Modified"));
        result.contentLength = reply->header(QNetworkRequest::ContentLengthHeader).toLongLong();
        if (result.contentLength <= 0) {
            const QByteArray rawLength = reply->rawHeader("Content-Length");
            if (!rawLength.isEmpty()) {
                result.contentLength = rawLength.toLongLong();
            }
        }
        result.digest = QString::fromUtf8(reply->rawHeader("Digest"));
        if (result.digest.isEmpty()) {
            result.digest = QString::fromUtf8(reply->rawHeader("X-Checksum-Sha256"));
        }
        result.finalUrl = logical;
        result.bytesTransferred = written;
        const QVariant redir = reply->attribute(QNetworkRequest::RedirectionTargetAttribute);
        if (truncated && !request.metadataOnly) {
            result.truncated = true;
            result.error = QStringLiteral("Download exceeded size bound");
            abortWrite();
            reply->deleteLater();
            return result;
        }
        if (writeFailed) {
            result.error = writeError;
            abortWrite();
            reply->deleteLater();
            return result;
        }
        if (redir.isValid()) {
            const QUrl next = logical.resolved(redir.toUrl());
            if (isDowngrade(logical, next)) {
                result.error = QStringLiteral("HTTPS to HTTP downgrade rejected");
                abortWrite();
                reply->deleteLater();
                return result;
            }
            UrlCheck nextCheck = UrlGuard::validateResolved(next, resolver, request.allowHttp, request.allowPrivate, request.allowFtp);
            if (!nextCheck.ok) {
                result.error = nextCheck.error;
                abortWrite();
                reply->deleteLater();
                return result;
            }
            if (++redirects > request.maxRedirects) {
                result.error = QStringLiteral("Too many redirects");
                abortWrite();
                reply->deleteLater();
                return result;
            }
            result.redirected = true;
            logical = next;
            abortWrite();
            result.body.clear();
            written = 0;
            usedHead = false;
            reply->deleteLater();
            continue;
        }
        const QNetworkReply::NetworkError netError = reply->error();
        const bool metadataCanceled = request.metadataOnly && metadataAbort
            && (netError == QNetworkReply::OperationCanceledError || netError == QNetworkReply::NoError);
        if (wantHead && (result.status == 405 || result.status == 501)) {
            usedHead = true;
            abortWrite();
            result.body.clear();
            written = 0;
            reply->deleteLater();
            continue;
        }
        if (netError != QNetworkReply::NoError && !metadataCanceled) {
            result.error = reply->errorString();
            abortWrite();
            reply->deleteLater();
            return result;
        }
        if (!streaming && !request.metadataOnly) {
            beginStreaming();
        }
        if (destOpen && sink) {
            if (!sink->flush() || !sink->sync()) {
                result.error = QStringLiteral("Failed to flush download destination");
                abortWrite();
                reply->deleteLater();
                return result;
            }
            sink->close();
            result.savedPath = request.destinationPath;
            if (result.contentLength > 0 && written != result.contentLength) {
                result.error = QStringLiteral("Download size does not match Content-Length");
                QFile::remove(request.destinationPath);
                result.savedPath.clear();
                reply->deleteLater();
                return result;
            }
        }
        result.bytesTransferred = written;
        reply->deleteLater();
        if (request.metadataOnly) {
            result.ok = (result.status >= 200 && result.status < 300) || result.status == 0;
            result.body.clear();
            result.bytesTransferred = 0;
            if (!result.ok && result.error.isEmpty()) {
                result.error = QStringLiteral("HTTP status %1").arg(result.status);
            }
            return result;
        }
        result.ok = result.status >= 200 && result.status < 300;
        if (!result.ok && result.error.isEmpty()) {
            result.error = QStringLiteral("HTTP status %1").arg(result.status);
            if (!request.destinationPath.isEmpty()) {
                QFile::remove(request.destinationPath);
            }
        }
        return result;
    }
}

} // namespace GoshAim
