pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The take (history.png): the Audio block with its length and rate, the
// play control, the bars of the recording with the played portion in the
// accent, and the clock. Without a take the block says why (retention
// off, or the sweep) instead of an empty strip.
Rectangle {
    id: root

    property var player: null
    property bool retained: false
    property string reason: ""
    property string meta: ""

    readonly property var peaks: root.player ? root.player.peaks : []
    readonly property real fraction: root.player ? root.player.fraction : 0
    readonly property bool playing: root.player ? root.player.playing : false
    readonly property string clock: root.player ? root.player.clock : ""
    readonly property string playerError: root.player ? root.player.error : ""

    border.color: Theme.roleHairline
    border.width: Theme.hairlineWidth
    color: "transparent"
    implicitHeight: header.height + Theme.hairlineWidth + body.height
    radius: Theme.radius

    Item {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: Theme.controlHeight + Theme.space1

        SectionLabel {
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4 - Theme.spacingXs
            anchors.verticalCenter: parent.verticalCenter
            text: qsTr("Audio")
        }

        Text {
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleMutedText
            font.capitalization: Font.AllUppercase
            font.family: Theme.fontFamily
            font.letterSpacing: Theme.typeLabelSize * Theme.typeLabelTracking
            font.pixelSize: Theme.typeLabelSize
            text: root.meta
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Audio facts")
        }

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            color: Theme.roleHairline
            height: Theme.hairlineWidth
        }
    }

    Item {
        id: body
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        height: Theme.controlHeight + Theme.space4 * 2

        Rectangle {
            id: play
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4
            anchors.verticalCenter: parent.verticalCenter
            border.color: Theme.roleBorder
            border.width: Theme.stateNormalBorderWidth
            color: playHover.hovered ? Theme.roleHoverFill : "transparent"
            height: Theme.controlHeight
            radius: Theme.radius
            visible: root.retained
            width: Theme.controlHeight

            Icon {
                accessibleName: ""
                anchors.centerIn: parent
                color: Theme.roleText
                size: Theme.iconSize
                source: root.playing ? "pause" : "play"
            }

            HoverHandler {
                id: playHover
            }

            TapHandler {
                onTapped: {
                    if (root.player)
                        root.player.toggle();
                }
            }

            activeFocusOnTab: true
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                    if (root.player)
                        root.player.toggle();
                    event.accepted = true;
                }
            }

            FocusRing {}

            Accessible.role: Accessible.Button
            Accessible.name: root.playing ? qsTr("Pause") : qsTr("Play")
            Accessible.onPressAction: {
                if (root.player)
                    root.player.toggle();
            }
        }

        Row {
            id: bars
            anchors.left: play.right
            anchors.leftMargin: Theme.space4
            anchors.verticalCenter: parent.verticalCenter
            height: Theme.controlHeight - Theme.space2
            spacing: Theme.waveformBarGap
            visible: root.retained

            Repeater {
                model: Theme.historyBarCount

                delegate: Item {
                    id: slot
                    required property int index
                    readonly property real level: {
                        const values = root.peaks || [];
                        const value = values[slot.index];
                        return value === undefined ? 0 : Math.max(0, Math.min(1, value));
                    }
                    readonly property bool played: (slot.index + 0.5) / Theme.historyBarCount <= root.fraction
                    height: bars.height
                    width: Theme.waveformBarWidth

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        color: slot.played ? Theme.roleAccent : Theme.roleFaintText
                        height: Math.max(Theme.space1, parent.height * Math.max(0.08, slot.level))
                        radius: Theme.radius
                        width: parent.width
                    }
                }
            }

            TapHandler {
                onTapped: eventPoint => {
                    if (root.player)
                        root.player.seek(eventPoint.position.x / bars.width);
                }
            }

            Accessible.role: Accessible.Graphic
            Accessible.name: qsTr("Take")
        }

        Text {
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.features: Theme.typeTabularNumerals
            font.pixelSize: Theme.typeBodySize
            text: root.retained ? (root.playerError.length > 0 ? root.playerError : root.clock) : root.reason
            Accessible.role: Accessible.StaticText
            Accessible.name: root.retained ? qsTr("Playback position") : root.reason
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Audio")
}
