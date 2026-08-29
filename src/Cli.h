#pragma once

#include <QStringList>

class QCoreApplication;
class QApplication;

namespace GoshAim {

class AppController;

int runCli(AppController &controller, const QStringList &arguments, bool interactiveTty);
int runSelfTest(QApplication &app, AppController &controller);
int runHostProbe(AppController &controller);
int runInspectProbe(AppController &controller, const QString &path);

} // namespace GoshAim
