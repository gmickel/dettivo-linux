pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The right rail (home.png): the four engines with their state and the
// 2 px progress hairline, then the agent surfaces with their last call.
Item {
    id: root

    property var engines: null
    property var status: null

    readonly property string gpu: root.status && root.status.gpu.length > 0 ? root.status.gpu : ""

    Column {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: 0

        SectionHeading {
            title: qsTr("Engines")
            trailing: root.gpu
            width: parent.width
        }

        Repeater {
            model: root.engines

            delegate: RailRow {
                id: engineRow
                required property var model
                accent: engineRow.model.state === "warm"
                busy: engineRow.model.busy
                name: engineRow.model.name
                progress: engineRow.model.progress
                showProgress: true
                value: engineRow.model.state
            }
        }

        Item {
            height: Theme.space6
            width: parent.width
        }

        SectionHeading {
            title: qsTr("Agents")
            trailing: qsTr("Local only")
            width: parent.width
        }

        RailRow {
            name: qsTr("socket")
            value: root.status && root.status.socketMode.length > 0 ? root.status.socketMode : qsTr("peer")
        }

        RailRow {
            name: qsTr("mcp")
            value: root.status && root.status.mcpHosts.length > 0 ? root.status.mcpHosts : qsTr("not checked")
        }

        RailRow {
            name: qsTr("rest")
            value: root.status && root.status.restState.length > 0 ? root.status.restState : qsTr("off")
        }

        RailRow {
            name: qsTr("last call")
            value: root.status && root.status.lastCall.length > 0 ? root.status.lastCall : qsTr("none yet")
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Engines and agents")
}
