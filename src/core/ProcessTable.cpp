#include "ProcessTable.h"

#include "ProcessRunner.h"

#include <QDir>
#include <QFile>

#include <unistd.h>

namespace GoshAim {

ProcProcessTable::ProcProcessTable(ProcessRunner *runner)
    : m_runner(runner)
{
}

QList<qint64> ProcProcessTable::pidsForExecutable(const QString &canonicalPath) const
{
    QList<qint64> pids;
    if (canonicalPath.isEmpty()) {
        return pids;
    }
    if (ProcessRunner::inFlatpak() && m_runner) {
        ProcessRequest req;
        req.program = QStringLiteral("find");
        req.arguments = QStringList{QStringLiteral("/proc"),
                                    QStringLiteral("-maxdepth"),
                                    QStringLiteral("1"),
                                    QStringLiteral("-mindepth"),
                                    QStringLiteral("1"),
                                    QStringLiteral("-type"),
                                    QStringLiteral("d")};
        req.host = true;
        req.timeoutMs = 5000;
        const ProcessResult listed = m_runner->run(req);
        for (const QByteArray &line : listed.standardOutput.split('\n')) {
            const QString dir = QString::fromUtf8(line).trimmed();
            bool ok = false;
            const qint64 pid = dir.section(QLatin1Char('/'), -1).toLongLong(&ok);
            if (!ok || pid <= 0) {
                continue;
            }
            ProcessRequest link;
            link.program = QStringLiteral("readlink");
            link.arguments = QStringList{dir + QStringLiteral("/exe")};
            link.host = true;
            link.timeoutMs = 2000;
            const ProcessResult target = m_runner->run(link);
            const QString exe = QString::fromUtf8(target.standardOutput).trimmed();
            if (exe == canonicalPath) {
                pids.append(pid);
            }
        }
        return pids;
    }
    QDir proc(QStringLiteral("/proc"));
    const QFileInfoList entries = proc.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot);
    for (const QFileInfo &entry : entries) {
        bool ok = false;
        const qint64 pid = entry.fileName().toLongLong(&ok);
        if (!ok || pid <= 0 || pid == ::getpid()) {
            continue;
        }
        const QString exe = QFile::symLinkTarget(entry.absoluteFilePath() + QStringLiteral("/exe"));
        if (exe == canonicalPath) {
            pids.append(pid);
        }
    }
    return pids;
}

} // namespace GoshAim
