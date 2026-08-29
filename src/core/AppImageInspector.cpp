#include "AppImageInspector.h"

#include "DesktopParser.h"
#include "ElfParser.h"
#include "ProcessRunner.h"
#include "SafeFs.h"
#include "ManagedRegistry.h"
#include "SettingsStore.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QTemporaryDir>
#include <QUuid>
#include <sys/stat.h>
#include <unistd.h>

namespace GoshAim {

AppImageInspector::AppImageInspector(ProcessRunner *runner, SettingsStore *settings, ManagedRegistry *registry)
    : m_runner(runner)
    , m_settings(settings)
    , m_registry(registry)
{
}

EmbeddedUpdateInfo AppImageInspector::parseUpdInfo(const QByteArray &raw)
{
    EmbeddedUpdateInfo info;
    QByteArray cleaned = raw;
    cleaned.replace('\0', ' ');
    info.raw = QString::fromUtf8(cleaned).trimmed();
    if (info.raw.startsWith(QLatin1String("gh-releases-zsync|"))) {
        const QStringList parts = info.raw.split(QLatin1Char('|'));
        if (parts.size() == 5) {
            info.managerHint = QStringLiteral("github");
            info.fields.insert(QStringLiteral("username"), parts[1]);
            info.fields.insert(QStringLiteral("repo"), parts[2]);
            info.fields.insert(QStringLiteral("release"), parts[3]);
            info.fields.insert(QStringLiteral("filename"), parts[4]);
        }
    } else if (info.raw.startsWith(QLatin1String("zsync|"))) {
        info.managerHint = QStringLiteral("static");
        info.fields.insert(QStringLiteral("url"), info.raw.mid(6).trimmed());
    }
    return info;
}

InspectionResult AppImageInspector::inspect(const QString &path, const InspectOptions &options, std::atomic<bool> *cancel)
{
    InspectionResult result;
    result.identity.path = path;
    QString error;
    if (!SafeFs::isRegularFile(path, &error)) {
        result.error = error;
        return result;
    }
    const QFileInfo info(path);
    result.identity.size = info.size();
    if (result.identity.size <= 0) {
        result.error = QStringLiteral("Empty file");
        return result;
    }
    if (result.identity.size > options.maxBytes) {
        result.error = QStringLiteral("File exceeds configured size bound");
        return result;
    }

    const ElfInfo elf = ElfParser::parseFile(path);
    result.type = elf.appImageType;
    result.architecture = elf.architecture;
    result.magicValid = elf.appImageType != AppImageType::Unknown;
    result.architectureSupported = ElfParser::architectureSupported(elf.architecture);
    result.truncated = elf.truncated;
    result.payloadOffset = elf.payloadOffset;
    if (!elf.updInfo.isEmpty()) {
        result.updateInfo = parseUpdInfo(elf.updInfo);
    }
    if (!result.magicValid) {
        result.error = elf.error.isEmpty() ? QStringLiteral("Not a valid AppImage") : elf.error;
        return result;
    }
    if (elf.architecture == Architecture::Unknown) {
        result.warnings.append(QStringLiteral("Unknown or unsupported ELF architecture"));
    } else if (!result.architectureSupported) {
        result.warnings.append(QStringLiteral("Architecture %1 is not supported for integration")
                                   .arg(architectureName(elf.architecture)));
    }
    if (result.truncated) {
        result.warnings.append(QStringLiteral("ELF headers look truncated"));
    }

    if (options.computeHash) {
        const HashResult hash = SafeFs::sha256File(path, options.maxBytes, cancel);
        if (hash.cancelled) {
            result.error = QStringLiteral("Cancelled");
            return result;
        }
        if (!hash.error.isEmpty() && hash.sha256.isEmpty()) {
            result.error = hash.error;
            return result;
        }
        result.identity.sha256 = hash.sha256;
    }

    if (m_registry) {
        const InstalledApp existing = m_registry->byPath(path);
        if (!existing.uuid.isEmpty()) {
            result.alreadyManaged = true;
            result.existingManagedId = existing.uuid;
        } else if (!result.identity.sha256.isEmpty()) {
            for (const InstalledApp &app : m_registry->apps()) {
                if (app.sha256 == result.identity.sha256) {
                    result.alreadyManaged = true;
                    result.existingManagedId = app.uuid;
                    break;
                }
            }
        }
    }

    result.metadata.name = QFileInfo(path).completeBaseName();
    if (options.extractMetadata) {
        extractMetadata(result, options, cancel);
    }
    return result;
}

bool AppImageInspector::extractMetadata(InspectionResult &result, const InspectOptions &options, std::atomic<bool> *cancel)
{
    result.extractionAttempted = true;
    QTemporaryDir tmp(QDir::tempPath() + QStringLiteral("/gosh-aim-XXXXXX"));
    tmp.setAutoRemove(true);
    if (!tmp.isValid()) {
        result.warnings.append(QStringLiteral("Cannot create private extraction directory"));
        return false;
    }
    ::chmod(tmp.path().toLocal8Bit().constData(), 0700);
    const QString dest = tmp.path() + QStringLiteral("/root");
    SafeFs::mkdir0700(dest);

    bool extracted = false;
    if (result.type == AppImageType::Type2 || result.type == AppImageType::Unknown) {
        extracted = extractWithUnsquashfs(result, dest, cancel);
    }
    if (!extracted && result.type == AppImageType::Dwarfs) {
        extracted = extractWithDwarfs(result, dest, cancel);
    }
    if (!extracted && (result.type == AppImageType::Type1 || result.type == AppImageType::Type2)) {
        extracted = extractWith7z(result, dest, cancel);
    }
    if (!extracted && result.type == AppImageType::Dwarfs) {
        extracted = extractWithDwarfs(result, dest, cancel);
    }
    if (!extracted) {
        const bool allowed = options.allowUnsafeExtract && options.confirmUnsafeExtract
            && m_settings && m_settings->unsafeExtractionFallback();
        if (allowed) {
            result.warnings.append(
                QStringLiteral("Unsafe extractor fallback executes untrusted AppImage code and was explicitly confirmed"));
            extracted = extractUnsafe(result, dest, cancel);
            result.extractionUsedUnsafeFallback = extracted;
        } else {
            result.warnings.append(QStringLiteral("Metadata extraction skipped; unsafe fallback is disabled"));
        }
    }
    if (extracted) {
        ingestExtracted(result, dest);
    }
    SafeFs::removeTreeNoFollow(tmp.path());
    return extracted;
}

bool AppImageInspector::extractWithUnsquashfs(InspectionResult &result, const QString &dest, std::atomic<bool> *cancel)
{
    if (!m_runner) {
        return false;
    }
    ProcessRequest list;
    list.program = QStringLiteral("unsquashfs");
    list.arguments = QStringList{QStringLiteral("-o"),
                                 QString::number(result.payloadOffset < 0 ? 0 : result.payloadOffset),
                                 QStringLiteral("-ll"),
                                 result.identity.path};
    list.timeoutMs = kExtractTimeoutMs;
    list.maxStdoutBytes = kMaxProcessOutputBytes;
    ProcessResult listed = m_runner->run(list, cancel);
    if (listed.refused || listed.failedToStart || listed.timedOut || listed.cancelled || listed.exitCode != 0) {
        list.arguments[2] = QStringLiteral("-l");
        listed = m_runner->run(list, cancel);
    }
    if (listed.truncated) {
        result.warnings.append(QStringLiteral("Archive listing exceeded output bound"));
        return false;
    }
    if (listed.refused || listed.failedToStart || listed.timedOut || listed.cancelled || listed.exitCode != 0) {
        return false;
    }
    QString parseError;
    QVector<ArchiveEntry> entries = ArchiveGuard::parseUnsquashfsList(listed.standardOutput, &parseError);
    if (!parseError.isEmpty()) {
        result.warnings.append(parseError);
        return false;
    }
    if (entries.isEmpty()) {
        QStringList paths;
        for (const QByteArray &line : listed.standardOutput.split('\n')) {
            QString path = QString::fromUtf8(line).trimmed();
            path.replace(QLatin1String("squashfs-root/"), QString());
            if (path.startsWith(QLatin1Char('/'))) {
                path = path.mid(1);
            }
            if (!path.isEmpty()) {
                paths.append(path);
            }
        }
        QString filterError;
        const QStringList wantedFallback = ArchiveGuard::filterExtractable(paths, &filterError);
        if (!filterError.isEmpty() || wantedFallback.isEmpty()) {
            if (!filterError.isEmpty()) {
                result.warnings.append(filterError);
            }
            return false;
        }
        for (const QString &path : wantedFallback) {
            ArchiveEntry entry;
            entry.path = path;
            entries.append(entry);
        }
    }
    QString filterError;
    const QStringList wanted = ArchiveGuard::filterExtractable(entries, &filterError);
    if (!filterError.isEmpty() || wanted.isEmpty()) {
        if (!filterError.isEmpty()) {
            result.warnings.append(filterError);
        }
        return false;
    }
    ProcessRequest extract;
    extract.program = QStringLiteral("unsquashfs");
    extract.arguments = QStringList{QStringLiteral("-o"),
                                    QString::number(result.payloadOffset < 0 ? 0 : result.payloadOffset),
                                    QStringLiteral("-d"),
                                    dest,
                                    result.identity.path};
    for (const QString &entry : wanted) {
        extract.arguments << QStringLiteral("-e") << entry;
    }
    extract.timeoutMs = kExtractTimeoutMs;
    extract.maxStdoutBytes = kMaxProcessOutputBytes;
    const ProcessResult extracted = m_runner->run(extract, cancel);
    if (extracted.exitCode != 0 || extracted.failedToStart || extracted.refused || extracted.truncated) {
        return false;
    }
    QString walkError;
    if (!ArchiveGuard::verifyExtractedTree(dest, kMaxExtractedBytes, &walkError)) {
        result.warnings.append(walkError);
        SafeFs::removeTreeNoFollow(dest);
        return false;
    }
    result.extractorUsed = QStringLiteral("unsquashfs");
    return true;
}

bool AppImageInspector::extractWith7z(InspectionResult &result, const QString &dest, std::atomic<bool> *cancel)
{
    if (!m_runner) {
        return false;
    }
    auto listWith = [&](const QString &program) {
        ProcessRequest list;
        list.program = program;
        list.arguments = QStringList{QStringLiteral("l"), QStringLiteral("-slt"), result.identity.path};
        list.timeoutMs = kExtractTimeoutMs;
        list.maxStdoutBytes = kMaxProcessOutputBytes;
        return m_runner->run(list, cancel);
    };
    ProcessResult listed = listWith(QStringLiteral("7zz"));
    QString program = QStringLiteral("7zz");
    if (listed.failedToStart) {
        listed = listWith(QStringLiteral("7z"));
        program = QStringLiteral("7z");
    }
    if (listed.truncated) {
        result.warnings.append(QStringLiteral("Archive listing exceeded output bound"));
        return false;
    }
    if (listed.refused || listed.failedToStart || listed.timedOut || listed.cancelled || listed.exitCode != 0) {
        return false;
    }
    QString parseError;
    const QVector<ArchiveEntry> entries = ArchiveGuard::parse7zList(listed.standardOutput, &parseError);
    if (!parseError.isEmpty()) {
        result.warnings.append(parseError);
        return false;
    }
    QString filterError;
    const QStringList wanted = ArchiveGuard::filterExtractable(entries, &filterError);
    if (!filterError.isEmpty() || wanted.isEmpty()) {
        if (!filterError.isEmpty()) {
            result.warnings.append(filterError);
        }
        return false;
    }
    ProcessRequest extract;
    extract.program = program;
    extract.arguments = QStringList{QStringLiteral("x"),
                                    result.identity.path,
                                    QStringLiteral("-o") + dest,
                                    QStringLiteral("-y"),
                                    QStringLiteral("-bso0"),
                                    QStringLiteral("-bsp0")};
    for (const QString &entry : wanted) {
        extract.arguments << entry;
    }
    extract.timeoutMs = kExtractTimeoutMs;
    extract.maxStdoutBytes = kMaxProcessOutputBytes;
    const ProcessResult extracted = m_runner->run(extract, cancel);
    if (extracted.exitCode != 0 || extracted.failedToStart || extracted.refused || extracted.truncated) {
        return false;
    }
    QString walkError;
    if (!ArchiveGuard::verifyExtractedTree(dest, kMaxExtractedBytes, &walkError)) {
        result.warnings.append(walkError);
        SafeFs::removeTreeNoFollow(dest);
        return false;
    }
    result.extractorUsed = program;
    return true;
}

bool AppImageInspector::extractWithDwarfs(InspectionResult &result, const QString &dest, std::atomic<bool> *cancel)
{
    if (!m_runner) {
        return false;
    }
    ProcessRequest list;
    list.program = QStringLiteral("dwarfsck");
    list.arguments = QStringList{QStringLiteral("--input=") + result.identity.path, QStringLiteral("--list")};
    list.timeoutMs = kExtractTimeoutMs;
    list.maxStdoutBytes = kMaxProcessOutputBytes;
    ProcessResult listed = m_runner->run(list, cancel);
    if (listed.failedToStart || listed.exitCode != 0) {
        list.arguments = QStringList{QStringLiteral("-i"), result.identity.path, QStringLiteral("-l")};
        listed = m_runner->run(list, cancel);
    }
    if (listed.truncated) {
        result.warnings.append(QStringLiteral("Archive listing exceeded output bound"));
        return false;
    }
    if (listed.refused || listed.failedToStart || listed.timedOut || listed.cancelled || listed.exitCode != 0) {
        result.warnings.append(QStringLiteral("DwarFS listing is required; refusing unbounded extraction"));
        return false;
    }
    QString parseError;
    const QVector<ArchiveEntry> entries = ArchiveGuard::parseDwarfsList(listed.standardOutput, &parseError);
    if (!parseError.isEmpty()) {
        result.warnings.append(parseError);
        return false;
    }
    QString filterError;
    const QStringList wanted = ArchiveGuard::filterExtractable(entries, &filterError);
    if (!filterError.isEmpty() || wanted.isEmpty()) {
        if (!filterError.isEmpty()) {
            result.warnings.append(filterError);
        } else {
            result.warnings.append(QStringLiteral("No safe DwarFS metadata entries"));
        }
        return false;
    }
    for (const QString &entry : wanted) {
        ProcessRequest extract;
        extract.program = QStringLiteral("dwarfsextract");
        extract.arguments = QStringList{QStringLiteral("--input=") + result.identity.path,
                                        QStringLiteral("--output=") + dest,
                                        QStringLiteral("--pattern=") + entry};
        extract.timeoutMs = kExtractTimeoutMs;
        extract.maxStdoutBytes = kMaxProcessOutputBytes;
        const ProcessResult extracted = m_runner->run(extract, cancel);
        if (extracted.exitCode != 0 || extracted.failedToStart || extracted.refused || extracted.truncated) {
            return false;
        }
    }
    QString walkError;
    if (!ArchiveGuard::verifyExtractedTree(dest, kMaxExtractedBytes, &walkError)) {
        result.warnings.append(walkError);
        SafeFs::removeTreeNoFollow(dest);
        return false;
    }
    result.extractorUsed = QStringLiteral("dwarfsextract");
    return true;
}

bool AppImageInspector::extractUnsafe(InspectionResult &result, const QString &dest, std::atomic<bool> *cancel)
{
    if (!m_runner) {
        return false;
    }
    const QString staging = dest + QStringLiteral(".bin");
    QString error;
    qint64 copied = 0;
    if (!SafeFs::copyBounded(result.identity.path, staging, kDefaultMaxAppImageBytes, cancel, &copied, &error)) {
        result.warnings.append(error);
        return false;
    }
    SafeFs::chmodPath(staging, 0700);
    ProcessRequest req;
    req.program = staging;
    req.arguments = QStringList{QStringLiteral("--appimage-extract")};
    req.workingDirectory = QFileInfo(dest).absolutePath();
    req.timeoutMs = kExtractTimeoutMs;
    const ProcessResult extracted = m_runner->run(req, cancel);
    SafeFs::removeFileNoFollow(staging);
    if (extracted.exitCode != 0 || extracted.failedToStart || extracted.refused) {
        return false;
    }
    result.extractorUsed = QStringLiteral("appimage-extract");
    return true;
}

void AppImageInspector::ingestExtracted(InspectionResult &result, const QString &dest)
{
    QDir dir(dest);
    const QFileInfoList desktopFiles = dir.entryInfoList(QStringList{QStringLiteral("*.desktop")}, QDir::Files);
    if (desktopFiles.isEmpty()) {
        const QFileInfoList nested = QDir(dest + QStringLiteral("/squashfs-root"))
                                         .entryInfoList(QStringList{QStringLiteral("*.desktop")}, QDir::Files);
        if (!nested.isEmpty()) {
            dir.setPath(dest + QStringLiteral("/squashfs-root"));
        }
    }
    QFileInfo desktopInfo;
    const QFileInfoList files = dir.entryInfoList(QStringList{QStringLiteral("*.desktop")}, QDir::Files);
    if (!files.isEmpty()) {
        desktopInfo = files.first();
    }
    if (desktopInfo.exists()) {
        QFile file(desktopInfo.absoluteFilePath());
        if (file.open(QIODevice::ReadOnly)) {
            const QByteArray data = file.read(kMaxDesktopFileBytes + 1);
            if (data.size() <= kMaxDesktopFileBytes) {
                const ParsedDesktop parsed = DesktopParser::parse(data);
                if (parsed.ok) {
                    result.metadata = parsed.metadata;
                } else if (!parsed.error.isEmpty()) {
                    result.warnings.append(parsed.error);
                }
            }
        }
    }
    QString iconSource;
    const QFileInfo dirIcon(dir.filePath(QStringLiteral(".DirIcon")));
    if (dirIcon.exists() && dirIcon.isFile()) {
        iconSource = dirIcon.absoluteFilePath();
    } else if (!result.metadata.iconName.isEmpty()) {
        const QFileInfo named(dir.filePath(result.metadata.iconName));
        if (named.exists() && named.isFile()) {
            iconSource = named.absoluteFilePath();
        } else {
            const QFileInfo png(dir.filePath(result.metadata.iconName + QStringLiteral(".png")));
            const QFileInfo svg(dir.filePath(result.metadata.iconName + QStringLiteral(".svg")));
            if (png.exists()) {
                iconSource = png.absoluteFilePath();
            } else if (svg.exists()) {
                iconSource = svg.absoluteFilePath();
            }
        }
    }
    if (!iconSource.isEmpty() && SafeFs::isRegularFile(iconSource)) {
        if (iconSource.contains(QLatin1String(".."))) {
            result.warnings.append(QStringLiteral("Rejected icon path"));
            return;
        }
        const QString cacheRoot = m_settings ? m_settings->cacheDir() : QDir::tempPath();
        SafeFs::mkdir0700(cacheRoot);
        const QString destIcon = cacheRoot + QLatin1Char('/') + QUuid::createUuid().toString(QUuid::WithoutBraces)
            + QFileInfo(iconSource).suffix().prepend(QLatin1Char('.'));
        QString error;
        qint64 copied = 0;
        if (SafeFs::copyBounded(iconSource, destIcon, kMaxIconBytes, nullptr, &copied, &error)) {
            result.metadata.extractedIconPath = destIcon;
        }
    }
}

} // namespace GoshAim
