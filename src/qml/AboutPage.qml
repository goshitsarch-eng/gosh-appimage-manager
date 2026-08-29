import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: i18n("About")

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing
        Kirigami.Icon { source: "com.goshapps.AppImageManager"; Layout.preferredWidth: Kirigami.Units.iconSizes.huge; Layout.preferredHeight: Kirigami.Units.iconSizes.huge }
        Kirigami.Heading { text: i18n("Gosh AppImage Manager") }
        Controls.Label { text: i18n("Version 0.1.0") }
        Controls.Label {
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            text: i18n("A native KDE application for inspecting, integrating, launching, updating, and removing AppImages. Written in C++20, Qt 6, KDE Frameworks 6, and Kirigami by Gosh-Its-Arch.")
        }
        Controls.Label {
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            text: i18n("Licence: GNU General Public License version 3 or later. There is no warranty.")
        }
        Controls.Label {
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            text: i18n("Behavioural reference: Gear Lever by Lorenzo Paderi (https://github.com/mijorus/gearlever) at commit a2917f2adafc78e0478e47d5843de9ede6c1aa3f. This is an independent original implementation and is not endorsed by Gear Lever's authors. Gear Lever source, icons, screenshots, and branding were not copied.")
        }
        Controls.Button {
            text: i18n("View licence text")
            Accessible.name: i18n("View GNU General Public License text")
            Controls.ToolTip.text: i18n("Show the GPL-3.0-or-later licence")
            onClicked: licenseSheet.open()
        }
    }

    Kirigami.OverlaySheet {
        id: licenseSheet
        header: Kirigami.Heading { text: i18n("GNU General Public License") }
        Controls.TextArea {
            readOnly: true
            wrapMode: TextEdit.Wrap
            text: Store.licenseText
            Accessible.name: i18n("Licence text")
        }
    }
}
