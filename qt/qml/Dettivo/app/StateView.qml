import QtQuick
import QtQuick.Controls
import Dettivo

// A designed state (states-and-hint-sheet.png): the state sentence first
// in heading size and emphasis weight, the reason second in muted, the
// action third as a key or a small button, and the urgent colour only as
// the 8 px square. A loading state shows the shimmer thread with its
// elapsed time; nothing spins and nothing shouts. `complete` says whether
// the instance follows the rule: a sentence, a reason, and an action or a
// key unless the state says it has none (`actionless`).
Item {
    id: root

    property string title: ""
    property string reason: ""
    property string keyHint: ""
    property string action: ""
    property string secondaryAction: ""
    property bool urgent: false
    property bool loading: false
    property bool actionless: false
    property bool actionEnabled: true
    property bool primary: false

    readonly property bool complete: root.title.length > 0 && root.reason.length > 0 && (root.actionless || root.loading || root.keyHint.length > 0 || root.action.length > 0)

    signal actionTriggered
    signal secondaryTriggered

    implicitHeight: column.implicitHeight

    Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: Theme.space4

        OsdThread {
            running: true
            visible: root.loading
            width: Theme.space8 * 3 + Theme.space5
        }

        Row {
            spacing: Theme.space3

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleUrgent
                height: Theme.space3
                visible: root.urgent
                width: Theme.space3
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeHeadingSize
                font.weight: Theme.typeEmphasisWeight
                text: root.title
                Accessible.role: Accessible.StaticText
                Accessible.name: root.title
            }
        }

        Text {
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            lineHeight: Theme.typeBodyLeading
            lineHeightMode: Text.ProportionalHeight
            text: root.reason
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: root.reason
        }

        Row {
            spacing: Theme.space3
            visible: root.keyHint.length > 0 || root.action.length > 0

            KeyCap {
                anchors.verticalCenter: parent.verticalCenter
                text: root.keyHint
                visible: root.keyHint.length > 0
            }

            // The sheet's small button: the key cap's height, not a
            // control row's.
            Button {
                anchors.verticalCenter: parent.verticalCenter
                enabled: root.actionEnabled
                highlighted: root.primary
                implicitHeight: Theme.controlHeight * 0.8
                text: root.action
                visible: root.action.length > 0
                onClicked: root.actionTriggered()
            }

            Button {
                anchors.verticalCenter: parent.verticalCenter
                enabled: root.actionEnabled
                implicitHeight: Theme.controlHeight * 0.8
                text: root.secondaryAction
                visible: root.secondaryAction.length > 0
                onClicked: root.secondaryTriggered()
            }
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.title
}
