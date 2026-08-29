#include "NetworkClient.h"

#include "SafeFs.h"

#include <QAbstractSocket>
#include <QEventLoop>
#include <QFile>
#include <QHostAddress>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QTimer>
#include <sys/stat.h>
#include <unistd.h>

namespace GoshAim {

namespace {

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

} // namespace

QtNetworkClient::QtNetworkClient(HostResolver *resolver)
    : m_resolver(resolver)
{
}

void QtNetworkClient::setResolver(HostResolver *resolver)
{
    m_resolver = resolver;
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

        QNetworkReply *reply = manager.get(req);
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
        QFile outFile;
        bool destOpen = false;

        auto abortWrite = [&]() {
            if (destOpen) {
                outFile.close();
                QFile::remove(request.destinationPath);
                destOpen = false;
            }
            held.clear();
        };

        auto beginStreaming = [&]() -> bool {
            if (streaming || isRedirect) {
                return true;
            }
            if (!peerReady && logical.scheme().toLower() == QLatin1String("https")) {
                return true;
            }
            streaming = true;
            if (!request.destinationPath.isEmpty() && !destOpen) {
                outFile.setFileName(request.destinationPath);
                if (!outFile.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                    result.error = QStringLiteral("Cannot open download destination");
                    reply->abort();
                    return false;
                }
                destOpen = true;
                ::fchmod(outFile.handle(), 0600);
            }
            if (!held.isEmpty()) {
                written += held.size();
                if (written > request.maxBytes) {
                    truncated = true;
                    reply->abort();
                    return false;
                }
                if (destOpen) {
                    outFile.write(held);
                } else {
                    result.body += held;
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
            if (destOpen) {
                outFile.write(chunk);
            } else {
                result.body += chunk;
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
        result.finalUrl = logical;
        const QVariant redir = reply->attribute(QNetworkRequest::RedirectionTargetAttribute);
        if (truncated) {
            result.truncated = true;
            result.error = QStringLiteral("Download exceeded size bound");
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
            reply->deleteLater();
            continue;
        }
        if (reply->error() != QNetworkReply::NoError) {
            result.error = reply->errorString();
            abortWrite();
            reply->deleteLater();
            return result;
        }
        if (!streaming) {
            beginStreaming();
        }
        if (destOpen) {
            outFile.flush();
            if (outFile.handle() >= 0) {
                ::fsync(outFile.handle());
            }
            outFile.close();
            result.savedPath = request.destinationPath;
        }
        reply->deleteLater();
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
