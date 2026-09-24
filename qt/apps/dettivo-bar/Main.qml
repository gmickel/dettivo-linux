import QtQuick
import Dettivo
import DettivoBar

// dettivo-bar: the panel or one bar glyph cell in a window only as large
// as the surface, so `--render` writes exactly what the shell would show.
Window {
    id: root

    readonly property bool glyphSurface: BarPreview.surface === "glyph"

    objectName: "barWindow"
    title: qsTr("Dettivo bar")
    visible: true
    color: root.glyphSurface ? Theme.colorBar : Theme.roleSurface
    width: Math.max(1, Math.ceil(content.implicitWidth))
    height: Math.max(1, Math.ceil(content.implicitHeight))

    Item {
        id: content
        anchors.fill: parent
        implicitWidth: root.glyphSurface ? cell.implicitWidth : panel.implicitWidth
        implicitHeight: root.glyphSurface ? cell.implicitHeight : panel.implicitHeight

        PanelSample {
            id: panel
            objectName: "panel"
            visible: !root.glyphSurface
            width: parent.width
            state: BarPreview.state
        }

        BarCell {
            id: cell
            objectName: "cell"
            visible: root.glyphSurface
            anchors.fill: parent
            state: BarPreview.state
        }
    }
}
