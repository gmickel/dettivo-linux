pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

Button {
    id: root
    property var settings: null
    property var resetter: null
    property string key: ""
    property bool busy: false
    readonly property int revision: settings ? settings.revision : 0
    readonly property string lockedBy: settings && revision >= 0 ? settings.lockedBy(key) : ""
    readonly property string defaultValue: settings && settings.registryLoaded ? settings.defaultText(key) : ""
    text: qsTr("Reset")
    enabled: !busy && settings !== null && settings.registryLoaded === true && lockedBy.length === 0
    ToolTip.visible: hovered || activeFocus
    ToolTip.text: lockedBy.length > 0 ? qsTr("Set by %1").arg(lockedBy) : qsTr("Use the default: %1. Empty paths follow the system location.").arg(defaultValue)
    Accessible.role: Accessible.Button
    Accessible.name: qsTr("Reset %1 to default").arg(key)
    onClicked: {
        if (resetter)
            resetter();
        else
            settings.unset(key);
    }
}
