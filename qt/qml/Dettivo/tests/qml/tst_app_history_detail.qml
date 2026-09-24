import Dettivo
import QtQuick
import QtTest

// History's detail (fn-22): Enhanced, Raw, the strip and the facts from a
// fake detail model, Re-run enabled only with a take, and the delete
// confirmation that asks with urgent text before the action runs.
TestCase {
    id: root

    name: "AppHistoryDetail"
    width: 1280
    height: 820
    visible: true
    when: windowShown

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

    function test_detail_shows_texts_take_facts_and_asks_before_delete() {
        const detail = createTemporaryObject(detailC, root);
        const actions = createTemporaryObject(actionsC, root);
        const player = createTemporaryObject(playerC, root);
        const pane = createTemporaryObject(detailPaneC, root, {
            "width": 680,
            "height": 800,
            "itemId": "x",
            "detail": detail,
            "actions": actions,
            "player": player
        });
        waitForRendering(pane);
        verify(findByName(pane, "Enhanced"));
        verify(findByName(pane, "Raw words."));
        verify(findByName(pane, "um raw words"));
        verify(findByName(pane, "Play"));
        verify(findByName(pane, "stop to insert: 0.9 s"));
        verify(findByName(pane, "insertion: virtual keyboard"));
        compare(findByName(pane, "Re-run").enabled, true);
        const del = findByName(pane, "Delete");
        verify(del);
        mouseClick(del);
        // The confirmation opens in the window's overlay, above the pane.
        const overlay = root.Window.window.contentItem;
        tryVerify(() => {
            return findByName(overlay, "Delete for good") !== null;
        });
        mouseClick(findByName(overlay, "Delete for good"));
        compare(actions.removedIds[0], "x");
        detail.audioRetained = false;
        detail.canRerun = false;
        compare(findByName(pane, "Re-run").enabled, false);
    }

    function test_copy_transcript_uses_final_then_raw_without_insertion() {
        const detail = createTemporaryObject(detailC, root);
        const actions = createTemporaryObject(actionsC, root);
        const pane = createTemporaryObject(detailPaneC, root, {
            width: 680,
            height: 800,
            itemId: "x",
            detail: detail,
            actions: actions
        });
        waitForRendering(pane);
        const copy = findByName(pane, "Copy transcript");
        verify(copy, "History needs a usable copy action instead of inserting into itself");
        mouseClick(copy);
        compare(actions.copiedText, "Raw words.");
        verify(pane.actionNote.indexOf("paste") >= 0);
        detail.enhancedText = "";
        mouseClick(copy);
        compare(actions.copiedText, "um raw words");
        detail.rawText = "";
        compare(copy.enabled, false);
    }

    Component {
        id: detailC

        QtObject {
            property string itemId: "x"
            property bool loaded: true
            property bool loading: false
            property string error: ""
            property string title: "Raw words."
            property string whenLine: "Fri 13 Feb · 16:00 · inserted into ghostty"
            property string enhancedText: "Raw words."
            property string rawText: "um raw words"
            property string mode: "enhanced"
            property string modeLabel: "Enhanced"
            property string outcomeLabel: "Inserted"
            property string enhancedMeta: "Enhanced"
            property string rawMeta: "large-v3-turbo · 0.8 s"
            property bool audioRetained: true
            property string audioPath: "sample"
            property string audioReason: ""
            property string audioMeta: "30.0 s · 16 kHz"
            property bool canRerun: true
            property string rerunBlockedReason: ""
            property real progress: -1
            property string stage: ""
            property var loadedIds: []
            property var facts: [
                {
                    "label": "app",
                    "value": "ghostty"
                },
                {
                    "label": "stop to insert",
                    "value": "0.9 s"
                },
                {
                    "label": "mode",
                    "value": "Enhanced"
                },
                {
                    "label": "insertion",
                    "value": "virtual keyboard"
                }
            ]

            signal changed

            function load(id) {
                loadedIds.push(id);
            }

            function trackJob(jobId) {
            }
        }
    }

    Component {
        id: actionsC

        QtObject {
            property bool busy: false
            property string exportDir: ""
            property var providers: []
            property string selectedProvider: ""
            property string selectedModel: ""
            property var removedIds: []
            property string copiedText: ""
            function copyText(text) {
                copiedText = text;
                return true;
            }

            signal rerunStarted(string itemId, string newItemId, string jobId)
            signal exported(string path, int bytes)
            signal removed(string id)
            signal inserted(string id, string outcome)
            signal failed(string action, string reason)

            function remove(id) {
                removedIds.push(id);
                removed(id);
            }

            function loadProviders() {
            }
        }
    }

    Component {
        id: playerC

        QtObject {
            property string source: ""
            property var peaks: [0.2, 0.8, 0.4]
            property real fraction: 0.25
            property bool playing: false
            property string clock: "0:01 / 0:04"
            property string error: ""
            property int position: 1000
            property int duration: 4000

            function toggle() {
                playing = !playing;
            }

            function seek(f) {
                fraction = f;
            }
        }
    }

    Component {
        id: detailPaneC

        HistoryDetail {}
    }
}
