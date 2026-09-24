import QtQuick
import "file:///usr/lib/dettivo/qml/Dettivo" as Dettivo
// A directory import loads the style library before the panel resolves its URI imports.
import "file:///usr/lib/dettivo/qml/DettivoStyle" as DettivoStyle

// The module's panel bound to the daemon's state; every action is one
// `dettivo` call. Loaded by PanelPopup.qml once the module is known to
// exist.
Dettivo.BarPanel {
    id: root

    // The popup closes itself after an action that opens the app.
    signal openRequested

    state: DettivoState.panelState
    hint: DettivoState.hint === "" ? "install" : DettivoState.hint
    hintDetail: DettivoState.hintDetail
    target: DettivoState.insertedTarget
    words: DettivoState.pillWords
    elapsed: DettivoState.meetingElapsed
    modeIndex: DettivoState.modeIndex
    engine: DettivoState.engineFact
    enhanced: DettivoState.enhancedFact
    insertTarget: DettivoState.insertFact
    recent: DettivoState.recent
    shortcut: DettivoState.openShortcut

    onDictate: DettivoState.dictate()
    onStopDictation: DettivoState.stopDictation()
    onCancelDictation: DettivoState.cancelDictation()
    onStartMeeting: DettivoState.startMeeting()
    onStopMeeting: DettivoState.stopMeeting()
    onModeSelected: index => DettivoState.selectMode(index)
    onOpenDettivo: {
        DettivoState.openApp(DettivoState.hint === "" ? "home" : "onboarding");
        root.openRequested();
    }
    onOpenItem: index => {
        DettivoState.openApp("history");
        root.openRequested();
    }
}
