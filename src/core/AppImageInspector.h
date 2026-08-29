#pragma once

#include "Types.h"

#include <atomic>

namespace GoshAim {

class ProcessRunner;
class SettingsStore;
class ManagedRegistry;

class AppImageInspector
{
public:
    AppImageInspector(ProcessRunner *runner, SettingsStore *settings, ManagedRegistry *registry);

    InspectionResult inspect(const QString &path, const InspectOptions &options, std::atomic<bool> *cancel = nullptr);

    static EmbeddedUpdateInfo parseUpdInfo(const QByteArray &raw);

private:
    bool extractMetadata(InspectionResult &result, const InspectOptions &options, std::atomic<bool> *cancel);
    bool extractWithUnsquashfs(InspectionResult &result, const QString &dest, std::atomic<bool> *cancel);
    bool extractWith7z(InspectionResult &result, const QString &dest, std::atomic<bool> *cancel);
    bool extractWithDwarfs(InspectionResult &result, const QString &dest, std::atomic<bool> *cancel);
    bool extractUnsafe(InspectionResult &result, const QString &dest, std::atomic<bool> *cancel);
    void ingestExtracted(InspectionResult &result, const QString &dest);
    ProcessRunner *m_runner = nullptr;
    SettingsStore *m_settings = nullptr;
    ManagedRegistry *m_registry = nullptr;
};

} // namespace GoshAim
