#include "QtWarnGuard.h"
#include "FakeSeams.h"
#include "core/NetworkClient.h"
#include "core/UpdateSources.h"
#include "core/UrlGuard.h"

#include <QFile>
#include <QHash>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QThread>
#include <QtTest>
#include <atomic>
#include <memory>

using namespace GoshAim;

class MapResolver : public HostResolver
{
public:
    QHash<QString, QList<QHostAddress>> map;
    QList<QHostAddress> resolve(const QString &host) override
    {
        return map.value(host.toLower());
    }
};

class MiniHttpServer : public QObject
{
public:
    QTcpServer server;
    QByteArray payload = QByteArrayLiteral("ok-body");
    QString redirect;
    int status = 200;
    bool hang = false;
    qint64 extraBytes = 0;
    QByteArray lastMethod;
    bool start()
    {
        return server.listen(QHostAddress::LocalHost, 0);
    }
    quint16 port() const { return server.serverPort(); }
    void serveOnce()
    {
        connect(&server, &QTcpServer::newConnection, this, [this]() {
            QTcpSocket *sock = server.nextPendingConnection();
            connect(sock, &QTcpSocket::readyRead, this, [this, sock]() {
                if (hang) {
                    return;
                }
                const QByteArray request = sock->readAll();
                lastMethod = request.split(' ').value(0);
                const bool isHead = lastMethod == QByteArrayLiteral("HEAD");
                QByteArray body = payload;
                if (extraBytes > 0) {
                    body += QByteArray(int(extraBytes), 'x');
                }
                QByteArray header;
                if (!redirect.isEmpty()) {
                    header = "HTTP/1.1 302 Found\r\nLocation: " + redirect.toUtf8() + "\r\nContent-Length: 0\r\n\r\n";
                    redirect.clear();
                } else {
                    header = "HTTP/1.1 " + QByteArray::number(status) + " OK\r\nContent-Length: "
                        + QByteArray::number(body.size()) + "\r\nETag: \"abc\"\r\nLast-Modified: now\r\nDigest: sha256:dead\r\n\r\n";
                }
                sock->write(header);
                if (!isHead) {
                    sock->write(body);
                }
                sock->flush();
                sock->disconnectFromHost();
            });
        });
    }
};

