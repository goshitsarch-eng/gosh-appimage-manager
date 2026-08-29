#include "SafeFs.h"

#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QRandomGenerator>
#include <QStorageInfo>

#include <cerrno>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#include <ftw.h>

namespace GoshAim {

namespace {

thread_local QString g_nftwError;
thread_local QString g_nftwRoot;

int ntfwUnlink(const char *fpath, const struct stat *sb, int typeflag, struct FTW *ftwbuf)
{
    Q_UNUSED(sb);
    Q_UNUSED(ftwbuf);
    if (typeflag == FTW_SL || typeflag == FTW_SLN) {
        if (::unlink(fpath) != 0) {
            g_nftwError = QStringLiteral("Failed to unlink symlink %1").arg(QString::fromLocal8Bit(fpath));
            return 1;
        }
        return 0;
    }
    if (typeflag == FTW_DP || typeflag == FTW_D) {
        if (::rmdir(fpath) != 0 && errno != ENOTEMPTY) {
            if (::rmdir(fpath) != 0) {
                g_nftwError = QStringLiteral("Failed to rmdir %1").arg(QString::fromLocal8Bit(fpath));
                return 1;
            }
        }
        return 0;
    }
    if (::unlink(fpath) != 0) {
        g_nftwError = QStringLiteral("Failed to unlink %1").arg(QString::fromLocal8Bit(fpath));
        return 1;
    }
    return 0;
}

bool lstatRegular(const QString &path, struct stat *st, QString *error)
{
    if (::lstat(path.toLocal8Bit().constData(), st) != 0) {
        if (error) {
            *error = QStringLiteral("Cannot stat %1").arg(path);
        }
        return false;
    }
    return true;
}

} // namespace

bool SafeFs::isRegularFile(const QString &path, QString *error)
{
    struct stat st {};
    if (!lstatRegular(path, &st, error)) {
        return false;
    }
    if (S_ISLNK(st.st_mode)) {
        QString resolved;
        QString hopError;
        QString current = path;
        for (int hop = 0; hop < kSymlinkHopLimit; ++hop) {
            struct stat lst {};
            if (!lstatRegular(current, &lst, &hopError)) {
                if (error) {
                    *error = hopError;
                }
                return false;
            }
            if (!S_ISLNK(lst.st_mode)) {
                if (!S_ISREG(lst.st_mode)) {
                    if (error) {
                        *error = QStringLiteral("Path is not a regular file: %1").arg(path);
                    }
                    return false;
                }
                resolved = current;
                break;
            }
            const QByteArray target(4096, Qt::Uninitialized);
            const ssize_t n = ::readlink(current.toLocal8Bit().constData(), const_cast<char *>(target.constData()), 4095);
            if (n <= 0) {
                if (error) {
                    *error = QStringLiteral("Dangling or unreadable symlink: %1").arg(path);
                }
                return false;
            }
            const QString link = QString::fromLocal8Bit(target.constData(), static_cast<int>(n));
            QFileInfo info(current);
            current = QFileInfo(info.dir(), link).filePath();
            if (hop == kSymlinkHopLimit - 1) {
                if (error) {
                    *error = QStringLiteral("Symlink hop limit exceeded: %1").arg(path);
                }
                return false;
            }
        }
        Q_UNUSED(resolved);
        return true;
    }
    if (!S_ISREG(st.st_mode)) {
        if (error) {
            *error = QStringLiteral("Path is not a regular file: %1").arg(path);
        }
        return false;
    }
    return true;
}

QString SafeFs::canonicalExisting(const QString &path, QString *error)
{
    QFileInfo info(path);
    if (!info.exists()) {
        if (error) {
            *error = QStringLiteral("Path does not exist: %1").arg(path);
        }
        return {};
    }
    const QString canonical = info.canonicalFilePath();
    if (canonical.isEmpty()) {
        if (error) {
            *error = QStringLiteral("Cannot resolve canonical path: %1").arg(path);
        }
        return {};
    }
    return canonical;
}

bool SafeFs::mkdir0700(const QString &path, QString *error)
{
    QDir dir;
    if (dir.exists(path)) {
        struct stat st {};
        if (!lstatRegular(path, &st, error)) {
            return false;
        }
        if (!S_ISDIR(st.st_mode) || S_ISLNK(st.st_mode)) {
            if (error) {
                *error = QStringLiteral("Path exists and is not a directory: %1").arg(path);
            }
            return false;
        }
        return true;
    }
    if (!dir.mkpath(path)) {
        if (error) {
            *error = QStringLiteral("Cannot create directory %1").arg(path);
        }
        return false;
    }
    if (::chmod(path.toLocal8Bit().constData(), 0700) != 0) {
        if (error) {
            *error = QStringLiteral("Cannot chmod directory %1").arg(path);
        }
        return false;
    }
    return true;
}

bool SafeFs::atomicWrite(const QString &path, const QByteArray &data, QString *error, int mode)
{
    const QFileInfo info(path);
    if (!mkdir0700(info.absolutePath(), error)) {
        return false;
    }
    const QString tmp = siblingTemp(path, QStringLiteral(".gosh-tmp-"));
    QFile file(tmp);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        if (error) {
            *error = QStringLiteral("Cannot open temp file %1").arg(tmp);
        }
        return false;
    }
    if (file.write(data) != data.size()) {
        if (error) {
            *error = QStringLiteral("Short write to %1").arg(tmp);
        }
        file.close();
        QFile::remove(tmp);
        return false;
    }
    file.flush();
    const int fd = file.handle();
    if (fd >= 0) {
        ::fsync(fd);
    }
    file.close();
    if (::chmod(tmp.toLocal8Bit().constData(), static_cast<mode_t>(mode)) != 0) {
        if (error) {
            *error = QStringLiteral("Cannot chmod %1").arg(tmp);
        }
        QFile::remove(tmp);
        return false;
    }
    if (!renameOver(tmp, path, error)) {
        QFile::remove(tmp);
        return false;
    }
    return true;
}

