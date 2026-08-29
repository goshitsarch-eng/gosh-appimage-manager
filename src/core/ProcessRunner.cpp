#include "ProcessRunner.h"

#include <QElapsedTimer>
#include <QFile>
#include <QProcess>
#include <QThread>

namespace GoshAim {

bool ProcessRunner::looksLikeShell(const QString &program)
{
    const QString base = program.section(QLatin1Char('/'), -1).toLower();
    return base == QLatin1String("sh") || base == QLatin1String("bash") || base == QLatin1String("zsh")
        || base == QLatin1String("dash") || base == QLatin1String("fish") || base == QLatin1String("csh")
        || base == QLatin1String("ksh") || base == QLatin1String("cmd.exe") || base == QLatin1String("powershell");
}

bool ProcessRunner::containsNul(const QString &value)
{
    return value.contains(QChar(0));
}

bool ProcessRunner::containsNul(const QStringList &values)
{
    for (const QString &value : values) {
        if (containsNul(value)) {
            return true;
        }
    }
    return false;
}

bool ProcessRunner::inFlatpak()
{
    return QFile::exists(QStringLiteral("/.flatpak-info"));
}

ProcessRequest ProcessRunner::hostWrap(const ProcessRequest &request)
{
    if (!request.host || !inFlatpak()) {
        return request;
    }
    ProcessRequest wrapped = request;
    wrapped.program = QStringLiteral("flatpak-spawn");
    QStringList arguments;
    arguments << QStringLiteral("--host") << QStringLiteral("--") << request.program;
    arguments.append(request.arguments);
    wrapped.arguments = arguments;
    wrapped.host = false;
    return wrapped;
}

ProcessResult ProcessRunner::refuse(const ProcessRequest &request, const QString &error)
{
    ProcessResult result;
    result.program = request.program;
    result.arguments = request.arguments;
    result.exitCode = -2;
    result.refused = true;
    result.error = error;
    result.standardError = error.toUtf8();
    return result;
}

namespace {

ProcessResult validate(const ProcessRequest &request)
{
    if (request.program.isEmpty() || ProcessRunner::looksLikeShell(request.program)) {
        return ProcessRunner::refuse(request, QStringLiteral("Refusing to invoke a shell or empty program"));
    }
    if (ProcessRunner::containsNul(request.program) || ProcessRunner::containsNul(request.arguments)
        || ProcessRunner::containsNul(request.workingDirectory)) {
        return ProcessRunner::refuse(request, QStringLiteral("NUL byte in process request"));
    }
    return {};
}

void stopProcess(QProcess *process)
{
    if (!process || process->state() == QProcess::NotRunning) {
        return;
    }
    process->terminate();
    if (!process->waitForFinished(1500)) {
        process->kill();
        process->waitForFinished(1500);
    }
}

} // namespace

ProcessResult QtProcessRunner::run(const ProcessRequest &request, std::atomic<bool> *cancel)
{
    const ProcessResult invalid = validate(request);
    if (invalid.refused) {
        return invalid;
    }
    const ProcessRequest resolved = hostWrap(request);
    ProcessResult result;
    result.program = resolved.program;
    result.arguments = resolved.arguments;

    if (cancel && cancel->load()) {
        result.cancelled = true;
        result.error = QStringLiteral("Cancelled");
        return result;
    }

    QProcess process;
    process.setProgram(resolved.program);
    process.setArguments(resolved.arguments);
    if (!resolved.workingDirectory.isEmpty()) {
        process.setWorkingDirectory(resolved.workingDirectory);
    }
    if (resolved.useEnvironment) {
        process.setProcessEnvironment(resolved.environment);
    }
    process.setProcessChannelMode(QProcess::SeparateChannels);
    process.start();
    if (!process.waitForStarted(5000)) {
        result.failedToStart = true;
        result.error = process.errorString();
        result.standardError = result.error.toUtf8();
        return result;
    }

    QElapsedTimer timer;
    timer.start();
    while (process.state() != QProcess::NotRunning) {
        if (cancel && cancel->load()) {
            stopProcess(&process);
            result.cancelled = true;
            result.error = QStringLiteral("Cancelled");
            break;
        }
        if (resolved.timeoutMs > 0 && timer.elapsed() > resolved.timeoutMs) {
            stopProcess(&process);
            result.timedOut = true;
            result.error = QStringLiteral("Process timed out");
            break;
        }
        process.waitForFinished(50);
        const QByteArray out = process.readAllStandardOutput();
        const QByteArray err = process.readAllStandardError();
        result.standardOutput += out;
        result.standardError += err;
        if (result.standardOutput.size() > resolved.maxStdoutBytes
            || result.standardError.size() > resolved.maxStderrBytes) {
            result.truncated = true;
            result.error = QStringLiteral("Process output exceeded bound");
            stopProcess(&process);
            break;
        }
    }
    result.standardOutput += process.readAllStandardOutput();
    result.standardError += process.readAllStandardError();
    if (result.standardOutput.size() > resolved.maxStdoutBytes) {
        result.standardOutput = result.standardOutput.left(static_cast<int>(resolved.maxStdoutBytes));
        result.truncated = true;
    }
    if (result.standardError.size() > resolved.maxStderrBytes) {
        result.standardError = result.standardError.left(static_cast<int>(resolved.maxStderrBytes));
        result.truncated = true;
    }
    if (process.state() != QProcess::NotRunning) {
        stopProcess(&process);
    }
    if (!result.cancelled && !result.timedOut && !result.truncated) {
        result.exitCode = process.exitStatus() == QProcess::NormalExit ? process.exitCode() : -1;
    }
    if (process.exitStatus() != QProcess::NormalExit && result.error.isEmpty()) {
        result.error = process.errorString();
    }
    return result;
}

ProcessResult QtProcessRunner::startDetached(const ProcessRequest &request)
{
    const ProcessResult invalid = validate(request);
    if (invalid.refused) {
        return invalid;
    }
    const ProcessRequest resolved = hostWrap(request);
    ProcessResult result;
    result.program = resolved.program;
    result.arguments = resolved.arguments;

    QProcess process;
    process.setProgram(resolved.program);
    process.setArguments(resolved.arguments);
    if (!resolved.workingDirectory.isEmpty()) {
        process.setWorkingDirectory(resolved.workingDirectory);
    }
    if (resolved.useEnvironment) {
        process.setProcessEnvironment(resolved.environment);
    }
    qint64 pid = 0;
    if (!process.startDetached(&pid)) {
        result.failedToStart = true;
        result.exitCode = -1;
        result.error = process.errorString().isEmpty() ? QStringLiteral("Failed to start process") : process.errorString();
        result.standardError = result.error.toUtf8();
        return result;
    }
    result.exitCode = 0;
    return result;
}

} // namespace GoshAim
