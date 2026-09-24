pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// Re-run (designed inline against the settings pattern): the engine and
// the model from the catalogue, the mode pinned to Raw with the reason
// (ADR 0023: a re-run runs the raw layer), one primary action. A refusal
// from the daemon lands under the pickers, named.
Popup {
    id: root

    property var actions: null
    property var detail: null
    property string itemId: ""
    property string error: ""

    readonly property var providers: root.actions ? root.actions.providers : []
    readonly property var models: {
        const provider = root.providers[engine.currentIndex];
        return provider && provider.models ? provider.models : [];
    }

    function openFor(id) {
        root.itemId = id;
        root.error = "";
        if (root.actions)
            root.actions.loadProviders();
        root.open();
        root.pickSelection();
    }

    function pickSelection() {
        if (!root.actions)
            return;
        const provider = root.actions.selectedProvider;
        for (let i = 0; i < root.providers.length; ++i) {
            if (root.providers[i].id === provider)
                engine.currentIndex = i;
        }
        const model = root.actions.selectedModel;
        for (let j = 0; j < root.models.length; ++j) {
            if (root.models[j].id === model)
                modelBox.currentIndex = j;
        }
    }

    function start() {
        const provider = root.providers[engine.currentIndex];
        const model = root.models[modelBox.currentIndex];
        root.error = "";
        if (root.actions)
            root.actions.rerun(root.itemId, provider ? provider.id : "", model ? model.id : "");
    }

    modal: true
    padding: Theme.space5
    width: Theme.space8 * 8

    Connections {
        target: root.actions
        function onProvidersChanged() {
            root.pickSelection();
        }
        function onRerunStarted(itemId, newItemId, jobId) {
            if (itemId === root.itemId)
                root.close();
        }
        function onFailed(action, reason) {
            if (action === "rerun")
                root.error = reason;
        }
    }

    Column {
        width: root.availableWidth
        spacing: Theme.space4

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Re-run dialog")

        Text {
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeHeadingSize
            font.weight: Theme.typeEmphasisWeight
            text: qsTr("Re-run this dictation")
            Accessible.role: Accessible.Heading
            Accessible.name: qsTr("Re-run this dictation")
        }

        Text {
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: qsTr("The retained take is transcribed again into a new item linked to this one.")
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        SectionLabel {
            leftPadding: 0
            text: qsTr("Engine")
        }

        ComboBox {
            id: engine
            model: root.providers
            textRole: "label"
            width: parent.width
            onActivated: modelBox.currentIndex = 0
            Accessible.role: Accessible.ComboBox
            Accessible.name: qsTr("Engine")
        }

        SectionLabel {
            leftPadding: 0
            text: qsTr("Model")
        }

        ComboBox {
            id: modelBox
            model: root.models
            textRole: "label"
            width: parent.width
            Accessible.role: Accessible.ComboBox
            Accessible.name: qsTr("Model")
        }

        SectionLabel {
            leftPadding: 0
            text: qsTr("Mode")
        }

        Text {
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: qsTr("Raw. A re-run runs the raw layer; Polish and Enhanced arrive with the import spec.")
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Mode")
        }

        Text {
            color: Theme.roleUrgent
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeBodySize
            text: root.error
            visible: root.error.length > 0
            width: parent.width
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Re-run refused")
        }

        Row {
            anchors.right: parent.right
            spacing: Theme.space2

            Button {
                text: qsTr("Cancel")
                onClicked: root.close()
            }

            Button {
                enabled: root.actions !== null && !root.actions.busy && root.models.length > 0
                highlighted: true
                text: qsTr("Start re-run")
                onClicked: root.start()
            }
        }
    }
}
