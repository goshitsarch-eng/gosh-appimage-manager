#pragma once

#include "Limits.h"

#include <QByteArray>
#include <QProcessEnvironment>
#include <QString>
#include <QStringList>
#include <atomic>
#include <functional>

namespace GoshAim {

struct ProcessRequest {
    QString program;
    QStringList arguments;
    QString workingDirectory;
    QProcessEnvironment environment;
    bool useEnvironment = false;
    int timeoutMs = kDefaultProcessTimeoutMs;
    qint64 maxStdoutBytes = kMaxProcessOutputBytes;
    qint64 maxStderrBytes = kMaxProcessOutputBytes;
    bool host = false;
};

struct ProcessResult {
    QString program;
    QStringList arguments;
    int exitCode = -1;
    QByteArray standardOutput;
    QByteArray standardError;
    bool timedOut = false;
    bool cancelled = false;
    bool truncated = false;
    bool failedToStart = false;
    bool refused = false;
    QString error;
};

class ProcessRunner
{
public:
    virtual ~ProcessRunner() = default;

    virtual ProcessResult run(const ProcessRequest &request, std::atomic<bool> *cancel = nullptr) = 0;
    virtual ProcessResult startDetached(const ProcessRequest &request) = 0;

    static bool looksLikeShell(const QString &program);
    static bool containsNul(const QString &value);
    static bool containsNul(const QStringList &values);
    static bool inFlatpak();
    static ProcessRequest hostWrap(const ProcessRequest &request);
    static ProcessResult refuse(const ProcessRequest &request, const QString &error);
};

class QtProcessRunner : public ProcessRunner
{
public:
    ProcessResult run(const ProcessRequest &request, std::atomic<bool> *cancel = nullptr) override;
    ProcessResult startDetached(const ProcessRequest &request) override;
};

} // namespace GoshAim
