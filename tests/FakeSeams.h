#pragma once

#include "core/NetworkClient.h"
#include "core/ProcessRunner.h"

#include <QFile>
#include <QThread>
#include <QVector>
#include <atomic>

namespace GoshAim {

class FakeProcessRunner : public ProcessRunner
{
public:
    struct Rule {
        QStringList contains;
        ProcessResult result;
        bool hangUntilCancel = false;
    };

    QVector<QPair<QString, QStringList>> calls;
    QVector<QPair<QString, QStringList>> detachedCalls;
    QVector<Rule> rules;
    ProcessResult defaultResult;

    FakeProcessRunner()
    {
        defaultResult.exitCode = 0;
    }

    ProcessResult run(const ProcessRequest &request, std::atomic<bool> *cancel = nullptr) override
    {
        ProcessRequest resolved = hostWrap(request);
        if (looksLikeShell(resolved.program) || resolved.program.isEmpty()) {
            return refuse(resolved, QStringLiteral("Refusing to invoke a shell or empty program"));
        }
        calls.push_back({resolved.program, resolved.arguments});
        for (const Rule &rule : rules) {
            if (!matches(resolved.program, resolved.arguments, rule.contains)) {
                continue;
            }
            if (rule.hangUntilCancel) {
                while (cancel && !cancel->load()) {
                    QThread::msleep(10);
                }
                ProcessResult cancelled = rule.result;
                cancelled.cancelled = true;
                cancelled.program = resolved.program;
                cancelled.arguments = resolved.arguments;
                return cancelled;
            }
            ProcessResult copy = rule.result;
            copy.program = resolved.program;
            copy.arguments = resolved.arguments;
            return copy;
        }
        ProcessResult copy = defaultResult;
        copy.program = resolved.program;
        copy.arguments = resolved.arguments;
        return copy;
    }

    ProcessResult startDetached(const ProcessRequest &request) override
    {
        ProcessRequest resolved = hostWrap(request);
        if (looksLikeShell(resolved.program) || resolved.program.isEmpty()) {
            return refuse(resolved, QStringLiteral("Refusing to invoke a shell or empty program"));
        }
        detachedCalls.push_back({resolved.program, resolved.arguments});
        for (const Rule &rule : rules) {
            if (!matches(resolved.program, resolved.arguments, rule.contains)) {
                continue;
            }
            ProcessResult copy = rule.result;
            copy.program = resolved.program;
            copy.arguments = resolved.arguments;
            return copy;
        }
        ProcessResult copy = defaultResult;
        copy.program = resolved.program;
        copy.arguments = resolved.arguments;
        copy.exitCode = 0;
        return copy;
    }

    bool sawProgram(const QString &program) const
    {
        for (const auto &call : calls) {
            if (call.first == program || call.second.contains(program)) {
                return true;
            }
        }
        for (const auto &call : detachedCalls) {
            if (call.first == program || call.second.contains(program)) {
                return true;
            }
        }
        return false;
    }

private:
    static bool matches(const QString &program, const QStringList &arguments, const QStringList &needles)
    {
        QStringList haystack = arguments;
        haystack.prepend(program);
        for (const QString &needle : needles) {
            if (!haystack.contains(needle)) {
                return false;
            }
        }
        return true;
    }
};

class FakeNetworkClient : public NetworkClient
{
public:
    struct Rule {
        QString hostContains;
        QString pathContains;
        NetworkResult result;
        bool hangUntilCancel = false;
    };
    QVector<QUrl> urls;
    QVector<NetworkRequest> requests;
    QVector<Rule> rules;
    NetworkResult defaultResult;

    NetworkResult fetch(const NetworkRequest &request, std::atomic<bool> *cancel = nullptr) override
    {
        urls.append(request.url);
        requests.append(request);
        const UrlCheck check = UrlGuard::validate(request.url, request.allowHttp, request.allowPrivate, request.allowFtp);
        if (!check.ok) {
            NetworkResult result;
            result.error = check.error;
            return result;
        }
        for (const Rule &rule : rules) {
            const QString host = request.url.host();
            const QString path = request.url.path();
            if (!rule.hostContains.isEmpty() && !host.contains(rule.hostContains)) {
                continue;
            }
            if (!rule.pathContains.isEmpty() && !path.contains(rule.pathContains) && !request.url.toString().contains(rule.pathContains)) {
                continue;
            }
            if (rule.hangUntilCancel) {
                while (cancel && !cancel->load()) {
                    QThread::msleep(10);
                }
                NetworkResult cancelled = rule.result;
                cancelled.cancelled = true;
                return cancelled;
            }
            NetworkResult copy = rule.result;
            if (!request.destinationPath.isEmpty() && copy.ok) {
                QFile file(request.destinationPath);
                if (file.open(QIODevice::WriteOnly)) {
                    file.write(copy.body);
                    file.close();
                    copy.savedPath = request.destinationPath;
                }
            }
            return copy;
        }
        return defaultResult;
    }
};

} // namespace GoshAim
