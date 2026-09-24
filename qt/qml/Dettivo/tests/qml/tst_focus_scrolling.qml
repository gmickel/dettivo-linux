import QtQuick
import QtQuick.Controls
import QtTest
import Dettivo

TestCase {
    id: root
    name: "FocusScrolling"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    function findNamed(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;
            const found = findNamed(child, name);
            if (found)
                return found;
        }
        return null;
    }

    function inside(item, viewport) {
        const point = item.mapToItem(viewport, 0, 0);
        return point.x >= -1 && point.y >= -1 && point.x + item.width <= viewport.width + 1 && point.y + item.height <= viewport.height + 1;
    }

    Component {
        id: scrollC
        FocusFlickable {
            width: 300
            height: 180
            contentWidth: 800
            contentHeight: 600
            TextField {
                x: 640
                y: 520
                width: 150
                Accessible.name: "Far editor"
            }
        }
    }

    Component {
        id: firstRunC
        OnboardingRoute {
            width: 1024
            height: 410
        }
    }

    Component {
        id: lockedC
        SettingField {
            enabled: false
            name: "ipc.socket"
            value: "/tmp/test.sock"
            width: 256
        }
    }

    function test_keyboard_focus_reveals_an_editor_on_both_axes() {
        const page = createTemporaryObject(scrollC, root);
        const editor = findNamed(page, "Far editor");
        verify(editor);
        verify(!inside(editor, page));
        editor.forceActiveFocus();
        tryVerify(() => inside(editor, page));
        verify(page.contentX > 0);
        verify(page.contentY > 0);
    }

    function test_first_run_footer_stays_reachable_in_a_short_window() {
        const page = createTemporaryObject(firstRunC, root);
        const next = findNamed(page, "Continue");
        verify(next);
        verify(page.contentHeight >= Theme.appWindowHeight);
        next.forceActiveFocus();
        tryVerify(() => inside(next, page));
    }

    function test_locked_setting_is_read_only_to_accessibility() {
        const page = createTemporaryObject(lockedC, root);
        const field = findNamed(page, "ipc.socket");
        verify(field);
        compare(field.readOnly, true);
        compare(field.Accessible.editable, false);
        compare(field.Accessible.readOnly, true);
        compare(field.text, "/tmp/test.sock");
    }
}
