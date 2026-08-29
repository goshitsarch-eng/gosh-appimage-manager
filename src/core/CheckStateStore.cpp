#include "CheckStateStore.h"

#include "Limits.h"
#include "SafeFs.h"
#include "SettingsStore.h"

#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>

namespace GoshAim {

CheckStateStore::CheckStateStore(SettingsStore *settings)
    : m_settings(settings)
{
    load();
}

bool CheckStateStore::load(QString *error)
{
    m_state.clear();
    if (!m_settings) {
        return false;
    }
    const QString path = m_settings->dataDir() + QStringLiteral("/check-state.json");
    QFile file(path);
    if (!file.exists()) {
        return true;
    }
    if (!file.open(QIODevice::ReadOnly)) {
        if (error) {
            *error = QStringLiteral("Cannot read check-state store");
        }
        return false;
    }
    const QByteArray data = file.read(kMaxJsonBodyBytes + 1);
    if (data.size() > kMaxJsonBodyBytes) {
        if (error) {
            *error = QStringLiteral("Check-state store exceeds size bound");
        }
        return false;
    }
    const QJsonObject root = QJsonDocument::fromJson(data).object();
    const QJsonObject apps = root.value(QStringLiteral("apps")).toObject();
    for (auto it = apps.begin(); it != apps.end(); ++it) {
        m_state.insert(it.key(), it.value().toObject().toVariantMap());
    }
    return true;
}

bool CheckStateStore::save(QString *error)
{
    if (!m_settings) {
        if (error) {
            *error = QStringLiteral("Settings unavailable");
        }
        return false;
    }
    QJsonObject apps;
    for (auto it = m_state.begin(); it != m_state.end(); ++it) {
        apps.insert(it.key(), QJsonObject::fromVariantMap(it.value()));
    }
    QJsonObject root;
    root.insert(QStringLiteral("schema_version"), kRegistrySchemaVersion);
    root.insert(QStringLiteral("apps"), apps);
    const QByteArray data = QJsonDocument(root).toJson(QJsonDocument::Compact);
    SafeFs::mkdir0700(m_settings->dataDir(), error);
    return SafeFs::atomicWrite(m_settings->dataDir() + QStringLiteral("/check-state.json"), data, error, 0600);
}

QVariantMap CheckStateStore::get(const QString &uuid) const
{
    return m_state.value(uuid);
}

void CheckStateStore::set(const QString &uuid, const QVariantMap &state)
{
    if (!uuid.isEmpty()) {
        m_state.insert(uuid, state);
    }
}

void CheckStateStore::clear(const QString &uuid)
{
    m_state.remove(uuid);
}

} // namespace GoshAim
