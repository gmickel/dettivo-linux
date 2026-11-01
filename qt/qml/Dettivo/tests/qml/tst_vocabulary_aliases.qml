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
            signal errorChanged(string key)
            function value(key) {
                return values[key] === undefined ? "" : values[key];
            }
            function lockedBy(key) {
                return "";
            }
            function pending(key) {
                return false;
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
                const next = Object.assign({}, values);
                next[key] = value;
                values = next;
                revision++;
            }
        }
    }
}
