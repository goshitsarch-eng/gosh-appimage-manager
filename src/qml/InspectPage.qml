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
                required property int index
                required property string name
                required property string path
                required property string plannedTarget
                required property string copyOutcome
                required property string appImageType
                required property string architecture
                required property int size
                required property string comment
                required property bool alreadyManaged
                required property string existingManagedId
                required property string conflictStatus
                required property bool needsDecision
                required property bool canReplace
                required property string conflictingUuid
                required property string conflictingName
                required property string warnings
                required property string error
                required property string updateSource
                required property int chosenPolicy

                Layout.fillWidth: true
                contentItem: ColumnLayout {
                    Controls.Label { text: name; font.bold: true }
                    Controls.Label { text: i18n("Source: %1", path); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                    Controls.Label { text: i18n("Planned target: %1", plannedTarget); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                    Controls.Label { text: i18n("Outcome: %1", copyOutcome === "move" ? i18n("Delete the source file after success") : i18n("Copy source")) }
                    Controls.Label { text: i18n("Type %1 · %2 · %3", appImageType, architecture, Store.formatSize(size)) }
                    Controls.Label { text: comment; wrapMode: Text.WordWrap; Layout.fillWidth: true; visible: comment.length > 0 }
                    Controls.Label { text: i18n("Already managed as %1", existingManagedId); visible: alreadyManaged; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                    Controls.Label { text: i18n("Conflict: %1", conflictStatus); visible: needsDecision || (conflictStatus.length > 0 && conflictStatus !== "none") }
                    Controls.Label { text: warnings; wrapMode: Text.WordWrap; Layout.fillWidth: true; visible: warnings.length > 0; color: Kirigami.Theme.neutralTextColor }
                    Controls.Label { text: error; wrapMode: Text.WordWrap; Layout.fillWidth: true; visible: error.length > 0; color: Kirigami.Theme.negativeTextColor }
                    Controls.Label { text: i18n("Update source: %1", updateSource); visible: updateSource.length > 0; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                    Controls.ComboBox {
                        id: conflictBox
                        visible: needsDecision
                        Accessible.name: i18n("Conflict policy for %1", name)
                        currentIndex: chosenPolicy === 2 ? 1 : (chosenPolicy === 1 ? 0 : -1)
                        displayText: currentIndex < 0 ? i18n("Choose keep both or replace…") : currentText
                        property var policyChoices: canReplace
                            ? [i18n("Keep both (new filename)"), i18n("Replace owned install %1", conflictingName)]
                            : [i18n("Keep both (new filename)")]
                        model: policyChoices
                        onActivated: function(choiceIndex) {
                            if (canReplace && choiceIndex === 1) {
                                Store.setCandidateConflict(index, 2, conflictingUuid)
                            } else {
                                Store.setCandidateConflict(index, 1, "")
                            }
                        }
                    }
                }
            }
        }
        Controls.Button {
            text: Store.settings.moveSource ? i18n("Integrate (move) into %1", Store.settings.managedFolder)
                                            : i18n("Integrate copies into %1", Store.settings.managedFolder)
            Accessible.name: i18n("Confirm integration")
            enabled: !Store.inspecting && Store.conflictsResolved
            Controls.ToolTip.text: i18n("Each valid AppImage is copied or the source file is deleted after success, only after an explicit keep-both or replace choice")
            onClicked: {
                Store.confirmIntegrate(0, Store.settings.moveSource, "")
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
