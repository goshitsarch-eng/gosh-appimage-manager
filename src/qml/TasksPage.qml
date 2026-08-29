import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: i18n("Tasks")

    StatusPage {
        visible: Store.taskModel.count === 0
        anchors.centerIn: parent
        width: parent.width
        count: Store.taskModel.count
        emptyText: i18n("No recent tasks")
        emptyExplanation: i18n("Integration, updates, and removals appear here with progress and errors.")
        onRetry: Store.refreshLibrary()
    }

    ListView {
        visible: Store.taskModel.count > 0
        model: Store.taskModel
        clip: true
        delegate: Kirigami.AbstractCard {
            width: ListView.view.width
            contentItem: ColumnLayout {
                Controls.Label { text: model.title; font.bold: true }
                Controls.Label { text: i18n("%1 · %2", model.kind, model.state) }
                Controls.ProgressBar { value: model.progress / 100.0; from: 0; to: 1; visible: model.state === "running"; Layout.fillWidth: true }
                Controls.Label { text: model.error; visible: model.error.length > 0; wrapMode: Text.WordWrap; Layout.fillWidth: true; color: Kirigami.Theme.negativeTextColor }
                Controls.Button {
                    text: i18n("Cancel")
                    visible: model.state === "queued" || model.state === "running"
                    Accessible.name: i18n("Cancel task %1", model.title)
                    onClicked: Store.cancelTask(model.id)
                }
            }
        }
    }
}
