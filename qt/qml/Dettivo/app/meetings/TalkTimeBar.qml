pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// Who spoke how much (meeting-detail.png): one bar split by talk time in
// the speaker colours, then a legend of square, name, talk time and share
// per speaker with `rename a speaker: click the name` on the right. A
// click on a name opens the rename popover anchored to it.
Item {
    id: root

    // `{speakerId, name, colorIndex, talkMs, talk, share, shareText}`.
    property var speakers: []
    property int currentIndex: -1
    property string hint: qsTr("rename a speaker: click the name")

    signal speakerClicked(int index, Item anchorItem)

    readonly property var colors: Theme.roleSpeakerColors
    readonly property int count: root.speakers ? root.speakers.length : 0

    function anchorFor(index) {
        return legend.itemAt(index);
    }

    implicitHeight: barRow.height + Theme.space3 + legendRow.implicitHeight

    Row {
        id: barRow
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: Theme.space2
        spacing: Theme.hairlineWidth

        Repeater {
            model: root.speakers

            delegate: Rectangle {
                id: slice
                required property var modelData
                required property int index
                color: root.colors[slice.modelData.colorIndex % root.colors.length]
                height: barRow.height
                opacity: root.currentIndex < 0 || root.currentIndex === slice.index ? 1 : 0.55
                radius: Theme.radius
                width: Math.max(Theme.space1, (barRow.width - barRow.spacing * (root.count - 1)) * slice.modelData.share)
                Accessible.role: Accessible.ProgressBar
                Accessible.name: slice.modelData.name + ": " + slice.modelData.shareText
            }
        }

        Accessible.role: Accessible.Pane
        Accessible.name: qsTr("Talk time")
    }

    Row {
        id: legendRow
        anchors.left: parent.left
        anchors.top: barRow.bottom
        anchors.topMargin: Theme.space3
        spacing: Theme.space5

        Repeater {
            id: legend
            model: root.speakers

            delegate: Row {
                id: entry
                required property var modelData
                required property int index
                spacing: Theme.space2

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    color: root.colors[entry.modelData.colorIndex % root.colors.length]
                    height: Theme.space2 + Theme.space1
                    radius: Theme.radius
                    width: height
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: root.colors[entry.modelData.colorIndex % root.colors.length]
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeCaptionSize
                    font.underline: root.currentIndex === entry.index
                    font.weight: Theme.typeEmphasisWeight
                    text: entry.modelData.name
                    textFormat: Text.PlainText
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: Theme.roleMutedText
                    font.family: Theme.fontFamily
                    font.features: Theme.typeTabularNumerals
                    font.pixelSize: Theme.typeCaptionSize
                    text: qsTr("%1 · %2").arg(entry.modelData.talk).arg(entry.modelData.shareText)
                }

                TapHandler {
                    onTapped: root.speakerClicked(entry.index, entry)
                }

                activeFocusOnTab: true
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                        root.speakerClicked(entry.index, entry);
                        event.accepted = true;
                    }
                }

                FocusRing {}

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Rename %1").arg(entry.modelData.name)
                Accessible.description: qsTr("%1 · %2").arg(entry.modelData.talk).arg(entry.modelData.shareText)
                Accessible.onPressAction: root.speakerClicked(entry.index, entry)
            }
        }
    }

    Text {
        anchors.right: parent.right
        anchors.verticalCenter: legendRow.verticalCenter
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.hint
        visible: root.count > 0
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Speakers")
}
