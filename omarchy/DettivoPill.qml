import QtQuick
import "file:///usr/lib/dettivo/qml/Dettivo" as Dettivo

// The module's recording pill bound to the daemon's state: the same
// component dettivo-osd hosts, fed from the same events. Loaded by
// Panel.qml once the module is known to exist.
Dettivo.Osd {
    id: root

    state: DettivoState.pillState
    // Visual dB scale: -60 dBFS is the floor, 0 dBFS is full height.
    // Keep in sync with the standalone OsdModel; capture gain stays untouched.
    level: {
        const rms = Math.max(0, Math.min(1, DettivoState.level));
        return rms > 0 ? Math.max(0, Math.min(1, (20 * Math.log(rms) / Math.LN10 + 60) / 60)) : 0;
    }
    title: DettivoState.pillTitle
    hint: DettivoState.pillHint
    engine: DettivoState.engineLabel
    words: DettivoState.pillWords
    target: DettivoState.pillTarget
    reason: DettivoState.pillReason
    action: DettivoState.pillAction
    showLevel: DettivoState.levelMeter
    onHidden: DettivoState.pillHidden()
}
