import QtQuick
import Dettivo

// The pill's row: the lead (dot or icon), the bars or the thread, the
// title and the one-line detail. Split out of Osd.qml so each file stays
// readable; `pill` is the Osd that owns the state.
Row {
    id: root

    required property var pill

    readonly property string pillState: pill.effectiveState
    readonly property bool listening: pillState === "listening"
    readonly property bool transcribing: pillState === "transcribing"
    readonly property bool enhancing: pillState === "enhancing"
    readonly property bool inserted: pillState === "inserted"
    readonly property bool copied: pillState === "copied"
    readonly property bool error: pillState === "error"
    readonly property bool hasBars: listening || transcribing
    readonly property bool hasDot: listening || transcribing || enhancing || error
    readonly property color barColor: transcribing ? Qt.alpha(Theme.roleAccent, 0.45) : Theme.roleAccent
    readonly property color dotColor: error ? Theme.roleUrgent : (listening ? Theme.roleAccent : Theme.roleSelected)
    // The words shown while enhancing (raw) and after insertion.
    readonly property string sentence: pill.firstSentence(pill.words)
    // The muted detail after the title, one line.
    readonly property string detailText: {
        switch (pillState) {
        case "listening":
            return pill.hint;
        case "transcribing":
            return [pill.engine, pill.elapsed].filter(part => part.length > 0).join(" · ");
        case "enhancing":
            return sentence;
        case "inserted":
            return sentence;
        case "copied":
            return [pill.reason, pill.action].filter(part => part.length > 0).join(" · ");
        case "error":
            return pill.reason;
        default:
            return "";
        }
    }

    // The width the row will have once laid out, computed from the visible
    // children right away: the positioner's own implicit width settles on
    // the next frame, and a host sizing its window before the first frame
    // needs the number now.
    readonly property real contentWidth: {
        let total = 0;
        let shown = 0;
        for (const child of root.children) {
            if (child.visible) {
                total += child.width;
                shown += 1;
            }
        }
        return total + Math.max(0, shown - 1) * root.spacing;
    }

    spacing: Theme.space4
    height: Theme.rowHeight

    OsdDot {
        visible: root.hasDot
        anchors.verticalCenter: parent.verticalCenter
        color: root.dotColor
        pulsing: root.listening && !root.pill.reduced
    }

    Icon {
        visible: root.inserted || root.copied
        anchors.verticalCenter: parent.verticalCenter
        source: root.inserted ? "insert" : "copy"
        color: root.inserted ? Theme.roleAccent : Theme.roleText
        size: Theme.iconSize
        accessibleName: root.inserted ? qsTr("Inserted") : qsTr("Copied")
    }

    OsdBars {
        visible: root.hasBars
        anchors.verticalCenter: parent.verticalCenter
        height: Theme.controlHeight - Theme.space3
        barWidth: Theme.waveformBarWidth
        gap: Theme.waveformBarGap
        color: root.barColor
        level: root.pill.showLevel ? root.pill.level : 0
        live: root.listening
        reducedMotion: root.pill.reduced
        Accessible.role: Accessible.Graphic
        Accessible.name: qsTr("Audio level")
    }

    OsdThread {
        visible: root.enhancing
        anchors.verticalCenter: parent.verticalCenter
        running: root.enhancing && !root.pill.reduced
    }

    Text {
        id: title
        visible: text.length > 0
        anchors.verticalCenter: parent.verticalCenter
        text: root.pill.effectiveTitle
        color: Theme.roleText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        font.weight: Theme.typeEmphasisWeight
        elide: Text.ElideRight
        maximumLineCount: 1
        width: Math.min(implicitWidth, root.pill.maxTextWidth)
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Text {
        id: target
        visible: root.inserted && text.length > 0
        anchors.verticalCenter: parent.verticalCenter
        text: root.pill.target
        color: Theme.roleAccent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        font.weight: Theme.typeEmphasisWeight
        elide: Text.ElideRight
        maximumLineCount: 1
        width: Math.min(implicitWidth, root.pill.maxTextWidth)
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Text {
        id: detail
        visible: text.length > 0
        anchors.verticalCenter: parent.verticalCenter
        text: root.detailText
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        elide: Text.ElideRight
        maximumLineCount: 1
        width: Math.min(implicitWidth, root.pill.maxTextWidth)
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    // The error action in the urgent colour, after a separator.
    Text {
        visible: root.error && root.pill.action.length > 0
        anchors.verticalCenter: parent.verticalCenter
        text: (root.pill.reason.length > 0 ? "· " : "") + root.pill.action
        color: Theme.roleUrgent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        elide: Text.ElideRight
        maximumLineCount: 1
        width: Math.min(implicitWidth, root.pill.maxTextWidth)
        Accessible.role: Accessible.StaticText
        Accessible.name: root.pill.action
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: root.pill.sentence
}
