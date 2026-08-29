#pragma once

#include "Limits.h"

#include <QByteArray>
#include <QDateTime>
#include <QHash>
#include <QMetaType>
#include <QString>
#include <QStringList>
#include <QUrl>
#include <QVariantMap>
#include <QVector>

namespace GoshAim {

enum class AppImageType { Unknown = 0, Type1 = 1, Type2 = 2, Dwarfs = 3 };
enum class Architecture { Unknown = 0, X86_64 = 1, AArch64 = 2, I386 = 3, Arm = 4 };
enum class ConflictPolicy { Unspecified = 0, KeepBoth = 1, Replace = 2 };
enum class RemovalMode { Trash = 0, Permanent = 1 };
enum class Appearance { System = 0, Light = 1, Dark = 2 };
enum class TaskKind { Inspect, Integrate, Update, Remove, RefreshMetadata, CheckUpdate, Adopt };
enum class TaskState { Queued, Running, Cancelling, Succeeded, Failed, Cancelled };
enum class CopyMode { Copy, Move };
enum class ExitCode {
    Ok = 0,
    Failure = 1,
    Usage = 2,
    NotFound = 3,
    NotIntegrated = 4,
    NeedsConfirmation = 5,
    Validation = 6,
    Running = 7,
    Network = 8
};

struct EnvPair {
    QString name;
    QString value;
};

struct DesktopAction {
    QString id;
    QString name;
    QStringList arguments;
};

enum class IntegrateFailPoint { None, AfterStage, DesktopWrite, DesktopInstall, RegistrySave, SourceDelete, BackupCreate, BeforeCommit };
enum class UpdateFailPoint { None, AfterDownload, AfterReplace, DesktopInstall, RegistrySave, BackupCreate };
enum class ArchiveEntryKind { File, Directory, Symlink, Device, Other };

struct ArchiveEntry {
    QString path;
    ArchiveEntryKind kind = ArchiveEntryKind::File;
    QString linkTarget;
    qint64 size = 0;
};

struct FileIdentity {
    QString path;
    qint64 size = 0;
    QByteArray sha256;
};

struct AppImageMetadata {
    QString name;
    QString version;
    QString comment;
    QString iconName;
    QString extractedIconPath;
    bool terminal = false;
    QStringList categories;
    QStringList mimeTypes;
    QString execRaw;
    QStringList execArguments;
    QString tryExec;
    QString website;
    QString startupWmClass;
    QVector<DesktopAction> actions;
};

struct EmbeddedUpdateInfo {
    QString raw;
    QString managerHint;
    QVariantMap fields;
};

struct InspectionResult {
    FileIdentity identity;
    AppImageType type = AppImageType::Unknown;
    Architecture architecture = Architecture::Unknown;
    bool magicValid = false;
    bool architectureSupported = false;
    bool truncated = false;
    AppImageMetadata metadata;
    EmbeddedUpdateInfo updateInfo;
    QStringList warnings;
    QString error;
    bool alreadyManaged = false;
    QString existingManagedId;
    bool extractionAttempted = false;
    bool extractionUsedUnsafeFallback = false;
    qint64 payloadOffset = -1;
    QString extractorUsed;
    QString plannedTarget;
    QString copyOutcome;
    QString conflictStatus;
    QString conflictingUuid;
    QString conflictingPath;
    QString conflictingName;
    bool needsConflictDecision = false;
    bool canReplace = false;
    ConflictPolicy chosenPolicy = ConflictPolicy::Unspecified;
    QString chosenReplaceUuid;
};

struct InspectOptions {
    bool extractMetadata = true;
    bool computeHash = true;
    bool allowUnsafeExtract = false;
    bool confirmUnsafeExtract = false;
    qint64 maxBytes = kDefaultMaxAppImageBytes;
};

struct InstalledApp {
    QString uuid;
    QString name;
    QString version;
    QString comment;
    QString managedPath;
    QString desktopId;
    QString desktopPath;
    QString iconPath;
    QByteArray sha256;
    AppImageType type = AppImageType::Unknown;
    Architecture architecture = Architecture::Unknown;
    qint64 size = 0;
    QStringList arguments;
    QStringList defaultArguments;
    QVector<EnvPair> environment;
    QString updateManager;
    QVariantMap updateConfig;
    QString embeddedUpdate;
    QDateTime lastUpdateCheck;
    QString availableVersion;
    QString availableUrl;
    qint64 availableSize = 0;
    bool updateAvailable = false;
    QString digest;
    bool reducedVerification = false;
    bool running = false;
    bool externalFolder = false;
    bool owned = true;
    bool adopted = false;
    QString website;
    bool terminal = false;
    QVector<DesktopAction> actions;
};

struct IntegrateRequest {
    QString sourcePath;
    ConflictPolicy conflict = ConflictPolicy::Unspecified;
    QString replaceUuid;
    CopyMode copyMode = CopyMode::Copy;
    bool assumeYes = false;
};

struct IntegrateResult {
    bool ok = false;
    bool partial = false;
    QString error;
    InstalledApp app;
    QStringList rolledBack;
    bool sourceRemoved = false;
};

struct RemovalResult {
    bool ok = false;
    bool partial = false;
    QString error;
};

struct RemovalRequest {
    QString pathOrUuid;
    RemovalMode mode = RemovalMode::Trash;
    bool assumeYes = false;
};

struct UpdateOffer {
    QString uuid;
    QString name;
    QString currentVersion;
    QString availableVersion;
    QString manager;
    QString url;
    qint64 downloadSize = 0;
    QString digest;
    bool reducedVerification = false;
    QString embeddedSource;
    bool running = false;
};

struct TaskItem {
    QString id;
    TaskKind kind = TaskKind::Inspect;
    TaskState state = TaskState::Queued;
    QString title;
    QString target;
    int progress = 0;
    QString statusText;
    QString error;
    QDateTime createdAt;
    QDateTime finishedAt;
    bool retryable = false;
};

inline QString appImageTypeName(AppImageType type)
{
    switch (type) {
    case AppImageType::Type1:
        return QStringLiteral("type-1");
    case AppImageType::Type2:
        return QStringLiteral("type-2");
    case AppImageType::Dwarfs:
        return QStringLiteral("dwarfs");
    default:
        return QStringLiteral("unknown");
    }
}

inline QString architectureName(Architecture architecture)
{
    switch (architecture) {
    case Architecture::X86_64:
        return QStringLiteral("x86_64");
    case Architecture::AArch64:
        return QStringLiteral("aarch64");
    case Architecture::I386:
        return QStringLiteral("i386");
    case Architecture::Arm:
        return QStringLiteral("arm");
    default:
        return QStringLiteral("unknown");
    }
}

inline QString appearanceName(Appearance appearance)
{
    switch (appearance) {
    case Appearance::Light:
        return QStringLiteral("light");
    case Appearance::Dark:
        return QStringLiteral("dark");
    default:
        return QStringLiteral("system");
    }
}

inline Appearance appearanceFromString(const QString &value)
{
    if (value == QLatin1String("light")) {
        return Appearance::Light;
    }
    if (value == QLatin1String("dark")) {
        return Appearance::Dark;
    }
    return Appearance::System;
}

inline QString taskKindName(TaskKind kind)
{
    switch (kind) {
    case TaskKind::Integrate:
        return QStringLiteral("integrate");
    case TaskKind::Update:
        return QStringLiteral("update");
    case TaskKind::Remove:
        return QStringLiteral("remove");
    case TaskKind::RefreshMetadata:
        return QStringLiteral("refresh");
    case TaskKind::CheckUpdate:
        return QStringLiteral("check-update");
    case TaskKind::Adopt:
        return QStringLiteral("adopt");
    default:
        return QStringLiteral("inspect");
    }
}

} // namespace GoshAim
