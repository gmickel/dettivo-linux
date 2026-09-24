import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "CleanupSearch"
    width: 800
    height: 600
    visible: true
    when: windowShown
    Component {
        id: fieldC
        SearchField {
            width: 300
        }
    }
    function named(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;
            const found = named(child, name);
            if (found)
                return found;
        }
        return null;
    }
    function test_search_instances_keep_names_and_query_state_independent() {
        const history = createTemporaryObject(fieldC, root);
        const meetings = createTemporaryObject(fieldC, root, {
            y: 100,
            placeholder: "Search meetings",
            fieldName: "Search meetings",
            clearName: "Clear meetings search",
            containerName: "Meetings search"
        });
        verify(meetings);
        const calls = [];
        meetings.queryChanged.connect(query => calls.push(query));
        meetings.takeFocus();
        keyClick(Qt.Key_A);
        keyClick(Qt.Key_B);
        keyClick(Qt.Key_Return);
        compare(calls.length, 1);
        compare(calls[0], "ab");
        compare(history.text, "");
        compare(named(history, "Search history").placeholderText, "Search");
        const clear = named(meetings, "Clear meetings search");
        verify(clear.activeFocusOnTab);
        clear.forceActiveFocus(Qt.TabFocusReason);
        keyClick(Qt.Key_Space);
        compare(calls.length, 2);
        compare(calls[1], "");
        compare(meetings.text, "");
        meetings.takeFocus();
        verify(meetings.focused);
        keyClick(Qt.Key_C);
        tryCompare(calls, "length", 3);
        compare(calls[2], "c");
    }
}
