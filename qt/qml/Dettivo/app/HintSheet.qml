pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The keyboard hint sheet (states-and-hint-sheet.png): the in-app keys in
// two columns, the global keys deferred to Hyprland, and Escape named as
// the key that closes anything. `?` opens it over any page and any key
// closes it; the sheet is a bordered box with the sheet's header strip.
Rectangle {
    id: root

    readonly property var leftKeys: [["j", qsTr("next item")], ["k", qsTr("previous item")], ["enter", qsTr("open or insert")], ["/", qsTr("search")], ["?", qsTr("this sheet")]]
    readonly property var rightKeys: [["n", qsTr("new meeting")], ["i", qsTr("import audio")], ["r", qsTr("re-run")], ["t", qsTr("rename meeting")], ["e", qsTr("export")], ["d", qsTr("delete")]]
    readonly property int headerHeight: Theme.space7 + Theme.hairlineWidth

    signal closed

    border.color: Theme.roleHairline
    border.width: Theme.hairlineWidth
    color: Theme.roleSurface
    focus: visible
    // The designed five rows plus the sixth the right column gained with
    // the meeting's title key.
    implicitHeight: Theme.space8 * 5 + Theme.space5 + Theme.space1 + Math.round(Theme.controlHeight * 0.8) + Theme.space2 + Theme.space1
    implicitWidth: Theme.space8 * 12 + Theme.space2 + Theme.hairlineWidth
    radius: Theme.radius

    Keys.onShortcutOverride: event => {
        event.accepted = true;
    }

    Keys.onPressed: event => {
        event.accepted = true;
        // The opening ? chord can deliver its Shift press after the shortcut
        // opens this sheet. A modifier alone is not a dismissal action.
        if ([Qt.Key_Shift, Qt.Key_Control, Qt.Key_Alt, Qt.Key_Meta, Qt.Key_AltGr].indexOf(event.key) !== -1)
            return;
        root.closed();
    }

    Item {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: root.headerHeight

        SectionLabel {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space3 + Theme.space1
            anchors.verticalCenter: parent.verticalCenter
            text: qsTr("Keyboard hint sheet · press ?")
        }

        SectionLabel {
            anchors.right: parent.right
            anchors.rightMargin: Theme.space3 + Theme.space1
            anchors.verticalCenter: parent.verticalCenter
            text: qsTr("Closes on any key")
        }

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            color: Theme.roleHairline
            height: Theme.hairlineWidth
        }
    }

    Row {
        id: columns
        anchors.left: parent.left
        anchors.leftMargin: Theme.space5 - Theme.space1
        anchors.top: header.bottom
        anchors.topMargin: Theme.space6
        spacing: Theme.space8 * 2 + Theme.space6

        Repeater {
            model: [root.leftKeys, root.rightKeys]

            delegate: Column {
                id: keyColumn
                required property var modelData
                spacing: Theme.space2 + Theme.space1

                Repeater {
                    model: keyColumn.modelData

                    delegate: Row {
                        id: keyRow
                        required property var modelData
                        spacing: Theme.space4

                        KeyCap {
                            anchors.verticalCenter: parent.verticalCenter
                            text: keyRow.modelData[0]
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            color: Theme.roleText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.typeBodySize
                            text: keyRow.modelData[1]
                            Accessible.role: Accessible.StaticText
                            Accessible.name: keyRow.modelData[0] + ": " + keyRow.modelData[1]
                        }
                    }
                }
            }
        }
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: Theme.space5 - Theme.space1
        anchors.right: parent.right
        anchors.rightMargin: Theme.space5
        anchors.top: columns.bottom
        anchors.topMargin: Theme.space5
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: qsTr("Global keys stay with Hyprland: F9 hold, Super+Ctrl+X toggle, Super+Shift+D open. Escape closes anything.")
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Global keys")
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Keyboard hint sheet")
}
