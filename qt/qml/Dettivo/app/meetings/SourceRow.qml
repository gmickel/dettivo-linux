import QtQuick
import Dettivo

// One capture source of the new-meeting rail (meetings-list.png): the
// accent square that says it records, the source's name, the device line
// under it in muted, and the level meter at the right that shows the
// `audio.level` stream while a meeting runs. The microphone always
// records (a meeting is the room first), so its square is fixed on.
Item {
    id: root

    property string name: ""
    property string device: ""
    property bool checked: true
    property bool fixed: false
    property real level: 0
    property real peak: 0

    signal toggled

    function toggle() {
        if (root.enabled && !root.fixed)
            root.toggled();
    }

    implicitHeight: column.implicitHeight + Theme.space3

    Rectangle {
        id: square
        enabled: !root.fixed
        activeFocusOnTab: root.enabled && !root.fixed
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.topMargin: Theme.space2 + Theme.space1
        border.color: Theme.roleAccent
        border.width: Theme.hairlineWidth
        color: root.checked ? Theme.roleAccent : "transparent"
        height: Theme.space3
        radius: Theme.radius
        width: Theme.space3

        TapHandler {
            enabled: !root.fixed
            onTapped: root.toggle()
        }

        Keys.onPressed: event => {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                if (!event.isAutoRepeat)
                    root.toggle();
                event.accepted = true;
            }
        }

        Accessible.role: Accessible.CheckBox
        Accessible.name: root.name
        Accessible.checkable: !root.fixed
        Accessible.checked: root.checked
        Accessible.onToggleAction: root.toggle()
    }

    Column {
        id: column
        anchors.left: square.right
        anchors.leftMargin: Theme.space3
        anchors.right: meter.left
        anchors.rightMargin: Theme.space3
        anchors.top: parent.top
        spacing: Theme.space1

        Text {
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            font.weight: Theme.typeEmphasisWeight
            text: root.name
        }

        Text {
            color: Theme.roleFaintText
            elide: Text.ElideRight
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            maximumLineCount: 2
            text: root.device
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: root.device
        }
    }

    LevelBars {
        id: meter
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: Theme.space1
        label: qsTr("%1 level").arg(root.name)
        level: root.checked ? root.level : 0
        opacity: root.checked ? 1 : 0.4
        peak: root.checked ? root.peak : 0
    }

    FocusRing {
        visible: square.activeFocus
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("%1 source").arg(root.name)
}
