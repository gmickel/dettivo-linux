pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// Diagnostics: `dettivo doctor` on screen, line by line with its verdict,
// the actions to run it again and to copy it for a bug report, and the
// QA switch the route writes.
SettingsPage {
    id: root

    property var doctor: null

    readonly property var lines: root.doctor ? root.doctor.lines : []

    section: "diagnostics"
    subtitle: root.doctor ? root.doctor.verdict : ""

    trailing: Row {
        spacing: Theme.space3

        Button {
            enabled: root.doctor ? !root.doctor.running : false
            text: qsTr("Run again")
            onClicked: {
                if (root.doctor)
                    root.doctor.run();
            }
        }

        Button {
            enabled: root.doctor ? root.doctor.report.length > 0 : false
            text: qsTr("Copy report")
            onClicked: {
                if (root.doctor)
                    root.doctor.copy();
            }
        }
    }

    SectionLabel {
        bottomPadding: Theme.space2
        leftPadding: 0
        text: root.doctor && root.doctor.ranAt.length > 0 ? qsTr("%1 · %2").arg(root.doctor.command).arg(root.doctor.ranAt) : (root.doctor ? root.doctor.command : "")
        topPadding: 0
    }

    Rectangle {
        border.color: root.doctor && root.doctor.report.length > 0 && !root.doctor.healthy ? Qt.alpha(Theme.roleUrgent, 0.5) : Theme.roleBorder
        border.width: Theme.stateNormalBorderWidth
        color: "transparent"
        height: reportColumn.implicitHeight + Theme.space4 * 2
        radius: Theme.radius
        width: parent.width

        Column {
            id: reportColumn
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.top: parent.top
            anchors.topMargin: Theme.space4
            spacing: Theme.space1

            Repeater {
                model: root.lines

                delegate: Text {
                    id: line
                    required property string modelData
                    readonly property bool trouble: line.modelData.includes("INVALID") || line.modelData.includes("DEGRADED") || line.modelData.includes("unreachable") || line.modelData.includes("NOT FOUND")

                    color: line.trouble ? Theme.roleUrgent : Theme.roleText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeCaptionSize
                    text: line.modelData
                    width: reportColumn.width
                    wrapMode: Text.WrapAnywhere
                    Accessible.role: Accessible.StaticText
                    Accessible.name: line.modelData
                }
            }

            Text {
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: root.doctor && root.doctor.running ? qsTr("Running.") : qsTr("No report yet. Run again asks the daemon.")
                visible: root.lines.length === 0
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }

        Accessible.role: Accessible.Pane
        Accessible.name: qsTr("Doctor report")
    }

    SettingsGroup {
        title: qsTr("QA")

        SettingRow {
            hint: qsTr("accept the QA rig's fake capture and insertion")
            key: "qa.mode"
            kind: "switch"
            label: qsTr("QA mode")
            settings: root.settings
        }
    }
}
