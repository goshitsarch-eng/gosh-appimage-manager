import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: i18n("Library")

    actions: [
        Kirigami.Action {
            text: i18n("Open AppImage")
            icon.name: "document-open"
            onTriggered: fileDialog.open()
            Accessible.name: i18n("Open AppImage files")
        },
        Kirigami.Action {
            text: i18n("Refresh")
            icon.name: "view-refresh"
            onTriggered: Store.refreshLibrary()
            Accessible.name: i18n("Refresh library")
        }
    ]

    header: RowLayout {
        Kirigami.SearchField {
            id: searchField
            Layout.fillWidth: true
            placeholderText: i18n("Search integrated AppImages")
            Accessible.name: i18n("Search integrated AppImages")
            onTextChanged: Store.search = text
        }
        Binding {
            target: searchField
            property: "text"
            value: Store.search
            restoreMode: Binding.RestoreBinding
        }
        Controls.ComboBox {
            id: sortBox
            model: [i18n("Name"), i18n("Size"), i18n("Version")]
            Accessible.name: i18n("Sort library")
            onActivated: Store.sort = ["name", "size", "version"][currentIndex]
        }
        Binding {
            target: sortBox
            property: "currentIndex"
            value: Store.sort === "size" ? 1 : (Store.sort === "version" ? 2 : 0)
            restoreMode: Binding.RestoreBinding
        }
    }

    StatusPage {
        anchors.centerIn: parent
        width: parent.width
        visible: Store.libraryModel.count === 0
        count: Store.libraryModel.count
        emptyText: Store.search.length > 0 ? i18n("No AppImages match this search") : i18n("No AppImages are integrated yet")
        emptyExplanation: i18n("Open an AppImage to inspect it. Integration never happens until you confirm.")
        onRetry: Store.refreshLibrary()
    }

    ListView {
        visible: Store.libraryModel.count > 0
        model: Store.libraryModel
        clip: true
        spacing: Kirigami.Units.smallSpacing
        delegate: Kirigami.AbstractCard {
            width: ListView.view.width
            contentItem: RowLayout {
                Kirigami.Icon {
                    source: model.iconPath.length > 0 ? model.iconPath : "application-x-executable"
                    Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                    Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    Controls.Label { text: model.name; font.bold: true; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                    Controls.Label {
                        text: i18n("%1 · %2 · %3", model.version, model.architecture, Store.formatSize(model.size))
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    RowLayout {
                        Controls.Label { text: i18n("Update"); visible: model.updateAvailable; color: Kirigami.Theme.positiveTextColor }
                        Controls.Label { text: i18n("Running"); visible: model.running }
                        Controls.Label { text: i18n("External folder"); visible: model.externalFolder }
                    }
                }
                Controls.Button {
                    text: i18n("Launch")
                    enabled: model.owned
                    Accessible.name: i18n("Launch %1", model.name)
                    Controls.ToolTip.text: i18n("Start %1 without waiting for it to exit", model.name)
                    onClicked: Store.launchApp(model.uuid)
                }
                Controls.Button {
                    text: i18n("Details")
                    Accessible.name: i18n("Open details for %1", model.name)
                    onClicked: {
                        Store.selectApp(model.uuid)
                        applicationWindow().pageStack.push(Qt.resolvedUrl("DetailsPage.qml"))
                    }
                }
                Controls.Button {
                    text: i18n("Adopt")
                    visible: !model.owned
                    Accessible.name: i18n("Adopt %1", model.name)
                    Controls.ToolTip.text: i18n("Register this unmanaged AppImage without rewriting files")
                    onClicked: {
                        Store.selectApp(model.uuid)
                        adoptDialog.open()
                    }
                }
            }
        }
    }

    FileDialog {
        id: fileDialog
        title: i18n("Open AppImage")
        fileMode: FileDialog.OpenFiles
        nameFilters: [i18n("AppImage files (*.AppImage *.appimage)"), i18n("All files (*)")]
        onAccepted: Store.openAppImages(selectedFiles)
    }

    Kirigami.PromptDialog {
        id: adoptDialog
        title: i18n("Adopt unmanaged AppImage?")
        subtitle: i18n("Adoption records this discovered AppImage in the Gosh registry without rewriting or deleting the AppImage, desktop file, or icon. After adoption this application may update or remove it.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: Store.adoptApp(Store.selectedUuid)
    }
}
