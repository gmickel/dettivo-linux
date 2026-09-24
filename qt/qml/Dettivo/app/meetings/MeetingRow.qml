pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// One row of Meetings (meetings-list.png): when it started (weekday and
// clock, the date under it), the title with its one-line summary in
// muted, the length in tabular numerals, the speaker swatches with their
// names, and the state chip. Recover follows validated retained audio;
// Discard belongs to partial meetings. The hairline sits under every row.
Rectangle {
    id: root

    property string meetingId: ""
    property string when: ""
    property string date: ""
    property string title: ""
    property string summary: ""
    property string length: ""
    property var speakers: []
    property int speakerCount: 0
    property string chip: ""
    property string chipKind: "plain"
    property bool partial: false
    property bool recoverable: false
    property bool cancellable: false
    property bool selected: false
    property var actions: null
    property int whenWidth: Theme.space8 * 3
    property int lengthWidth: Theme.space8 * 2
    property int speakersWidth: Theme.space8 * 3
    property int stateWidth: Theme.space8 * 2

    signal activated

    readonly property bool hovered: hoverHandler.hovered
    readonly property string summaryLine: root.summary.length > 0 ? root.summary : (root.partial ? qsTr("Recovered after a crash") : "")

    color: root.selected ? Theme.roleSelectedFill : (root.hovered ? Theme.roleHoverFill : "transparent")
    implicitHeight: Math.max(Theme.space8 * 2, titleColumn.implicitHeight + Theme.space5 * 2)
    radius: Theme.radius

    Behavior on color {
        enabled: !Motion.reducedMotion
        ColorAnimation {
            duration: Motion.duration(Motion.durationEnter)
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Motion.easingEnter
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.top: parent.top
        color: Theme.roleSelected
        visible: root.selected
        width: Theme.railWidth
    }

    Column {
        id: whenColumn
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space1
        width: root.whenWidth

        Text {
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.features: Theme.typeTabularNumerals
            font.pixelSize: Theme.typeBodySize
            text: root.when
        }

        Text {
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.features: Theme.typeTabularNumerals
            font.pixelSize: Theme.typeCaptionSize
            text: root.date
        }
    }

    Column {
        id: titleColumn
        anchors.left: whenColumn.right
        anchors.right: lengthText.left
        anchors.rightMargin: Theme.space4
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space1

        Text {
            color: Theme.roleText
            elide: Text.ElideRight
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            font.weight: Theme.typeEmphasisWeight
            maximumLineCount: 2
            text: root.title
            textFormat: Text.PlainText
            width: parent.width
            wrapMode: Text.WordWrap
        }

        Text {
            color: Theme.roleFaintText
            elide: Text.ElideRight
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            maximumLineCount: 2
            text: root.summaryLine
            textFormat: Text.PlainText
            visible: text.length > 0
            width: parent.width
            wrapMode: Text.WordWrap
        }

        Row {
            id: inlineActions
            spacing: Theme.space3
            visible: root.recoverable || root.partial || root.cancellable

            Text {
                color: Theme.roleAccent
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: qsTr("Cancel")
                visible: root.cancellable

                TapHandler {
                    onTapped: {
                        if (root.actions)
                            root.actions.cancel(root.meetingId);
                    }
                }

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Cancel transcription")
                Accessible.onPressAction: {
                    if (root.actions)
                        root.actions.cancel(root.meetingId);
                }
            }

            Text {
                color: Theme.roleAccent
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: qsTr("Recover")
                visible: root.recoverable

                TapHandler {
                    onTapped: {
                        if (root.actions)
                            root.actions.recover(root.meetingId);
                    }
                }

                activeFocusOnTab: true
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                        if (root.actions)
                            root.actions.recover(root.meetingId);
                        event.accepted = true;
                    }
                }

                FocusRing {}

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Recover")
                Accessible.onPressAction: {
                    if (root.actions)
                        root.actions.recover(root.meetingId);
                }
            }

            Text {
                color: Theme.roleUrgent
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: qsTr("Discard")
                visible: root.partial

                TapHandler {
                    onTapped: {
                        if (root.actions)
                            root.actions.discard(root.meetingId);
                    }
                }

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Discard")
                Accessible.onPressAction: {
                    if (root.actions)
                        root.actions.discard(root.meetingId);
                }
            }
        }
    }

    Text {
        id: lengthText
        anchors.right: swatches.left
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeBodySize
        text: root.length
        width: root.lengthWidth
    }

    SpeakerSwatches {
        id: swatches
        anchors.right: chipItem.left
        anchors.verticalCenter: parent.verticalCenter
        count: root.speakerCount
        speakers: root.speakers
        width: root.speakersWidth
    }

    Item {
        id: chipItem
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        height: stateChip.implicitHeight
        width: root.stateWidth

        StateChip {
            id: stateChip
            anchors.left: parent.left
            kind: root.chipKind
            text: root.chip
            width: parent.width
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    HoverHandler {
        id: hoverHandler
    }

    TapHandler {
        onTapped: eventPoint => {
            if (!inlineActions.visible || !inlineActions.contains(inlineActions.mapFromItem(root, eventPoint.position)))
                root.activated();
        }
    }

    Accessible.role: Accessible.ListItem
    Accessible.name: root.title
    Accessible.description: root.summaryLine.length > 0 ? root.summaryLine + " · " + root.length : root.length
    Accessible.selectable: true
    Accessible.selected: root.selected
    Accessible.onPressAction: root.activated()
}
