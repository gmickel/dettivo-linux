import QtQuick
import QtQuick.Controls
import Dettivo

// One fact of the new-meeting rail (meetings-list.png): the name in the
// emphasis weight, the value right-aligned in muted, the hairline under
// it. The row is a control: a click raises `clicked` (a menu, a toggle),
// and an editable row swaps its value for a field until Enter or a blur
// hands the text back through `edited`.
Item {
    id: root

    property string name: ""
    property string value: ""
    property string accessibleName: ""
    property bool editable: false
    property bool editing: false

    signal clicked
    signal edited(string text)

    readonly property bool hovered: hover.hovered
    activeFocusOnTab: true

    implicitHeight: Math.max(Theme.rowHeight + Theme.space2, valueText.implicitHeight + Theme.space4)
    width: parent ? parent.width : implicitWidth

    function beginEdit() {
        root.editing = true;
        field.text = "";
        field.forceActiveFocus();
    }

    function activate() {
        if (!root.enabled || root.editing)
            return;
        if (root.editable)
            root.beginEdit();
        else
            root.clicked();
    }

    Keys.onShortcutOverride: event => {
        if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
            event.accepted = true;
    }

    Keys.onPressed: event => {
        if (!root.editing && (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
            if (!event.isAutoRepeat)
                root.activate();
            event.accepted = true;
        }
    }

    function finishEdit() {
        if (!root.editing)
            return;
        root.editing = false;
        root.edited(field.text);
    }

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: -Theme.space2
        anchors.rightMargin: -Theme.space2
        color: root.hovered && !root.editing ? Theme.roleHoverFill : "transparent"
        radius: Theme.radius
    }

    Text {
        id: nameText
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        font.weight: Theme.typeEmphasisWeight
        text: root.name
    }

    Text {
        id: valueText
        anchors.left: nameText.right
        anchors.leftMargin: Theme.space4
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        color: root.hovered ? Theme.roleText : Theme.roleMutedText
        wrapMode: Text.Wrap
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeBodySize
        horizontalAlignment: Text.AlignRight
        text: root.value
        visible: !root.editing
    }

    TextField {
        id: field
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        inputMethodHints: Qt.ImhDigitsOnly
        placeholderText: qsTr("count, empty decides")
        visible: root.editing
        width: Theme.space8 * 3
        onAccepted: root.finishEdit()
        onActiveFocusChanged: {
            if (!activeFocus)
                root.finishEdit();
        }
        Accessible.role: Accessible.EditableText
        Accessible.name: qsTr("%1 field").arg(root.accessibleName)
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        enabled: !root.editing
        onTapped: root.activate()
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    FocusRing {}

    Accessible.role: Accessible.Button
    Accessible.name: root.accessibleName.length > 0 ? root.accessibleName : root.name
    Accessible.description: root.value
    Accessible.onPressAction: root.activate()
}
