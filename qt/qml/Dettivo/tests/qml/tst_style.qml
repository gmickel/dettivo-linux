import QtQuick
import QtQuick.Controls
import QtTest
import Dettivo

// Every Quick Controls type the style covers is drawn by DettivoStyle, has
// an accessible role and name, and draws its states from the shell tokens.
TestCase {
    id: root
    name: "DettivoStyle"
    width: 480
    height: 360
    visible: true
    when: windowShown

    Component {
        id: buttonC
        Button {
            text: "Start dictation"
        }
    }
    Component {
        id: textFieldC
        TextField {
            placeholderText: "Search transcripts"
        }
    }
    Component {
        id: comboC
        ComboBox {
            model: ["Raw", "Polish"]
        }
    }
    Component {
        id: checkC
        CheckBox {
            text: "Microphone"
        }
    }
    Component {
        id: radioC
        RadioButton {
            text: "Large v3 Turbo"
        }
    }
    Component {
        id: switchC
        Switch {
            text: "Sounds"
        }
    }
    Component {
        id: scrollC
        ScrollBar {
            orientation: Qt.Vertical
        }
    }
    Component {
        id: menuItemC
        MenuItem {
            text: "Copy"
        }
    }
    Component {
        id: toolTipC
        ToolTip {
            text: "Hold F9"
        }
    }
    Component {
        id: delegateC
        ItemDelegate {
            text: "Row"
        }
    }
    Component {
        id: tabBarC
        TabBar {
            TabButton {
                text: "Transcript"
            }
            TabButton {
                text: "Notes"
            }
        }
    }
    Component {
        id: progressC
        ProgressBar {
            value: 0.4
        }
    }
    Component {
        id: popupC
        Popup {}
    }
    Component {
        id: menuC
        Menu {
            MenuItem {
                text: "Export"
            }
        }
    }

    function test_style_is_dettivo() {
        compare(ApplicationWindow.window === null, true);
        const button = createTemporaryObject(buttonC, root);
        verify(button.background !== null);
        compare(button.background.radius, Theme.radius);
        compare(button.implicitHeight, Theme.controlHeight);
        compare(button.font.family, Theme.fontFamily);
    }

    function test_hidden_buttons_leave_the_accessibility_tree() {
        for (const component of [buttonC, switchC]) {
            const control = createTemporaryObject(component, root);
            compare(control.Accessible.ignored, false);
            control.visible = false;
            compare(control.Accessible.ignored, true);
            control.visible = true;
            compare(control.Accessible.ignored, false);
        }
    }

    function test_controls_carry_roles_and_names() {
        const cases = [[buttonC, Accessible.Button, "Start dictation"], [textFieldC, Accessible.EditableText, "Search transcripts"], [comboC, Accessible.ComboBox, "Raw"], [checkC, Accessible.CheckBox, "Microphone"], [radioC, Accessible.RadioButton, "Large v3 Turbo"], [switchC, Accessible.Button, "Sounds"], [scrollC, Accessible.ScrollBar, "Vertical scroll bar"], [menuItemC, Accessible.MenuItem, "Copy"], [delegateC, Accessible.ListItem, "Row"], [tabBarC, Accessible.PageTabList, "Tabs"], [progressC, Accessible.ProgressBar, "Progress"],];
        for (const [component, role, name] of cases) {
            const item = createTemporaryObject(component, root);
            verify(item, "control instantiates under DettivoStyle");
            compare(item.Accessible.role, role);
            compare(item.Accessible.name, name);
        }
        const tip = createTemporaryObject(toolTipC, root);
        compare(tip.contentItem.Accessible.name, "Hold F9");
        compare(tip.contentItem.Accessible.role, Accessible.ToolTip);
        const popup = createTemporaryObject(popupC, root);
        verify(popup.background !== null);
        const menu = createTemporaryObject(menuC, root);
        verify(menu.background !== null);
    }

    function test_button_states_use_shell_alphas() {
        const button = createTemporaryObject(buttonC, root);
        compare(button.fillState, "Normal");
        compare(button.borderState, "Normal");
        compare(button.background.border.width, Theme.stateNormalBorderWidth);
        button.checked = true;
        compare(button.fillState, "Selected");
        compare(button.background.border.width, Theme.stateSelectedBorderWidth);
        button.checked = false;
        button.forceActiveFocus(Qt.TabFocusReason);
        tryCompare(button, "borderState", "Focus");
        compare(button.background.border.width, Theme.stateFocusBorderWidth);
    }

    function test_primary_and_danger_variants() {
        const primary = createTemporaryObject(buttonC, root, {
            highlighted: true
        });
        compare(primary.background.border.width, Theme.stateNormalBorderWidth);
        fuzzyCompare(primary.background.color.a, Theme.stateFocusFillAlpha, 0.02);
        const danger = createTemporaryObject(buttonC, root, {
            danger: true,
            text: "Delete meeting"
        });
        compare(danger.accentColor, Theme.roleUrgent);
    }

    function test_check_box_and_switch_toggle() {
        const check = createTemporaryObject(checkC, root);
        compare(check.Accessible.checked, false);
        check.toggle();
        compare(check.Accessible.checked, true);
        compare(check.fillState, "Selected");
        const toggle = createTemporaryObject(switchC, root);
        toggle.toggle();
        compare(toggle.Accessible.checked, true);
    }

    function test_text_field_focus_border() {
        const field = createTemporaryObject(textFieldC, root);
        compare(field.borderState, "Normal");
        field.forceActiveFocus();
        tryCompare(field, "borderState", "Focus");
        compare(field.background.border.width, Theme.stateFocusBorderWidth);
    }
}
