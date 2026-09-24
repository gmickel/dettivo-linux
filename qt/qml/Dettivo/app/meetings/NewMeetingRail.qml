pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// The new-meeting rail (meetings-list.png): the two sources with their
// device names and meters, then the facts a start is made of as rows of
// name and value, each a control on a click: the engine from the
// meeting-capable providers (a menu), the expected speaker count (a
// field), the analysis after the stop (a toggle), the disclosure state;
// one primary Start meeting, the sentence on what happens next, and the
// footer naming the config.toml keys that set the defaults.
Item {
    id: root

    property var router: null
    property var status: null
    property var live: null
    property var actions: null
    property var config: null

    property bool systemAudio: true
    // The analysis choice starts on `[meetings.analysis] auto` and is
    // sent only once the user changed it here; until then the daemon
    // applies the configuration's default.
    readonly property bool configuredAnalyze: root.configuredAuto(root.config ? root.config.revision : 0)
    property bool analyzeTouched: false
    property bool analyze: root.configuredAnalyze
    property int engineIndex: 0
    property int modelIndex: 0
    property int expectedSpeakers: 0
    readonly property var providers: root.actions ? root.actions.providers : []
    readonly property var provider: root.providers[root.engineIndex] ? root.providers[root.engineIndex] : null
    readonly property var models: root.provider && root.provider.models ? root.provider.models : []
    readonly property var model: root.models[root.modelIndex] ? root.models[root.modelIndex] : null
    readonly property string engineLabel: root.provider ? (root.model ? qsTr("%1 %2 · timestamps").arg(root.provider.id).arg(root.model.id) : root.provider.label.toLowerCase()) : qsTr("selected engine")
    // Every provider and model pair the menu offers, `{label, engine, model}`.
    readonly property var choices: {
        const out = [];
        for (let e = 0; e < root.providers.length; ++e) {
            const models = root.providers[e].models || [];
            if (models.length === 0)
                out.push({
                    "label": root.providers[e].label,
                    "engine": e,
                    "model": 0
                });
            for (let m = 0; m < models.length; ++m)
                out.push({
                    "label": root.providers[e].label + " · " + models[m].label,
                    "engine": e,
                    "model": m
                });
        }
        return out;
    }
    readonly property bool recording: root.live ? root.live.active : false
    readonly property bool acknowledged: root.live ? root.live.disclosureAcknowledged : false
    readonly property string micDevice: root.live && root.live.micDevice.length > 0 ? root.live.micDevice : (root.status && root.status.inputName.length > 0 ? root.status.inputName : qsTr("default source"))
    readonly property string systemDevice: root.live && root.live.systemDevice.length > 0 ? root.live.systemDevice : qsTr("monitor of the default sink")

    // `revision` is read so the binding follows every config refresh.
    function configuredAuto(revision) {
        if (!root.config || revision < 0)
            return true;
        const value = root.config.value("meetings.analysis.auto");
        return value === undefined || value === null ? true : Boolean(value);
    }

    function start() {
        if (root.recording) {
            if (root.router)
                root.router.open("meetings.live");
            return;
        }
        if (!root.live)
            return;
        root.live.start("", root.systemAudio, root.provider ? root.provider.id : "", root.model ? root.model.id : "", root.expectedSpeakers, root.analyzeTouched ? root.analyze : undefined, undefined);
    }

    function pickSelection() {
        if (!root.actions)
            return;
        for (let i = 0; i < root.providers.length; ++i) {
            if (root.providers[i].id === root.actions.selectedProvider)
                root.engineIndex = i;
        }
        for (let j = 0; j < root.models.length; ++j) {
            if (root.models[j].id === root.actions.selectedModel)
                root.modelIndex = j;
        }
    }

    Component.onCompleted: root.pickSelection()

    Connections {
        target: root.actions
        function onProvidersChanged() {
            root.pickSelection();
        }
    }

    FocusFlickable {
        id: scroll
        anchors.top: parent.top
        anchors.bottom: separator.top
        anchors.bottomMargin: Theme.space4
        anchors.left: parent.left
        anchors.right: parent.right
        contentWidth: width
        contentHeight: column.implicitHeight + Theme.space4 * 2

        Column {
            id: column
            anchors.left: parent.left
            anchors.leftMargin: Theme.space4
            anchors.right: parent.right
            anchors.rightMargin: Theme.space4
            anchors.top: parent.top
            anchors.topMargin: Theme.space4
            spacing: Theme.space4

            Text {
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeHeadingSize
                font.weight: Theme.typeEmphasisWeight
                text: qsTr("New meeting")
                Accessible.role: Accessible.Heading
                Accessible.name: qsTr("New meeting")
            }

            SourceRow {
                device: qsTr("%1 · default source").arg(root.micDevice)
                fixed: true
                level: root.live ? root.live.micLevel : 0
                name: qsTr("Microphone")
                peak: root.live ? root.live.micPeak : 0
                width: parent.width
            }

            SourceRow {
                checked: root.systemAudio
                device: qsTr("%1 · follows default sink").arg(root.systemDevice)
                level: root.live ? root.live.systemLevel : 0
                name: qsTr("System audio")
                peak: root.live ? root.live.systemPeak : 0
                width: parent.width
                onToggled: root.systemAudio = !root.systemAudio
            }

            Column {
                spacing: 0
                width: parent.width

                RailFactRow {
                    accessibleName: qsTr("Meeting engine")
                    name: qsTr("engine")
                    value: root.engineLabel
                    onClicked: engineMenu.open()

                    Menu {
                        id: engineMenu

                        Repeater {
                            model: root.choices

                            delegate: MenuItem {
                                id: entry
                                required property var modelData
                                text: entry.modelData.label
                                onTriggered: {
                                    root.engineIndex = entry.modelData.engine;
                                    root.modelIndex = entry.modelData.model;
                                    if (root.actions && root.actions.pickEngine)
                                        root.actions.pickEngine(root.provider ? root.provider.id : "", root.model ? root.model.id : "");
                                }
                            }
                        }
                    }
                }

                RailFactRow {
                    id: speakersRow
                    accessibleName: qsTr("Expected speakers")
                    editable: true
                    name: qsTr("speakers")
                    value: root.expectedSpeakers > 0 ? qsTr("%1 · after stop").arg(root.expectedSpeakers) : qsTr("after stop · count decided")
                    onEdited: text => {
                        const count = parseInt(text, 10);
                        root.expectedSpeakers = isNaN(count) || count < 0 ? 0 : count;
                    }
                }

                RailFactRow {
                    accessibleName: qsTr("Analysis after stop")
                    name: qsTr("analysis")
                    value: root.analyze ? qsTr("summary · after stop") : qsTr("off")
                    onClicked: {
                        root.analyze = !root.analyze;
                        root.analyzeTouched = true;
                    }
                }

                RailFactRow {
                    accessibleName: qsTr("Disclosure state")
                    name: qsTr("disclosure")
                    value: root.acknowledged ? (root.live && root.live.disclosureAt.length > 0 ? qsTr("acknowledged %1").arg(root.live.disclosureAt) : qsTr("acknowledged")) : qsTr("asked on start")
                    onClicked: {
                        if (root.live)
                            root.live.copyDisclosure();
                    }
                }
            }

            Button {
                enabled: root.status ? root.status.daemonConnected : true
                highlighted: true
                icon.name: "meeting"
                text: root.recording ? qsTr("Return to meeting") : qsTr("Start meeting")
                width: parent.width
                onClicked: root.start()
            }

            Text {
                color: Theme.roleFaintText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: qsTr("Recording starts in the background. The bar shows the timer; this window can close.")
                width: parent.width
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }
    }

    Rectangle {
        id: separator
        anchors.bottom: footer.top
        anchors.bottomMargin: Theme.space4
        anchors.left: parent.left
        anchors.leftMargin: Theme.space4
        anchors.right: parent.right
        anchors.rightMargin: Theme.space4
        color: Theme.roleHairline
        height: Theme.hairlineWidth
    }

    Text {
        id: footer
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.pagePaddingY
        anchors.left: parent.left
        anchors.leftMargin: Theme.space4
        anchors.right: parent.right
        anchors.rightMargin: Theme.space4
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        linkColor: Theme.roleAccent
        text: qsTr("[meetings] in config.toml sets these defaults. <a href=\"settings.meetings\">open config.toml</a>")
        textFormat: Text.StyledText
        wrapMode: Text.WordWrap
        onLinkActivated: link => {
            if (root.router)
                root.router.open(link);
        }

        HoverHandler {
            cursorShape: parent.hoveredLink.length > 0 ? Qt.PointingHandCursor : Qt.ArrowCursor
        }

        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                if (root.router)
                    root.router.open("settings.meetings");
                event.accepted = true;
            }
        }

        FocusRing {}

        Accessible.role: Accessible.Link
        Accessible.name: qsTr("Configuration keys")
        Accessible.description: qsTr("speech.meeting_model · meetings.diarization.max_speakers · meetings.diarization.auto · meetings.analysis.auto · meetings.delete_artifact_policy")
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("New meeting rail")
}
