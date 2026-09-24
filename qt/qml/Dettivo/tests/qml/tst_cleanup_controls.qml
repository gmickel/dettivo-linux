pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtTest
import Dettivo

TestCase {
    id: root
    name: "CleanupControls"
    width: 1200
    height: 800
    visible: true
    when: windowShown
    property int authoritativeIndex: 0
    // Context property supplied by quick_test_main.cpp.
    readonly property var accessibility: accessibilityDriver // qmllint disable unqualified

    Component {
        id: segmentC
        SegmentedControl {
            model: ["Raw", "Polish", "Enhanced"]
            currentIndex: root.authoritativeIndex
        }
    }
    Component {
        id: fieldC
        SettingField {
            width: 300
            value: "A"
        }
    }
    Component {
        id: switchC
        SettingSwitch {
            name: "Analysis"
            width: 200
        }
    }
    Component {
        id: checkC
        CheckBox {
            text: "Enabled"
        }
    }
    Component {
        id: radioC
        RadioButton {
            text: "Model"
        }
    }
    Component {
        id: buttonC
        Button {
            text: "Enabled"
            checkable: true
        }
    }
    Component {
        id: tabsC
        TabBar {
            TabButton {
                text: "One"
            }
            TabButton {
                text: "Two"
            }
        }
    }
    Component {
        id: menuC
        Menu {
            MenuItem {
                text: "Select"
                checkable: true
            }
        }
    }
    Component {
        id: popupC
        Popup {}
    }
    Component {
        id: comboC
        ComboBox {
            model: ["Raw", "Polish"]
        }
    }

    function test_segment_keeps_authoritative_binding_after_rejected_and_accepted_requests() {
        root.authoritativeIndex = 0;
        const segment = createTemporaryObject(segmentC, root);
        let selected = -1;
        segment.selected.connect(index => selected = index);
        mouseClick(segment.segmentAt(1));
        compare(selected, 1);
        compare(segment.currentIndex, 0, "rejected request keeps the daemon value");
        root.authoritativeIndex = 2;
        compare(segment.currentIndex, 2, "external update still reaches the selector");
        segment.selected.connect(index => root.authoritativeIndex = index);
        mouseClick(segment.segmentAt(1));
        compare(segment.currentIndex, 1);
        root.authoritativeIndex = 0;
        compare(segment.currentIndex, 0);
    }

    function test_field_new_edit_session_data() {
        return [
            {
                tag: "external-reset",
                value: "B",
                accept: true
            },
            {
                tag: "failed-write-retry",
                value: "B",
                accept: false
            },
            {
                tag: "empty-retry",
                value: "",
                accept: false
            }
        ];
    }

    function test_field_new_edit_session(data) {
        const setting = createTemporaryObject(fieldC, root);
        const field = setting.children[0];
        let commits = [];
        setting.committed.connect(text => commits.push(text));
        field.forceActiveFocus();
        field.text = data.value;
        field.accepted();
        root.forceActiveFocus();
        compare(commits.length, 1, "Enter and blur write once");
        if (data.accept) {
            setting.value = data.value;
            setting.value = "A";
        }
        field.forceActiveFocus();
        field.text = data.value;
        root.forceActiveFocus();
        compare(commits.length, 2, "a later edit may retry the same value");
        compare(commits[1], data.value);
    }

    function test_accessible_switch_writes_once() {
        const setting = createTemporaryObject(switchC, root);
        let writes = [];
        setting.toggled.connect(value => writes.push(value));
        verify(root.accessibility.accessibleAction(setting.children[0], "toggle"));
        compare(writes.length, 1);
        compare(writes[0], true);
    }

    function test_accessible_checkable_controls_data() {
        return [
            {
                tag: "check",
                component: checkC,
                action: "toggle"
            },
            {
                tag: "radio",
                component: radioC,
                action: "toggle"
            },
            {
                tag: "button",
                component: buttonC,
                action: "press"
            }
        ];
    }

    function test_accessible_checkable_controls(data) {
        const control = createTemporaryObject(data.component, root);
        let clicks = 0;
        control.clicked.connect(() => clicks++);
        verify(root.accessibility.accessibleAction(control, data.action));
        compare(control.checked, true);
        compare(clicks, 1);
    }

    function test_accessible_tab_selects() {
        const tabs = createTemporaryObject(tabsC, root);
        verify(root.accessibility.accessibleAction(tabs.itemAt(1), "press"));
        compare(tabs.currentIndex, 1);
    }

    function test_accessible_menu_activates() {
        const menu = createTemporaryObject(menuC, root);
        menu.open();
        tryCompare(menu, "opened", true);
        let triggers = 0;
        menu.itemAt(0).triggered.connect(() => triggers++);
        verify(root.accessibility.accessibleAction(menu.itemAt(0), "press"));
        compare(triggers, 1);
        compare(menu.itemAt(0).checked, true);
        tryCompare(menu, "visible", false);
    }

    function test_shared_floating_surfaces_are_opaque() {
        const popup = createTemporaryObject(popupC, root);
        const menu = createTemporaryObject(menuC, root);
        const combo = createTemporaryObject(comboC, root);
        for (const floating of [popup, menu, combo.popup]) {
            compare(floating.background.color.a, 1);
            compare(floating.background.color, Qt.tint(Theme.roleSurface, Theme.roleRaisedSurface));
        }
    }
}
