import QtQuick
import Dettivo
import DettivoOsd

// dettivo-osd: the pill in its own window. The host (main.cpp) decides
// whether this window is a layer-shell overlay or a plain always-on-top
// window and shows it when the model has something to say; the window is
// only ever as large as the pill.
Window {
    id: root

    objectName: "osdWindow"
    color: "transparent"
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus | Qt.Tool
    title: qsTr("Dettivo OSD")
    visible: false
    width: Math.max(1, Math.ceil(pill.implicitWidth))
    height: Math.max(1, Math.ceil(pill.implicitHeight))

    Osd {
        id: pill
        objectName: "pill"
        anchors.fill: parent
        state: OsdModel.state
        level: OsdModel.level
        title: OsdModel.title
        hint: OsdModel.hint
        engine: OsdModel.engine
        elapsed: OsdModel.elapsed
        words: OsdModel.words
        target: OsdModel.target
        reason: OsdModel.reason
        action: OsdModel.action
        hideAfterMs: OsdModel.hideAfterMs
        errorHideAfterMs: OsdModel.errorHideAfterMs
        showLevel: OsdModel.showLevel
        reducedMotion: OsdModel.reducedMotion
        onHidden: OsdModel.pillHidden()
    }
}
