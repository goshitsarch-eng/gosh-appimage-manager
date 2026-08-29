#pragma once

#include <QStringList>
#include <QtGlobal>

class QtWarnGuard
{
public:
    QtWarnGuard()
    {
        s_messages.clear();
        s_previous = qInstallMessageHandler(&QtWarnGuard::handler);
    }
    ~QtWarnGuard()
    {
        qInstallMessageHandler(s_previous);
    }
    QStringList messages() const { return s_messages; }
    bool sawLiveDestruction() const
    {
        for (const QString &msg : s_messages) {
            if (msg.contains(QLatin1String("Destroyed while")) || msg.contains(QLatin1String("destroyed while"))) {
                return true;
            }
        }
        return false;
    }

private:
    static void handler(QtMsgType, const QMessageLogContext &, const QString &msg)
    {
        s_messages.append(msg);
        if (s_previous) {
            s_previous(QtDebugMsg, QMessageLogContext(), msg);
        }
    }
    static QStringList s_messages;
    static QtMessageHandler s_previous;
};

inline QStringList QtWarnGuard::s_messages;
inline QtMessageHandler QtWarnGuard::s_previous = nullptr;
