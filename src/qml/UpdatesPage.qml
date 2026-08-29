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
            onTriggered: Store.checkUpdate("")
        },
        Kirigami.Action {
            text: i18n("Update all")
            icon.name: "update-none"
            onTriggered: Store.updateAll(false)
            Accessible.name: i18n("Update all AppImages")
            Controls.ToolTip.text: i18n("Apply available updates one at a time. A running app is skipped unless you force it.")
        }
    ]

    StatusPage {
        visible: Store.updatesModel.count === 0
        anchors.centerIn: parent
        width: parent.width
        count: Store.updatesModel.count
        emptyText: i18n("No updates are available")
        emptyExplanation: i18n("Background checks never download or apply updates.")
        onRetry: Store.checkUpdate("")
    }

    ListView {
        visible: Store.updatesModel.count > 0
        model: Store.updatesModel
        clip: true
        delegate: Kirigami.AbstractCard {
            width: ListView.view.width
            contentItem: RowLayout {
                ColumnLayout {
                    Layout.fillWidth: true
                    Controls.Label { text: model.name; font.bold: true }
                    Controls.Label { text: i18n("%1 → %2 · %3", model.currentVersion, model.availableVersion, model.manager) }
                    Controls.Label { text: i18n("Running — update blocked"); visible: model.running }
                    Controls.Label { text: i18n("Reduced verification"); visible: model.reducedVerification }
                }
                Controls.Button {
                    text: i18n("Update")
                    Accessible.name: i18n("Update %1", model.name)
                    onClicked: Store.updateApp(model.uuid, false)
                }
            }
        }
    }
}
