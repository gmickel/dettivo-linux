import QtQuick
import QtQuick.Controls
import Dettivo

// A text, number, list or table key: one field that commits on Enter or
// when the focus leaves it with a changed text, never on every keystroke.
// Focus selects the whole text so typing replaces it. A value that
// changes on disk while the field is being edited keeps the edit and says
// so under the field; a field that is not being edited follows the file.
Item {
    id: root

    property string value: ""
    property string name: ""
    property string placeholder: ""

    signal committed(string text)

    readonly property bool editing: field.activeFocus && field.text !== root.value
    readonly property bool stale: root.editing && root.shownValue !== root.value
    property string shownValue: ""
    // The text the last commit sent, so the focus leaving after Enter
    // does not send it a second time.
    property var committedText: null

    implicitHeight: Theme.controlHeight

    onValueChanged: {
        if (!field.activeFocus) {
            field.text = root.value;
            root.shownValue = root.value;
        }
    }

    TextField {
        id: field
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        placeholderText: root.placeholder
        readOnly: !root.enabled
        text: root.value
        onActiveFocusChanged: {
            if (field.activeFocus) {
                root.committedText = null;
                root.shownValue = root.value;
                // A click selects the whole text once its release is done,
                // so typing replaces the value instead of adding to it.
                field.selectOnRelease = true;
                Qt.callLater(field.selectAll);
            } else if (field.text !== root.value && field.text !== root.committedText) {
                root.committedText = field.text;
                root.committed(field.text);
            } else {
                root.shownValue = root.value;
            }
        }
        onAccepted: {
            if (field.text !== root.value) {
                root.committedText = field.text;
                root.committed(field.text);
            }
            field.focus = false;
        }
        Accessible.name: root.name

        property bool selectOnRelease: false

        onReleased: {
            if (field.selectOnRelease) {
                field.selectOnRelease = false;
                field.selectAll();
            }
        }
    }

    Text {
        anchors.right: parent.right
        anchors.top: field.bottom
        anchors.topMargin: Theme.space1
        color: Theme.roleAccent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeLabelSize
        text: qsTr("changed on disk to %1 · Enter keeps yours").arg(root.value)
        visible: root.stale
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Changed on disk")
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("%1 field").arg(root.name)
}
