#pragma once

#include "Types.h"

#include <QByteArray>
#include <QString>
#include <QStringList>
#include <QVector>

namespace GoshAim {

struct ParsedDesktop {
    bool ok = false;
    QString error;
    AppImageMetadata metadata;
    QVector<DesktopAction> actions;
    QString rawName;
};

class DesktopParser
{
public:
    static ParsedDesktop parse(const QByteArray &data);
    static QString sanitizeName(const QString &value);
    static QString sanitizeComment(const QString &value);
    static QString sanitizeIconName(const QString &value);
    static QStringList tokenizeExec(const QString &exec);
    static QStringList rewriteArguments(const QStringList &tokens);
    static bool isValidEnvName(const QString &name);
    static bool isDangerousEnvName(const QString &name);
    static bool isValidEnvValue(const QString &value);
    static QString escapeDesktopValue(const QString &value);
    static QString escapeExecArg(const QString &token);
    static QString buildExecLine(const QString &program, const QStringList &arguments, const QVector<EnvPair> &environment);
    static QString sanitizeFileBase(const QString &name);
};

class ArchiveGuard
{
public:
    static bool isSafeEntry(const QString &entry, QString *error = nullptr);
    static bool isSafeLinkTarget(const QString &entry, const QString &target, QString *error = nullptr);
    static QStringList filterExtractable(const QStringList &entries, QString *error = nullptr);
    static QStringList filterExtractable(const QVector<ArchiveEntry> &entries, QString *error = nullptr);
    static QVector<ArchiveEntry> parseUnsquashfsList(const QByteArray &listing, QString *error = nullptr);
    static QVector<ArchiveEntry> parse7zList(const QByteArray &listing, QString *error = nullptr);
    static QVector<ArchiveEntry> parseDwarfsList(const QByteArray &listing, QString *error = nullptr);
    static bool verifyExtractedTree(const QString &root, qint64 maxBytes, QString *error = nullptr);
};

} // namespace GoshAim
