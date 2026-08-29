import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: i18n("Updates")
    actions: [
        Kirigami.Action {
            text: i18n("Check all")
            icon.name: "view-refresh"
            onTriggered: Store.checkAll()
        },
        Kirigami.Action {
            text: i18n("Update all")
            icon.name: "update-none"
            onTriggered: updateAllDialog.open()
            Accessible.name: i18n("Update all AppImages")
            Controls.ToolTip.text: i18n("Apply available updates one at a time. A running app is skipped unless you force it.")
        }
    ]

    Controls.Label {
        visible: Store.updateSummary.length > 0
        text: Store.updateSummary
        wrapMode: Text.WordWrap
        width: parent.width
        padding: Kirigami.Units.smallSpacing
    }

    StatusPage {
        visible: Store.updatesModel.count === 0
        anchors.centerIn: parent
        width: parent.width
        count: Store.updatesModel.count
        emptyText: i18n("No updates are available")
        emptyExplanation: i18n("Background checks never download or apply updates.")
        onRetry: Store.checkAll()
    }

    ListView {
        visible: Store.updatesModel.count > 0
        model: Store.updatesModel
        clip: true
        delegate: Kirigami.AbstractCard {
            width: ListView.view.width
            contentItem: ColumnLayout {
                RowLayout {
                    ColumnLayout {
                        Layout.fillWidth: true
                        Controls.Label { text: model.name; font.bold: true }
                        Controls.Label { text: i18n("%1 → %2 · %3", model.currentVersion, model.availableVersion, model.manager) }
                        Controls.Label { text: i18n("Running — update blocked"); visible: model.running }
                        Controls.Label { text: i18n("Reduced verification"); visible: model.reducedVerification }
                        Controls.Label {
                            visible: Store.updateTaskId(model.uuid).length > 0
                            text: Store.taskTick >= 0 ? Store.updateStatus(model.uuid) : ""
                        }
                        Controls.ProgressBar {
                            visible: Store.updateTaskId(model.uuid).length > 0
                            from: 0
                            to: 100
                            value: Store.taskTick >= 0 ? Store.updateProgress(model.uuid) : 0
                            Layout.fillWidth: true
                        }
                    }
                    Controls.Button {
                        text: i18n("Update")
                        Accessible.name: i18n("Update %1", model.name)
                        onClicked: Store.updateApp(model.uuid, false)
                    }
                    Controls.Button {
                        text: i18n("Cancel")
                        Accessible.name: i18n("Cancel update for %1", model.name)
                        enabled: Store.updateTaskId(model.uuid).length > 0
                        onClicked: Store.cancelTask(Store.updateTaskId(model.uuid))
                    }
                }
            }
        }
    }

    Kirigami.PromptDialog {
        id: updateAllDialog
        title: i18n("Update all AppImages?")
        subtitle: i18n("Each owned AppImage with an available update is applied one at a time. Running apps are skipped unless you force them later.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: Store.updateAll(false)
    }
}
