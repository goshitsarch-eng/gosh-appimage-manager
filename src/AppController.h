#pragma once

#include "ThemeController.h"
#include "core/SettingsStore.h"
#include "core/Types.h"
#include "models/Models.h"

#include <QObject>
#include <QPointer>
#include <QTimer>
#include <QVariantMap>
#include <atomic>
#include <functional>
#include <memory>

namespace GoshAim {

class ProcessRunner;
class NetworkClient;
class ProcessTable;
class ManagedRegistry;
class AppImageInspector;
class DesktopIntegration;
class IntegrationService;
class RemovalService;
class LaunchService;
class UpdateService;
class AppImageLibrary;
class TaskQueue;

class AppController : public QObject
{
    Q_OBJECT
    Q_PROPERTY(LibraryModel *libraryModel READ libraryModel CONSTANT)
    Q_PROPERTY(UpdatesModel *updatesModel READ updatesModel CONSTANT)
    Q_PROPERTY(TaskModel *taskModel READ taskModel CONSTANT)
    Q_PROPERTY(CandidateModel *candidateModel READ candidateModel CONSTANT)
    Q_PROPERTY(SettingsStore *settings READ settings CONSTANT)
    Q_PROPERTY(ThemeController *theme READ theme CONSTANT)
    Q_PROPERTY(QString search READ search WRITE setSearch NOTIFY searchChanged)
    Q_PROPERTY(QString sort READ sort WRITE setSort NOTIFY searchChanged)
    Q_PROPERTY(QString selectedUuid READ selectedUuid NOTIFY selectedChanged)
    Q_PROPERTY(QString statusMessage READ statusMessage NOTIFY statusMessageChanged)
    Q_PROPERTY(bool modelsReady READ modelsReady CONSTANT)
    Q_PROPERTY(QString licenseText READ licenseText CONSTANT)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString inspectSummary READ inspectSummary NOTIFY inspectSummaryChanged)
    Q_PROPERTY(bool inspecting READ inspecting NOTIFY inspectingChanged)
    Q_PROPERTY(int inspectProgress READ inspectProgress NOTIFY inspectProgressChanged)
    Q_PROPERTY(QString updateSummary READ updateSummary NOTIFY updateSummaryChanged)

public:
    explicit AppController(QObject *parent = nullptr,
                           ProcessRunner *runner = nullptr,
                           NetworkClient *network = nullptr,
                           ProcessTable *processes = nullptr,
                           bool ownServices = true);
    ~AppController() override;

    LibraryModel *libraryModel() const { return m_libraryModel; }
    UpdatesModel *updatesModel() const { return m_updatesModel; }
    TaskModel *taskModel() const { return m_taskModel; }
    CandidateModel *candidateModel() const { return m_candidateModel; }
    SettingsStore *settings() const { return m_settings; }
    ThemeController *theme() const { return m_theme; }
    QString search() const { return m_search; }
    void setSearch(const QString &value);
    QString sort() const { return m_sort; }
    void setSort(const QString &value);
    QString selectedUuid() const { return m_selectedUuid; }
    QString statusMessage() const { return m_status; }
    bool modelsReady() const;
    QString licenseText() const;
    bool busy() const;
    QString inspectSummary() const { return m_inspectSummary; }
    bool inspecting() const { return m_inspecting; }
    int inspectProgress() const { return m_inspectProgress; }
    QString updateSummary() const { return m_updateSummary; }

    ManagedRegistry *registry() const { return m_registry; }
    AppImageInspector *inspector() const { return m_inspector; }
    IntegrationService *integration() const { return m_integration; }
    RemovalService *removal() const { return m_removal; }
    LaunchService *launchService() const { return m_launch; }
    UpdateService *updates() const { return m_updates; }
    TaskQueue *taskQueue() const { return m_tasks; }
    ProcessRunner *runner() const { return m_runner; }
    DesktopIntegration *desktop() const { return m_desktop; }

