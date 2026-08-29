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
            Controls.Button {
                text: i18n("Update now")
                onClicked: details.running ? forceDialog.open() : Store.updateApp(details.uuid, false)
            }
        }

        Controls.CheckBox {
            id: forceBox
            visible: details.running
            text: i18n("Force update while running")
            Accessible.name: i18n("Force update while running")
        }

        Kirigami.FormLayout {
            Repeater {
                model: details.arguments || []
                delegate: RowLayout {
                    Controls.TextField {
                        id: argField
                        text: modelData
                        Accessible.name: i18n("Argument %1", index + 1)
                        Layout.fillWidth: true
                    }
                    Controls.Button {
                        text: i18n("Remove")
                        onClicked: {
                            const next = (details.arguments || []).slice()
                            next.splice(index, 1)
                            Store.setArguments(details.uuid, next)
                            details = Store.selectedDetails()
                        }
                    }
                }
            }
            RowLayout {
                Controls.TextField {
                    id: newArg
                    placeholderText: i18n("New argument")
                    Accessible.name: i18n("New argument")
                    Layout.fillWidth: true
                }
                Controls.Button {
                    text: i18n("Add argument")
                    onClicked: {
                        if (newArg.text.length === 0)
                            return
                        const next = (details.arguments || []).slice()
                        next.push(newArg.text)
                        Store.setArguments(details.uuid, next)
                        newArg.text = ""
                        details = Store.selectedDetails()
                    }
                }
            }

            Repeater {
                model: Object.keys(details.environment || {})
                delegate: RowLayout {
                    Controls.TextField {
                        text: modelData
                        readOnly: true
                        Accessible.name: i18n("Environment name")
                    }
                    Controls.TextField {
                        id: envValue
                        text: details.environment[modelData]
                        Accessible.name: i18n("Environment value for %1", modelData)
                        Layout.fillWidth: true
                    }
                    Controls.Button {
                        text: i18n("Save")
                        onClicked: {
                            const env = Object.assign({}, details.environment)
                            env[modelData] = envValue.text
                            Store.setEnvironment(details.uuid, env)
                            details = Store.selectedDetails()
                        }
                    }
                    Controls.Button {
                        text: i18n("Remove")
                        onClicked: {
                            const env = Object.assign({}, details.environment)
                            delete env[modelData]
                            Store.setEnvironment(details.uuid, env)
                            details = Store.selectedDetails()
                        }
                    }
                }
            }
            RowLayout {
                Controls.TextField { id: envName; placeholderText: i18n("NAME"); Accessible.name: i18n("New environment name") }
                Controls.TextField { id: envVal; placeholderText: i18n("value"); Accessible.name: i18n("New environment value"); Layout.fillWidth: true }
                Controls.Button {
                    text: i18n("Add environment")
                    onClicked: {
                        if (envName.text.length === 0)
                            return
                        const env = Object.assign({}, details.environment)
                        env[envName.text] = envVal.text
                        Store.setEnvironment(details.uuid, env)
                        envName.text = ""
                        envVal.text = ""
                        details = Store.selectedDetails()
                    }
                }
            }

            Controls.ComboBox {
                id: managerBox
                Kirigami.FormData.label: i18n("Update manager")
                model: Store.updateManagers()
                Accessible.name: i18n("Update manager")
            }
            Controls.TextField {
                id: sourceUrl
                Kirigami.FormData.label: i18n("URL / project")
                text: (details.updateConfig && (details.updateConfig.url || details.updateConfig.project)) || ""
                Accessible.name: i18n("Update source configuration")
            }
            Controls.Label {
                visible: managerBox.currentText === "ftp"
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
                text: i18n("FTP is a legacy insecure transport. Credentials are rejected.")
                color: Kirigami.Theme.neutralTextColor
            }
            RowLayout {
                Controls.Button {
                    text: i18n("Save update source")
                    onClicked: {
                        const cfg = {}
                        if (managerBox.currentText === "static" || managerBox.currentText === "ftp")
                            cfg.url = sourceUrl.text
                        else if (managerBox.currentText === "gitlab")
                            cfg.project = sourceUrl.text
                        else {
                            const parts = sourceUrl.text.split("/")
                            cfg.username = parts[0] || ""
                            cfg.repo = parts[1] || ""
                        }
                        Store.setUpdateSource(details.uuid, managerBox.currentText, cfg)
                        details = Store.selectedDetails()
                    }
                }
                Controls.Button {
                    text: i18n("Reset update source")
                    onClicked: {
                        Store.unsetUpdateSource(details.uuid)
                        details = Store.selectedDetails()
                    }
                }
            }
        }

        RowLayout {
            Controls.Button {
                text: i18n("Refresh metadata")
                onClicked: Store.refreshMetadata(details.uuid)
            }
            Controls.Button {
                visible: !details.owned
                text: i18n("Adopt")
                Accessible.name: i18n("Adopt unmanaged AppImage")
                Controls.ToolTip.text: i18n("Register this discovered AppImage as owned without rewriting or deleting files. Ownership means Gosh AppImage Manager may later update or remove it.")
                onClicked: adoptDialog.open()
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
    Kirigami.PromptDialog {
        id: forceDialog
        title: i18n("Force update while running?")
        subtitle: i18n("%1 is running. Forcing an update can crash it.", details.path)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: Store.updateApp(details.uuid, true)
    }
    Kirigami.PromptDialog {
        id: adoptDialog
        title: i18n("Adopt unmanaged AppImage?")
        subtitle: i18n("Adoption records %1 in the Gosh registry without rewriting or deleting the AppImage, desktop file, or icon. After adoption this application may update or remove it.", details.path)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: Store.adoptApp(details.uuid)
    }
}