bool SafeFs::copyBounded(const QString &from,
                         const QString &to,
                         qint64 maxBytes,
                         std::atomic<bool> *cancel,
                         qint64 *copied,
                         QString *error)
{
    if (copied) {
        *copied = 0;
    }
    QString inspectError;
    if (!isRegularFile(from, &inspectError)) {
        if (error) {
            *error = inspectError;
        }
        return false;
    }
    QFile in(from);
    if (!in.open(QIODevice::ReadOnly)) {
        if (error) {
            *error = QStringLiteral("Cannot read %1").arg(from);
        }
        return false;
    }
    QFile out(to);
    if (!out.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        if (error) {
            *error = QStringLiteral("Cannot write %1").arg(to);
        }
        return false;
    }
    QByteArray buffer(64 * 1024, Qt::Uninitialized);
    qint64 total = 0;
    while (!in.atEnd()) {
        if (cancel && cancel->load()) {
            out.close();
            QFile::remove(to);
            if (error) {
                *error = QStringLiteral("Cancelled");
            }
            return false;
        }
        const qint64 n = in.read(buffer.data(), buffer.size());
        if (n < 0) {
            if (error) {
                *error = QStringLiteral("Read failed from %1").arg(from);
            }
            out.close();
            QFile::remove(to);
            return false;
        }
        if (n == 0) {
            break;
        }
        total += n;
        if (total > maxBytes) {
            if (error) {
                *error = QStringLiteral("Copy exceeded size bound");
            }
            out.close();
            QFile::remove(to);
            return false;
        }
        if (out.write(buffer.constData(), n) != n) {
            if (error) {
                *error = QStringLiteral("Write failed to %1").arg(to);
            }
            out.close();
            QFile::remove(to);
            return false;
        }
    }
    out.flush();
    const int fd = out.handle();
    if (fd >= 0) {
        ::fsync(fd);
    }
    out.close();
    if (copied) {
        *copied = total;
    }
    return true;
}

bool SafeFs::fsyncPath(const QString &path, QString *error)
{
    const int fd = ::open(path.toLocal8Bit().constData(), O_RDONLY);
    if (fd < 0) {
        if (error) {
            *error = QStringLiteral("Cannot open %1 for fsync").arg(path);
        }
        return false;
    }
    const int rc = ::fsync(fd);
    ::close(fd);
    if (rc != 0) {
        if (error) {
            *error = QStringLiteral("fsync failed for %1").arg(path);
        }
        return false;
    }
    return true;
}

bool SafeFs::chmodPath(const QString &path, int mode, QString *error)
{
    struct stat st {};
    if (!lstatRegular(path, &st, error)) {
        return false;
    }
    if (S_ISLNK(st.st_mode)) {
        if (error) {
            *error = QStringLiteral("Refusing to chmod a symlink: %1").arg(path);
        }
        return false;
    }
    if (::chmod(path.toLocal8Bit().constData(), static_cast<mode_t>(mode)) != 0) {
        if (error) {
            *error = QStringLiteral("chmod failed for %1").arg(path);
        }
        return false;
    }
    return true;
}

bool SafeFs::renameOver(const QString &from, const QString &to, QString *error)
{
    if (::rename(from.toLocal8Bit().constData(), to.toLocal8Bit().constData()) != 0) {
        if (error) {
            *error = QStringLiteral("rename %1 -> %2 failed").arg(from, to);
        }
        return false;
    }
    return true;
}

