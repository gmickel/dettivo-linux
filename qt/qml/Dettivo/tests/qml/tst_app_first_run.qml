pragma ComponentBehavior: Bound
import Dettivo
import QtQuick
import QtTest

// First run (fn-21, ADR 0024): the frame marks the step, Keys shows the
// bindings, the snippet and the live confirmation and Continue writes,
// Models lists the rows with the selected one marked and holds Continue
// until a model is ready, Try it names the field and the result, and the
// footer's buttons drive the model.
TestCase {
    id: root

    function findByName(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;

            const nested = findByName(child, name);
            if (nested)
                return nested;
        }
        return null;
    }

    function test_keys_shows_bindings_snippet_and_the_press() {
        const firstRun = createTemporaryObject(firstRunC, root);
        const page = createTemporaryObject(routeC, root, {
            "firstRun": firstRun,
            "width": 1280,
            "height": 820
        });
        waitForRendering(page);
        compare(page.Accessible.name, "First run");
        const marker = findByName(page, "Step 1 of 3");
        verify(marker);
        compare(marker.height, Theme.typeBodySize + Theme.space3);
        verify(findByName(page, "Keys"));
        verify(findByName(page, "Hold to talk: F9"));
        verify(findByName(page, "Toggle dictation: Super+Ctrl+X"));
        verify(findByName(page, "Bindings file"));
        verify(findByName(page, "bind = , F9, exec, dettivo --quiet dictation start"));
        verify(findByName(page, "Waiting for the key."));
        firstRun.pressed = true;
        firstRun.pressLine = "F9 held · listening · Arctis Nova";
        verify(findByName(page, "F9 held · listening · Arctis Nova"));
        findByName(page, "Continue").clicked();
        compare(firstRun.nexts, 1);
        findByName(page, "Skip").clicked();
        compare(firstRun.skips, 1);
        firstRun.snippetSupported = false;
        firstRun.portalLine = "No compositor snippet and no portal here: no portal";
        verify(findByName(page, "Portal bindings"));
    }

    function test_models_lists_rows_and_holds_continue_until_ready() {
        const firstRun = createTemporaryObject(firstRunC, root, {
            "step": "models",
            "stepIndex": 1
        });
        const page = createTemporaryObject(routeC, root, {
            "firstRun": firstRun,
            "width": 1280,
            "height": 820
        });
        waitForRendering(page);
        verify(findByName(page, "Step 2 of 3"));
        verify(findByName(page, "Models"));
        const row = findByName(page, "Parakeet TDT 0.6B v3");
        verify(row);
        compare(row.Accessible.checked, true);
        verify(findByName(page, "Whisper small.en"));
        verify(findByName(page, "62 %"));
        verify(findByName(page, "Download progress"));
        verify(findByName(page, "Use Ollama instead"));
        const cont = findByName(page, "Continue when ready");
        verify(cont);
        compare(cont.enabled, false);
        findByName(page, "Whisper small.en").activated();
        compare(firstRun.models.selections[0], "whisper/small.en");
        firstRun.models.ready = true;
        verify(findByName(page, "Continue").enabled);
        findByName(page, "Back").clicked();
        compare(firstRun.backs, 1);
    }

    function test_try_it_names_the_field_and_the_result_and_done_finishes() {
        const firstRun = createTemporaryObject(firstRunC, root, {
            "step": "try",
            "stepIndex": 2
        });
        const page = createTemporaryObject(routeC, root, {
            "firstRun": firstRun,
            "width": 1280,
            "height": 820
        });
        waitForRendering(page);
        verify(findByName(page, "Step 3 of 3"));
        verify(findByName(page, "Try it"));
        compare(firstRun.allowances[0], true);
        const field = findByName(page, "Try it field");
        verify(field);
        verify(field.activeFocus);
        verify(findByName(page, "inserted via: virtual keyboard"));
        verify(findByName(page, "stop to insert: 0.8 s"));
        verify(findByName(page, "mode: Enhanced · Qwen3 4B"));
        findByName(page, "Open config.toml").clicked();
        compare(firstRun.opens, 1);
        findByName(page, "Done").clicked();
        compare(firstRun.finishes, 1);
    }

    // The label, the explanation and the clipboard claim follow the take's
    // outcome: only `inserted` reads Inserted, a clipboard fallback says
    // the words are on the clipboard, and a failure claims no clipboard.
    function test_try_it_names_the_outcome_not_the_presence_of_a_result() {
        const firstRun = createTemporaryObject(firstRunC, root, {
            "step": "try",
            "stepIndex": 2,
            "dictationState": "idle"
        });
        const page = createTemporaryObject(routeC, root, {
            "firstRun": firstRun,
            "width": 1280,
            "height": 820
        });
        waitForRendering(page);
        verify(findByName(page, "Inserted · Parakeet v3"));
        verify(findByName(page, "That works in any app that takes keyboard input. Terminals get Ctrl+Shift+V pastes when a backend needs the clipboard."));

        firstRun.resultOutcome = "copied_to_clipboard";
        firstRun.insertedVia = "clipboard";
        firstRun.resultReason = "no keystroke backend";
        verify(findByName(page, "On the clipboard · Parakeet v3"));
        verify(findByName(page, "Not typed: no keystroke backend. The words are on the clipboard, paste them into the field."));
        verify(findByName(page, "inserted via: clipboard"));

        firstRun.resultOutcome = "failed";
        firstRun.insertedVia = "not inserted";
        firstRun.resultReason = "target_is_self";
        verify(findByName(page, "Not inserted · Parakeet v3"));
        verify(findByName(page, "Not inserted: target_is_self. The take is kept in History."));
        verify(!findByName(page, "Not typed: target_is_self. The words are on the clipboard, paste them into the field."));

        firstRun.resultKnown = false;
        firstRun.resultOutcome = "";
        verify(findByName(page, "Idle · Parakeet v3"));
    }

    Component {
        id: routeC

        OnboardingRoute {}
    }

    Component {
        id: modelsC

        ListModel {
            property bool ready: false
            property string modelsDir: "~/.local/share/dettivo/models"
            property string tierLine: "Vulkan will run them."
            property bool downloading: true
            property real downloadFraction: 0.62
            property string downloadFile: "parakeet-v3 · q8_0"
            property string downloadLine: "397 of 640 MB · 41 MB/s · sha256 pending"
            property string enhanced: "local"
            property string localTitle: "Qwen3 4B Instruct, local"
            property string localLine: "Rewrites dictation in Enhanced mode."
            property string localSize: "2.5 GB"
            property string localState: "soon"
            property string ollamaLine: "Not running on localhost:11434."
            property bool ollamaAvailable: false
            property var selections: []

            function select(provider, id) {
                selections.push(provider + "/" + id);
            }

            function setEnhanced(choice) {
                enhanced = choice;
            }

            ListElement {
                provider: "parakeet"
                modelId: "parakeet-v3"
                name: "Parakeet TDT 0.6B v3"
                detail: "25 European languages · word timestamps · fastest on GPU"
                size: "640 MB"
                state: "62 %"
                recommended: true
                selected: true
                ready: false
            }

            ListElement {
                provider: "whisper"
                modelId: "small.en"
                name: "Whisper small.en"
                detail: "English · light enough for CPU-only laptops"
                size: "466 MB"
                state: "get"
                recommended: false
                selected: false
                ready: false
            }
        }
    }

    Component {
        id: firstRunC

        QtObject {
            property bool decided: true
            property bool required: true
            property string step: "keys"
            property int stepIndex: 0
            property string compositor: "Hyprland"
            property bool snippetSupported: true
            property string snippetText: "bind = , F9, exec, dettivo --quiet dictation start\n"
            property string snippetPath: "~/.config/hypr/dettivo.conf"
            property string includeLine: "source = ~/.config/hypr/dettivo.conf"
            property string mainConfigPath: "~/.config/hypr/hyprland.conf"
            property bool snippetWritten: false
            property bool snippetSourced: false
            property string portalLine: ""
            property string keysError: ""
            property var bindings: [
                {
                    "action": "Hold to talk",
                    "keys": ["F9"],
                    "hint": "press starts, release inserts",
                    "runs": "dettivo dictation start / stop"
                },
                {
                    "action": "Toggle dictation",
                    "keys": ["Super", "Ctrl", "X"],
                    "hint": "",
                    "runs": "dettivo dictation toggle"
                }
            ]
            property string holdKey: "F9"
            property bool pressed: false
            property string pressLine: ""
            property string configPath: "~/.config/dettivo/config.toml"
            property var models: root.createTemporaryObject(modelsC, root)
            property bool micAvailable: true
            property string micLine: ""
            property string dictationState: "recording"
            property string engineLine: "Parakeet v3"
            property bool resultKnown: true
            property string resultOutcome: "inserted"
            property string insertedVia: "virtual keyboard"
            property string stopToInsert: "0.8 s"
            property string modeLine: "Enhanced · Qwen3 4B"
            property string resultReason: ""
            property bool allowanceArmed: false
            property string sampleText: ""
            property int nexts: 0
            property int backs: 0
            property int skips: 0
            property int finishes: 0
            property int opens: 0
            property var allowances: []

            function writeSnippet() {
                snippetWritten = true;
            }

            function next() {
                nexts += 1;
            }

            function back() {
                backs += 1;
            }

            function skip() {
                skips += 1;
            }

            function setAllowance(enabled) {
                allowances.push(enabled);
            }

            function finish() {
                finishes += 1;
            }

            function openConfig() {
                opens += 1;
            }
        }
    }
}
