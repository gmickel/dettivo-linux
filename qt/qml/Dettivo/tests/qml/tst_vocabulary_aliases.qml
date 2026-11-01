pragma ComponentBehavior: Bound
import QtQuick
import QtTest
import Dettivo

// The vocabulary editor's per-entry aliases (fn-83, ADR 0086).
TestCase {
    id: root
    name: "VocabularyAliases"
    width: 1080
    height: 820
    visible: true
    when: windowShown

    function named(item, name) {
        if (item.Accessible && item.Accessible.name === name)
            return item;
        for (const child of item.children) {
            const found = named(child, name);
            if (found)
                return found;
        }
        return null;
    }

    function test_vocabulary_aliases_show_and_edit_per_entry() {
        const settings = createTemporaryObject(settingsC, root);
        settings.values["dictation.vocabulary"] = ["Dettivo",
            {
                "term": "Clef",
                "sounds_like": ["clay", "cleff"]
            }
        ];
        const vocabulary = createTemporaryObject(vocabularyC, root, {
            settings: settings,
            width: 1080,
            height: 820
        });
        waitForRendering(vocabulary);
        compare(named(vocabulary, "Sounds like for Clef").text, "clay, cleff");
        const plain = named(vocabulary, "Sounds like for Dettivo");
        plain.text = "debt evo, detivo ";
        plain.accepted();
        compare(settings.value("dictation.vocabulary"), [
            {
                "term": "Dettivo",
                "sounds_like": ["debt evo", "detivo"]
            },
            {
                "term": "Clef",
                "sounds_like": ["clay", "cleff"]
            }
        ]);
        waitForRendering(vocabulary);
        const clef = named(vocabulary, "Sounds like for Clef");
        clef.text = " ";
        clef.accepted();
        compare(settings.value("dictation.vocabulary")[1], "Clef");
        named(vocabulary, "Vocabulary term").text = "clef";
        compare(named(vocabulary, "Add term").enabled, false, "a term differing only in case is a duplicate");
        mouseClick(named(vocabulary, "Remove vocabulary term Clef"));
        compare(settings.value("dictation.vocabulary"), [
            {
                "term": "Dettivo",
                "sounds_like": ["debt evo", "detivo"]
            }
        ]);
    }

    function vocabularyWith(entries, deferred) {
        const settings = createTemporaryObject(settingsC, root, {
            deferred: deferred
        });
        settings.values["dictation.vocabulary"] = entries;
        const vocabulary = createTemporaryObject(vocabularyC, root, {
            settings: settings,
            width: 1080,
            height: 820
        });
        waitForRendering(vocabulary);
        return [settings, vocabulary];
    }

    // The click on Remove must remove, not commit the alias being edited.
    function test_remove_during_an_alias_edit_removes_the_term() {
        const [settings, vocabulary] = vocabularyWith(["Dettivo", "Clef"], true);
        const field = named(vocabulary, "Sounds like for Clef");
        mouseClick(field);
        verify(field.activeFocus);
        field.text = "clay";
        mouseClick(named(vocabulary, "Remove vocabulary term Clef"));
        verify(settings.queued !== null, "the click wrote the list");
        compare(settings.queued, ["Dettivo"]);
    }

    // A change from another client keeps the field being edited.
    function test_an_outside_change_keeps_the_alias_edit_in_progress() {
        const [settings, vocabulary] = vocabularyWith(["Dettivo", "Clef"], false);
        const field = named(vocabulary, "Sounds like for Dettivo");
        mouseClick(field);
        field.text = "detivo";
        settings.values = {
            "dictation.vocabulary": ["Dettivo",
                {
                    "term": "Clef",
                    "sounds_like": ["clay"]
                },
                "Omarchy"]
        };
        settings.revision++;
        waitForRendering(vocabulary);
        compare(named(vocabulary, "Sounds like for Dettivo"), field, "the editor is kept");
        verify(field.activeFocus);
        compare(field.text, "detivo");
        compare(named(vocabulary, "Sounds like for Clef").text, "clay", "a row not being edited follows");
        verify(named(vocabulary, "Sounds like for Omarchy") !== null);
    }

    Component {
        id: vocabularyC
        VocabularySection {}
    }
    Component {
        id: settingsC
        QtObject {
            property int revision: 0
            property string notice: ""
            property bool registryLoaded: true
            property var values: ({})
            // A deferred write stays queued and pending, as a daemon round trip does.
            property bool deferred: false
            property var queued: null
            signal errorChanged(string key)
            function value(key) {
                return values[key] === undefined ? "" : values[key];
            }
            function lockedBy(key) {
                return "";
            }
            function pending(key) {
                return queued !== null;
            }
            function error(key) {
                return "";
            }
            function writesBlock(section) {
                return "";
            }
            function defaultText(key) {
                return "[]";
            }
            function set(key, value) {
                if (deferred) {
                    queued = value;
                    revision++;
                    return;
                }
                const next = Object.assign({}, values);
                next[key] = value;
                values = next;
                revision++;
            }
        }
    }
}
