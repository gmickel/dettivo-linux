import QtQuick
import Dettivo

// The top of every page: the title, one line under it and a right-aligned
// fact (the clock on Home). The title carries the route's accessible name.
Item {
    id: root

    property string title: ""
    property string subtitle: ""
    property string trailing: ""
    // A rich-text subtitle for the status sentence; plain when empty.
    property string subtitleRich: ""
    property string subtitlePlain: ""
    // A subtitle that wraps at this width instead of eliding (first run).
    property int subtitleMaxWidth: 0

    // A page without a subtitle (Try it draws its own line with a key cap)
    // ends at the title.
    implicitHeight: titleText.implicitHeight + (subtitleText.visible ? Theme.space2 + subtitleText.implicitHeight : 0)

    Text {
        id: titleText
        anchors.left: parent.left
        anchors.top: parent.top
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.letterSpacing: Theme.typeTitleSize * Theme.typeTitleTracking
        font.pixelSize: Theme.typeTitleSize
        font.weight: Theme.typeEmphasisWeight
        text: root.title
        Accessible.role: Accessible.Heading
        Accessible.name: root.title
    }

    Text {
        id: subtitleText
        anchors.left: parent.left
        anchors.right: root.subtitleMaxWidth > 0 ? undefined : trailingText.left
        anchors.rightMargin: Theme.space4
        anchors.top: titleText.bottom
        anchors.topMargin: Theme.space2
        color: Theme.roleMutedText
        elide: root.subtitleMaxWidth > 0 ? Text.ElideNone : Text.ElideRight
        width: root.subtitleMaxWidth > 0 ? Math.min(root.subtitleMaxWidth, root.width) : undefined
        wrapMode: root.subtitleMaxWidth > 0 ? Text.WordWrap : Text.NoWrap
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: root.subtitleRich.length > 0 ? root.subtitleRich : root.subtitle
        textFormat: root.subtitleRich.length > 0 ? Text.StyledText : Text.PlainText
        visible: text.length > 0
        Accessible.role: Accessible.StaticText
        Accessible.name: root.subtitlePlain.length > 0 ? root.subtitlePlain : root.subtitle
    }

    Text {
        id: trailingText
        anchors.right: parent.right
        anchors.verticalCenter: subtitleText.verticalCenter
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeBodySize
        text: root.trailing
        Accessible.role: Accessible.StaticText
        Accessible.name: root.trailing
    }
}
