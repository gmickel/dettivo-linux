pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// Models (first-run-2-models.png): the speech models first run offers as
// a radio list with size and one-line reason, the download's progress
// hairline with the file and the byte count under it, the optional
// Enhanced block (the local model or Ollama), and Continue once the
// selected speech model is ready.
Item {
    id: root

    property var firstRun: null

    readonly property var models: root.firstRun ? root.firstRun.models : null
    readonly property bool ready: root.models ? root.models.ready : false
    readonly property bool downloading: root.models ? root.models.downloading : false
    readonly property string enhanced: root.models ? root.models.enhanced : "none"

    PageHeader {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        subtitleMaxWidth: Theme.space8 * 14
        subtitle: qsTr("One speech model is required. Models are downloaded to %1 and verified. %2").arg(root.models ? root.models.modelsDir : "").arg(root.models ? root.models.tierLine : "")
        title: qsTr("Models")
    }

    SectionLabel {
        id: speechLabel
        anchors.left: parent.left
        anchors.top: header.bottom
        anchors.topMargin: Theme.space7 + Theme.space1
        leftPadding: 0
        text: qsTr("Speech")
    }

    Column {
        id: speechList
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: speechLabel.bottom
        anchors.topMargin: Theme.space3
        spacing: -Theme.hairlineWidth

        Repeater {
            model: root.models

            delegate: ModelRow {
                id: row
                required property var model
                accentState: row.model.state.endsWith("%") || row.model.ready
                accessibleName: row.model.name
                detail: row.model.detail
                name: row.model.name
                recommended: row.model.recommended
                selected: row.model.selected
                size: row.model.size
                stateText: row.model.state
                width: speechList.width
                onActivated: {
                    if (root.models)
                        root.models.select(row.model.provider, row.model.modelId);
                }
            }
        }

        Accessible.role: Accessible.List
        Accessible.name: qsTr("Speech models")
    }

    Rectangle {
        id: progressTrack
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: speechList.bottom
        anchors.topMargin: Theme.space5
        color: Theme.roleHairline
        height: Theme.progressHairlineWidth

        Rectangle {
            anchors.left: parent.left
            anchors.top: parent.top
            color: Theme.roleAccent
            height: parent.height
            width: parent.width * (root.models ? root.models.downloadFraction : 0)
        }

        Accessible.role: Accessible.ProgressBar
        Accessible.name: qsTr("Download progress")
    }

    Text {
        anchors.left: parent.left
        anchors.top: progressTrack.bottom
        anchors.topMargin: Theme.space3
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.downloading && root.models ? root.models.downloadFile : (root.ready ? qsTr("Selected model ready") : qsTr("Pick a model to start its download"))
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Text {
        anchors.right: parent.right
        anchors.top: progressTrack.bottom
        anchors.topMargin: Theme.space3
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.features: Theme.typeTabularNumerals
        font.pixelSize: Theme.typeCaptionSize
        text: root.models ? root.models.downloadLine : ""
        Accessible.role: Accessible.StaticText
        Accessible.name: text.length > 0 ? text : qsTr("No download running")
    }

    SectionLabel {
        id: enhancedLabel
        anchors.left: parent.left
        anchors.top: progressTrack.bottom
        anchors.topMargin: Theme.space8 + Theme.space2
        leftPadding: 0
        text: qsTr("Enhanced · optional")
    }

    Column {
        id: enhancedList
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: enhancedLabel.bottom
        anchors.topMargin: Theme.space3
        spacing: -Theme.hairlineWidth

        ModelRow {
            accentState: false
            accessibleName: root.models ? root.models.localTitle : qsTr("Local language model")
            checkable: true
            detail: root.models ? root.models.localLine : ""
            name: root.models ? root.models.localTitle : ""
            selected: root.enhanced === "local"
            size: root.models ? root.models.localSize : ""
            stateText: root.models ? root.models.localState : ""
            width: enhancedList.width
            onActivated: {
                if (root.models)
                    root.models.setEnhanced(root.enhanced === "local" ? "none" : "local");
            }
        }

        ModelRow {
            accentState: root.models ? root.models.ollamaAvailable : false
            accessibleName: qsTr("Use Ollama instead")
            checkable: true
            detail: root.models ? root.models.ollamaLine : ""
            name: qsTr("Use Ollama instead")
            selected: root.enhanced === "ollama"
            stateText: root.models && root.models.ollamaAvailable ? qsTr("running") : ""
            width: enhancedList.width
            onActivated: {
                if (root.models)
                    root.models.setEnhanced(root.enhanced === "ollama" ? "none" : "ollama");
            }
        }

        Accessible.role: Accessible.List
        Accessible.name: qsTr("Enhanced models")
    }

    FirstRunFooter {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        note: qsTr("Writes [speech] and [llm] to config.toml. Change any of this later in Settings or the file.")
        primary: root.ready ? qsTr("Continue") : qsTr("Continue when ready")
        primaryEnabled: root.ready
        secondary: qsTr("Back")
        onPrimaryTriggered: {
            if (root.firstRun)
                root.firstRun.next();
        }
        onSecondaryTriggered: {
            if (root.firstRun)
                root.firstRun.back();
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Models")
}
