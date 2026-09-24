import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "HomeKeyboard"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    Component {
        id: homeC
        HomeRoute {
            width: 1000
            height: 700
        }
    }
    Component {
        id: rowsC
        ListModel {
            ListElement {
                itemId: "one"
                title: "First take"
                kind: "dictation"
                time: "10:00"
                duration: "2 s"
            }
            ListElement {
                itemId: "two"
                title: "Second take"
                kind: "dictation"
                time: "10:01"
                duration: "3 s"
            }
        }
    }
    Component {
        id: routerC
        QtObject {
            property string opened: ""
            property string itemId: ""
            function open(route, id) {
                opened = route;
                itemId = id;
            }
        }
    }

    function test_j_and_k_focus_today_and_enter_opens_the_selected_take() {
        const rows = createTemporaryObject(rowsC, root);
        const router = createTemporaryObject(routerC, root);
        const home = createTemporaryObject(homeC, root, {
            "today": rows,
            "router": router
        });
        waitForRendering(home);
        home.forceActiveFocus();
        keyClick(Qt.Key_J);
        keyClick(Qt.Key_J);
        keyClick(Qt.Key_K);
        keyClick(Qt.Key_Return);
        tryCompare(router, "opened", "history.detail");
        compare(router.itemId, "one");
    }
}
