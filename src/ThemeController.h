#pragma once

#include "core/Types.h"

#include <QObject>

namespace GoshAim {

class ThemeController : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool dark READ isDark NOTIFY changed)
    Q_PROPERTY(QString appearance READ appearanceName NOTIFY changed)
public:
    explicit ThemeController(QObject *parent = nullptr);
    void setAppearance(const QString &appearance);
    bool isDark() const;
    QString appearanceName() const;
    void apply();

Q_SIGNALS:
    void changed();

private:
    Appearance m_appearance = Appearance::System;
};

} // namespace GoshAim
