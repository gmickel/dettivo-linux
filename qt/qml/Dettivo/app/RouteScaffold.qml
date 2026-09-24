import QtQuick
import Dettivo

// A route that a later spec fills: the page header with the route's
// title, its designed state, and the line naming the config.toml keys
// the route will write, so the file stays the path to every setting.
Item {
    id: root

    property string title: ""
    property string subtitle: ""
    property string stateTitle: ""
    property string stateReason: ""
    property string keyHint: ""
    property string action: ""
    property bool actionEnabled: true
    property bool urgent: false
    property string configKeys: ""
    default property alias content: extra.data

    signal actionTriggered

    PageHeader {
        id: header
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: parent.top
        anchors.topMargin: Theme.pagePaddingY
        subtitle: root.subtitle
        title: root.title
    }

    Item {
        id: extra
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: header.bottom
        anchors.topMargin: Theme.space6
        height: childrenRect.height
    }

    StateView {
        id: state
        action: root.action
        actionEnabled: root.actionEnabled
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: extra.bottom
        anchors.topMargin: extra.height > 0 ? Theme.space6 : 0
        keyHint: root.keyHint
        reason: root.stateReason
        title: root.stateTitle
        urgent: root.urgent
        visible: root.stateTitle.length > 0
        onActionTriggered: root.actionTriggered()
    }

    Text {
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.space6
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        color: Theme.roleFaintText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: qsTr("This route writes %1").arg(root.configKeys)
        visible: root.configKeys.length > 0
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Configuration keys")
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.title
}
