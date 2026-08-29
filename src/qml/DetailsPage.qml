import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: Store.selectedDetails().name || i18n("Details")

    property var details: Store.selectedDetails()

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing
        Kirigami.Heading { text: details.name }
        Controls.Label { text: details.comment; wrapMode: Text.WordWrap; Layout.fillWidth: true }
        Controls.Label { text: i18n("Path: %1", details.path); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
        Controls.Label { text: i18n("Desktop ID: %1", details.desktopId) }
        Controls.Label { text: i18n("SHA-256: %1", details.hash); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
        Controls.Label { text: i18n("Type %1 · %2 · %3", details.type, details.architecture, Store.formatSize(details.size)) }
        Controls.Label { text: i18n("Update manager: %1", details.manager.length ? details.manager : i18n("none")) }
        Controls.Label { text: i18n("Embedded source: %1", details.embedded); visible: details.embedded.length > 0; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }

        RowLayout {
            Controls.Button { text: i18n("Launch"); Accessible.name: i18n("Launch"); onClicked: Store.launchApp(details.uuid) }
            Controls.Button { text: i18n("Reveal"); Accessible.name: i18n("Reveal in file manager"); onClicked: Store.revealApp(details.uuid) }
            Controls.Button { text: i18n("Check update"); onClicked: Store.checkUpdate(details.uuid) }
            Controls.Button { text: i18n("Update now"); onClicked: Store.updateApp(details.uuid, false) }
        }

        Kirigami.FormLayout {
            Controls.TextField {
                id: argsField
                Kirigami.FormData.label: i18n("Arguments")
                text: (details.arguments || []).join(" ")
                Accessible.name: i18n("Command arguments")
            }
            Controls.Button {
                text: i18n("Save arguments")
                onClicked: Store.setArguments(details.uuid, argsField.text.split(/\s+/).filter(function (item) { return item.length > 0 }))
            }
        }

        RowLayout {
            Controls.Button {
                text: i18n("Refresh metadata")
                onClicked: Store.refreshMetadata(details.uuid)
            }
            Controls.Button {
                text: i18n("Move to Trash")
                Accessible.name: i18n("Move %1 to Trash", details.name)
                Controls.ToolTip.text: i18n("Trash %1. Permanent deletion is a separate confirmation.", details.path)
                onClicked: trashDialog.open()
            }
            Controls.Button {
                text: i18n("Delete permanently")
                Accessible.name: i18n("Permanently delete %1", details.name)
                Controls.ToolTip.text: i18n("Permanently delete %1 after an extra confirmation", details.path)
                onClicked: deleteDialog.open()
            }
        }
    }

    Kirigami.PromptDialog {
        id: trashDialog
        title: i18n("Move to Trash?")
        subtitle: i18n("This moves %1 to Trash and then removes owned desktop and icon files. If Trash fails, nothing is deleted.", details.path)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: Store.removeApp(details.uuid, false)
    }
    Kirigami.PromptDialog {
        id: deleteDialog
        title: i18n("Permanently delete?")
        subtitle: i18n("This permanently deletes %1. This cannot be undone.", details.path)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: Store.removeApp(details.uuid, true)
    }
}
