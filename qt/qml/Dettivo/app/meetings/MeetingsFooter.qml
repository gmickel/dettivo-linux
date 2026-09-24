import QtQuick
import Dettivo

// The foot of a meetings screen: the keys on the left in the faint
// colour, a fact on the right in tabular numerals (`142 segments · 0
// gaps` on the live meeting), the hairline above when the screen asks
// for one. A screen that puts its own control at the right edge (the
// detail's delete) names that control's width as `trailingReserve`, so
// the keys stop before it; a window too narrow for one line wraps the
// keys, three lines at the narrowest window, before eliding.
Item {
    id: root

    property string hints: ""
    property string trailing: ""
    property bool hairline: false
    property real trailingReserve: 0

    implicitHeight: hintText.implicitHeight + Theme.space5 * 2

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        color: Theme.roleHairline
        height: Theme.hairlineWidth
        visible: root.hairline
    }

    Text {
        id: hintText
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: trailingText.left
        anchors.rightMargin: Theme.space4 + root.trailingReserve
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleFaintText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        maximumLineCount: 3
        text: root.hints
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Keyboard hints")
    }

    Text {
        id: trailingText
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeCaptionSize
        text: root.trailing
        visible: text.length > 0
        Accessible.role: Accessible.StaticText
        Accessible.name: root.trailing
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Meetings footer")
}
