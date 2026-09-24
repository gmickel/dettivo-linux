import Dettivo
import QtQuick
import QtTest

TestCase {
    id: root
    name: "MeetingsListLayout"
    width: 1280
    height: 820
    visible: true
    when: windowShown

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

    function test_layout_data() {
        return [
            {
                tag: "minimum",
                windowWidth: Theme.appWindowMinWidth,
                windowHeight: 820
            },
            {
                tag: "minimum-short",
                windowWidth: Theme.appWindowMinWidth,
                windowHeight: 410
            },
            {
                tag: "normal",
                windowWidth: 1280,
                windowHeight: 820
            },
            {
                tag: "normal-short",
                windowWidth: 1280,
                windowHeight: 410
            }
        ];
    }

    function test_layout(data) {
        const model = createTemporaryObject(modelC, root);
        const list = createTemporaryObject(listC, root, {
            width: data.windowWidth - Theme.sidebarWidth - Theme.rightRailWidth,
            height: data.windowHeight - Theme.space8,
            meetings: model
        });
        waitForRendering(list);
        const heading = named(list, "Meetings");
        const search = named(list, "Meetings search");
        const headingAt = heading.mapToItem(list, 0, 0);
        const searchAt = search.mapToItem(list, 0, 0);
        if (data.windowWidth === Theme.appWindowMinWidth)
            compare(searchAt.x, headingAt.x);
        verify(headingAt.x + heading.width <= searchAt.x || headingAt.y + heading.height <= searchAt.y, "heading overlaps search");
        const columns = named(list, "Columns");
        for (const column of columns.children)
            verify(column.width >= Theme.space8, "column must retain usable width");
        list.focusSearch();
        verify(list.searchFocused);
        const imports = [];
        list.importRequested.connect(() => imports.push(true));
        const button = named(list, "Import audio");
        keyClick(Qt.Key_Backtab);
        verify(button.activeFocus);
        keyClick(Qt.Key_Space);
        compare(imports.length, 1);
        const viewport = named(list, "Meetings table");
        verify(viewport);
        const ring = findChild(viewport, "meetingsTableFocusRing");
        verify(ring);
        compare(ring.visible, false);
        if (viewport.contentWidth > viewport.width) {
            keyClick(Qt.Key_Tab);
            verify(list.searchFocused);
            keyClick(Qt.Key_Tab);
            verify(viewport.activeFocus);
            compare(ring.visible, true);
            compare(ring.border.color, StyleHelpers.borderColor("Focus"));
            compare(ring.width, viewport.width);
            keyClick(Qt.Key_End);
            compare(viewport.contentX, viewport.contentWidth - viewport.width);
            compare(ring.mapToItem(viewport, 0, 0).x, 0);
            const state = columns.children[4];
            const stateAt = state.mapToItem(viewport, 0, 0);
            verify(stateAt.x >= 0 && stateAt.x + state.width <= viewport.width);
            keyClick(Qt.Key_Home);
            compare(viewport.contentX, 0);
        }
        model.clear();
        waitForRendering(list);
        const empty = named(list, "No meetings recorded.");
        verify(empty);
        const at = empty.mapToItem(list, 0, 0);
        verify(at.x >= 0 && at.x + empty.width <= list.width);
        verify(at.y >= 0 && at.y + empty.height <= list.height);
    }

    Component {
        id: listC
        MeetingsList {}
    }
    Component {
        id: modelC
        FakeMeetingsModel {}
    }
}
