pragma ComponentBehavior: Bound
import Dettivo
import QtQuick
import QtTest

TestCase {
    id: root
    name: "SettingsLayout"
    width: 880
    height: 820
    visible: true
    when: windowShown

    function cleanup() {
        SettingsUi.advanced = false;
    }

    function test_model_choices_preserve_unknown_and_inherited_values() {
        const page = createTemporaryObject(pageC, root, {
            width: 880,
            height: 820
        });
        waitForRendering(page);
        for (const key of ["speech.model", "speech.meeting_model"]) {
            const row = page.rowFor(page, key);
            verify(row);
            const text = row.controlItem.controlItem.contentItem;
            verify(text.contentWidth <= text.width, key + " label must fit");
        }
        const meeting = page.rowFor(page, "speech.meeting_model");
        compare(meeting.choices, ["", "tiny.en"]);
        const idle = page.rowFor(page, "engines.stt_idle_seconds");
        compare(idle.visible, false);
        SettingsUi.advanced = true;
        compare(idle.visible, true);
        compare(idle.valueText, "300");
        idle.controlItem.committed("600");
        compare(fakeSettings.lastKey, "engines.stt_idle_seconds");
        compare(fakeSettings.lastValue, "600");
        page.width = 480;
        page.scrollToKey("engines.stt_idle_seconds");
        waitForRendering(page);
        const field = idle.controlItem;
        verify(field.mapToItem(page, 0, 0).x >= 0);
        verify(field.mapToItem(page, field.width, 0).x <= page.width);
        const choice = createTemporaryObject(choiceC, root, {
            choices: ["", "known"],
            choiceLabels: ["Use dictation model", "Known"],
            current: "custom",
            segmentLimit: 0
        });
        compare(choice.effectiveChoices, ["", "known", "custom"]);
        compare(choice.currentIndex, 2);
        choice.choices = [];
        compare(choice.effectiveChoices, ["custom"]);
        compare(choice.current, "custom");
        choice.current = "";
        compare(choice.labelAt(0), "Not selected");
    }

    Component {
        id: choiceC
        SettingChoice {}
    }

    QtObject {
        id: fakeSettings
        property int revision: 0
        property bool registryLoaded: false
        property string notice: ""
        property string lastKey: ""
        property string lastValue: ""
        function text(key) {
            return ({
                    "speech.provider": "parakeet",
                    "speech.model": "v3",
                    "engines.stt_idle_seconds": "300"
                })[key] || "";
        }
        function writesBlock(section) {
            return "";
        }
        function value(key) {
            return text(key);
        }
        function lockedBy(key) {
            return "";
        }
        function error(key) {
            return "";
        }
        signal errorChanged(string key)
        function set(key, value) {
            lastKey = key;
            lastValue = value;
        }
    }
    ListModel {
        id: fakeTable
        property var speechChoices: [
            {
                key: "parakeet/v3",
                label: "Parakeet TDT v3"
            },
            {
                key: "whisper/tiny.en",
                label: "Whisper Tiny English"
            }
        ]
        property var llmChoices: [
            {
                key: "qwen",
                label: "Qwen3 4B Instruct"
            }
        ]
        property string lastRefusal: ""
        property string headline: "Models"
    }
    Component {
        id: pageC
        ModelsSection {
            settings: fakeSettings
            table: fakeTable
        }
    }
}
