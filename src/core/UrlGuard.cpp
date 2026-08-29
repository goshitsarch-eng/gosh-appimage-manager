#include "UrlGuard.h"

#include <QAbstractSocket>
#include <QHostAddress>
#include <QHostInfo>
#include <QRegularExpression>

namespace GoshAim {

bool UrlGuard::isPrivateHost(const QString &host)
{
    if (host.isEmpty()) {
        return true;
    }
    const QString lower = host.toLower();
    if (lower == QLatin1String("localhost") || lower.endsWith(QLatin1String(".localhost"))
        || lower.endsWith(QLatin1String(".local"))) {
        return true;
    }
    QHostAddress address(host);
    if (address.isNull()) {
        return false;
    }
    if (address.isLoopback() || address.isLinkLocal()) {
        return true;
    }
    if (address.protocol() == QAbstractSocket::IPv4Protocol) {
        const quint32 v = address.toIPv4Address();
        const quint8 a = quint8(v >> 24);
        const quint8 b = quint8(v >> 16);
        if (a == 10) {
            return true;
        }
        if (a == 172 && b >= 16 && b <= 31) {
            return true;
        }
        if (a == 192 && b == 168) {
            return true;
        }
        if (a == 169 && b == 254) {
            return true;
        }
        if (a == 127) {
            return true;
        }
    }
    if (address.protocol() == QAbstractSocket::IPv6Protocol) {
        if (address.isSiteLocal()) {
            return true;
        }
        const Q_IPV6ADDR v6 = address.toIPv6Address();
        if (v6[0] == 0xfc || v6[0] == 0xfd) {
            return true;
        }
    }
    return false;
}

bool UrlGuard::isDisallowedAddress(const QHostAddress &address)
{
    if (address.isNull()) {
        return true;
    }
    if (address.isLoopback() || address.isLinkLocal() || address.isMulticast() || address.isBroadcast()) {
        return true;
    }
    if (address == QHostAddress::Any || address == QHostAddress::AnyIPv4 || address == QHostAddress::AnyIPv6) {
        return true;
    }
    return isPrivateHost(address.toString());
}

QList<QHostAddress> QtHostResolver::resolve(const QString &host)
{
    const QHostInfo info = QHostInfo::fromName(host);
    return info.addresses();
}

UrlCheck UrlGuard::validateResolved(const QUrl &url, HostResolver *resolver, bool allowHttp, bool allowPrivate, bool allowFtp)
{
    UrlCheck check = validate(url, allowHttp, allowPrivate, allowFtp);
    if (!check.ok) {
        return check;
    }
    const QString host = url.host();
    QHostAddress literal(host);
    if (!literal.isNull()) {
        check.resolved = {literal};
        if (isDisallowedAddress(literal) && !allowPrivate) {
            check.ok = false;
            check.privateHost = true;
            check.error = QStringLiteral("Private, loopback or link-local destinations are not allowed");
        }
        return check;
    }
    if (!resolver) {
        return check;
    }
    const QList<QHostAddress> addresses = resolver->resolve(host);
    check.resolved = addresses;
    if (addresses.isEmpty()) {
        check.ok = false;
        check.error = QStringLiteral("DNS resolution failed");
        return check;
    }
    for (const QHostAddress &address : addresses) {
        if (isDisallowedAddress(address)) {
            check.privateHost = true;
            if (!allowPrivate) {
                check.ok = false;
                check.error = QStringLiteral("Resolved address is private, loopback, link-local, multicast or unspecified");
                return check;
            }
        }
    }
    return check;
}

bool UrlGuard::isSafeRepoComponent(const QString &value)
{
    static const QRegularExpression re(QStringLiteral("^[A-Za-z0-9._-]+$"));
    return !value.isEmpty() && value.size() <= 128 && !value.contains(QLatin1String("..")) && re.match(value).hasMatch();
}

QString UrlGuard::encodePathSegment(const QString &segment)
{
    return QString::fromUtf8(QUrl::toPercentEncoding(segment));
}

bool UrlGuard::isSafeApiHost(const QString &host, const QStringList &allowed)
{
    const QString lower = host.toLower();
    return allowed.contains(lower);
}

UrlCheck UrlGuard::validate(const QString &url, bool allowHttp, bool allowPrivate, bool allowFtp)
{
    return validate(QUrl(url), allowHttp, allowPrivate, allowFtp);
}

UrlCheck UrlGuard::validate(const QUrl &url, bool allowHttp, bool allowPrivate, bool allowFtp)
{
    UrlCheck check;
    check.url = url;
    if (!url.isValid() || url.isEmpty()) {
        check.error = QStringLiteral("Invalid URL");
        return check;
    }
    const QString scheme = url.scheme().toLower();
    if (scheme == QLatin1String("file") || scheme == QLatin1String("data") || scheme == QLatin1String("javascript")
        || scheme == QLatin1String("about") || scheme == QLatin1String("blob")) {
        check.error = QStringLiteral("Rejected URL scheme");
        return check;
    }
    if (scheme == QLatin1String("ftp") || scheme == QLatin1String("ftps")) {
        if (!allowFtp) {
            check.error = QStringLiteral("FTP is only allowed as an explicit legacy update source");
            return check;
        }
        check.insecure = scheme == QLatin1String("ftp");
    } else if (scheme == QLatin1String("http")) {
        if (!allowHttp) {
            check.error = QStringLiteral("HTTP is not allowed");
            return check;
        }
        check.insecure = true;
    } else if (scheme != QLatin1String("https")) {
        check.error = QStringLiteral("Unsupported URL scheme");
        return check;
    }
    if (!url.userName().isEmpty() || !url.password().isEmpty()) {
        check.error = QStringLiteral("Embedded credentials are not allowed");
        return check;
    }
    const QString encoded = url.toString();
    if (encoded.contains(QChar(0)) || encoded.contains(QLatin1Char('\n')) || encoded.contains(QLatin1Char('\r'))) {
        check.error = QStringLiteral("URL contains control characters");
        return check;
    }
    const QString host = url.host();
    if (host.isEmpty()) {
        check.error = QStringLiteral("URL is missing a host");
        return check;
    }
    check.privateHost = isPrivateHost(host);
    if (check.privateHost && !allowPrivate) {
        check.error = QStringLiteral("Private, loopback or link-local destinations are not allowed");
        return check;
    }
    check.ok = true;
    return check;
}

} // namespace GoshAim
