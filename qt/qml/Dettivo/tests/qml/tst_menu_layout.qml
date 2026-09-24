import QtQuick
import QtQuick.Controls
import QtTest
import Dettivo

TestCase {
    id: root
    name: "MenuLayout"
    width: 969
    height: 410
    visible: true
    when: windowShown

    Component {
        id: menuC
        Menu {
            MenuItem {
                text: "Whisper (local) · Large v3 Turbo"
            }
            MenuItem {
                text: "Whisper (local) · Tiny (English)"
            }
        }
    }

    Component {
        id: itemC
        MenuItem {}
    }

    function test_large_menu_stays_inside_the_window_and_scrolls() {
        const menu = createTemporaryObject(menuC, root);
        for (let i = 0; i < 20; ++i) {
            const item = createTemporaryObject(itemC, root, {
                text: "Long model label ".repeat(20) + i
            });
            menu.addItem(item);
        }
        menu.open();
        tryCompare(menu, "visible", true);
        verify(menu.width <= root.Window.window.width);
        verify(menu.height <= root.Window.window.height);
        tryCompare(menu.contentItem, "interactive", true);
        menu.close();
    }

    function test_menu_background_hides_underlying_content() {
        const menu = createTemporaryObject(menuC, root);
        menu.open();
        tryCompare(menu, "visible", true);
        compare(menu.background.color.a, 1);
        compare(menu.background.color, Qt.tint(Theme.roleSurface, Theme.roleRaisedSurface));
        menu.close();
    }

    function test_menu_fits_labels_and_retains_keyboard_activation() {
        const menu = createTemporaryObject(menuC, root);
        menu.open();
        tryCompare(menu, "visible", true);
        const item = menu.itemAt(0);
        tryVerify(() => menu.width >= item.implicitWidth + menu.leftPadding + menu.rightPadding);
        verify(menu.width <= root.width);
        compare(item.contentItem.truncated, false);
        let invoked = 0;
        item.triggered.connect(() => invoked++);
        menu.currentIndex = 0;
        keyClick(Qt.Key_Return);
        tryCompare(menu, "visible", false);
        compare(invoked, 1);
    }
}
