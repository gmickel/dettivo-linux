import QtQuick
import Dettivo

// One line of muted body text with a key cap in it, the way the Try it
// header reads "Hold [F9], say a sentence, let go." (first-run-3-try-it.png):
// the text before the key, the cap, the text after it. Reads as one
// sentence to assistive technology.
Row {
    id: root

    property string lead: ""
    property string key: ""
    property string tail: ""

    spacing: Theme.space2

    Text {
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: root.lead
        visible: root.lead.length > 0
        Accessible.ignored: true
    }

    KeyCap {
        anchors.verticalCenter: parent.verticalCenter
        text: root.key
        Accessible.ignored: true
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: root.tail
        visible: root.tail.length > 0
        Accessible.ignored: true
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.lead + " " + root.key + root.tail
}
