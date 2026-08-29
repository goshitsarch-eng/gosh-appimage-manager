#include "DesktopParser.h"

#include "Limits.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QHash>
#include <QRegularExpression>

#include <fcntl.h>
#include <ftw.h>
#include <sys/stat.h>
#include <unistd.h>

namespace GoshAim {

namespace {

bool hasControl(const QString &value)
{
    for (const QChar ch : value) {
        if (ch.unicode() < 0x20 && ch != QLatin1Char('\t')) {
            return true;
        }
    }
    return false;
}

QString unescapeDesktop(const QString &value)
{
    QString out;
    out.reserve(value.size());
    for (int i = 0; i < value.size(); ++i) {
        if (value[i] == QLatin1Char('\\') && i + 1 < value.size()) {
            const QChar n = value[i + 1];
            if (n == QLatin1Char('s')) {
                out += QLatin1Char(' ');
            } else if (n == QLatin1Char('n')) {
                out += QLatin1Char('\n');
            } else if (n == QLatin1Char('t')) {
                out += QLatin1Char('\t');
            } else if (n == QLatin1Char('r')) {
                out += QLatin1Char('\r');
            } else if (n == QLatin1Char('\\')) {
                out += QLatin1Char('\\');
            } else {
                out += n;
            }
            ++i;
        } else {
            out += value[i];
        }
    }
    return out;
}

} // namespace

QString DesktopParser::sanitizeName(const QString &value)
{
    QString cleaned = unescapeDesktop(value).trimmed();
    cleaned.remove(QChar(0));
    if (cleaned.size() > kMaxNameLength) {
        cleaned = cleaned.left(kMaxNameLength);
    }
    QString out;
    for (const QChar ch : cleaned) {
        if (ch.unicode() >= 0x20 || ch == QLatin1Char('\t')) {
            out += ch;
        }
    }
    return out;
}

QString DesktopParser::sanitizeComment(const QString &value)
{
    return sanitizeName(value);
}

QString DesktopParser::sanitizeIconName(const QString &value)
{
    QString cleaned = unescapeDesktop(value).trimmed();
    cleaned.remove(QChar(0));
    if (cleaned.contains(QLatin1String("://")) || cleaned.startsWith(QLatin1Char('~'))) {
        return {};
    }
    if (cleaned.contains(QLatin1String(".."))) {
        return {};
    }
    return cleaned.left(kMaxPathLength);
}

QStringList DesktopParser::tokenizeExec(const QString &exec)
{
    QStringList tokens;
    QString current;
    bool inQuote = false;
    QChar quote;
    bool escape = false;
    for (int i = 0; i < exec.size(); ++i) {
        const QChar ch = exec[i];
        if (escape) {
            current += ch;
            escape = false;
            continue;
        }
        if (ch == QLatin1Char('\\')) {
            escape = true;
            continue;
        }
        if (!inQuote && (ch == QLatin1Char('"') || ch == QLatin1Char('\''))) {
            inQuote = true;
            quote = ch;
            continue;
        }
        if (inQuote && ch == quote) {
            inQuote = false;
            continue;
        }
        if (!inQuote && ch.isSpace()) {
            if (!current.isEmpty()) {
                tokens.append(current);
                current.clear();
            }
            continue;
        }
        current += ch;
    }
    if (!current.isEmpty()) {
        tokens.append(current);
    }
    if (tokens.size() > kMaxArguments) {
        tokens = tokens.mid(0, kMaxArguments);
    }
    return tokens;
}

QStringList DesktopParser::rewriteArguments(const QStringList &tokens)
{
    QStringList out;
    bool skipProgram = true;
    for (const QString &token : tokens) {
        if (skipProgram) {
            skipProgram = false;
            continue;
        }
        if (token.startsWith(QLatin1Char('%'))) {
            const QString code = token.left(2);
            if (code == QLatin1String("%f") || code == QLatin1String("%F") || code == QLatin1String("%u")
                || code == QLatin1String("%U") || code == QLatin1String("%c")) {
                out.append(code);
            }
            continue;
        }
        if (token.contains(QChar(0)) || hasControl(token) || token.size() > kMaxArgumentLength) {
            continue;
        }
        out.append(token);
    }
    return out;
}

bool DesktopParser::isValidEnvName(const QString &name)
{
    static const QRegularExpression re(QStringLiteral("^[A-Za-z_][A-Za-z0-9_]*$"));
    return re.match(name).hasMatch() && name.size() <= 64;
}

bool DesktopParser::isDangerousEnvName(const QString &name)
{
    static const QStringList dangerous = {
        QStringLiteral("LD_PRELOAD"),
        QStringLiteral("LD_AUDIT"),
        QStringLiteral("LD_LIBRARY_PATH"),
        QStringLiteral("LD_DEBUG"),
        QStringLiteral("BASH_ENV"),
        QStringLiteral("ENV"),
        QStringLiteral("IFS"),
        QStringLiteral("SHELLOPTS"),
        QStringLiteral("GCONV_PATH"),
        QStringLiteral("NLSPATH"),
        QStringLiteral("DYLD_INSERT_LIBRARIES"),
        QStringLiteral("DYLD_LIBRARY_PATH"),
    };
    return dangerous.contains(name) || name.startsWith(QLatin1String("LD_")) || name.startsWith(QLatin1String("DYLD_"));
}

bool DesktopParser::isValidEnvValue(const QString &value)
{
    return !value.contains(QChar(0)) && !hasControl(value) && value.size() <= kMaxArgumentLength;
}

QString DesktopParser::escapeDesktopValue(const QString &value)
{
    QString out;
    for (const QChar ch : value) {
        if (ch == QLatin1Char('\\')) {
            out += QLatin1String("\\\\");
        } else if (ch == QLatin1Char('\n')) {
            out += QLatin1String("\\n");
        } else if (ch == QLatin1Char('\t')) {
            out += QLatin1String("\\t");
        } else if (ch == QLatin1Char('\r')) {
            out += QLatin1String("\\r");
        } else {
            out += ch;
        }
    }
    return out;
}

QString DesktopParser::escapeExecArg(const QString &token)
{
    if (token.startsWith(QLatin1Char('%')) && token.size() == 2) {
        return token;
    }
    bool needQuote = false;
    for (const QChar ch : token) {
        if (ch.isSpace() || ch == QLatin1Char('"') || ch == QLatin1Char('\\') || ch == QLatin1Char('$')
            || ch == QLatin1Char('`') || ch == QLatin1Char('\'')) {
            needQuote = true;
            break;
        }
    }
    if (!needQuote) {
        return token;
    }
    QString out = QStringLiteral("\"");
    for (const QChar ch : token) {
        if (ch == QLatin1Char('"') || ch == QLatin1Char('\\') || ch == QLatin1Char('$') || ch == QLatin1Char('`')) {
            out += QLatin1Char('\\');
        }
        out += ch;
    }
    out += QLatin1Char('"');
    return out;
}

QString DesktopParser::buildExecLine(const QString &program, const QStringList &arguments, const QVector<EnvPair> &environment)
{
    QStringList parts;
    QVector<EnvPair> safeEnv;
    for (const EnvPair &pair : environment) {
        if (isValidEnvName(pair.name) && !isDangerousEnvName(pair.name) && isValidEnvValue(pair.value)) {
            safeEnv.append(pair);
        }
    }
    if (!safeEnv.isEmpty()) {
        parts.append(QStringLiteral("env"));
        for (const EnvPair &pair : safeEnv) {
            parts.append(pair.name + QLatin1Char('=') + escapeExecArg(pair.value));
        }
    }
    parts.append(escapeExecArg(program));
    int count = 0;
    for (const QString &argument : arguments) {
        if (count++ >= kMaxArguments) {
            break;
        }
        if (argument.contains(QChar(0)) || argument.size() > kMaxArgumentLength) {
            continue;
        }
        parts.append(escapeExecArg(argument));
    }
    return parts.join(QLatin1Char(' '));
}

QString DesktopParser::sanitizeFileBase(const QString &name)
{
    QString out;
    for (const QChar ch : name) {
        if (ch.isLetterOrNumber() || ch == QLatin1Char('.') || ch == QLatin1Char('_') || ch == QLatin1Char('-')
            || ch == QLatin1Char('+')) {
            out += ch;
        } else if (ch.isSpace()) {
            out += QLatin1Char('-');
        }
    }
    while (out.contains(QLatin1String("--"))) {
        out.replace(QLatin1String("--"), QLatin1String("-"));
    }
    if (out.isEmpty()) {
        out = QStringLiteral("AppImage");
    }
    return out.left(80);
}

ParsedDesktop DesktopParser::parse(const QByteArray &data)
{
    ParsedDesktop parsed;
    if (data.size() > kMaxDesktopFileBytes) {
        parsed.error = QStringLiteral("Desktop file exceeds size bound");
        return parsed;
    }
    if (data.contains('\0')) {
        parsed.error = QStringLiteral("Desktop file contains NUL");
        return parsed;
    }
    const QString text = QString::fromUtf8(data);
    QString section;
    QHash<QString, QString> entry;
    QHash<QString, QHash<QString, QString>> actions;
    int lines = 0;
    for (QString line : text.split(QLatin1Char('\n'))) {
        if (++lines > 400) {
            parsed.error = QStringLiteral("Desktop file has too many lines");
            return parsed;
        }
        line = line.trimmed();
        if (line.isEmpty() || line.startsWith(QLatin1Char('#'))) {
            continue;
        }
        if (line.startsWith(QLatin1Char('[')) && line.endsWith(QLatin1Char(']'))) {
            section = line.mid(1, line.size() - 2);
            continue;
        }
        const int eq = line.indexOf(QLatin1Char('='));
        if (eq <= 0) {
            continue;
        }
        const QString key = line.left(eq);
        const QString value = line.mid(eq + 1);
        if (section == QLatin1String("Desktop Entry")) {
            if (!key.contains(QLatin1Char('['))) {
                entry.insert(key, value);
            }
        } else if (section.startsWith(QLatin1String("Desktop Action "))) {
            const QString id = section.mid(QStringLiteral("Desktop Action ").size());
            if (!id.contains(QLatin1Char('/')) && id.size() < 64) {
                actions[id].insert(key, value);
            }
        }
    }
    parsed.metadata.name = sanitizeName(entry.value(QStringLiteral("Name")));
    parsed.rawName = parsed.metadata.name;
    parsed.metadata.comment = sanitizeComment(entry.value(QStringLiteral("Comment")));
    parsed.metadata.iconName = sanitizeIconName(entry.value(QStringLiteral("Icon")));
    parsed.metadata.version = sanitizeName(entry.value(QStringLiteral("X-AppImage-Version")));
    if (parsed.metadata.version.isEmpty()) {
        parsed.metadata.version = sanitizeName(entry.value(QStringLiteral("Version")));
    }
    parsed.metadata.terminal = entry.value(QStringLiteral("Terminal")).compare(QLatin1String("true"), Qt::CaseInsensitive) == 0;
    parsed.metadata.execRaw = entry.value(QStringLiteral("Exec"));
    parsed.metadata.execArguments = rewriteArguments(tokenizeExec(parsed.metadata.execRaw));
    parsed.metadata.tryExec = sanitizeIconName(entry.value(QStringLiteral("TryExec")));
    parsed.metadata.website = entry.value(QStringLiteral("X-AppImage-Title")).trimmed();
    parsed.metadata.startupWmClass = sanitizeName(entry.value(QStringLiteral("StartupWMClass")));
    const QString cats = entry.value(QStringLiteral("Categories"));
    for (const QString &cat : cats.split(QLatin1Char(';'), Qt::SkipEmptyParts)) {
        const QString s = sanitizeName(cat);
        if (!s.isEmpty()) {
            parsed.metadata.categories.append(s);
        }
    }
    const QString mimes = entry.value(QStringLiteral("MimeType"));
    for (const QString &mime : mimes.split(QLatin1Char(';'), Qt::SkipEmptyParts)) {
        const QString s = sanitizeName(mime);
        if (!s.isEmpty()) {
            parsed.metadata.mimeTypes.append(s);
        }
    }
    for (auto it = actions.constBegin(); it != actions.constEnd(); ++it) {
        DesktopAction action;
        action.id = sanitizeFileBase(it.key());
        action.name = sanitizeName(it.value().value(QStringLiteral("Name")));
        action.arguments = rewriteArguments(tokenizeExec(it.value().value(QStringLiteral("Exec"))));
        if (!action.id.isEmpty() && !action.name.isEmpty()) {
            parsed.actions.append(action);
        }
    }
    if (parsed.metadata.name.isEmpty()) {
        parsed.error = QStringLiteral("Desktop entry is missing a Name");
        return parsed;
    }
    parsed.ok = true;
    return parsed;
}

bool DesktopParser::hasExactKeyValue(const QString &text, const QString &key, const QString &value)
{
    return exactKeyValue(text, key) == value;
}

QString DesktopParser::exactKeyValue(const QString &text, const QString &key)
{
    const QStringList lines = text.split(QLatin1Char('\n'));
    for (QString line : lines) {
        if (line.endsWith(QLatin1Char('\r'))) {
            line.chop(1);
        }
        if (line.startsWith(QLatin1Char('#')) || line.startsWith(QLatin1Char('['))) {
            continue;
        }
        const int eq = line.indexOf(QLatin1Char('='));
        if (eq <= 0) {
            continue;
        }
        if (line.left(eq) == key) {
            return line.mid(eq + 1);
        }
    }
    return {};
}

bool ArchiveGuard::isSafeEntry(const QString &entry, QString *error)
{
    if (entry.isEmpty() || entry.contains(QChar(0))) {
        if (error) {
            *error = QStringLiteral("Empty or NUL archive path");
        }
        return false;
    }
    if (entry.size() > kMaxPathLength) {
        if (error) {
            *error = QStringLiteral("Archive path too long");
        }
        return false;
    }
    if (entry.startsWith(QLatin1Char('/')) || entry.startsWith(QLatin1Char('\\')) || QFileInfo(entry).isAbsolute()) {
        if (error) {
            *error = QStringLiteral("Absolute archive path rejected");
        }
        return false;
    }
    const QStringList parts = entry.split(QLatin1Char('/'), Qt::SkipEmptyParts);
    if (parts.size() > kMaxArchiveDepth) {
        if (error) {
            *error = QStringLiteral("Archive path nested too deeply");
        }
        return false;
    }
    for (const QString &part : parts) {
        if (part == QLatin1String("..")) {
            if (error) {
                *error = QStringLiteral("Archive path contains ..");
            }
            return false;
        }
    }
    return true;
}

bool ArchiveGuard::isSafeLinkTarget(const QString &entry, const QString &target, QString *error)
{
    if (target.startsWith(QLatin1Char('/')) || target.contains(QLatin1String(".."))) {
        if (error) {
            *error = QStringLiteral("Symlink target escapes extraction root");
        }
        return false;
    }
    return isSafeEntry(entry, error);
}

namespace {

bool isWantedMetadata(const QString &entry)
{
    const QString base = entry.section(QLatin1Char('/'), -1);
    if (entry.endsWith(QLatin1String(".desktop"), Qt::CaseInsensitive) && !entry.contains(QLatin1Char('/'))) {
        return true;
    }
    if (entry == QLatin1String(".DirIcon") || base == QLatin1String(".DirIcon")) {
        return true;
    }
    if ((entry.endsWith(QLatin1String(".png"), Qt::CaseInsensitive) || entry.endsWith(QLatin1String(".svg"), Qt::CaseInsensitive))
        && entry.count(QLatin1Char('/')) <= 2) {
        return true;
    }
    return false;
}

QString stripListPrefix(QString path)
{
    path = path.trimmed();
    if (path.startsWith(QLatin1String("./"))) {
        path = path.mid(2);
    }
    const QStringList prefixes = {QStringLiteral("squashfs-root/"), QStringLiteral("root/")};
    for (const QString &prefix : prefixes) {
        if (path.startsWith(prefix)) {
            path = path.mid(prefix.size());
        }
    }
    return path;
}

thread_local QString g_extractWalkError;
thread_local qint64 g_extractWalkBytes = 0;
thread_local int g_extractWalkFiles = 0;
thread_local qint64 g_extractWalkLimit = 0;
thread_local QString g_extractWalkRoot;

int ntfwVerify(const char *fpath, const struct stat *sb, int typeflag, struct FTW *ftwbuf)
{
    Q_UNUSED(ftwbuf);
    const QString path = QString::fromLocal8Bit(fpath);
    if (typeflag == FTW_SL || typeflag == FTW_SLN) {
        char buf[4096];
        const ssize_t n = ::readlink(fpath, buf, sizeof(buf) - 1);
        if (n <= 0) {
            g_extractWalkError = QStringLiteral("Unreadable symlink in extraction");
            return 1;
        }
        buf[n] = 0;
        const QString target = QString::fromLocal8Bit(buf);
        QString err;
        if (!ArchiveGuard::isSafeLinkTarget(QFileInfo(path).fileName(), target, &err)) {
            g_extractWalkError = err;
            return 1;
        }
        return 0;
    }
    if (S_ISCHR(sb->st_mode) || S_ISBLK(sb->st_mode) || S_ISFIFO(sb->st_mode) || S_ISSOCK(sb->st_mode)) {
        g_extractWalkError = QStringLiteral("Device or special node in extraction");
        return 1;
    }
    if (typeflag == FTW_F) {
        ++g_extractWalkFiles;
        g_extractWalkBytes += sb->st_size;
        if (g_extractWalkFiles > kMaxExtractedFiles) {
            g_extractWalkError = QStringLiteral("Too many extracted files");
            return 1;
        }
        if (g_extractWalkBytes > g_extractWalkLimit) {
            g_extractWalkError = QStringLiteral("Extracted size exceeded bound");
            return 1;
        }
        if (sb->st_size > kMaxExtractedBytes) {
            g_extractWalkError = QStringLiteral("Extracted file exceeded per-file bound");
            return 1;
        }
    }
    return 0;
}

} // namespace

QStringList ArchiveGuard::filterExtractable(const QStringList &entries, QString *error)
{
    QVector<ArchiveEntry> typed;
    typed.reserve(entries.size());
    for (const QString &entry : entries) {
        ArchiveEntry item;
        item.path = entry;
        typed.append(item);
    }
    return filterExtractable(typed, error);
}

QStringList ArchiveGuard::filterExtractable(const QVector<ArchiveEntry> &entries, QString *error)
{
    QStringList out;
    qint64 total = 0;
    for (const ArchiveEntry &entry : entries) {
        const bool wanted = isWantedMetadata(entry.path);
        QString itemError;
        if (entry.kind == ArchiveEntryKind::Device || entry.kind == ArchiveEntryKind::Other) {
            if (wanted) {
                if (error) {
                    *error = QStringLiteral("Archive contains a device or special node");
                }
                return {};
            }
            continue;
        }
        if (!isSafeEntry(entry.path, &itemError)) {
            if (wanted) {
                if (error) {
                    *error = itemError;
                }
                return {};
            }
            continue;
        }
        if (entry.kind == ArchiveEntryKind::Symlink) {
            if (wanted && !isSafeLinkTarget(entry.path, entry.linkTarget, &itemError)) {
                if (error) {
                    *error = itemError;
                }
                return {};
            }
            continue;
        }
        if (entry.kind == ArchiveEntryKind::Directory) {
            continue;
        }
        if (!wanted) {
            continue;
        }
        if (entry.size > kMaxExtractedBytes) {
            if (error) {
                *error = QStringLiteral("Archive member exceeds per-file bound");
            }
            return {};
        }
        total += qMax<qint64>(0, entry.size);
        if (total > kMaxExtractedBytes) {
            if (error) {
                *error = QStringLiteral("Archive expanded size exceeded bound");
            }
            return {};
        }
        out.append(entry.path);
        if (out.size() > kMaxExtractedFiles) {
            if (error) {
                *error = QStringLiteral("Too many extractable files");
            }
            return {};
        }
    }
    return out;
}

QVector<ArchiveEntry> ArchiveGuard::parseUnsquashfsList(const QByteArray &listing, QString *error)
{
    QVector<ArchiveEntry> out;
    const QString text = QString::fromUtf8(listing);
    for (QString line : text.split(QLatin1Char('\n'))) {
        line = line.trimmed();
        if (line.isEmpty() || line.startsWith(QLatin1String("unsquashfs")) || line.startsWith(QLatin1String("Parallel"))) {
            continue;
        }
        ArchiveEntry entry;
        if (line.size() >= 10 && (line[0] == QLatin1Char('-') || line[0] == QLatin1Char('d') || line[0] == QLatin1Char('l')
                                  || line[0] == QLatin1Char('c') || line[0] == QLatin1Char('b') || line[0] == QLatin1Char('p')
                                  || line[0] == QLatin1Char('s'))) {
            const QChar kind = line[0];
            if (kind == QLatin1Char('c') || kind == QLatin1Char('b') || kind == QLatin1Char('p') || kind == QLatin1Char('s')) {
                entry.kind = ArchiveEntryKind::Device;
            } else if (kind == QLatin1Char('d')) {
                entry.kind = ArchiveEntryKind::Directory;
            } else if (kind == QLatin1Char('l')) {
                entry.kind = ArchiveEntryKind::Symlink;
            } else {
                entry.kind = ArchiveEntryKind::File;
            }
            const int arrow = line.lastIndexOf(QLatin1String(" -> "));
            QString rest = line;
            if (arrow > 0) {
                entry.linkTarget = line.mid(arrow + 4).trimmed();
                rest = line.left(arrow);
            }
            const QStringList parts = rest.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
            if (parts.size() >= 6) {
                entry.size = parts.at(2).toLongLong();
                entry.path = stripListPrefix(parts.mid(5).join(QLatin1Char(' ')));
            } else if (parts.size() >= 2) {
                entry.path = stripListPrefix(parts.last());
            }
        } else {
            entry.path = stripListPrefix(line);
            entry.kind = ArchiveEntryKind::File;
        }
        if (entry.path.isEmpty() || entry.path == QLatin1String(".") || entry.path == QLatin1String("squashfs-root")) {
            continue;
        }
        out.append(entry);
        if (out.size() > kMaxArchiveListingEntries) {
            if (error) {
                *error = QStringLiteral("Archive listing exceeded entry bound");
            }
            return {};
        }
    }
    return out;
}

QVector<ArchiveEntry> ArchiveGuard::parse7zList(const QByteArray &listing, QString *error)
{
    QVector<ArchiveEntry> out;
    ArchiveEntry current;
    bool inItem = false;
    const QString text = QString::fromUtf8(listing);
    const auto flush = [&]() {
        if (inItem && !current.path.isEmpty()) {
            out.append(current);
        }
        current = {};
        inItem = false;
    };
    for (QString line : text.split(QLatin1Char('\n'))) {
        line = line.trimmed();
        if (line.startsWith(QLatin1String("Path = "))) {
            flush();
            current.path = stripListPrefix(line.mid(7).trimmed());
            inItem = true;
        } else if (line.startsWith(QLatin1String("Size = "))) {
            current.size = line.mid(7).trimmed().toLongLong();
        } else if (line.startsWith(QLatin1String("Folder = "))) {
            if (line.mid(9).trimmed() == QLatin1String("+")) {
                current.kind = ArchiveEntryKind::Directory;
            }
        } else if (line.startsWith(QLatin1String("Attributes = "))) {
            const QString attr = line.mid(13).trimmed();
            if (attr.contains(QLatin1Char('D'))) {
                current.kind = ArchiveEntryKind::Directory;
            }
        } else if (line.startsWith(QLatin1String("Symbolic Link = "))) {
            const QString target = line.mid(16).trimmed();
            if (!target.isEmpty()) {
                current.kind = ArchiveEntryKind::Symlink;
                current.linkTarget = target;
            }
        } else if (line.startsWith(QLatin1String("Character")) || line.startsWith(QLatin1String("Block"))
                   || line.contains(QLatin1String("Node = "))) {
            if (inItem) {
                current.kind = ArchiveEntryKind::Device;
            }
        }
        if (out.size() > kMaxArchiveListingEntries) {
            if (error) {
                *error = QStringLiteral("Archive listing exceeded entry bound");
            }
            return {};
        }
    }
    flush();
    return out;
}

QVector<ArchiveEntry> ArchiveGuard::parseDwarfsList(const QByteArray &listing, QString *error)
{
    QVector<ArchiveEntry> out;
    const QString text = QString::fromUtf8(listing);
    for (QString line : text.split(QLatin1Char('\n'))) {
        line = line.trimmed();
        if (line.isEmpty() || line.startsWith(QLatin1Char('#'))) {
            continue;
        }
        ArchiveEntry entry;
        if (line.size() >= 10 && (line[0] == QLatin1Char('-') || line[0] == QLatin1Char('d') || line[0] == QLatin1Char('l')
                                  || line[0] == QLatin1Char('c') || line[0] == QLatin1Char('b'))) {
            const QChar kind = line[0];
            if (kind == QLatin1Char('c') || kind == QLatin1Char('b')) {
                entry.kind = ArchiveEntryKind::Device;
            } else if (kind == QLatin1Char('d')) {
                entry.kind = ArchiveEntryKind::Directory;
            } else if (kind == QLatin1Char('l')) {
                entry.kind = ArchiveEntryKind::Symlink;
            }
            const int arrow = line.lastIndexOf(QLatin1String(" -> "));
            QString rest = line;
            if (arrow > 0) {
                entry.linkTarget = line.mid(arrow + 4).trimmed();
                rest = line.left(arrow);
            }
            const QStringList parts = rest.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
            if (!parts.isEmpty()) {
                entry.path = stripListPrefix(parts.last());
            }
            if (parts.size() >= 5) {
                entry.size = parts.at(4).toLongLong();
            }
        } else {
            const QStringList parts = line.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
            if (parts.size() >= 2 && parts.first().at(0).isDigit()) {
                entry.size = parts.first().toLongLong();
                entry.path = stripListPrefix(parts.last());
            } else {
                entry.path = stripListPrefix(line);
            }
        }
        if (entry.path.isEmpty() || entry.path == QLatin1String(".")) {
            continue;
        }
        out.append(entry);
        if (out.size() > kMaxArchiveListingEntries) {
            if (error) {
                *error = QStringLiteral("Archive listing exceeded entry bound");
            }
            return {};
        }
    }
    return out;
}

bool ArchiveGuard::verifyExtractedTree(const QString &root, qint64 maxBytes, QString *error)
{
    g_extractWalkError.clear();
    g_extractWalkBytes = 0;
    g_extractWalkFiles = 0;
    g_extractWalkLimit = maxBytes;
    g_extractWalkRoot = root;
    if (::nftw(root.toLocal8Bit().constData(), ntfwVerify, 16, FTW_PHYS) != 0) {
        if (error) {
            *error = g_extractWalkError.isEmpty() ? QStringLiteral("Extracted tree failed safety walk") : g_extractWalkError;
        }
        return false;
    }
    if (!g_extractWalkError.isEmpty()) {
        if (error) {
            *error = g_extractWalkError;
        }
        return false;
    }
    return true;
}

} // namespace GoshAim
