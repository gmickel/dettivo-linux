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

    // An ancestor that declares revealMargin is revealed whole, with that
    // margin around it, in place of the focused control inside it.
    function reveal(focused) {
        let item = focused;
        let margin = 0;
        let ancestor = focused;
        while (ancestor && ancestor !== root.contentItem) {
            if (ancestor.revealMargin !== undefined) {
                item = ancestor;
                margin = ancestor.revealMargin;
            }
            ancestor = ancestor.parent;
        }
        if (!ancestor || !item || !item.mapToItem)
            return;
        const corner = item.mapToItem(root.contentItem, 0, 0);
        const at = Qt.rect(corner.x - margin, corner.y - margin, item.width + 2 * margin, item.height + 2 * margin);
        if (at.x < root.contentX || at.width > root.width)
            root.contentX = at.x;
        else if (at.x + at.width > root.contentX + root.width)
            root.contentX = at.x + at.width - root.width;
        if (at.y < root.contentY || at.height > root.height)
            root.contentY = at.y;
        else if (at.y + at.height > root.contentY + root.height)
            root.contentY = at.y + at.height - root.height;
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