bool SafeFs::removeFileNoFollow(const QString &path, QString *error)
{
    struct stat st {};
    if (::lstat(path.toLocal8Bit().constData(), &st) != 0) {
        if (errno == ENOENT) {
            return true;
        }
        if (error) {
            *error = QStringLiteral("Cannot lstat %1").arg(path);
        }
        return false;
    }
    if (S_ISDIR(st.st_mode) && !S_ISLNK(st.st_mode)) {
        if (error) {
            *error = QStringLiteral("Refusing to unlink a directory: %1").arg(path);
        }
        return false;
    }
    if (::unlink(path.toLocal8Bit().constData()) != 0) {
        if (error) {
            *error = QStringLiteral("unlink failed for %1").arg(path);
        }
        return false;
    }
    return true;
}

bool SafeFs::removeTreeNoFollow(const QString &path, QString *error)
{
    struct stat st {};
    if (::lstat(path.toLocal8Bit().constData(), &st) != 0) {
        if (errno == ENOENT) {
            return true;
        }
        if (error) {
            *error = QStringLiteral("Cannot lstat %1").arg(path);
        }
        return false;
    }
    if (S_ISLNK(st.st_mode) || S_ISREG(st.st_mode)) {
        return removeFileNoFollow(path, error);
    }
    if (!S_ISDIR(st.st_mode)) {
        if (error) {
            *error = QStringLiteral("Not a directory: %1").arg(path);
        }
        return false;
    }
    g_nftwError.clear();
    g_nftwRoot = path;
    if (::nftw(path.toLocal8Bit().constData(), ntfwUnlink, 16, FTW_DEPTH | FTW_PHYS) != 0) {
        if (error) {
            *error = g_nftwError.isEmpty() ? QStringLiteral("Failed to remove tree %1").arg(path) : g_nftwError;
        }
        return false;
    }
    return true;
}

bool SafeFs::isForbiddenPermanentTarget(const QString &canonicalPath)
{
    if (canonicalPath.isEmpty() || canonicalPath == QLatin1String("/") || canonicalPath == QDir::homePath()) {
        return true;
    }
    const QStringList forbidden = {
        QStringLiteral("/"),
        QStringLiteral("/home"),
        QStringLiteral("/etc"),
        QStringLiteral("/usr"),
        QStringLiteral("/bin"),
        QStringLiteral("/boot"),
        QStringLiteral("/dev"),
        QStringLiteral("/proc"),
        QStringLiteral("/sys"),
        QStringLiteral("/root"),
        QDir::homePath(),
    };
    if (forbidden.contains(canonicalPath)) {
        return true;
    }
    const int depth = canonicalPath.count(QLatin1Char('/'));
    if (depth <= 1) {
        return true;
    }
    return false;
}

QString SafeFs::siblingTemp(const QString &destination, const QString &prefix)
{
    const QFileInfo info(destination);
    const quint32 rand = QRandomGenerator::global()->generate();
    return info.absolutePath() + QLatin1Char('/') + prefix + QString::number(rand, 16);
}

HashResult SafeFs::sha256File(const QString &path, qint64 maxBytes, std::atomic<bool> *cancel)
{
    HashResult result;
    QString error;
    if (!isRegularFile(path, &error)) {
        result.error = error;
        return result;
    }
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        result.error = QStringLiteral("Cannot read %1").arg(path);
        return result;
    }
    QCryptographicHash hash(QCryptographicHash::Sha256);
    QByteArray buffer(64 * 1024, Qt::Uninitialized);
    qint64 total = 0;
    while (!file.atEnd()) {
        if (cancel && cancel->load()) {
            result.cancelled = true;
            result.error = QStringLiteral("Cancelled");
            return result;
        }
        const qint64 n = file.read(buffer.data(), buffer.size());
        if (n < 0) {
            result.error = QStringLiteral("Read failed");
            return result;
        }
        total += n;
        if (total > maxBytes) {
            result.truncated = true;
            result.error = QStringLiteral("Hash exceeded size bound");
            return result;
        }
        hash.addData(QByteArrayView(buffer.constData(), static_cast<int>(n)));
    }
    result.bytesRead = total;
    result.sha256 = hash.result();
    return result;
}

QString SafeFs::hexSha256(const QByteArray &digest)
{
    return QString::fromLatin1(digest.toHex());
}

QByteArray SafeFs::sha256Bytes(const QByteArray &data)
{
    return QCryptographicHash::hash(data, QCryptographicHash::Sha256);
}

} // namespace GoshAim
