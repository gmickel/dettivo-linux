import QtQuick
import Dettivo

// One row of the right rail: a name on the left, a state on the right,
// the hairline under it and, for an engine, the 2 px progress hairline
// that fills while it loads or downloads and stays full while it is warm.
Item {
    id: root

    property string name: ""
    property string value: ""
    property bool accent: false
    property real progress: 0
    property bool busy: false
    property bool showProgress: false

    implicitHeight: root.showProgress ? Theme.rowHeight + Theme.space2 + Theme.space1 : Theme.rowHeight - Theme.space2
    width: parent ? parent.width : implicitWidth

    Text {
        anchors.left: parent.left
        anchors.right: valueText.left
        anchors.rightMargin: Theme.space3
        anchors.verticalCenter: parent.verticalCenter
        anchors.verticalCenterOffset: root.showProgress ? -Theme.space2 : 0
        color: Theme.roleText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: root.name
    }

    Text {
        id: valueText
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.verticalCenterOffset: root.showProgress ? -Theme.space2 : 0
        color: root.accent ? Theme.roleAccent : Theme.roleMutedText
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeBodySize
        text: root.value
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    Rectangle {
        id: progressTrack
        objectName: "engineProgressTrack"
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.space3
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.progressHairlineWidth
        visible: root.showProgress

        Rectangle {
            color: Theme.roleAccent
            height: parent.height
            width: parent.width * (root.busy ? Math.max(0, Math.min(1, root.progress)) : (root.accent ? 1 : 0))

            Behavior on width {
                enabled: !Motion.reducedMotion
                NumberAnimation {
                    duration: Motion.duration(Motion.durationReveal)
                    easing.type: Easing.BezierSpline
                    easing.bezierCurve: Motion.easingReveal
                }
            }
        }
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.name + ": " + root.value
}