    Q_INVOKABLE void refreshLibrary();
    Q_INVOKABLE void inspectPaths(const QStringList &paths);
    Q_INVOKABLE void confirmIntegrate(int conflictPolicy, bool moveSource, const QString &replaceUuid = {});
    Q_INVOKABLE void setCandidateConflict(int row, int policy, const QString &replaceUuid);
    Q_INVOKABLE void cancelInspect();
    Q_INVOKABLE void confirmUnsafeExtractFor(const QString &path, bool allow);
    Q_INVOKABLE void launchApp(const QString &uuid);
    Q_INVOKABLE void revealApp(const QString &uuid);
    Q_INVOKABLE void removeApp(const QString &uuid, bool permanent);
    Q_INVOKABLE void checkUpdate(const QString &uuid);
    Q_INVOKABLE void checkAll();
    Q_INVOKABLE void updateApp(const QString &uuid, bool force);
    Q_INVOKABLE void updateAll(bool force);
    Q_INVOKABLE void cancelTask(const QString &id);
    Q_INVOKABLE void retryTask(const QString &id);
    Q_INVOKABLE void refreshMetadata(const QString &uuid);
    Q_INVOKABLE void setArguments(const QString &uuid, const QStringList &arguments);
    Q_INVOKABLE void setEnvironment(const QString &uuid, const QVariantMap &env);
    Q_INVOKABLE void setUpdateSource(const QString &uuid, const QString &manager, const QVariantMap &config);
    Q_INVOKABLE void unsetUpdateSource(const QString &uuid);
    Q_INVOKABLE void adoptApp(const QString &uuid);
    Q_INVOKABLE void selectApp(const QString &uuid);
    Q_INVOKABLE QVariantMap selectedDetails() const;
    Q_INVOKABLE void setAppearance(const QString &name);
    Q_INVOKABLE QString formatSize(qint64 bytes) const;
    Q_INVOKABLE QStringList updateManagers() const;
    Q_INVOKABLE void openAppImages(const QStringList &urls);
    Q_INVOKABLE QString localPathFromUrl(const QString &url) const;
    Q_INVOKABLE void setManagedFolderFromUrl(const QString &url);
    Q_INVOKABLE QStringList qmlActionNames() const;
    Q_INVOKABLE void loadSyntheticCatalog();

Q_SIGNALS:
    void searchChanged();
    void selectedChanged();
    void statusMessageChanged();
    void busyChanged();
    void inspectSummaryChanged();
    void inspectingChanged();
    void inspectProgressChanged();
    void updateSummaryChanged();
    void toast(const QString &message);
    void confirmInspect();
    void confirmRemove(const QString &uuid, const QString &path, bool permanent);
    void confirmUnsafeExtract(const QString &path);

private:
    void setStatus(const QString &message);
    void reloadTasks();
    void finishInspect(const QVector<InspectionResult> &results, int generation);
    void syncBackgroundChecks();
    void applyDebugLogging();
    void notifyUpdates(int count);
    ProcessRunner *m_runner = nullptr;
    NetworkClient *m_network = nullptr;
    ProcessTable *m_processes = nullptr;
    bool m_own = true;
    bool m_ownRunner = false;
    bool m_ownNetwork = false;
    bool m_ownProcesses = false;
    SettingsStore *m_settings = nullptr;
    ThemeController *m_theme = nullptr;
    ManagedRegistry *m_registry = nullptr;
    AppImageInspector *m_inspector = nullptr;
    DesktopIntegration *m_desktop = nullptr;
    IntegrationService *m_integration = nullptr;
    RemovalService *m_removal = nullptr;
    LaunchService *m_launch = nullptr;
    UpdateService *m_updates = nullptr;
    AppImageLibrary *m_library = nullptr;
    TaskQueue *m_tasks = nullptr;
    LibraryModel *m_libraryModel = nullptr;
    UpdatesModel *m_updatesModel = nullptr;
    TaskModel *m_taskModel = nullptr;
    CandidateModel *m_candidateModel = nullptr;
    QTimer *m_backgroundTimer = nullptr;
    QString m_search;
    QString m_sort = QStringLiteral("name");
    QString m_selectedUuid;
    QString m_status;
    QString m_inspectSummary;
    QString m_updateSummary;
    QVector<InspectionResult> m_pendingInspect;
    QString m_inspectTaskId;
    int m_inspectGeneration = 0;
    int m_inspectProgress = 0;
    bool m_inspecting = false;
    int m_updateAllRemaining = 0;
    int m_updateAllFailed = 0;
    int m_updateAllSucceeded = 0;
};

} // namespace GoshAim
