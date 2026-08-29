import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: i18n("Inspect AppImage")

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing
        Controls.Label {
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            text: i18n("Nothing is integrated until you confirm. Opening or dropping a file never executes it.")
        }
        Repeater {
            model: Store.candidateModel
            delegate: Kirigami.AbstractCard {
                Layout.fillWidth: true
                contentItem: ColumnLayout {
                    Controls.Label { text: model.name; font.bold: true }
                    Controls.Label { text: model.path; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                    Controls.Label { text: i18n("Type %1 · %2 · %3", model.appImageType, model.architecture, Store.formatSize(model.size)) }
                    Controls.Label { text: model.comment; wrapMode: Text.WordWrap; Layout.fillWidth: true; visible: model.comment.length > 0 }
                    Controls.Label { text: i18n("Already managed"); visible: model.alreadyManaged }
                    Controls.Label { text: model.warnings; wrapMode: Text.WordWrap; Layout.fillWidth: true; visible: model.warnings.length > 0; color: Kirigami.Theme.neutralTextColor }
                    Controls.Label { text: model.error; wrapMode: Text.WordWrap; Layout.fillWidth: true; visible: model.error.length > 0; color: Kirigami.Theme.negativeTextColor }
                    Controls.Label { text: i18n("Update source: %1", model.updateSource); visible: model.updateSource.length > 0; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                }
            }
        }
        Controls.Button {
            text: i18n("Integrate copies into %1", Store.settings.managedFolder)
            Accessible.name: i18n("Confirm integration")
            Controls.ToolTip.text: i18n("Copy each valid AppImage into the managed folder and create owned desktop entries")
            onClicked: {
                Store.confirmIntegrate(0, Store.settings.moveSource)
                applicationWindow().pageStack.pop()
            }
        }
        Controls.Button {
            text: i18n("Cancel")
            onClicked: {
                Store.cancelInspect()
                applicationWindow().pageStack.pop()
            }
        }
    }
}
