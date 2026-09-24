import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "CleanupNavigation"
    width: 800
    height: 600
    visible: true
    when: windowShown
    readonly property var accessibility: accessibilityDriver // qmllint disable unqualified
    Component {
        id: sidebarC
        SidebarItem {
            text: "History"
            width: 200
        }
    }
    Component {
        id: modelC
        ModelRow {
            name: "Speech"
            width: 600
        }
    }
    Component {
        id: segmentC
        SegmentedControl {
            model: ["Raw", "Polish", "Enhanced"]
        }
    }
    function test_action_keyboard_data() {
        return [
            {
                tag: "sidebar",
                component: sidebarC,
                action: "press"
            },
            {
                tag: "onboarding",
                component: modelC,
                action: "toggle"
            }
        ];
    }
    function test_action_keyboard(data) {
        const item = createTemporaryObject(data.component, root);
        let calls = 0;
        item.activated.connect(() => calls++);
        verify(item.activeFocusOnTab);
        item.forceActiveFocus(Qt.TabFocusReason);
        keyClick(Qt.Key_Space);
        compare(calls, 1);
        keyClick(Qt.Key_Return);
        compare(calls, 2);
        verify(root.accessibility.accessibleAction(item, data.action));
        compare(calls, 3);
    }
    function test_segment_keyboard_preserves_authoritative_value() {
        const item = createTemporaryObject(segmentC, root);
        let requests = [];
        item.selected.connect(index => requests.push(index));
        const first = item.segmentAt(0);
        verify(first.activeFocusOnTab);
        first.forceActiveFocus(Qt.TabFocusReason);
        keyClick(Qt.Key_Right);
        keyClick(Qt.Key_Space);
        compare(requests[requests.length - 1], 1);
        compare(item.currentIndex, 0);
        const before = requests.length;
        verify(root.accessibility.accessibleAction(item.segmentAt(2), "press"));
        compare(requests.length, before + 1);
        compare(requests[requests.length - 1], 2);
        compare(item.currentIndex, 0);
    }
}