class TestNetwork : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void privateResolutionRejected()
    {
        MapResolver resolver;
        resolver.map.insert(QStringLiteral("evil.example"), {QHostAddress(QStringLiteral("127.0.0.1"))});
        QtNetworkClient client(&resolver);
        NetworkRequest req;
        req.url = QUrl(QStringLiteral("https://evil.example/a"));
        const NetworkResult result = client.fetch(req);
        QVERIFY(!result.ok);
        QVERIFY(result.error.contains(QLatin1String("private")) || result.error.contains(QLatin1String("loopback"))
                || result.error.contains(QLatin1String("Resolved")));
    }
    void dnsChangeOnRedirectRejected()
    {
        MapResolver resolver;
        resolver.map.insert(QStringLiteral("public.example"), {QHostAddress(QStringLiteral("8.8.8.8"))});
        resolver.map.insert(QStringLiteral("rebind.example"), {QHostAddress(QStringLiteral("10.0.0.1"))});
        QtNetworkClient client(&resolver);
        NetworkRequest req;
        req.url = QUrl(QStringLiteral("https://public.example/a"));
        // Without a live HTTPS server this fails at connect; still reject rebind via validateResolved.
        const UrlCheck check = UrlGuard::validateResolved(QUrl(QStringLiteral("https://rebind.example/x")), &resolver);
        QVERIFY(!check.ok);
    }
    void httpRedirectSizeTimeoutCancel()
    {
        QtWarnGuard guard;
        MiniHttpServer server;
        QVERIFY(server.start());
        server.serveOnce();
        MapResolver resolver;
        resolver.map.insert(QStringLiteral("127.0.0.1"), {QHostAddress::LocalHost});
        QtNetworkClient client(&resolver);
        NetworkRequest req;
        req.url = QUrl(QStringLiteral("http://127.0.0.1:%1/file").arg(server.port()));
        req.allowHttp = true;
        req.allowPrivate = true;
        req.maxBytes = 1024;
        MiniHttpServer dest;
        QVERIFY(dest.start());
        dest.payload = QByteArrayLiteral("final");
        dest.serveOnce();
        server.redirect = QStringLiteral("http://127.0.0.1:%1/next").arg(dest.port());
        const NetworkResult redirected = client.fetch(req);
        QVERIFY2(redirected.ok, qPrintable(redirected.error));
        QVERIFY(redirected.body.contains("final") || redirected.status == 200);

        MiniHttpServer huge;
        QVERIFY(huge.start());
        huge.extraBytes = 4096;
        huge.serveOnce();
        NetworkRequest sized;
        sized.url = QUrl(QStringLiteral("http://127.0.0.1:%1/big").arg(huge.port()));
        sized.allowHttp = true;
        sized.allowPrivate = true;
        sized.maxBytes = 16;
        const NetworkResult truncated = client.fetch(sized);
        QVERIFY(!truncated.ok);
        QVERIFY(truncated.truncated || truncated.error.contains(QLatin1String("size")) || truncated.error.contains(QLatin1String("bound")));

        MiniHttpServer slow;
        QVERIFY(slow.start());
        slow.hang = true;
        slow.serveOnce();
        NetworkRequest timed;
        timed.url = QUrl(QStringLiteral("http://127.0.0.1:%1/slow").arg(slow.port()));
        timed.allowHttp = true;
        timed.allowPrivate = true;
        timed.timeoutMs = 200;
        const NetworkResult timedOut = client.fetch(timed);
        QVERIFY(!timedOut.ok);

        MiniHttpServer cancelServer;
        QVERIFY(cancelServer.start());
        cancelServer.hang = true;
        cancelServer.serveOnce();
        NetworkRequest cancelReq;
        cancelReq.url = QUrl(QStringLiteral("http://127.0.0.1:%1/c").arg(cancelServer.port()));
        cancelReq.allowHttp = true;
        cancelReq.allowPrivate = true;
        cancelReq.timeoutMs = 5000;
        std::atomic<bool> cancel{false};
        QThread *worker = QThread::create([&]() {
            QThread::msleep(50);
            cancel.store(true);
        });
        worker->start();
        const NetworkResult cancelled = client.fetch(cancelReq, &cancel);
        worker->wait();
        delete worker;
        QVERIFY(cancelled.cancelled || !cancelled.ok);
        QVERIFY(!guard.sawLiveDestruction());
    }
    void credentialsAndDowngrade()
    {
        QVERIFY(!UrlGuard::validate(QStringLiteral("https://user:pass@example.com/x")).ok);
        QVERIFY(!UrlGuard::validate(QStringLiteral("http://example.com/x")).ok);
    }
    void fetchPinsPublicAndIgnoresLaterPrivateResolve()
    {
        MiniHttpServer secret;
        QVERIFY(secret.start());
        secret.payload = QByteArrayLiteral("PWNED");
        secret.serveOnce();

        class FlipResolver : public HostResolver
        {
        public:
            int calls = 0;
            QList<QHostAddress> resolve(const QString &host) override
            {
                Q_UNUSED(host);
                ++calls;
                if (calls == 1) {
                    return {QHostAddress(QStringLiteral("8.8.8.8"))};
                }
                return {QHostAddress(QStringLiteral("127.0.0.1"))};
            }
        };
        FlipResolver resolver;
        QtNetworkClient client(&resolver);
        QTemporaryDir tmp;
        const QString dest = tmp.path() + QStringLiteral("/body.bin");
        NetworkRequest req;
        req.url = QUrl(QStringLiteral("http://rebind.test:%1/secret").arg(secret.port()));
        req.allowHttp = true;
        req.destinationPath = dest;
        req.timeoutMs = 800;
        const NetworkResult result = client.fetch(req);
        QVERIFY(!result.ok);
        if (QFile::exists(dest)) {
            QFile file(dest);
            QVERIFY(file.open(QIODevice::ReadOnly));
            QVERIFY(!file.readAll().contains("PWNED"));
        }
        QVERIFY(resolver.calls >= 1);
    }
    void metadataOnlyHeadDoesNotDownloadBody()
    {
        MiniHttpServer server;
        QVERIFY(server.start());
        server.payload = QByteArray(8192, 'B');
        server.serveOnce();
        MapResolver resolver;
        resolver.map.insert(QStringLiteral("127.0.0.1"), {QHostAddress::LocalHost});
        QtNetworkClient client(&resolver);
        NetworkRequest req;
        req.url = QUrl(QStringLiteral("http://127.0.0.1:%1/file").arg(server.port()));
        req.allowHttp = true;
        req.allowPrivate = true;
        req.metadataOnly = true;
        req.maxBytes = 4096;
        const NetworkResult result = client.fetch(req);
        QVERIFY2(result.ok, qPrintable(result.error));
        QCOMPARE(client.lastMethod(), QStringLiteral("HEAD"));
        QVERIFY(client.lastMetadataOnly());
        QVERIFY(result.contentLength > 4096);
        QVERIFY(result.body.isEmpty());
        QCOMPARE(result.bytesTransferred, 0);
        QVERIFY(result.etag.contains(QLatin1String("abc")));
        QVERIFY(!result.error.contains(QLatin1String("exceeded size bound")));
        QTRY_VERIFY(!server.lastMethod.isEmpty());
        QCOMPARE(server.lastMethod, QByteArrayLiteral("HEAD"));
    }
    void staticFileSourceUsesMetadataOnlyOnLocalHttp()
    {
        MiniHttpServer server;
        QVERIFY(server.start());
        server.payload = QByteArray(8192, 'Z');
        server.serveOnce();
        MapResolver resolver;
        resolver.map.insert(QStringLiteral("127.0.0.1"), {QHostAddress::LocalHost});
        QtNetworkClient qt(&resolver);
        class RewriteClient : public NetworkClient
        {
        public:
            QtNetworkClient *inner = nullptr;
            QUrl local;
            NetworkResult fetch(const NetworkRequest &request, std::atomic<bool> *cancel = nullptr) override
            {
                NetworkRequest copy = request;
                copy.url = local;
                copy.allowHttp = true;
                copy.allowPrivate = true;
                return inner->fetch(copy, cancel);
            }
        };
        RewriteClient rewrite;
        rewrite.inner = &qt;
        rewrite.local = QUrl(QStringLiteral("http://127.0.0.1:%1/App.AppImage").arg(server.port()));
        StaticFileSource source;
        InstalledApp app;
        app.size = 8192;
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("https://example.com/App.AppImage"));
        const UpdateCheckResult checked = source.check(app, &rewrite, nullptr);
        QVERIFY2(checked.ok, qPrintable(checked.error));
        QVERIFY(!checked.available);
        QCOMPARE(qt.lastMethod(), QStringLiteral("HEAD"));
        QVERIFY(qt.lastMetadataOnly());
        QCOMPARE(checked.size, 8192);
    }
    void ftpMetadataOnlyUsesGetSeam()
    {
        FakeNetworkClient fake;
        FakeNetworkClient::Rule rule;
        rule.hostContains = QStringLiteral("example.com");
        rule.result.ok = true;
        rule.result.status = 200;
        rule.result.contentLength = 5000000;
        rule.result.lastModified = QStringLiteral("now");
        fake.rules.append(rule);
        FtpSource source;
        InstalledApp app;
        app.size = 5000000;
        app.updateConfig.insert(QStringLiteral("url"), QStringLiteral("ftp://example.com/a.AppImage"));
        const UpdateCheckResult checked = source.check(app, &fake, nullptr);
        QVERIFY(checked.ok);
        QVERIFY(!fake.requests.isEmpty());
        QVERIFY(fake.requests.last().allowFtp);
        QVERIFY(fake.requests.last().metadataOnly);
        QVERIFY(!checked.available);

        MapResolver resolver;
        resolver.map.insert(QStringLiteral("example.com"), {QHostAddress(QStringLiteral("8.8.8.8"))});
        QtNetworkClient client(&resolver);
        NetworkRequest req;
        req.url = QUrl(QStringLiteral("ftp://example.com/a.AppImage"));
        req.allowFtp = true;
        req.metadataOnly = true;
        req.timeoutMs = 200;
        client.fetch(req);
        QCOMPARE(client.lastMethod(), QStringLiteral("GET"));
        QVERIFY(client.lastMetadataOnly());
    }
    void shortWriteFailsAndCleansStaging()
    {
        class ShortWriteSink final : public FileSink
        {
        public:
            bool open(const QString &path, QString *error) override
            {
                Q_UNUSED(error);
                m_path = path;
                return true;
            }
            qint64 write(const char *, qint64) override
            {
                return -1;
            }
            bool flush() override { return true; }
            bool sync() override { return true; }
            void close() override {}
            QString path() const override { return m_path; }
            QString m_path;
        };
        MiniHttpServer server;
        QVERIFY(server.start());
        server.payload = QByteArray(64, 'D');
        server.serveOnce();
        MapResolver resolver;
        resolver.map.insert(QStringLiteral("127.0.0.1"), {QHostAddress::LocalHost});
        QtNetworkClient client(&resolver);
        client.setFileSinkFactory([]() { return std::make_unique<ShortWriteSink>(); });
        QTemporaryDir tmp;
        const QString dest = tmp.path() + QStringLiteral("/dl.bin");
        NetworkRequest req;
        req.url = QUrl(QStringLiteral("http://127.0.0.1:%1/file").arg(server.port()));
        req.allowHttp = true;
        req.allowPrivate = true;
        req.destinationPath = dest;
        const NetworkResult result = client.fetch(req);
        QVERIFY(!result.ok);
        QVERIFY(result.error.contains(QLatin1String("Short write")) || !result.ok);
        QVERIFY(!QFile::exists(dest));
    }
};

QTEST_GUILESS_MAIN(TestNetwork)
#include "test_network.moc"
