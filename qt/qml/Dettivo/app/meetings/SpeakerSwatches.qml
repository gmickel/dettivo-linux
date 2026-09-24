pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The speakers of a meeting at a glance (meetings-list.png): one small
// square per speaker in the speaker role its `color_index` names, then
// the names joined by commas in muted text. Without swatches the count
// stands alone (`2 speakers`); without any the cell is empty.
Item {
    id: root

    // `{name, colorIndex, speakerId}` maps in swatch order.
    property var speakers: []
    property int count: 0

    readonly property var colors: Theme.roleSpeakerColors
    readonly property int shown: root.speakers ? root.speakers.length : 0
    readonly property string names: {
        const list = [];
        for (let i = 0; i < root.shown; ++i)
            list.push(root.speakers[i].name);
        if (list.length > 0)
            return list.join(", ");
        if (root.count === 1)
            return qsTr("1 speaker");
        return root.count > 1 ? qsTr("%1 speakers").arg(root.count) : "";
    }

    implicitHeight: Math.max(row.implicitHeight, namesText.implicitHeight)

    Row {
        id: row
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.topMargin: Theme.space1
        spacing: Theme.space1

        Repeater {
            model: root.speakers

            delegate: Rectangle {
                id: swatch
                required property var modelData
                color: root.colors[swatch.modelData.colorIndex % root.colors.length]
                height: Theme.space2 + Theme.space1
                radius: Theme.radius
                width: height
                Accessible.role: Accessible.Graphic
                Accessible.name: swatch.modelData.name
            }
        }
    }

    Text {
        id: namesText
        anchors.left: row.right
        anchors.leftMargin: root.shown > 0 ? Theme.space2 : 0
        anchors.right: parent.right
        anchors.top: parent.top
        color: Theme.roleMutedText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        maximumLineCount: 2
        text: root.names
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.names
}
