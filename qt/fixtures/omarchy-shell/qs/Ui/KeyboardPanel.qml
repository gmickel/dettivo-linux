import QtQuick
import qs.Commons

// The shell's KeyboardPanel: the anchored popup card, here a plain item.
Item {
    id: root

    required property Item anchorItem
    required property QtObject bar
    property var owner: null
    property int margin: Style.gapsOut
    property int padding: Style.spacing.popupPadding
    property int contentWidth: Style.space(280)
    property int contentHeight: Style.space(200)
    property bool centerOnBar: false
    property bool open: false
    property Item focusTarget: null
    default property alias contentItem: contentHolder.children

    function fittedContentWidth(width, cap) {
        return width;
    }
    function fittedContentHeight(implicitHeight, cap) {
        return implicitHeight;
    }

    visible: open
    width: contentWidth + padding * 2
    height: contentHeight + padding * 2

    Item {
        id: contentHolder
        anchors.fill: parent
        anchors.margins: root.padding
    }
}
