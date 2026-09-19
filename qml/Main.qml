import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtCore
import io.github.applicationprioritysetter

ApplicationWindow {
    id: root
    width: 920
    height: 650
    minimumWidth: 720
    minimumHeight: 480
    visible: true
    title: qsTr("Application Priority Setter")
    color: palette.window

    property var applications: []
    property string selectedKey: ""
    property int selectedNice: 0
    property bool refreshIndicatorVisible: false
    // Set by the tray's Quit action so onClosing can tell a real quit apart
    // from a window close that should minimize to the tray instead.
    property bool allowQuit: false

    onClosing: (close) => {
        if (settings.closeToTray && !root.allowQuit) {
            close.accepted = false
            root.hide()
        }
    }

    // Secondary text derived from the theme's text color at reduced opacity,
    // so it stays readable in both light and dark modes. (palette.mid is a
    // 3D-bevel gray, not a text color — it vanishes on both themes.)
    readonly property color secondaryTextColor: Qt.rgba(palette.text.r, palette.text.g, palette.text.b, 0.65)

    function iconSource(icon) {
        if (!icon)
            return ""
        if (icon.charAt(0) === "/")
            return "file://" + icon
        return "image://theme/" + encodeURIComponent(icon)
    }

    function visibleApplications() {
        const query = searchField.text.toLowerCase()
        return applications.filter(function(app) {
            return app.name.toLowerCase().indexOf(query) !== -1
                || app.executable.toLowerCase().indexOf(query) !== -1
        })
    }

    function selectedApplication() {
        for (let i = 0; i < applications.length; ++i) {
            if (applications[i].key === selectedKey)
                return applications[i]
        }
        return null
    }

    function formatNice(value) {
        return value > 0 ? "+" + value : "" + value
    }

    function syncApplicationModel() {
        const next = visibleApplications()

        for (let targetIndex = 0; targetIndex < next.length; ++targetIndex) {
            const source = next[targetIndex]
            const application = {
                "key": source.key,
                "name": source.name,
                "iconName": source.icon,
                "executable": source.executable,
                "mainPid": source.mainPid,
                "processCount": source.processCount,
                "nice": source.nice,
                "mixedPriority": source.mixedPriority,
                "ruleKey": source.ruleKey || "",
                "savedNice": (source.savedNice === undefined) ? null : source.savedNice
            }
            let currentIndex = -1

            for (let index = targetIndex; index < applicationModel.count; ++index) {
                if (applicationModel.get(index).key === application.key) {
                    currentIndex = index
                    break
                }
            }

            if (currentIndex === -1) {
                applicationModel.insert(targetIndex, application)
            } else {
                if (currentIndex !== targetIndex)
                    applicationModel.move(currentIndex, targetIndex, 1)
                applicationModel.set(targetIndex, application)
            }
        }

        if (applicationModel.count > next.length)
            applicationModel.remove(next.length, applicationModel.count - next.length)
    }

    Settings {
        id: settings
        property int refreshInterval: 5
        property bool closeToTray: false
    }

    ListModel {
        id: applicationModel
        dynamicRoles: true
    }

    ProcessController {
        id: controller
        onBusyChanged: {
            if (busy) {
                root.refreshIndicatorVisible = true
                refreshIndicatorMinimum.restart()
            } else if (!refreshIndicatorMinimum.running) {
                root.refreshIndicatorVisible = false
            }
        }
        onSnapshotJsonChanged: {
            try {
                root.applications = JSON.parse(snapshotJson)
            } catch (error) {
                root.applications = []
            }
            root.syncApplicationModel()
        }
    }

    Timer {
        id: refreshIndicatorMinimum
        interval: 450
        onTriggered: {
            if (controller.busy)
                restart()
            else
                root.refreshIndicatorVisible = false
        }
    }

    Timer {
        interval: settings.refreshInterval * 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: controller.refresh()
    }

    header: ToolBar {
        height: 68
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 22
            anchors.rightMargin: 16
            spacing: 12

            ColumnLayout {
                spacing: 1
                Layout.fillWidth: true
                Label {
                    text: qsTr("Application Priority Setter")
                    font.pixelSize: 19
                    font.weight: Font.DemiBold
                }
                Label {
                    text: qsTr("%1 running applications").arg(controller.applicationCount)
                    color: root.secondaryTextColor
                    font.pixelSize: 12
                }
            }

            TextField {
                id: searchField
                Layout.preferredWidth: 250
                placeholderText: qsTr("Search applications")
                selectByMouse: true
                rightPadding: clearButton.visible ? clearButton.width + 4 : 8
                onTextChanged: root.syncApplicationModel()

                ToolButton {
                    id: clearButton
                    anchors.right: parent.right
                    anchors.rightMargin: 2
                    anchors.verticalCenter: parent.verticalCenter
                    width: 28
                    height: 28
                    visible: searchField.text.length > 0
                    text: qsTr("×")
                    font.pixelSize: 16
                    Accessible.name: qsTr("Clear search")
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Clear search")
                    onClicked: searchField.clear()
                }
            }

            Item {
                Layout.preferredWidth: 104
                Layout.preferredHeight: 32
                opacity: root.refreshIndicatorVisible ? 1 : 0

                Row {
                    anchors.centerIn: parent
                    spacing: 5
                    BusyIndicator {
                        width: 24
                        height: 24
                        running: root.refreshIndicatorVisible
                    }
                    Label {
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Refreshing")
                        color: root.secondaryTextColor
                        font.pixelSize: 12
                    }
                }

                Behavior on opacity {
                    NumberAnimation { duration: 100 }
                }
            }
            ToolButton {
                text: qsTr("Refresh")
                enabled: !controller.busy
                onClicked: controller.refresh()
            }
            ToolButton {
                text: qsTr("Preferences")
                onClicked: preferencesDialog.open()
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 38
            color: palette.alternateBase

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 24
                anchors.rightMargin: 24
                Label { text: qsTr("APPLICATION"); font.pixelSize: 11; font.bold: true; Layout.fillWidth: true }
                Label { text: qsTr("PROCESSES"); font.pixelSize: 11; font.bold: true; Layout.preferredWidth: 90; horizontalAlignment: Text.AlignHCenter }
                Label { text: qsTr("NICE"); font.pixelSize: 11; font.bold: true; Layout.preferredWidth: 110; horizontalAlignment: Text.AlignHCenter }
            }
        }

        ListView {
            id: applicationList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: applicationModel

            ScrollBar.vertical: ScrollBar {}

            delegate: ItemDelegate {
                id: applicationDelegate
                required property string key
                required property string name
                required property string iconName
                required property string executable
                required property int mainPid
                required property int processCount
                required property int nice
                required property bool mixedPriority
                required property string ruleKey
                required property var savedNice
                width: applicationList.width
                height: 58
                highlighted: root.selectedKey === key
                onClicked: root.selectedKey = key

                contentItem: RowLayout {
                    spacing: 12
                    Item {
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 34

                        Rectangle {
                            anchors.fill: parent
                            radius: 8
                            visible: applicationIcon.status !== Image.Ready
                            color: applicationDelegate.highlighted ? palette.highlight : palette.alternateBase
                        }

                        Image {
                            id: applicationIcon
                            anchors.fill: parent
                            anchors.margins: 2
                            source: root.iconSource(applicationDelegate.iconName)
                            sourceSize: Qt.size(32, 32)
                            fillMode: Image.PreserveAspectFit
                            smooth: true
                        }

                        Label {
                            anchors.centerIn: parent
                            visible: applicationIcon.status !== Image.Ready
                            text: name.length ? name.charAt(0).toUpperCase() : "?"
                            font.bold: true
                            color: applicationDelegate.highlighted ? palette.highlightedText : palette.text
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1
                        Label {
                            text: name
                            font.pixelSize: 14
                            font.weight: Font.Medium
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Label {
                            text: executable
                                ? qsTr("Main PID %1  ·  %2").arg(mainPid).arg(executable)
                                : qsTr("Main PID %1").arg(mainPid)
                            color: root.secondaryTextColor
                            font.pixelSize: 11
                            elide: Text.ElideMiddle
                            Layout.fillWidth: true
                        }
                    }
                    Label {
                        text: processCount
                        Layout.preferredWidth: 90
                        horizontalAlignment: Text.AlignHCenter
                    }
                    ColumnLayout {
                        Layout.preferredWidth: 110
                        spacing: 0
                        Label {
                            text: mixedPriority ? qsTr("Mixed") : root.formatNice(nice)
                            Layout.alignment: Qt.AlignHCenter
                            horizontalAlignment: Text.AlignHCenter
                            font.family: "monospace"
                        }
                        Label {
                            visible: savedNice !== null && savedNice !== undefined
                            text: qsTr("saved %1").arg(root.formatNice(savedNice))
                            Layout.alignment: Qt.AlignHCenter
                            color: root.secondaryTextColor
                            font.pixelSize: 11
                            font.family: "monospace"
                        }
                    }
                }
            }

            Label {
                anchors.centerIn: parent
                visible: applicationList.count === 0
                text: searchField.text.length ? qsTr("No applications match your search") : qsTr("No applications found")
                color: root.secondaryTextColor
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 76
            color: palette.alternateBase
            border.color: palette.midlight
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 22
                anchors.rightMargin: 22
                spacing: 14

                Label {
                    text: controller.statusMessage || qsTr("Select an application to change its CPU priority")
                    color: root.secondaryTextColor
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                ComboBox {
                    id: priorityCombo
                    enabled: root.selectedKey.length > 0
                    textRole: "label"
                    valueRole: "nice"
                    model: [
                        { label: qsTr("Very High (-10)"), nice: -10 },
                        { label: qsTr("High (-5)"), nice: -5 },
                        { label: qsTr("Normal (0)"), nice: 0 },
                        { label: qsTr("Below Normal (+5)"), nice: 5 },
                        { label: qsTr("Low (+10)"), nice: 10 }
                    ]
                    currentIndex: 2
                    onActivated: {
                        root.selectedNice = currentValue
                        if (root.selectedKey.length > 0)
                            controller.applyPriority(root.selectedKey, currentValue)
                    }
                }

                Button {
                    text: qsTr("Reset Priority")
                    enabled: root.selectedKey.length > 0
                    onClicked: {
                        priorityCombo.currentIndex = 2
                        root.selectedNice = 0
                        controller.applyPriority(root.selectedKey, 0)
                    }
                }

                Button {
                    text: qsTr("Save")
                    enabled: root.selectedKey.length > 0
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Remember this priority and re-apply it automatically")
                    onClicked: controller.savePriority(root.selectedKey, root.selectedNice)
                }

                Button {
                    text: qsTr("Forget")
                    enabled: {
                        if (root.selectedKey.length === 0)
                            return false
                        const app = root.selectedApplication()
                        return app !== null && app.savedNice !== null && app.savedNice !== undefined
                    }
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Stop re-applying a saved priority for this application")
                    onClicked: controller.clearSavedPriority(root.selectedKey)
                }
            }
        }
    }

    Dialog {
        id: preferencesDialog
        anchors.centerIn: parent
        width: 440
        modal: true
        title: qsTr("Preferences")
        standardButtons: Dialog.Close

        ColumnLayout {
            width: parent.width
            spacing: 16
            Label {
                text: qsTr("Automatic refresh")
                font.pixelSize: 16
                font.weight: Font.DemiBold
            }
            Label {
                text: qsTr("Update the running application list every %1 second(s).").arg(settings.refreshInterval)
                color: root.secondaryTextColor
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.fillWidth: true
                Slider {
                    id: refreshSlider
                    Layout.fillWidth: true
                    from: 1
                    to: 60
                    stepSize: 1
                    value: settings.refreshInterval
                    onMoved: settings.refreshInterval = Math.round(value)
                }
                SpinBox {
                    from: 1
                    to: 60
                    value: settings.refreshInterval
                    editable: true
                    onValueModified: settings.refreshInterval = value
                }
                Label { text: qsTr("seconds") }
            }
            Label {
                text: qsTr("Faster refreshes use slightly more CPU. Five seconds is a good balance for normal use.")
                color: root.secondaryTextColor
                font.pixelSize: 12
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Label {
                text: qsTr("Startup")
                font.pixelSize: 16
                font.weight: Font.DemiBold
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                Label {
                    text: qsTr("Start automatically when you log in")
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Switch {
                    checked: controller.autostartEnabled
                    Accessible.name: qsTr("Start automatically when you log in")
                    onToggled: controller.setAutostart(checked)
                }
            }
            Label {
                text: qsTr("Adds or removes a startup entry following the system autostart folder.")
                color: root.secondaryTextColor
                font.pixelSize: 12
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Label {
                text: qsTr("System Tray")
                font.pixelSize: 16
                font.weight: Font.DemiBold
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                Label {
                    text: qsTr("Close to the system tray instead of quitting")
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Switch {
                    checked: settings.closeToTray
                    Accessible.name: qsTr("Close to the system tray instead of quitting")
                    onToggled: settings.closeToTray = checked
                }
            }
            Label {
                text: qsTr("The tray icon shows the app is running. Use its Quit entry to exit fully.")
                color: root.secondaryTextColor
                font.pixelSize: 12
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }
    }
}
