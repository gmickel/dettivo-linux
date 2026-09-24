pragma ComponentBehavior: Bound
import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "AdvancedToggle"
    width: 320
    height: 820
    visible: true
    when: windowShown

    function named(item, name) {
        if (item.Accessible && item.Accessible.name === name)
            return item;
        for (const child of item.children) {
            const found = named(child, name);
            if (found)
                return found;
        }
        return null;
    }

    function cleanup() {
        SettingsUi.advanced = false;
    }

    function test_click_preserves_external_advanced_updates() {
        // Load the live component so the test can also run without rebuilding QML.
        const component = Qt.createComponent("../../app/settings/SettingsNav.qml");
        compare(component.status, Component.Ready, component.errorString());
        const nav = createTemporaryObject(component, root, {
            width: 320,
            height: 820
        });
        verify(nav);
        waitForRendering(nav);
        const toggle = named(nav, "Show advanced settings");
        verify(toggle);
        compare(toggle.checked, false);
        mouseClick(toggle);
        compare(SettingsUi.advanced, true);
        compare(toggle.checked, true);
        mouseClick(toggle);
        compare(SettingsUi.advanced, false);
        compare(toggle.checked, false);
        // A deep link changes this singleton after the user has clicked.
        SettingsUi.advanced = true;
        compare(toggle.checked, true);
        SettingsUi.advanced = false;
        compare(toggle.checked, false);
        SettingsUi.advanced = true;
        mouseClick(toggle);
        compare(SettingsUi.advanced, false);
        compare(toggle.checked, false);
    }
}
