#pragma once

#include "Types.h"

#include <QHash>
#include <QVariantMap>
#include <QVector>

namespace GoshAim {

class SettingsStore;

class CheckStateStore
{
public:
    explicit CheckStateStore(SettingsStore *settings);

    bool load(QString *error = nullptr);
    bool save(QString *error = nullptr);
    QVariantMap get(const QString &uuid) const;
    void set(const QString &uuid, const QVariantMap &state);
    void clear(const QString &uuid);

private:
    SettingsStore *m_settings = nullptr;
    QHash<QString, QVariantMap> m_state;
};

} // namespace GoshAim
