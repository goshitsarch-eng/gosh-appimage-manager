import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    title: i18n("Status")
    property int count: 0
    property string emptyText
    property string emptyExplanation
    signal retry()

    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(parent.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 28)
        spacing: Kirigami.Units.largeSpacing
        Kirigami.Icon {
            source: "application-x-executable"
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: Kirigami.Units.iconSizes.huge
            Layout.preferredHeight: Kirigami.Units.iconSizes.huge
        }
        Kirigami.Heading {
            text: emptyText
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
        }
        Controls.Label {
            text: emptyExplanation
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
        }
        Controls.Button {
            text: i18n("Refresh")
            Layout.alignment: Qt.AlignHCenter
            Accessible.name: i18n("Refresh")
            onClicked: retry()
        }
    }
}
