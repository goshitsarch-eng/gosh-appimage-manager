#pragma once

#include "Types.h"

#include <QString>
#include <atomic>

namespace GoshAim {

class ProcessTable
{
public:
    virtual ~ProcessTable() = default;
    virtual QList<qint64> pidsForExecutable(const QString &canonicalPath) const = 0;
};

class ProcProcessTable : public ProcessTable
{
public:
    explicit ProcProcessTable(class ProcessRunner *runner = nullptr);
    QList<qint64> pidsForExecutable(const QString &canonicalPath) const override;

private:
    class ProcessRunner *m_runner = nullptr;
};

class FakeProcessTable : public ProcessTable
{
public:
    QList<qint64> running;
    QString matchPath;
    QList<qint64> pidsForExecutable(const QString &canonicalPath) const override
    {
        if (matchPath.isEmpty() || matchPath == canonicalPath) {
            return running;
        }
        return {};
    }
};

} // namespace GoshAim
