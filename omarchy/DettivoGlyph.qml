import QtQuick
import "file:///usr/lib/dettivo/qml/Dettivo" as Dettivo

// The module's bar glyph bound to the daemon's state. Loaded by
// BarWidget.qml only once the module is known to exist.
Dettivo.BarGlyph {
    id: root

    state: DettivoState.glyphState
    level: DettivoState.level
    levelMeter: DettivoState.levelMeter
    glyph: DettivoState.glyph
    elapsed: DettivoState.meetingActive ? DettivoState.meetingElapsed.replace(/^00:/, "") : ""
    dimmed: DettivoState.dimmed
}
