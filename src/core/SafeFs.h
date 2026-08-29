#pragma once

#include "Limits.h"

#include <QByteArray>
#include <QString>
#include <QStringList>
#include <atomic>

class QDir;

namespace GoshAim {

struct HashResult {
    QByteArray sha256;
    qint64 bytesRead = 0;
    bool cancelled = false;
    bool truncated = false;
    QString error;
};

class SafeFs
{
public:
    static bool isRegularFile(const QString &path, QString *error = nullptr);
    static QString canonicalExisting(const QString &path, QString *error = nullptr);
    static bool mkdir0700(const QString &path, QString *error = nullptr);
    static bool atomicWrite(const QString &path, const QByteArray &data, QString *error = nullptr, int mode = 0600);
    static bool copyBounded(const QString &from,
                            const QString &to,
                            qint64 maxBytes,
                            std::atomic<bool> *cancel,
                            qint64 *copied,
                            QString *error);
    static bool fsyncPath(const QString &path, QString *error = nullptr);
    static bool chmodPath(const QString &path, int mode, QString *error = nullptr);
    static bool renameOver(const QString &from, const QString &to, QString *error = nullptr);
    static bool removeFileNoFollow(const QString &path, QString *error = nullptr);
    static bool removeTreeNoFollow(const QString &path, QString *error = nullptr);
    static bool isForbiddenPermanentTarget(const QString &canonicalPath);
    static QString siblingTemp(const QString &destination, const QString &prefix);
    static HashResult sha256File(const QString &path, qint64 maxBytes, std::atomic<bool> *cancel = nullptr);
    static QString hexSha256(const QByteArray &digest);
    static QByteArray sha256Bytes(const QByteArray &data);
};

} // namespace GoshAim
