import QtQuick
import Dettivo

// The recording pill (fn-12, baseline docs/design/studio/baselines/osd.png):
// one 36 px pill whose state is carried by the dot colour and the bars.
// Six states plus hidden; every colour, size and duration is a token of
// this module, every text is one line that elides, and the bars are one
// scene-graph node. The host owns placement only; this component owns
// the states and the hide timers.
Item {
    id: root

    // The pill state is the item's `state`: listening, transcribing,
    // enhancing, inserted, copied, error or hidden. Any other name is
    // refused with a warning and the pill hides.
    // 0..1 from audio.level while listening.
    property real level: 0
    // The error title (the other states name themselves).
    property string title: ""
    // "release to insert" while listening.
    property string hint: ""
    // The engine and its elapsed time while transcribing.
    property string engine: ""
    property string elapsed: ""
    // The first words: raw while enhancing, inserted text afterwards.
    property string words: ""
    // The target application (inserted).
    property string target: ""
    // Why (copied, error) and what to do next.
    property string reason: ""
    property string action: ""
    // How long inserted and copied stay, then error; 0 disables.
    property int hideAfterMs: 1800
    property int errorHideAfterMs: 4000
    // Bars follow the level while listening; off holds them at the floor.
    property bool showLevel: true
    // Reduced motion from the host's setting, on top of the desktop's.
    property bool reducedMotion: false
    // The longest run of text before it elides.
    property int maxTextWidth: Theme.rowHeight * 9

    readonly property var knownStates: ["listening", "transcribing", "enhancing", "inserted", "copied", "error", "hidden"]
    readonly property bool valid: knownStates.indexOf(root.state) >= 0
    readonly property bool reduced: root.reducedMotion || Motion.reducedMotion
    // What the pill actually shows: hidden after the timer, or on a name it
    // does not know.
    readonly property string effectiveState: (!root.valid || root.expired) ? "hidden" : root.state
    readonly property bool shown: root.effectiveState !== "hidden"
    readonly property bool urgent: root.effectiveState === "error"
    readonly property bool accented: root.effectiveState === "inserted"
    // The title the state names (error carries its own).
    readonly property string effectiveTitle: {
        switch (root.effectiveState) {
        case "listening":
            return qsTr("Listening");
        case "transcribing":
            return qsTr("Transcribing");
        case "inserted":
            return root.target.length > 0 ? qsTr("Inserted into") : qsTr("Inserted");
        case "copied":
            return qsTr("Copied to clipboard");
        case "error":
            return root.title.length > 0 ? root.title : qsTr("Something went wrong");
        default:
            return "";
        }
    }
    // Everything the pill says, for the accessible name.
    readonly property string sentence: [root.effectiveTitle, root.target, content.detailText].filter(part => part.length > 0).join(" ")
    property bool expired: false

    // The host or a test learns the pill hid itself.
    signal hidden

    // Cuts the first sentence out of a longer text.
    function firstSentence(text) {
        const trimmed = (text || "").replace(/\s+/g, " ").trim();
        // A sentence ends at . ! or ? followed by a space or the end; a
        // period between digits ("2.0") is part of a number, never an end.
        for (let i = 0; i < trimmed.length; ++i) {
            const ch = trimmed[i];
            if (ch !== "." && ch !== "!" && ch !== "?")
                continue;
            const next = i + 1 < trimmed.length ? trimmed[i + 1] : " ";
            if (next !== " ")
                continue;
            if (ch === "." && i > 0 && /\d/.test(trimmed[i - 1]) && i + 1 < trimmed.length && /\d/.test(trimmed[i + 1]))
                continue;
            if (i + 1 < trimmed.length)
                return trimmed.slice(0, i + 1) + "…";
            return trimmed;
        }
        return trimmed;
    }

    state: "hidden"
    states: [
        State {
            name: "listening"
        },
        State {
            name: "transcribing"
        },
        State {
            name: "enhancing"
        },
        State {
            name: "inserted"
        },
        State {
            name: "copied"
        },
        State {
            name: "error"
        },
        State {
            name: "hidden"
        }
    ]

    // The item stays visible and laid out while hidden so its implicit
    // size never collapses; `shown` is what a host binds its window to,
    // and the fade carries the exit.
    implicitWidth: frame.implicitWidth
    implicitHeight: Theme.rowHeight
    opacity: root.shown ? 1 : 0

    // Inside a change handler the dependent bindings (valid, effectiveState)
    // still hold their previous values, so the handlers work from `state`.
    function isKnown(name) {
        return knownStates.indexOf(name) >= 0;
    }

    onStateChanged: {
        root.expired = false;
        if (!isKnown(root.state))
            console.warn("Dettivo.Osd: unknown state \"" + root.state + "\"; hiding the pill");
        root.restartTimer();
    }

    // Reported on the next event-loop pass: a host that mirrors `hidden`
    // straight back into `state` would otherwise re-enter the state change
    // that is still being applied.
    onEffectiveStateChanged: {
        if (root.effectiveState === "hidden")
            Qt.callLater(root.reportHidden);
    }

    function reportHidden() {
        if (root.effectiveState === "hidden")
            root.hidden();
    }

    Behavior on opacity {
        NumberAnimation {
            duration: root.reduced ? 0 : (root.shown ? Motion.durationEnter : Motion.durationExit)
            easing.type: Easing.BezierSpline
            easing.bezierCurve: root.shown ? Motion.easingEnter : Motion.easingExit
        }
    }

    Timer {
        id: hideTimer
        running: false
        repeat: false
        onTriggered: {
            if (root.isTimed(root.state))
                root.expired = true;
        }
    }

    function isTimed(name) {
        return name === "inserted" || name === "copied" || name === "error";
    }

    function restartTimer() {
        hideTimer.stop();
        if (root.expired || !isKnown(root.state) || !isTimed(root.state))
            return;
        hideTimer.interval = root.state === "error" ? root.errorHideAfterMs : root.hideAfterMs;
        if (hideTimer.interval > 0)
            hideTimer.start();
    }

    Component.onCompleted: restartTimer()
    onHideAfterMsChanged: restartTimer()
    onErrorHideAfterMsChanged: restartTimer()

    Rectangle {
        id: frame
        anchors.fill: parent
        implicitWidth: content.contentWidth + Theme.rowPaddingX * 2
        radius: Theme.radius
        color: Theme.roleSurface
        border.width: Theme.hairlineWidth
        border.color: root.urgent ? Qt.alpha(Theme.roleUrgent, 0.75) : (root.accented ? Qt.alpha(Theme.roleAccent, 0.75) : Theme.roleBorder)

        Behavior on border.color {
            ColorAnimation {
                duration: root.reduced ? 0 : Motion.durationExit
            }
        }

        Rectangle {
            anchors.fill: parent
            radius: Theme.radius
            color: Theme.roleRaisedSurface
        }

        OsdContent {
            id: content
            anchors.left: parent.left
            anchors.leftMargin: Theme.rowPaddingX
            anchors.verticalCenter: parent.verticalCenter
            pill: root
        }
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.sentence
    Accessible.description: root.effectiveState
}
