import QtQuick
import QtQuick.Templates as T
import Dettivo

// One row of the Models step (first-run-2-models.png): a radio or a check
// mark, the name with an optional RECOMMENDED chip, the one-line reason,
// the size and the state (`get`, `62 %`, `ready`, `failed`, `queued`).
// The selected speech row carries the accent border and fill.
T.CheckBox {
    id: root

    property string name: ""
    property string detail: ""
    property string size: ""
    property string stateText: ""
    property bool recommended: false
    property bool selected: false
    checkable: false
    property bool accentState: false
    property bool separator: true
    property string accessibleName: root.name

    signal activated

    padding: 0
    checked: root.selected
    nextCheckState: () => root.checked ? Qt.Checked : Qt.Unchecked
    onClicked: root.activated()
    Keys.onReturnPressed: root.click()
    Keys.onEnterPressed: root.click()
    implicitHeight: Math.max(Theme.firstRunModelRowHeight, column.implicitHeight + Theme.space4 * 2)
    indicator: null
    background: Rectangle {
        border.color: root.selected && !root.checkable ? Theme.roleAccent : Theme.roleBorder
        border.width: root.selected && !root.checkable ? Theme.stateSelectedBorderWidth : Theme.stateNormalBorderWidth
        color: root.selected && !root.checkable ? Theme.roleSearchHighlight : "transparent"
        radius: Theme.radius
    }
    contentItem: Item {
        Item {
            id: indicator
            anchors.left: parent.left
            anchors.leftMargin: Theme.space5
            anchors.verticalCenter: parent.verticalCenter
            height: Theme.space4
            width: Theme.space4

            Rectangle {
                anchors.fill: parent
                border.color: root.selected ? Theme.roleAccent : Theme.roleMutedText
                border.width: Theme.hairlineWidth
                color: root.selected && root.checkable ? Theme.roleAccent : "transparent"
                radius: Theme.radius

                Rectangle {
                    anchors.centerIn: parent
                    color: Theme.roleAccent
                    height: Theme.space2 + Theme.space1
                    radius: Theme.radius
                    visible: root.selected && !root.checkable
                    width: height
                }
            }
        }

        Column {
            id: column
            anchors.left: parent.left
            anchors.leftMargin: Theme.space8
            anchors.right: sizeText.left
            anchors.rightMargin: Theme.space5
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.space2

            Row {
                spacing: Theme.space3

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    color: Theme.roleText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.typeBodySize
                    font.weight: Theme.typeEmphasisWeight
                    text: root.name
                }

                Chip {
                    accent: true
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Recommended")
                    visible: root.recommended
                }
            }

            Text {
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: root.detail
                width: column.width
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: root.detail
            }
        }

        Text {
            id: sizeText
            anchors.right: stateLabel.left
            anchors.rightMargin: Theme.space5
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleMutedText
            font.family: Theme.fontFamily
            font.features: Theme.typeTabularNumerals
            font.pixelSize: Theme.typeBodySize
            text: root.size
            width: Theme.space8 * 2 + Theme.space4
            Accessible.role: Accessible.StaticText
            Accessible.name: root.size
        }

        Text {
            id: stateLabel
            anchors.right: parent.right
            anchors.rightMargin: Theme.space5
            anchors.verticalCenter: parent.verticalCenter
            color: root.accentState ? Theme.roleAccent : Theme.roleFaintText
            font.family: Theme.fontFamily
            font.features: Theme.typeTabularNumerals
            font.pixelSize: Theme.typeBodySize
            horizontalAlignment: Text.AlignLeft
            text: root.stateText
            width: Theme.space8 * 2 - Theme.space3 + Theme.space1
            Accessible.role: Accessible.StaticText
            Accessible.name: root.stateText
        }
    }
    FocusRing {
        visible: root.visualFocus
    }

    Accessible.role: root.checkable ? Accessible.CheckBox : Accessible.RadioButton
    Accessible.name: root.accessibleName
    Accessible.checkable: true
    Accessible.checked: root.selected
}
