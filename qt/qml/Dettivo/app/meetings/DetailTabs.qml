import QtQuick
import QtQuick.Controls
import Dettivo

// The tab row of the meeting detail (meeting-detail.png): Transcript,
// Notes and Analysis on the left, the Raw and Polished toggle on the
// right while the transcript is open; `1`, `2`, `3` and `p` reach the
// same two properties.
Item {
    id: root

    property alias currentIndex: tabs.currentIndex
    property bool polished: true

    implicitHeight: tabs.implicitHeight

    TabBar {
        id: tabs
        anchors.left: parent.left
        anchors.right: polishedToggle.left
        anchors.rightMargin: Theme.space4
        Accessible.role: Accessible.PageTabList
        Accessible.name: qsTr("Meeting tabs")

        TabButton {
            text: qsTr("Transcript")
            width: implicitWidth
        }

        TabButton {
            text: qsTr("Notes")
            width: implicitWidth
        }

        TabButton {
            text: qsTr("Analysis")
            width: implicitWidth
        }
    }

    Row {
        id: polishedToggle
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space2
        visible: tabs.currentIndex === 0

        Text {
            color: root.polished ? Theme.roleFaintText : Theme.roleText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: qsTr("Raw")

            TapHandler {
                onTapped: root.polished = false
            }

            activeFocusOnTab: true
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                    root.polished = false;
                    event.accepted = true;
                }
            }

            FocusRing {}

            Accessible.role: Accessible.RadioButton
            Accessible.name: qsTr("Raw")
            Accessible.checked: !root.polished
            Accessible.onPressAction: root.polished = false
        }

        Text {
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: "·"
        }

        Text {
            color: root.polished ? Theme.roleText : Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: qsTr("Polished")

            TapHandler {
                onTapped: root.polished = true
            }

            activeFocusOnTab: true
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                    root.polished = true;
                    event.accepted = true;
                }
            }

            FocusRing {}

            Accessible.role: Accessible.RadioButton
            Accessible.name: qsTr("Polished")
            Accessible.checked: root.polished
            Accessible.onPressAction: root.polished = true
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Detail tabs")
}
