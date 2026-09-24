import QtQuick
import Dettivo

// A tracked-caps tag with a hairline border: Raw, Polish, Enhanced, an app
// name, or a state with a leading dot. Square corners, no fill, one accent.
Rectangle {
    id: root

    property string text: ""
    property bool accent: false
    property bool closable: false
    property color dotColor: "transparent"

    signal closed
    signal clicked

    readonly property bool hasDot: root.dotColor.a > 0

    implicitHeight: Theme.typeLabelSize + Theme.space2 + Theme.space1
    implicitWidth: row.implicitWidth + Theme.space2 * 2 + Theme.space1
    radius: Theme.radius
    color: "transparent"
    border.width: Theme.hairlineWidth
    border.color: root.accent ? Qt.alpha(Theme.roleAccent, 0.5) : Theme.roleHairline

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.space1 + Theme.space1 / 2

        Rectangle {
            visible: root.hasDot
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.space2 + Theme.space1
            height: width
            radius: Theme.radius
            color: root.dotColor
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.text
            color: root.accent ? Theme.roleAccent : Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeLabelSize
            font.capitalization: Font.AllUppercase
            font.letterSpacing: Theme.typeLabelSize * 0.08
        }

        Icon {
            visible: root.closable
            anchors.verticalCenter: parent.verticalCenter
            source: "close"
            size: Theme.typeLabelSize
            color: Theme.roleMutedText
            accessibleName: qsTr("Remove")

            TapHandler {
                onTapped: root.closed()
            }
        }
    }

    TapHandler {
        onTapped: root.clicked()
    }

    // A closable chip is a control: Tab reaches it and Return or Space
    // removes it, the way its Remove glyph does.
    activeFocusOnTab: root.closable
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.closed();
            event.accepted = true;
        }
    }

    FocusRing {}

    Accessible.role: root.closable ? Accessible.Button : Accessible.StaticText
    Accessible.name: root.text
}
