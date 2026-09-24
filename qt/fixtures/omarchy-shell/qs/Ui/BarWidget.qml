import QtQuick
import qs.Commons

// The shell's BarWidget base: the three injected properties and setting().
Item {
    id: root

    property QtObject bar: null
    property string moduleName: ""
    property var settings: ({})
    readonly property bool vertical: false
    readonly property int barSize: Style.bar.sizeHorizontal

    function setting(name, fallback) {
        const value = settings ? settings[name] : undefined;
        return value === undefined || value === null ? fallback : value;
    }
}
