import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: i18n("Settings")

    Kirigami.FormLayout {
        wideMode: applicationWindow().wideScreen

        Controls.TextField {
            id: folderField
            Kirigami.FormData.label: i18n("Managed AppImages folder")
            text: Store.settings.managedFolder
            Accessible.name: i18n("Managed AppImages folder")
            onEditingFinished: Store.settings.managedFolder = text
        }
        Controls.Button {
            text: i18n("Choose folder")
            onClicked: folderDialog.open()
        }
        Controls.CheckBox {
            Kirigami.FormData.label: i18n("Move source after successful integration")
            checked: Store.settings.moveSource
            onToggled: Store.settings.moveSource = checked
        }
        Controls.CheckBox {
            Kirigami.FormData.label: i18n("Discover AppImages outside the managed folder")
            checked: Store.settings.manageOutsideFolder
            onToggled: Store.settings.manageOutsideFolder = checked
        }
        Controls.CheckBox {
            Kirigami.FormData.label: i18n("Omit .AppImage suffix for terminal apps")
            checked: Store.settings.terminalOmitSuffix
            onToggled: Store.settings.terminalOmitSuffix = checked
        }
        Controls.CheckBox {
            Kirigami.FormData.label: i18n("Background update checks")
            checked: Store.settings.backgroundUpdateChecks
            onToggled: Store.settings.backgroundUpdateChecks = checked
        }
        Controls.CheckBox {
            id: unsafeBox
            Kirigami.FormData.label: i18n("Unsafe extraction fallback")
            checked: Store.settings.unsafeExtractionFallback
            onToggled: Store.settings.unsafeExtractionFallback = checked
        }
        Controls.Label {
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            text: i18n("The unsafe fallback executes untrusted AppImage code with --appimage-extract. It stays off unless you enable it here and confirm per file. It is never used in tests or background checks.")
        }
        Controls.ComboBox {
            id: appearanceBox
            Kirigami.FormData.label: i18n("Appearance")
            model: [i18n("System"), i18n("Light"), i18n("Dark")]
            onActivated: Store.setAppearance(["system", "light", "dark"][currentIndex])
            Accessible.name: i18n("Appearance")
        }
        Binding {
            target: appearanceBox
            property: "currentIndex"
            value: Store.settings.appearance === "light" ? 1 : (Store.settings.appearance === "dark" ? 2 : 0)
            restoreMode: Binding.RestoreBinding
        }
        Controls.CheckBox {
            Kirigami.FormData.label: i18n("Debug logging")
            checked: Store.settings.debugLogging
            onToggled: Store.settings.debugLogging = checked
        }
    }

    FolderDialog {
        id: folderDialog
        title: i18n("Managed folder")
        onAccepted: Store.settings.managedFolder = selectedFolder
    }
}
