// Dettivo style — TabBar (R3).
import QtQuick
import QtQuick.Templates as T
import Dettivo

T.TabBar {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, Theme.controlHeight)

    spacing: Theme.spacingXs

    Accessible.role: Accessible.PageTabList
    Accessible.name: qsTr("Tabs")

    contentItem: ListView {
        model: control.contentModel
        currentIndex: control.currentIndex

        spacing: control.spacing
        orientation: ListView.Horizontal
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.AutoFlickIfNeeded
        snapMode: ListView.SnapToItem

        highlightMoveDuration: Motion.duration(Motion.durationReveal)
        highlightRangeMode: ListView.ApplyRange
        preferredHighlightBegin: Theme.spacingXl
        preferredHighlightEnd: width - Theme.spacingXl
    }

    background: Rectangle {
        color: Theme.roleSurface
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: StyleHelpers.borderWidth("Normal")
            color: Theme.roleHairline
        }
    }
}
