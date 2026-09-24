pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// One binding of the Hotkeys route (settings-hotkeys.png): the action,
// the chord as key caps with a muted hint after it, and the field that
// edits the chord in Hyprland notation on the right; the daemon refuses
// a chord it cannot parse and the row says so under the field.
Item {
    id: root

    property var settings: null
    property string key: ""
    property string action: ""
    property string hint: ""

    readonly property int revision: root.settings ? root.settings.revision : 0
    readonly property string chord: root.readChord(root.revision)
    readonly property string error: root.readError(root.errorRevision)
    readonly property var caps: root.capsOf(root.chord)
    property int errorRevision: 0

    implicitHeight: Theme.settingsBindingRowHeight + (root.error.length > 0 ? errorText.implicitHeight + Theme.space2 : 0)

    function readChord(revision) {
        return root.settings && revision >= 0 ? root.settings.text(root.key) : "";
    }

    function readError(revision) {
        return root.settings && revision >= 0 ? root.settings.error(root.key) : "";
    }

    // `SUPER CTRL, X` becomes Super, Ctrl, X; a bare key stays one cap.
    function capsOf(chord) {
        const parts = chord.split(",");
        const mods = parts.length > 1 ? parts[0].trim().split(/\s+/).filter(m => m.length > 0) : [];
        const keyName = (parts.length > 1 ? parts[1] : parts[0]).trim();
        const pretty = m => m.length > 0 ? m.charAt(0).toUpperCase() + m.slice(1).toLowerCase() : m;
        return mods.map(pretty).concat(keyName.length > 0 ? [keyName] : []);
    }

    Connections {
        target: root.settings
        function onErrorChanged(key) {
            if (key === root.key)
                root.errorRevision += 1;
        }
    }

    Text {
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.topMargin: (Theme.settingsBindingRowHeight - height) / 2
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        font.weight: Theme.typeEmphasisWeight
        text: root.action
        Accessible.role: Accessible.StaticText
        Accessible.name: root.action
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: Theme.firstRunActionColumn
        anchors.top: parent.top
        anchors.topMargin: (Theme.settingsBindingRowHeight - height) / 2
        spacing: Theme.space2

        Repeater {
            model: root.caps

            delegate: Row {
                id: cap
                required property int index
                required property string modelData
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.space2

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: Theme.roleFaintText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeBodySize
                    text: "+"
                    visible: cap.index > 0
                    Accessible.role: Accessible.StaticText
                    Accessible.name: qsTr("plus")
                }

                KeyCap {
                    anchors.verticalCenter: parent.verticalCenter
                    text: cap.modelData
                }
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            leftPadding: Theme.space2
            text: root.hint
            visible: root.hint.length > 0
            Accessible.role: Accessible.StaticText
            Accessible.name: root.hint
        }
    }

    SettingField {
        id: field
        anchors.right: reset.left
        anchors.rightMargin: Theme.space2
        anchors.top: parent.top
        enabled: !root.settings || root.settings.lockedBy(root.key).length === 0
        anchors.topMargin: (Theme.settingsBindingRowHeight - Theme.controlHeight) / 2
        name: root.key
        value: root.chord
        width: Theme.settingsFieldWidth
        onCommitted: text => {
            if (root.settings)
                root.settings.set(root.key, text);
        }
    }

    SettingReset {
        id: reset
        anchors.right: parent.right
        anchors.verticalCenter: field.verticalCenter
        settings: root.settings
        key: root.key
    }

    Text {
        id: errorText
        anchors.right: parent.right
        anchors.top: field.bottom
        anchors.topMargin: Theme.space2
        color: Theme.roleUrgent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.error
        visible: root.error.length > 0
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("%1 refused").arg(root.key)
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    Accessible.role: Accessible.ListItem
    Accessible.name: root.action + ": " + root.caps.join("+")
}
