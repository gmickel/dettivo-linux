pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

SettingsPage {
    id: root
    section: "vocabulary"
    subtitle: qsTr("Add names, products and jargon to help recognition. Each entry is one term or phrase, with the spellings it tends to be misheard as.")
    // An entry is a term, or a { term, sounds_like } table (ADR 0086).
    readonly property var entries: root.settings && root.revision >= 0 ? root.settings.value("dictation.vocabulary") || [] : []
    readonly property var terms: root.entries.map(entry => root.termOf(entry))
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
    function termOf(entry) {
        return typeof entry === "string" ? entry : entry.term;
    }
    function aliasesOf(entry) {
        return typeof entry === "string" ? [] : entry.sounds_like || [];
    }
    // The daemon drops a term that differs from an earlier one only in case.
    function known(term) {
        return root.terms.some(t => t.toLowerCase() === term.toLowerCase());
    }
    function save(entries) {
        if (root.settings && !root.pending && !root.lockedBy.length)
            root.settings.set("dictation.vocabulary", entries);
    }
    function addTerm() {
        const term = entry.text.trim();
        if (!root.pending && term.length > 0 && !root.known(term)) {
            root.pendingTerm = term;
            root.save(root.entries.concat([term]));
        }
    }
    // A comma-separated list of spellings; none writes the plain term.
    function setAliases(index, text) {
        const term = root.termOf(root.entries[index]);
        const aliases = text.split(",").map(a => a.trim()).filter(a => a.length > 0);
        if (aliases.join(", ") === root.aliasesOf(root.entries[index]).join(", "))
            return;
        const next = aliases.length ? {
            term: term,
            sounds_like: aliases
        } : term;
        root.save(root.entries.map((e, i) => i === index ? next : e));
    }
    // One row per entry, updated in place so an editor and its draft
    // survive a write from this page or another client.
    ListModel {
        id: rows
    }
    onEntriesChanged: root.sync()
    Component.onCompleted: root.sync()
    function sync() {
        let i = 0;
        for (; i < root.entries.length; i++) {
            const term = root.termOf(root.entries[i]);
            const aliases = root.aliasesOf(root.entries[i]).join(", ");
            let j = i;
            while (j < rows.count && rows.get(j).term !== term)
                j++;
            if (j === rows.count) {
                rows.insert(i, {
                    term: term,
                    aliases: aliases
                });
                continue;
            }
            if (j > i)
                rows.remove(i, j - i);
            if (rows.get(i).aliases !== aliases)
                rows.setProperty(i, "aliases", aliases);
        }
        if (rows.count > i)
            rows.remove(i, rows.count - i);
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
            // A click leaves an alias field focused, so its blur save
            // cannot mark the list pending under the click.
            focusPolicy: Qt.TabFocus
            enabled: !root.pending && root.lockedBy.length === 0 && entry.text.trim().length > 0 && !root.known(entry.text.trim())
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
        model: rows
        delegate: Item {
            id: termRow
            required property string term
            required property string aliases
            required property int index
            // A field being edited keeps its draft; the others follow.
            onAliasesChanged: {
                if (!aliasField.activeFocus)
                    aliasField.text = termRow.aliases;
            }
            width: parent.width
            height: Theme.settingsRowHeight
            Text {
                anchors.left: parent.left
                anchors.right: aliasField.left
                anchors.rightMargin: Theme.space3
                anchors.verticalCenter: parent.verticalCenter
                text: termRow.term
                elide: Text.ElideRight
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
            TextField {
                id: aliasField
                anchors.right: remove.left
                anchors.rightMargin: Theme.space3
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.settingsControlWidth
                Component.onCompleted: text = termRow.aliases
                placeholderText: qsTr("Sounds like")
                enabled: !root.pending && root.lockedBy.length === 0
                Accessible.name: qsTr("Sounds like for %1").arg(termRow.term)
                Accessible.description: qsTr("Spellings this term tends to be transcribed as, separated by commas.")
                onAccepted: root.setAliases(termRow.index, text)
                onActiveFocusChanged: {
                    if (!activeFocus)
                        root.setAliases(termRow.index, text);
                }
            }
            Button {
                id: remove
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Remove")
                focusPolicy: Qt.TabFocus
                enabled: !root.pending && root.lockedBy.length === 0
                Accessible.name: qsTr("Remove vocabulary term %1").arg(termRow.term)
                onClicked: root.save(root.entries.filter((e, index) => index !== termRow.index))
            }
        }
    }
    Accessible.role: Accessible.Pane
    Accessible.name: root.title
}
