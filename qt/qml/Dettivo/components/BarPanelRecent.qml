pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// RECENT: the last items as list rows (time, title, the app in caps), or
// one muted line when nothing has been dictated yet.
Column {
    id: root

    required property var panel

    readonly property int count: root.panel.recent ? root.panel.recent.length : 0

    spacing: 0

    SectionLabel {
        leftPadding: Theme.rowPaddingX
        topPadding: Theme.space3
        bottomPadding: Theme.space2
        text: qsTr("Recent")
    }

    Repeater {
        id: repeater
        model: root.panel.recent

        delegate: ListRow {
            id: row
            required property var modelData
            required property int index

            width: root.width
            leading: String(row.modelData.time || "")
            text: String(row.modelData.title || "")
            trailing: String(row.modelData.app || "").toUpperCase()
            separator: true
            onActivated: root.panel.openItem(row.index)
        }
    }

    Text {
        visible: root.count === 0
        width: parent.width
        leftPadding: Theme.rowPaddingX
        bottomPadding: Theme.space3
        text: qsTr("Nothing dictated yet.")
        color: Theme.roleFaintText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Accessible.role: Accessible.List
    Accessible.name: qsTr("Recent dictations")
}
