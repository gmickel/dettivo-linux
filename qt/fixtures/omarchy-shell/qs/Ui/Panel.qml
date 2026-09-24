import QtQuick

// The shell's Panel base: the open/close lifecycle without the IPC.
Item {
    id: root

    property QtObject bar: null
    property string moduleName: ""
    property var settings: ({})
    property string ipcTarget: ""
    property bool manageIpc: true
    property alias controller: panelController
    property bool popoutSwitching: false
    property bool popoutSwitchClosing: false

    readonly property bool opened: panelController.open

    function open() {
        panelController.show();
    }
    function close() {
        panelController.hide();
    }
    function closeForPopoutSwitch() {
        popoutSwitchClosing = true;
        close();
        Qt.callLater(function () {
            popoutSwitchClosing = false;
        });
    }
    function toggle() {
        opened ? close() : open();
    }
    function setting(name, fallback) {
        const value = settings ? settings[name] : undefined;
        return value === undefined || value === null ? fallback : value;
    }

    QtObject {
        id: panelController
        property bool open: false
        function show() {
            open = true;
        }
        function hide() {
            open = false;
        }
    }
}
