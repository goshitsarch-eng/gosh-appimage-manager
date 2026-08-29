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
            text: Store.inspecting ? i18n("Inspecting… %1%", Store.inspectProgress)
                                   : i18n("Nothing is integrated until you confirm. Opening or dropping a file never executes it.")
        }
        Controls.ProgressBar {
            visible: Store.inspecting
            from: 0
            to: 100
            value: Store.inspectProgress
            Layout.fillWidth: true
        }
        Repeater {
            model: Store.candidateModel
            delegate: Kirigami.AbstractCard {
                Layout.fillWidth: true
                contentItem: ColumnLayout {
                    Controls.Label { text: model.name; font.bold: true }
                    Controls.Label { text: i18n("Source: %1", model.path); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                    Controls.Label { text: i18n("Planned target: %1", model.plannedTarget); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                    Controls.Label { text: i18n("Outcome: %1", model.copyOutcome === "move" ? i18n("Move source after success") : i18n("Copy source")) }
                    Controls.Label { text: i18n("Type %1 · %2 · %3", model.appImageType, model.architecture, Store.formatSize(model.size)) }
                    Controls.Label { text: model.comment; wrapMode: Text.WordWrap; Layout.fillWidth: true; visible: model.comment.length > 0 }
                    Controls.Label { text: i18n("Already managed as %1", model.existingManagedId); visible: model.alreadyManaged; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                    Controls.Label { text: i18n("Conflict: %1", model.conflictStatus); visible: model.needsDecision || model.conflictStatus.length > 0 && model.conflictStatus !== "none" }
                    Controls.Label { text: model.warnings; wrapMode: Text.WordWrap; Layout.fillWidth: true; visible: model.warnings.length > 0; color: Kirigami.Theme.neutralTextColor }
                    Controls.Label { text: model.error; wrapMode: Text.WordWrap; Layout.fillWidth: true; visible: model.error.length > 0; color: Kirigami.Theme.negativeTextColor }
                    Controls.Label { text: i18n("Update source: %1", model.updateSource); visible: model.updateSource.length > 0; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                    Controls.ComboBox {
                        visible: model.needsDecision
                        Accessible.name: i18n("Conflict policy for %1", model.name)
                        model: [i18n("Keep both (new filename)"), i18n("Replace owned install %1", conflictingName)]
                        onActivated: Store.setCandidateConflict(index, currentIndex === 1 ? 2 : 1, conflictingUuid)
                    }
                }
            }
        }
        Controls.Button {
            text: Store.settings.moveSource ? i18n("Integrate (move) into %1", Store.settings.managedFolder)
                                            : i18n("Integrate copies into %1", Store.settings.managedFolder)
            Accessible.name: i18n("Confirm integration")
            enabled: !Store.inspecting && Store.candidateModel.count > 0
            Controls.ToolTip.text: i18n("Each valid AppImage is copied or moved into the managed folder after an explicit keep-both or replace choice")
            onClicked: {
                Store.confirmIntegrate(1, Store.settings.moveSource, "")
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
