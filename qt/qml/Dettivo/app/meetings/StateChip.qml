import QtQuick
import Dettivo

// A meeting's state as a tracked-caps chip (meetings-list.png): an
// analysed meeting in the accent, a partial or failed one in the urgent
// colour, notes only and the plain states in muted. A hairline border,
// no fill, the text centred.
Rectangle {
    id: root

    property string text: ""
    // `analysed`, `partial`, `failed`, `notes`, `live` or `plain`.
    property string kind: "plain"

    readonly property color tone: {
        if (root.kind === "analysed" || root.kind === "live")
            return Theme.roleAccent;
        if (root.kind === "partial" || root.kind === "failed")
            return Theme.roleUrgent;
        return Theme.roleMutedText;
    }

    border.color: Qt.alpha(root.tone, root.kind === "plain" || root.kind === "notes" ? 0.5 : 0.7)
    border.width: Theme.hairlineWidth
    color: "transparent"
    implicitHeight: Theme.typeLabelSize + Theme.space3
    implicitWidth: label.implicitWidth + Theme.space4 * 2
    radius: Theme.radius
    visible: root.text.length > 0

    Text {
        id: label
        anchors.centerIn: parent
        color: root.tone
        font.capitalization: Font.AllUppercase
        font.family: Theme.fontFamily
        font.letterSpacing: Theme.typeLabelSize * Theme.typeLabelTracking
        font.pixelSize: Theme.typeLabelSize
        text: root.text
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.text
}
