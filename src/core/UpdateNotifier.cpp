#include "UpdateNotifier.h"

#include <QCoreApplication>

#if __has_include(<KNotification>)
#include <KNotification>
#define GOSHAIM_HAVE_KNOTIFICATIONS
#endif

namespace GoshAim {

void NullUpdateNotifier::notifyUpdatesAvailable(int)
{
}

void KdeUpdateNotifier::notifyUpdatesAvailable(int count)
{
    if (count <= 0) {
        return;
    }
#ifdef GOSHAIM_HAVE_KNOTIFICATIONS
    const QString text = QCoreApplication::translate("GoshAim", "%1 AppImage update(s) available").arg(count);
    auto *notification = new KNotification(QStringLiteral("updatesAvailable"), KNotification::CloseOnTimeout);
    notification->setComponentName(QStringLiteral("gosh-appimage-manager"));
    notification->setTitle(QCoreApplication::translate("GoshAim", "Gosh AppImage Manager"));
    notification->setText(text);
    notification->sendEvent();
#else
    Q_UNUSED(count);
#endif
}

} // namespace GoshAim
