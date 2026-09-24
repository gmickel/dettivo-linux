// DettivoStyle Button: the shell's state alphas for normal, hover, focus,
// selected and pressed; `highlighted` is the accent primary action and
// `danger` the urgent one, matching the Studio design sheet.
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.Button {
    id: control

    property bool danger: false

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, Theme.controlHeight)

    leftPadding: Theme.controlPaddingX
    rightPadding: Theme.controlPaddingX
    topPadding: 0
    bottomPadding: 0
    spacing: Theme.space3

    font.family: Theme.fontFamily
    font.pixelSize: Theme.typeBodySize

    readonly property string fillState: StyleHelpers.pickFillState(control.down, control.checked, control.visualFocus, control.hovered)
    readonly property string borderState: StyleHelpers.pickBorderState(control.checked, control.visualFocus, control.hovered)
    readonly property bool quiet: control.borderState === "Normal" && !control.down
    readonly property color accentColor: control.danger ? Theme.roleUrgent : Theme.roleAccent

    Accessible.role: Accessible.Button
    Accessible.name: control.text
    Accessible.ignored: !control.visible

    contentItem: Row {
        spacing: control.spacing
        leftPadding: 0

        Icon {
            visible: control.icon.name.length > 0
            anchors.verticalCenter: parent.verticalCenter
            source: control.icon.name
            size: Theme.iconSize
            color: label.color
            accessibleName: ""
        }

        Text {
            id: label
            anchors.verticalCenter: parent.verticalCenter
            text: control.text
            font: control.font
            color: control.highlighted || control.danger ? control.accentColor : (control.enabled ? Theme.roleText : Theme.roleFaintText)
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
    }

    background: Rectangle {
        implicitWidth: Theme.controlHeight * 3
        implicitHeight: Theme.controlHeight
        radius: Theme.radius

        color: {
            if (control.highlighted && control.quiet)
                return Qt.alpha(control.accentColor, Theme.stateFocusFillAlpha);
            return StyleHelpers.fillColor(control.fillState);
        }
        border.width: (control.highlighted || control.danger) && control.quiet ? Theme.stateNormalBorderWidth : StyleHelpers.borderWidth(control.borderState)
        border.color: {
            if (control.danger && control.quiet)
                return Qt.alpha(control.accentColor, Theme.stateHoverBorderAlpha);
            if (control.highlighted && control.quiet)
                return Qt.alpha(control.accentColor, Theme.stateFocusBorderAlpha);
            if (!control.enabled)
                return Theme.roleHairline;
            return StyleHelpers.borderColor(control.borderState);
        }

        Behavior on color {
            enabled: !Motion.reducedMotion
            ColorAnimation {
                duration: Motion.duration(Motion.durationEnter)
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Motion.easingEnter
            }
        }
    }
}
