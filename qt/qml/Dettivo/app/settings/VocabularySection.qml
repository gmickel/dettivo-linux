pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

SettingsPage {
    id: root
    section: "vocabulary"
    subtitle: qsTr("Add names, products and jargon to help recognition. Each entry is one term or phrase.")
    readonly property var terms: root.settings && root.revision >= 0 ? root.settings.value("dictation.vocabulary") || [] : []
    readonly property string lockedBy: root.settings && root.revision >= 0 ? root.settings.lockedBy("dictation.vocabulary") : ""
    readonly property bool pending: settings && revision >= 0 ? settings.pending("dictation.vocabulary") : false
    property int errorRevision: 0
    property string pendingTerm: ""
    onTermsChanged: {
        if (pendingTerm.length > 0 && terms.indexOf(pendingTerm) >= 0) {
            if (entry.text.trim() === pendingTerm)
                entry.clear();
            pendingTerm = "";
        }
    }
    function save(terms) {
        if (root.settings && !root.pending && !root.lockedBy.length)
            root.settings.set("dictation.vocabulary", terms);
    }
    function addTerm() {
        const term = entry.text.trim();
        if (!root.pending && term.length > 0 && root.terms.indexOf(term) < 0) {
            root.pendingTerm = term;
            root.save(root.terms.concat([term]));
        }
    }
    Connections {
        target: root.settings
        function onErrorChanged(key) {
            if (key === "dictation.vocabulary") {
                root.errorRevision++;
                if (root.settings.error(key).length)
                    root.pendingTerm = "";
            }
        }
    }
    Row {
        spacing: Theme.space3
        TextField {
            id: entry
            property string key: "dictation.vocabulary"
            width: Theme.settingsControlWidth
            placeholderText: qsTr("A name or phrase")
            enabled: root.lockedBy.length === 0
            Accessible.name: qsTr("Vocabulary term")
            onAccepted: root.addTerm()
        }
        Button {
            text: qsTr("Add term")
            enabled: !root.pending && root.lockedBy.length === 0 && entry.text.trim().length > 0 && root.terms.indexOf(entry.text.trim()) < 0
            onClicked: root.addTerm()
        }
        SettingReset {
            settings: root.settings
            key: "dictation.vocabulary"
            busy: root.pending
        }
    }
    Text {
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        padding: Theme.space3
        text: root.lockedBy.length ? qsTr("Set by %1").arg(root.lockedBy) : (root.terms.length ? qsTr("%1 saved terms").arg(root.terms.length) : qsTr("No custom terms yet."))
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
    Text {
        color: Theme.roleUrgent
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeCaptionSize
        text: root.settings && root.errorRevision >= 0 ? root.settings.error("dictation.vocabulary") : ""
        visible: text.length > 0
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
    Repeater {
        model: root.terms
        delegate: Item {
            id: termRow
            required property string modelData
            required property int index
            width: parent.width
            height: Theme.settingsRowHeight
            Text {
                anchors.left: parent.left
                anchors.right: remove.left
                anchors.rightMargin: Theme.space3
                anchors.verticalCenter: parent.verticalCenter
                text: termRow.modelData
                elide: Text.ElideRight
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
            Button {
                id: remove
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Remove")
                enabled: !root.pending && root.lockedBy.length === 0
                Accessible.name: qsTr("Remove vocabulary term %1").arg(termRow.modelData)
                onClicked: root.save(root.terms.filter((term, index) => index !== termRow.index))
            }
        }
    }
    Accessible.role: Accessible.Pane
    Accessible.name: root.title
}
