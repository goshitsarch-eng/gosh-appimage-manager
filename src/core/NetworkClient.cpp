#include "NetworkClient.h"

#include "SafeFs.h"

#include <QEventLoop>
#include <QFile>
#include <QNetworkAccessManager>
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
    QUrl current = request.url;
    int redirects = 0;
    while (true) {
        if (cancel && cancel->load()) {
            result.cancelled = true;
            result.error = QStringLiteral("Cancelled");
            return result;
        }
        QNetworkRequest req(current);
        req.setAttribute(QNetworkRequest::RedirectPolicyAttribute, QNetworkRequest::ManualRedirectPolicy);
        req.setMaximumRedirectsAllowed(0);
        req.setTransferTimeout(request.timeoutMs);
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
        QFile outFile;
        qint64 written = 0;
        bool destOpen = false;
        if (!request.destinationPath.isEmpty()) {
            outFile.setFileName(request.destinationPath);
            if (!outFile.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                reply->abort();
                reply->deleteLater();
                result.error = QStringLiteral("Cannot open download destination");
                return result;
            }
            destOpen = true;
            ::fchmod(outFile.handle(), 0600);
        }
        QObject::connect(reply, &QNetworkReply::readyRead, &loop, [&]() {
            if (cancel && cancel->load()) {
                reply->abort();
                return;
            }
            const QByteArray chunk = reply->readAll();
            written += chunk.size();
            if (written > request.maxBytes) {
                result.truncated = true;
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
            if (destOpen) {
                outFile.close();
                QFile::remove(request.destinationPath);
            }
            reply->deleteLater();
            return result;
        }
        result.status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
        result.etag = QString::fromUtf8(reply->rawHeader("ETag"));
        result.lastModified = QString::fromUtf8(reply->rawHeader("Last-Modified"));
        result.contentLength = reply->header(QNetworkRequest::ContentLengthHeader).toLongLong();
        result.finalUrl = reply->url();
        const QVariant redir = reply->attribute(QNetworkRequest::RedirectionTargetAttribute);
        if (result.truncated) {
            result.error = QStringLiteral("Download exceeded size bound");
            if (destOpen) {
                outFile.close();
                QFile::remove(request.destinationPath);
            }
            reply->deleteLater();
            return result;
        }
        if (redir.isValid()) {
            const QUrl next = current.resolved(redir.toUrl());
            if (isDowngrade(current, next)) {
                result.error = QStringLiteral("HTTPS to HTTP downgrade rejected");
                reply->deleteLater();
                if (destOpen) {
                    outFile.close();
                    QFile::remove(request.destinationPath);
                }
                return result;
            }
            UrlCheck nextCheck = UrlGuard::validateResolved(next, resolver, request.allowHttp, request.allowPrivate, request.allowFtp);
            if (!nextCheck.ok) {
                result.error = nextCheck.error;
                reply->deleteLater();
                if (destOpen) {
                    outFile.close();
                    QFile::remove(request.destinationPath);
                }
                return result;
            }
            if (++redirects > request.maxRedirects) {
                result.error = QStringLiteral("Too many redirects");
                reply->deleteLater();
                if (destOpen) {
                    outFile.close();
                    QFile::remove(request.destinationPath);
                }
                return result;
            }
            result.redirected = true;
            current = next;
            if (destOpen) {
                outFile.resize(0);
                written = 0;
                result.body.clear();
            } else {
                result.body.clear();
                written = 0;
            }
            reply->deleteLater();
            continue;
        }
        if (reply->error() != QNetworkReply::NoError) {
            result.error = reply->errorString();
            if (destOpen) {
                outFile.close();
                QFile::remove(request.destinationPath);
            }
            reply->deleteLater();
            return result;
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
        }
        return result;
    }
}

} // namespace GoshAim
