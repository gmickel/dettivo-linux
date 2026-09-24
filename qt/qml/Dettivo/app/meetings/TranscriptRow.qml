import QtQuick
import Dettivo

// One transcript row (meeting-live.png, meeting-detail.png): the time in
// tabular numerals, a small square and the speaker or source label in
// the speaker role of its colour index, and the text at a 1.7 line
// height. A provisional row reads in the light weight and the muted
// colour; a capture gap before the row is a hairline with its length.
Item {
    id: root

    property string time: ""
    property string label: ""
    property int colorIndex: 0
    property string text: ""
    property bool provisional: false
    property real gapBefore: 0
    property bool selected: false

    signal labelClicked

    readonly property var colors: Theme.roleSpeakerColors
    readonly property color tone: root.colors[root.colorIndex % root.colors.length]
    readonly property int timeWidth: Theme.space8 + Theme.space5
    readonly property int labelWidth: Theme.space8 * 2

    implicitHeight: gap.height + body.implicitHeight

    Item {
        id: gap
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: root.gapBefore > 0 ? Theme.space5 : 0
        visible: root.gapBefore > 0

        Rectangle {
            anchors.left: parent.left
            anchors.right: gapText.left
            anchors.rightMargin: Theme.space3
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleHairline
            height: Theme.hairlineWidth
        }

        Text {
            id: gapText
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.features: Theme.typeTabularNumerals
            font.pixelSize: Theme.typeCaptionSize
            text: qsTr("gap %1 s").arg((root.gapBefore / 1000).toFixed(1))
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.leftMargin: -Theme.space2
        anchors.right: parent.right
        anchors.top: gap.bottom
        color: Theme.roleSelectedFill
        radius: Theme.radius
        visible: root.selected
    }

    Text {
        id: timeText
        anchors.left: parent.left
        anchors.top: gap.bottom
        anchors.topMargin: Theme.space1
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeBodySize
        text: root.time
        width: root.timeWidth

        // The tilde the command line prints on a provisional line.
        Text {
            anchors.left: parent.right
            anchors.leftMargin: -Theme.space4
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: "~"
            visible: root.provisional
            Accessible.role: Accessible.StaticText
            Accessible.name: root.provisional ? qsTr("provisional") : ""
        }
    }

    Row {
        id: labelRow
        anchors.left: timeText.right
        anchors.top: gap.bottom
        anchors.topMargin: Theme.space1
        spacing: Theme.space2
        width: root.labelWidth

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            color: root.tone
            height: Theme.space2 + Theme.space1
            radius: Theme.radius
            width: height
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: root.tone
            elide: Text.ElideRight
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.label
            textFormat: Text.PlainText
            width: root.labelWidth - Theme.space4
        }

        TapHandler {
            onTapped: root.labelClicked()
        }

        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                root.labelClicked();
                event.accepted = true;
            }
        }

        FocusRing {}

        Accessible.role: Accessible.Button
        Accessible.name: root.label
        Accessible.onPressAction: root.labelClicked()
    }

    Text {
        id: body
        anchors.left: labelRow.right
        anchors.right: parent.right
        anchors.top: gap.bottom
        color: root.provisional ? Theme.roleMutedText : Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        font.weight: root.provisional ? Theme.typeProvisionalWeight : Font.Normal
        lineHeight: 1.7
        text: root.text
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: root.text
    }

    Accessible.role: Accessible.ListItem
    Accessible.name: root.label + ": " + root.text
    Accessible.description: root.provisional ? qsTr("provisional") : root.time
}
