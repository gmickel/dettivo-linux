pragma ComponentBehavior: Bound
import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "ReleaseSettings"
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
    function cleanup() {
        SettingsUi.advanced = false;
    }

    function test_simple_advanced_reveals_without_writes_and_reset_respects_locks() {
        const settings = createTemporaryObject(settingsC, root);
        const page = createTemporaryObject(generalC, root, {
            settings: settings,
            width: 1080,
            height: 820
        });
        waitForRendering(page);
        const path = page.rowFor(page, "paths.data_dir");
        compare(path.visible, false);
        const language = page.rowFor(page, "dictation.language");
        compare(language.kind, "choice");
        compare(language.choices.length, 101);
        compare(language.choiceLabels[language.choices.indexOf("de")], "German");
        SettingsUi.advanced = true;
        compare(path.visible, true);
        compare(settings.writes, 0);
        compare(settings.value("paths.data_dir"), "/custom/path");
        const reset = named(path, "Reset paths.data_dir to default");
        verify(reset.enabled);
        page.scrollToKey("paths.data_dir");
        waitForRendering(page);
        mouseClick(reset);
        compare(settings.lastUnset, "paths.data_dir");
        settings.locked = true;
        settings.revision++;
        compare(reset.enabled, false);
        compare(path.controlItem.enabled, false);
        SettingsUi.advanced = false;
        compare(path.visible, false);
        compare(settings.writes, 1);
        page.visible = false;
        const cold = createTemporaryObject(generalC, root, {
            settings: settings,
            width: 1080,
            height: 360
        });
        waitForRendering(cold);
        cold.scrollToKey("omarchy.history_items");
        waitForRendering(cold);
        const last = cold.rowFor(cold, "omarchy.history_items");
        verify(last.mapToItem(cold, 0, 0).y >= 0);
        verify(last.mapToItem(cold, 0, last.height).y <= cold.height, "A deep link reveals a previously hidden advanced row");
    }

    function test_vocabulary_preserves_whole_terms_and_transform_custom_values() {
        const settings = createTemporaryObject(settingsC, root);
        const vocabulary = createTemporaryObject(vocabularyC, root, {
            settings: settings,
            width: 1080,
            height: 820
        });
        waitForRendering(vocabulary);
        const entry = named(vocabulary, "Vocabulary term");
        entry.text = "Mickel, Tech";
        mouseClick(named(vocabulary, "Add term"));
        compare(settings.value("dictation.vocabulary"), ["Dettivo", "Mickel, Tech"]);
        compare(named(vocabulary, "Add term").enabled, false);
        compare(entry.text, "");
        waitForRendering(vocabulary);
        mouseClick(named(vocabulary, "Remove vocabulary term Dettivo"));
        compare(settings.value("dictation.vocabulary"), ["Mickel, Tech"]);
        settings.reject = true;
        entry.text = "Keep on refusal";
        mouseClick(named(vocabulary, "Add term"));
        compare(entry.text, "Keep on refusal");
        settings.reject = false;
        settings.holdWrites = true;
        entry.text = "Queued term";
        mouseClick(named(vocabulary, "Add term"));
        entry.text = "Newer typing";
        settings.flush();
        compare(entry.text, "Newer typing");
        vocabulary.visible = false;
        const polish = createTemporaryObject(polishC, root, {
            settings: settings,
            width: 1080,
            height: 820
        });
        waitForRendering(polish);
        const fillers = named(polish, "Remove fillers");
        mouseClick(fillers);
        compare(settings.value("polish.transforms"), ["customTransform", "fixGrammar", "removeFillers"]);
        mouseClick(fillers);
        compare(settings.value("polish.transforms"), ["customTransform", "fixGrammar"]);
        compare(polish.rowFor(polish, "llm.polish_experiment").visible, false);
        SettingsUi.advanced = true;
        compare(polish.rowFor(polish, "llm.polish_experiment").visible, true);
    }

    function test_list_writes_lock_until_refresh_and_recover_after_refusal() {
        const settings = createTemporaryObject(settingsC, root);
        settings.holdWrites = true;
        const vocabulary = createTemporaryObject(vocabularyC, root, {
            settings: settings,
            width: 1080,
            height: 820
        });
        waitForRendering(vocabulary);
        const entry = named(vocabulary, "Vocabulary term");
        const add = named(vocabulary, "Add term");
        const remove = named(vocabulary, "Remove vocabulary term Dettivo");
        const reset = named(vocabulary, "Reset dictation.vocabulary to default");
        entry.text = "First";
        mouseClick(add);
        verify(!add.enabled && !remove.enabled && !reset.enabled);
        entry.text = "Second";
        mouseClick(add);
        mouseClick(remove);
        mouseClick(reset);
        compare(settings.deferred.value, ["Dettivo", "First"]);
        settings.flush();
        compare(entry.text, "Second");
        mouseClick(add);
        compare(settings.value("dictation.vocabulary"), ["Dettivo", "First", "Second"]);
        settings.holdWrites = true;
        entry.text = "Retry me";
        mouseClick(add);
        settings.reject = true;
        settings.flush();
        compare(entry.text, "Retry me");
        verify(add.enabled && named(vocabulary, "Remove vocabulary term Dettivo").enabled && reset.enabled);
        settings.reject = false;
        mouseClick(add);
        compare(settings.value("dictation.vocabulary"), ["Dettivo", "First", "Second", "Retry me"]);
        vocabulary.visible = false;
        const polish = createTemporaryObject(polishC, root, {
            settings: settings,
            width: 1080,
            height: 820
        });
        waitForRendering(polish);
        const fillers = named(polish, "Remove fillers");
        const punctuation = named(polish, "Smart punctuation");
        settings.holdWrites = true;
        mouseClick(fillers);
        verify(!fillers.enabled && !punctuation.enabled);
        verify(!named(polish, "Reset polish.transforms to default").enabled);
        mouseClick(punctuation);
        compare(settings.deferred.value, ["customTransform", "fixGrammar", "removeFillers"]);
        settings.flush();
        verify(fillers.checked);
        mouseClick(punctuation);
        compare(settings.value("polish.transforms"), ["customTransform", "fixGrammar", "removeFillers", "smartPunctuation"]);
        settings.holdWrites = true;
        mouseClick(fillers);
        settings.reject = true;
        settings.flush();
        verify(fillers.enabled && fillers.checked);
        settings.reject = false;
        mouseClick(fillers);
        verify(!fillers.checked);
    }

    Component {
        id: generalC
        GeneralSection {}
    }
    Component {
        id: vocabularyC
        VocabularySection {}
    }
    Component {
        id: polishC
        PolishSection {}
    }
    Component {
        id: settingsC
        QtObject {
            property int revision: 0
            property string notice: ""
            property bool registryLoaded: true
            property bool locked: false
            property bool reject: false
            property bool holdWrites: false
            property var deferred: null
            function pending(key) {
                return deferred !== null && deferred.key === key;
            }
            function flush() {
                holdWrites = false;
                const write = deferred;
                deferred = null;
                set(write.key, write.value);
                revision++;
            }
            property int writes: 0
            property string lastUnset: ""
            property var values: ({
                    "paths.data_dir": "/custom/path",
                    "dictation.language": "de",
                    "dictation.vocabulary": ["Dettivo"],
                    "polish.transforms": ["customTransform", "fixGrammar"]
                })
            signal errorChanged(string key)
            function value(key) {
                return values[key] === undefined ? "" : values[key];
            }
            function text(key) {
                return String(value(key));
            }
            function lockedBy(key) {
                return locked ? "DETTIVO_DATA_DIR" : "";
            }
            function error(key) {
                return reject ? "Write refused" : "";
            }
            function writesBlock(section) {
                return "";
            }
            function defaultText(key) {
                return '""';
            }
            function set(key, value) {
                if (reject) {
                    errorChanged(key);
                    return;
                }
                if (holdWrites) {
                    deferred = {
                        key: key,
                        value: value
                    };
                    revision++;
                    return;
                }
                const next = Object.assign({}, values);
                next[key] = value;
                values = next;
                writes++;
                revision++;
            }
            function unset(key) {
                lastUnset = key;
                set(key, "");
            }
        }
    }
}
