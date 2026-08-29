#include "ThemeController.h"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QStyleHints>

namespace GoshAim {

ThemeController::ThemeController(QObject *parent)
    : QObject(parent)
{
}

void ThemeController::setAppearance(const QString &appearance)
{
    const Appearance next = appearanceFromString(appearance);
    if (next == m_appearance) {
        return;
    }
    m_appearance = next;
    apply();
    Q_EMIT changed();
}

QString ThemeController::appearanceName() const
{
    return GoshAim::appearanceName(m_appearance);
}

bool ThemeController::isDark() const
{
    if (m_appearance == Appearance::Dark) {
        return true;
    }
    if (m_appearance == Appearance::Light) {
        return false;
    }
    if (auto *app = qobject_cast<QGuiApplication *>(QCoreApplication::instance())) {
        return app->styleHints()->colorScheme() == Qt::ColorScheme::Dark;
    }
    return false;
}

void ThemeController::apply()
{
    auto *app = qobject_cast<QGuiApplication *>(QCoreApplication::instance());
    if (!app) {
        return;
    }
    switch (m_appearance) {
    case Appearance::Light:
        app->styleHints()->setColorScheme(Qt::ColorScheme::Light);
        break;
    case Appearance::Dark:
        app->styleHints()->setColorScheme(Qt::ColorScheme::Dark);
        break;
    case Appearance::System:
        app->styleHints()->setColorScheme(Qt::ColorScheme::Unknown);
        break;
    }
}

} // namespace GoshAim
