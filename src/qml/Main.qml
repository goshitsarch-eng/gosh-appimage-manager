import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ApplicationWindow {
    id: root
    objectName: "mainWindow"
    title: i18n("Gosh AppImage Manager")
    minimumWidth: Kirigami.Units.gridUnit * 22
    minimumHeight: Kirigami.Units.gridUnit * 28
    width: Kirigami.Units.gridUnit * 70
    height: Kirigami.Units.gridUnit * 45

    property string currentPage: "library"

    function pageUrl(pageName) {
        const pages = {
            "library": "LibraryPage.qml",
            "updates": "UpdatesPage.qml",
            "tasks": "TasksPage.qml",
            "settings": "SettingsPage.qml"
        }
        return Qt.resolvedUrl(pages[pageName] || pages.library)
    }

    function go(pageName) {
        currentPage = pageName
        pageStack.replace(pageUrl(pageName))
    }

    DropArea {
        anchors.fill: parent
        keys: ["text/uri-list"]
        onDropped: function (drop) {
            if (drop.hasUrls) {
                const paths = []
                for (let i = 0; i < drop.urls.length; ++i) {
                    paths.push(drop.urls[i])
                }
                Store.openAppImages(paths)
            }
        }
    }

    globalDrawer: Kirigami.GlobalDrawer {
        title: i18n("Gosh AppImage Manager")
        titleIcon: "com.goshapps.AppImageManager"
        isMenu: !root.wideScreen
        modal: !root.wideScreen
        actions: [
            Kirigami.Action { text: i18n("Library"); icon.name: "view-list-icons"; checked: root.currentPage === "library"; onTriggered: root.go("library") },
            Kirigami.Action { text: i18n("Updates"); icon.name: "update-none"; checked: root.currentPage === "updates"; onTriggered: root.go("updates") },
            Kirigami.Action { text: i18n("Tasks"); icon.name: "view-task"; checked: root.currentPage === "tasks"; onTriggered: root.go("tasks") },
            Kirigami.Action { separator: true },
            Kirigami.Action { text: i18n("Settings"); icon.name: "settings-configure"; onTriggered: root.go("settings") },
            Kirigami.Action { text: i18n("About"); icon.name: "help-about-symbolic"; onTriggered: pageStack.push(Qt.resolvedUrl("AboutPage.qml")) }
        ]
    }

    footer: Kirigami.NavigationTabBar {
        visible: !root.wideScreen
        actions: [
            Kirigami.Action { text: i18n("Library"); icon.name: "view-list-icons"; onTriggered: root.go("library") },
            Kirigami.Action { text: i18n("Updates"); icon.name: "update-none"; onTriggered: root.go("updates") },
            Kirigami.Action { text: i18n("Tasks"); icon.name: "view-task"; onTriggered: root.go("tasks") },
            Kirigami.Action { text: i18n("Settings"); icon.name: "settings-configure"; onTriggered: root.go("settings") }
        ]
    }

    pageStack.initialPage: Qt.resolvedUrl("LibraryPage.qml")
    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.ToolBar

    Connections {
        target: Store
        function onToast(message) {
            root.showPassiveNotification(message)
        }
        function onConfirmInspect() {
            for (let i = 0; i < pageStack.depth; ++i) {
                const page = pageStack.get(i)
                if (page && page.objectName === "inspectPage")
                    return
            }
            pageStack.push(Qt.resolvedUrl("InspectPage.qml"))
        }
        function onConfirmUnsafeExtract(path) {
            unsafeDialog.unsafePath = path
            unsafeDialog.open()
        }
    }

    Kirigami.PromptDialog {
        id: unsafeDialog
        property string unsafePath: ""
        title: i18n("Execute untrusted AppImage code?")
        subtitle: i18n("Unsafe extraction runs %1 with --appimage-extract. This executes untrusted code. Continue only if you accept that risk.", unsafePath)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: Store.confirmUnsafeExtractFor(unsafePath, true)
    }
}
