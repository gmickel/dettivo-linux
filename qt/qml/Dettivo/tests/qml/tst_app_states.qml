import Dettivo
import QtQuick
import QtTest

// The designed states and the hint sheet (fn-38, ADR 0042): a StateView
// without its reason, or without an action or a key it did not declare
// itself free of, is incomplete; every sample state of the states page is
// complete; the hint sheet opens from the window and closes on any key.
TestCase {
    id: root
    property bool routeShortcutEnabled: false
    property int routeShortcutHits: 0

    Shortcut {
        sequence: "J"
        enabled: root.routeShortcutEnabled
        onActivated: root.routeShortcutHits++
    }

    function cleanup() {
        root.routeShortcutEnabled = false;
    }

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

    // The rule of the sheet (states-and-hint-sheet.png): a state without
    // its reason, or without an action or a key it did not declare it has
    // none of, is incomplete; every sample state on the page is complete.
    function test_state_view_fails_a_missing_reason_or_action() {
        const noReason = createTemporaryObject(emptyC, root, {
            "reason": ""
        });
        verify(!noReason.complete);
        const noAction = createTemporaryObject(emptyC, root, {
            "keyHint": ""
        });
        verify(!noAction.complete);
        const declared = createTemporaryObject(emptyC, root, {
            "keyHint": "",
            "actionless": true
        });
        verify(declared.complete);
        const loading = createTemporaryObject(emptyC, root, {
            "keyHint": "",
            "loading": true
        });
        verify(loading.complete);
        const page = createTemporaryObject(statesC, root, {
            "width": 1280,
            "height": 820
        });
        for (const name of Object.keys(page.samples)) {
            page.stateName = name;
            const view = findByName(page, page.samples[name].title);
            verify(view, name);
            verify(view.complete, name + " is incomplete");
        }
        page.stateName = "hint-sheet";
        verify(findByName(page, "Keyboard hint sheet"));
        verify(findByName(page, "?: this sheet"));
        verify(findByName(page, "t: rename meeting"));
    }

    function test_hint_sheet_opens_on_question_mark_and_closes_on_any_key() {
        const window = createTemporaryObject(windowC, root, {
            "router": null,
            "status": null
        });
        verify(window);
        const sheet = findByName(window.contentItem, "Keyboard hint sheet");
        verify(sheet);
        compare(sheet.visible, false);
        window.toggleHintSheet();
        compare(sheet.visible, true);
        sheet.closed();
        compare(sheet.visible, false);
        // Any key closes: the sheet in this test's own shown window.
        const own = createTemporaryObject(hintC, root);
        const closed = createTemporaryObject(spyC, root, {
            "target": own,
            "signalName": "closed"
        });
        own.forceActiveFocus();
        tryCompare(own, "activeFocus", true);
        root.routeShortcutHits = 0;
        root.routeShortcutEnabled = true;
        keyClick(Qt.Key_J);
        compare(closed.count, 1);
        compare(root.routeShortcutHits, 0);
        keyClick(Qt.Key_Escape);
        compare(closed.count, 2);
        verify(findByName(own, "?: this sheet"));
        verify(findByName(own, "Global keys"));
    }

    function test_hint_sheet_survives_opening_chord_modifiers() {
        const sheet = createTemporaryObject(hintC, root);
        const closed = createTemporaryObject(spyC, root, {
            "target": sheet,
            "signalName": "closed"
        });
        sheet.forceActiveFocus();
        tryCompare(sheet, "activeFocus", true);
        // A queued modifier press can reach the sheet after ? opens it.
        for (const key of [Qt.Key_Shift, Qt.Key_Control, Qt.Key_Alt, Qt.Key_Meta]) {
            keyClick(key);
            compare(closed.count, 0);
        }
        keyClick(Qt.Key_J);
        compare(closed.count, 1);
    }

    Component {
        id: emptyC

        StateView {
            title: "Nothing dictated yet."
            reason: "Hold F9 in any app and it will show up here."
            keyHint: "F9"
        }
    }

    Component {
        id: statesC

        StatesPage {}
    }

    Component {
        id: hintC

        HintSheet {}
    }

    Component {
        id: spyC

        SignalSpy {}
    }

    Component {
        id: windowC

        AppWindow {
            visible: false
        }
    }

    name: "AppStates"
    width: 1280
    height: 820
    visible: true
    when: windowShown
}
