import QtQuick

// A layer-shell window, reduced to the properties the plugin sets; the
// children are kept so a Loader inside it instantiates.
QtObject {
    id: root

    component Edges: QtObject {
        property bool top: false
        property bool bottom: false
        property bool left: false
        property bool right: false
    }
    component Margins: QtObject {
        property int top: 0
        property int bottom: 0
        property int left: 0
        property int right: 0
    }

    default property list<QtObject> data
    property bool visible: false
    property color color: "transparent"
    property int exclusiveZone: 0
    property bool aboveWindows: false
    property bool focusable: false
    property real implicitWidth: 0
    property real implicitHeight: 0
    property real width: implicitWidth
    property real height: implicitHeight
    property Edges anchors: Edges {}
    property Margins margins: Margins {}
    property var mask: null
}
