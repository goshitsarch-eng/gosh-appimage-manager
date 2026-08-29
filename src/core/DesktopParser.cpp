#include "DesktopParser.h"

#include "Limits.h"

#include <QFileInfo>
#include <QHash>
#include <QRegularExpression>

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

QStringList ArchiveGuard::filterExtractable(const QStringList &entries, QString *error)
{
    QStringList out;
    if (entries.size() > kMaxArchiveEntries) {
        if (error) {
            *error = QStringLiteral("Archive listing exceeded entry bound");
        }
        return {};
    }
    for (const QString &entry : entries) {
        QString itemError;
        if (!isSafeEntry(entry, &itemError)) {
            if (error) {
                *error = itemError;
            }
            return {};
        }
        const QString base = entry.section(QLatin1Char('/'), -1);
        if (entry.endsWith(QLatin1String(".desktop"), Qt::CaseInsensitive) && !entry.contains(QLatin1Char('/'))) {
            out.append(entry);
        } else if (entry == QLatin1String(".DirIcon") || base == QLatin1String(".DirIcon")) {
            out.append(entry);
        } else if ((entry.endsWith(QLatin1String(".png"), Qt::CaseInsensitive)
                    || entry.endsWith(QLatin1String(".svg"), Qt::CaseInsensitive))
                   && entry.count(QLatin1Char('/')) <= 2) {
            out.append(entry);
        }
        if (out.size() > kMaxExtractedFiles) {
            if (error) {
                *error = QStringLiteral("Too many extractable files");
            }
            return {};
        }
    }
    return out;
}

} // namespace GoshAim
