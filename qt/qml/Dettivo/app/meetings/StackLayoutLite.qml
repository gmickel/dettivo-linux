import QtQuick

// The tab pages of the meeting detail: every child fills the item and
// the one at `currentIndex` is shown, so a page keeps its scroll and
// its caret while another is open.
Item {
    id: root

    property int currentIndex: 0

    onCurrentIndexChanged: root.apply()
    Component.onCompleted: root.apply()

    function apply() {
        for (let i = 0; i < root.children.length; ++i)
            root.children[i].visible = i === root.currentIndex;
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Tab pages")
}
