pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The designed states as the sheet draws them (states-and-hint-sheet.png):
// one state (`stateName`, from `DETTIVO_E2E_STATE`), in a bordered box with the
// sheet's header strip at the artboard's first box, rendered with sample
// facts and no daemon so the visual job holds every state to its box.
// The hint sheet renders the same way. The page is the window while it
// lasts, like first run: no sidebar, no hints.
Item {
    id: root

    property string stateName: "history-empty"

    readonly property var samples: ({
            "history-empty": {
                "label": qsTr("History · empty"),
                "title": qsTr("Nothing dictated yet."),
                "reason": qsTr("Hold F9 in any app and it will show up here."),
                "key": "F9"
            },
            "meetings-empty": {
                "label": qsTr("Meetings · empty"),
                "title": qsTr("No meetings recorded."),
                "reason": qsTr("Start one from the rail or with n. Audio files can be imported too."),
                "key": "n",
                "action": qsTr("Import audio")
            },
            "search-no-results": {
                "label": qsTr("Search · no results"),
                "title": qsTr("No match for \"merger\"."),
                "reason": qsTr("Search covers dictations, meeting transcripts, notes and analysis."),
                "actionless": true
            },
            "engine-loading": {
                "label": qsTr("Engine · loading"),
                "trailing": "4.2 s",
                "title": qsTr("Warming Parakeet v3 on Vulkan."),
                "reason": qsTr("First dictation after a restart takes a moment; later ones will not."),
                "loading": true
            },
            "engine-crashed": {
                "label": qsTr("Engine · crashed"),
                "trailing": qsTr("dettivo-engine-whisper"),
                "title": qsTr("Whisper stopped unexpectedly."),
                "reason": qsTr("Your meeting kept recording. It restarts on the next request; three crashes in a row switch it to CPU."),
                "urgent": true,
                "action": qsTr("Restart now"),
                "secondary": qsTr("Show log")
            },
            "download-failed": {
                "label": qsTr("Download · failed"),
                "trailing": qsTr("SHA256 mismatch"),
                "title": qsTr("Checksum did not match."),
                "reason": qsTr("The file was quarantined and nothing was loaded. Retrying downloads from the catalogue source again."),
                "urgent": true,
                "action": qsTr("Retry"),
                "secondary": qsTr("Show source")
            },
            "microphone-missing": {
                "label": qsTr("Microphone · missing"),
                "title": qsTr("No input device."),
                "reason": qsTr("PipeWire reports no source. Plug in a microphone or pick one in Settings › General. History and meetings still open."),
                "urgent": true,
                "actionless": true
            },
            "insertion-fell-back": {
                "label": qsTr("Insertion · fell back"),
                "title": qsTr("Copied instead of typed."),
                "reason": qsTr("The focused window did not accept a virtual keyboard. Press Ctrl+V, or pin a backend for chromium in Settings › Insertion."),
                "actionless": true
            },
            "meeting-recovered": {
                "label": qsTr("Meeting · recovered"),
                "title": qsTr("A meeting was interrupted."),
                "reason": qsTr("Customer call · Nordwind, 52 min, 12 of 14 chunks saved. Recover it from the journal or discard it."),
                "action": qsTr("Recover"),
                "primary": true,
                "secondary": qsTr("Discard")
            }
        })
    readonly property var sample: root.samples[root.stateName] || ({})
    readonly property bool hintSheet: root.stateName === "hint-sheet"

    // The artboard's first box: 49, 161, 381 by 222; the hint sheet's box
    // is 581 by 258 and renders at the same corner.
    readonly property int boxX: Theme.space8 + Theme.hairlineWidth
    readonly property int boxY: Theme.space8 * 3 + Theme.space5 + Theme.hairlineWidth
    readonly property int boxWidth: Theme.space8 * 8 - Theme.space1 - Theme.hairlineWidth
    readonly property int boxHeight: Theme.space8 * 4 + Theme.space6 + Theme.space2 + Theme.space1
    readonly property int headerHeight: Theme.space7 + Theme.hairlineWidth

    HintSheet {
        anchors.left: parent.left
        anchors.leftMargin: root.boxX
        anchors.top: parent.top
        anchors.topMargin: root.boxY
        visible: root.hintSheet
    }

    Rectangle {
        id: box
        anchors.left: parent.left
        anchors.leftMargin: root.boxX
        anchors.top: parent.top
        anchors.topMargin: root.boxY
        border.color: Theme.roleHairline
        border.width: Theme.hairlineWidth
        color: "transparent"
        height: root.boxHeight
        radius: Theme.radius
        visible: !root.hintSheet
        width: root.boxWidth

        Item {
            id: header
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: root.headerHeight

            SectionLabel {
                anchors.left: parent.left
                anchors.leftMargin: Theme.space3 + Theme.space1
                anchors.verticalCenter: parent.verticalCenter
                text: root.sample.label || ""
            }

            SectionLabel {
                anchors.right: parent.right
                anchors.rightMargin: Theme.space3 + Theme.space1
                anchors.verticalCenter: parent.verticalCenter
                text: root.sample.trailing || ""
                visible: text.length > 0
            }

            Rectangle {
                anchors.bottom: parent.bottom
                anchors.left: parent.left
                anchors.right: parent.right
                color: Theme.roleHairline
                height: Theme.hairlineWidth
            }
        }

        StateView {
            action: root.sample.action || ""
            actionless: root.sample.actionless === true
            anchors.left: parent.left
            anchors.leftMargin: Theme.space5 - Theme.space1
            anchors.right: parent.right
            anchors.rightMargin: Theme.space5 - Theme.space1
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: root.headerHeight / 2 - Theme.space2
            keyHint: root.sample.key || ""
            loading: root.sample.loading === true
            primary: root.sample.primary === true
            reason: root.sample.reason || ""
            secondaryAction: root.sample.secondary || ""
            title: root.sample.title || ""
            urgent: root.sample.urgent === true
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("States")
}
