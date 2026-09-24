pragma Singleton
import QtQuick

// The shell's Color singleton, reduced to the roles the plugin reads.
QtObject {
    id: root

    component Surface: QtObject {
        readonly property color background: "#101315"
        readonly property color text: "#cacccc"
        readonly property color border: "#f5bf03"
        readonly property color active: "#f5bf03"
    }

    readonly property color foreground: "#cacccc"
    readonly property color background: "#101315"
    readonly property color accent: "#f5bf03"
    readonly property color urgent: "#a55555"
    readonly property color muted: "#707880"
    readonly property Surface bar: Surface {}
    readonly property Surface popups: Surface {}
}
