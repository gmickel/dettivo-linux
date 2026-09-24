import Dettivo
import QtQuick
import QtTest

// History's list side (fn-22): the search field carries the hit count and
// hands a query on, a row paints its matches, the filter chips choose a
// kind, and the route keeps the list beside the detail with the route's
// names; tst_app_history_detail.qml covers the detail.
TestCase {
    id: root

    name: "AppHistory"
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

    function test_search_field_reports_hits_and_hands_the_query_on() {
        const field = createTemporaryObject(searchC, root, {
            "width": 360,
            "hits": 3,
            "searching": true
        });
        waitForRendering(field);
        const count = findByName(field, "3 hits");
        verify(count);
        compare(count.text, "3 hits");
        const queries = [];
        field.queryChanged.connect(q => queries.push(q));
        field.takeFocus();
        compare(field.focused, true);
        keyClick(Qt.Key_M);
        keyClick(Qt.Key_Return);
        compare(queries[queries.length - 1], "m");
        verify(findByName(field, "Clear search"));
        field.clear();
        compare(queries[queries.length - 1], "");
        field.dropFocus();
        compare(field.focused, false);
    }

    function test_row_paints_its_matches_with_the_accent_fill() {
        const row = createTemporaryObject(rowC, root, {
            "width": 400,
            "time": "13:12",
            "text": "the merger overlap",
            "meta": "ghostty · Enhanced · 4 s",
            "matches": [[4, 10]],
            "progress": 0.5
        });
        waitForRendering(row);
        compare(row.painted, true);
        compare(row.working, true);
        compare(row.Accessible.name, "the merger overlap");
        compare(row.Accessible.description, "ghostty · Enhanced · 4 s");
        const painted = Html.highlighted("the merger overlap", [[4, 10]], Theme.roleSearchHighlight);
        verify(painted.indexOf("<span style=\"background-color:") === 4);
        verify(painted.indexOf("merger</span>") > 0);
        compare(Html.highlighted("a < b", [], Theme.roleSearchHighlight), "a &lt; b");
    }

    function test_filter_chips_choose_a_kind() {
        const chips = createTemporaryObject(chipsC, root, {
            "current": "all"
        });
        waitForRendering(chips);
        const chosen = [];
        chips.chosen.connect(f => chosen.push(f));
        const meeting = findByName(chips, "Meeting");
        verify(meeting);
        compare(meeting.Accessible.checked, false);
        mouseClick(meeting);
        compare(chosen[0], "meeting");
        chips.current = "meeting";
        compare(meeting.Accessible.checked, true);
    }

    function test_route_keeps_list_beside_detail_and_opens_rows() {
        const router = createTemporaryObject(routerC, root, {
            "route": "history"
        });
        const history = createTemporaryObject(historyC, root);
        const detail = createTemporaryObject(detailC, root);
        const page = createTemporaryObject(routeC, root, {
            "width": 1080,
            "height": 800,
            "router": router,
            "history": history,
            "detail": detail
        });
        waitForRendering(page);
        compare(page.Accessible.name, "History");
        verify(findByName(page, "History list"));
        verify(findByName(page, "Dictations"));
        verify(findByName(page, "Search history"));
        const row = findByName(page, "Add a regression test");
        verify(row);
        mouseClick(row);
        compare(router.opened[router.opened.length - 1], "history.detail:a");
        router.sub = "detail";
        router.arg = "a";
        compare(page.Accessible.name, "Dictation");
        compare(detail.loadedIds[detail.loadedIds.length - 1], "a");
        compare(page.focusSearch(), true);
        compare(page.dismiss(), true);
        compare(page.dismiss(), false);
    }

    Component {
        id: routerC

        QtObject {
            property string route: "history"
            property string sub: ""
            property string arg: ""
            property bool detail: sub === "detail"
            property var opened: []
            readonly property string page: sub.length > 0 ? route + "." + sub : route
            readonly property string title: route

            function open(name, id) {
                opened.push(id ? name + ":" + id : name);
                return true;
            }

            function back() {
                sub = "";
                arg = "";
            }
        }
    }

    Component {
        id: historyC

        ListModel {
            property string query: ""
            property bool searching: false
            property int hitCount: 0
            property string error: ""
            property bool loaded: true
            property string filter: "all"

            function search(q) {
                query = q;
                searching = q.length > 0;
            }

            function idAt(row) {
                return row === 0 ? "a" : "";
            }

            function rowOf(id) {
                return id === "a" ? 0 : -1;
            }

            function refresh() {
            }

            function remove(id) {
            }

            ListElement {
                itemId: "a"

                kind: "dictation"
                title: "Add a regression test"
                snippet: ""
                time: "13:12"
                dayHeading: "Today · Wed 3 Sep"
                meta: "ghostty · Enhanced · 4 s"
                matches: []
                progress: -1
            }
        }
    }

    Component {
        id: detailC

        QtObject {
            property bool loaded: false
            property string audioPath: ""
            property bool audioRetained: false
            property var loadedIds: []

            signal changed

            function load(id) {
                loadedIds.push(id);
            }
        }
    }

    Component {
        id: searchC

        SearchField {}
    }

    Component {
        id: rowC

        HistoryRow {}
    }

    Component {
        id: chipsC

        FilterChips {}
    }

    Component {
        id: routeC

        HistoryRoute {}
    }
}
