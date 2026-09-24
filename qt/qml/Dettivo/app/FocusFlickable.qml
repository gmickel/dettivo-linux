import QtQuick
import QtQuick.Controls

// Keep the focused editor inside a clipped page, including after Tab or a
// jump to a configuration key. Scroll rather than hide fixed-width tables.
Flickable {
    id: root

    boundsBehavior: Flickable.StopAtBounds
    clip: true
    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Scrollable content")

    function reveal(item) {
        let ancestor = item;
        while (ancestor && ancestor !== root.contentItem)
            ancestor = ancestor.parent;
        if (!ancestor || !item || !item.mapToItem)
            return;
        const at = item.mapToItem(root.contentItem, 0, 0);
        if (at.x < root.contentX || item.width > root.width)
            root.contentX = at.x;
        else if (at.x + item.width > root.contentX + root.width)
            root.contentX = at.x + item.width - root.width;
        if (at.y < root.contentY || item.height > root.height)
            root.contentY = at.y;
        else if (at.y + item.height > root.contentY + root.height)
            root.contentY = at.y + item.height - root.height;
        root.contentX = Math.max(0, Math.min(root.contentX, root.contentWidth - root.width));
        root.contentY = Math.max(0, Math.min(root.contentY, root.contentHeight - root.height));
    }

    function revealFocus() {
        if (root.Window.window)
            root.reveal(root.Window.window.activeFocusItem);
    }

    Connections {
        target: root.Window.window
        function onActiveFocusItemChanged() {
            Qt.callLater(root.revealFocus);
        }
    }

    ScrollBar.vertical: ScrollBar {
        policy: root.contentHeight > root.height ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
    }
    ScrollBar.horizontal: ScrollBar {
        policy: root.contentWidth > root.width ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
    }
}
