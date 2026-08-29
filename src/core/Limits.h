#pragma once

#include <QtGlobal>

namespace GoshAim {

inline constexpr qint64 kDefaultMaxAppImageBytes = qint64(8) * 1024 * 1024 * 1024;
inline constexpr qint64 kMinMaxAppImageBytes = qint64(1024) * 1024;
inline constexpr qint64 kAbsoluteMaxAppImageBytes = qint64(32) * 1024 * 1024 * 1024;
inline constexpr qint64 kMaxDesktopFileBytes = 64 * 1024;
inline constexpr qint64 kMaxExtractedBytes = 8 * 1024 * 1024;
inline constexpr qint64 kMaxIconBytes = 2 * 1024 * 1024;
inline constexpr qint64 kMaxProcessOutputBytes = 1024 * 1024;
inline constexpr qint64 kMaxJsonBodyBytes = 2 * 1024 * 1024;
inline constexpr qint64 kMaxZsyncBytes = 256 * 1024;
inline constexpr int kMaxArchiveEntries = 64;
inline constexpr int kMaxExtractedFiles = 12;
inline constexpr int kMaxPathLength = 255;
inline constexpr int kMaxArchiveDepth = 3;
inline constexpr int kMaxRedirects = 3;
inline constexpr int kDefaultProcessTimeoutMs = 30000;
inline constexpr int kExtractTimeoutMs = 30000;
inline constexpr int kNetworkTimeoutMs = 30000;
inline constexpr int kHashCancelCheckBytes = 1024 * 1024;
inline constexpr int kMaxEnvPairs = 32;
inline constexpr int kMaxArguments = 64;
inline constexpr int kMaxArgumentLength = 4096;
inline constexpr int kMaxNameLength = 256;
inline constexpr int kSymlinkHopLimit = 8;
inline constexpr int kRegistrySchemaVersion = 1;
inline constexpr int kJsonSchemaVersion = 1;
inline constexpr const char *kAppId = "com.goshapps.AppImageManager";
inline constexpr const char *kExecutableName = "gosh-appimage-manager";
inline constexpr const char *kOwnershipKey = "X-Gosh-AppImage-Manager";
inline constexpr const char *kOwnershipUuidKey = "X-Gosh-AppImage-Id";
inline constexpr const char *kOwnershipPathKey = "X-Gosh-Managed-Path";
inline constexpr const char *kDesktopPrefix = "gosh-appimage-";

} // namespace GoshAim
