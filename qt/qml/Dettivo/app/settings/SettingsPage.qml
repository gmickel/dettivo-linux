pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// A scrollable settings page. Advanced mode also shows the keys as TOML.
Item {
    id: root

    readonly property bool advanced: SettingsUi.advanced
    property string title: ""
    property string subtitle: ""
    property string section: ""
    property var settings: null
    property alias trailing: trailingSlot.data
    default property alias content: contentColumn.data

    readonly property int revision: root.settings ? root.settings.revision : 0
    readonly property string writes: root.readWrites(root.revision)
    readonly property string notice: root.settings ? root.settings.notice : ""
    readonly property int minimumContentWidth: Theme.appWindowWidth - Theme.sidebarWidth - Theme.settingsNavWidth - 2 * Theme.pagePaddingX

    // The revision is read so the block follows every refresh.
    function readWrites(revision) {
        return root.settings && revision >= 0 ? root.settings.writesBlock(root.section) : "";
    }

    // Scrolls the page so `y` (in content coordinates) is at the top.
    function scrollTo(y) {
        scroll.contentY = Math.max(0, Math.min(y, scroll.contentHeight - scroll.height));
    }

    // The row or control that edits `key`, anywhere under the content.
    function rowFor(item, key) {
        for (const child of item.children) {
            if ((child.key !== undefined && child.key === key) || (child.name !== undefined && child.name === key))
                return child;
            const nested = rowFor(child, key);
            if (nested)
                return nested;
        }
        return null;
    }

    // Scrolls the row that edits `key` to the top of the page
    // (`dettivo app open settings.<section> --id <key>`).
    function layoutRows(item) {
        for (const child of item.children)
            root.layoutRows(child);
        if (item.forceLayout)
            item.forceLayout();
    }

    function scrollToKey(key) {
        const row = key.length > 0 ? rowFor(contentColumn, key) : null;
        if (row) {
            SettingsUi.advanced = true;
            root.layoutRows(flow);
            root.scrollTo(row.mapToItem(flow, 0, 0).y);
            scroll.reveal(row.controlItem ? row.controlItem : row);
        }
    }

    PageHeader {
        id: header
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: trailingSlot.left
        anchors.rightMargin: Theme.space5
        anchors.top: parent.top
        anchors.topMargin: Theme.pagePaddingY
        subtitle: root.notice.length > 0 ? root.notice : root.subtitle
        subtitleMaxWidth: width
        title: root.title
    }

    Item {
        id: trailingSlot
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: parent.top
        anchors.topMargin: Theme.space4
        height: childrenRect.height
        width: childrenRect.width
    }

    FocusFlickable {
        id: scroll
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.space4
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        anchors.top: header.bottom
        anchors.topMargin: Theme.space5
        boundsBehavior: Flickable.StopAtBounds
        clip: true
        contentHeight: flow.implicitHeight
        contentWidth: Math.max(width, root.minimumContentWidth)

        Column {
            id: flow
            spacing: 0
            width: scroll.contentWidth

            Column {
                id: contentColumn
                spacing: 0
                width: parent.width
            }

            // Pushes the block to the foot of a page shorter than the window.
            Item {
                height: Math.max(Theme.space5, scroll.height - contentColumn.implicitHeight - writesBlock.implicitHeight)
                width: parent.width
            }

            SettingsWrites {
                id: writesBlock
                visible: root.advanced
                text: root.writes
                width: parent.width
            }
        }

        ScrollBar.vertical: ScrollBar {
            policy: scroll.contentHeight > scroll.height ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.title
}
