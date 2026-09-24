pragma Singleton
import QtQuick
import Dettivo

// The motion library (ADR 0010): one easing set, named for what it does,
// and a reduced-motion switch from the desktop portal. Components read
// `duration(...)` so every animation collapses to zero when the user asks
// for reduced motion, while the raw tokens stay readable for tests.
QtObject {
    id: motion

    readonly property bool reducedMotion: ThemeBackend.reducedMotion

    // Durations in milliseconds.
    readonly property int durationEnter: 180
    readonly property int durationExit: 120
    readonly property int durationReveal: 240
    readonly property int durationPill: 400
    readonly property int durationShimmer: 1200

    // Easing curves as cubic-bezier control points, the form the design
    // system states them in.
    readonly property var easingEnter: [0.2, 0.0, 0.0, 1.0, 1.0, 1.0]
    readonly property var easingExit: [0.4, 0.0, 1.0, 1.0, 1.0, 1.0]
    readonly property var easingReveal: easingEnter
    readonly property int easingLinear: Easing.Linear

    // The OSD pill spring: damping 0.8 over the pill duration.
    readonly property real pillSpring: 2.5
    readonly property real pillDamping: 0.8

    // The waveform follows audio.level events at their own rate.
    readonly property int levelFrameMs: 16

    // A duration that honours reduced motion.
    function duration(ms) {
        return reducedMotion ? 0 : ms;
    }
}
