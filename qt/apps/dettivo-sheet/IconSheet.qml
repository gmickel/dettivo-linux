pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The icon page (icon-and-osd-elsewhere.png): the six-bar mark as the
// desktop icon at 128 and 64 px on its own ground, the bar mark at 32 and
// 16 px in the accent, and the light desktop icon, each at the artboard's
// position so the visual job cuts the same rectangles from both.
Item {
    id: root

    readonly property int margin: Theme.space8 + Theme.hairlineWidth
    readonly property int markTop: Theme.space8 * 3 + Theme.space6 + Theme.space1
    readonly property int rowTop: Theme.space8 * 5 - Theme.space2 - Theme.space1
    readonly property int largeSize: Theme.space8 * 2 + Theme.space7
    readonly property int mediumSize: Theme.space8 + Theme.space5
    readonly property int smallSize: Theme.space7
    readonly property int barSize: Theme.space5
    readonly property bool light: Theme.source === "builtin-light"

    Column {
        anchors.left: parent.left
        anchors.leftMargin: root.margin
        anchors.top: parent.top
        anchors.topMargin: Theme.space8 - Theme.space2
        spacing: Theme.space3

        SectionLabel {
            text: qsTr("App icon · OSD outside Omarchy")
        }

        Text {
            color: Theme.roleText
            font.family: Theme.fontFamily
            font.letterSpacing: Theme.typeTitleSize * Theme.typeTitleTracking
            font.pixelSize: Theme.typeTitleSize
            font.weight: Theme.typeEmphasisWeight
            text: qsTr("The mark, and the pill on other desktops")
            Accessible.role: Accessible.Heading
            Accessible.name: text
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.leftMargin: root.margin
        anchors.right: parent.right
        anchors.rightMargin: root.margin
        color: Theme.roleHairline
        height: Theme.hairlineWidth
        y: Theme.space8 * 2 + Theme.space7 + Theme.space1
    }

    SectionLabel {
        anchors.left: parent.left
        anchors.leftMargin: root.margin
        text: qsTr("Icon · waveform mark, square, no rounding of its own")
        y: Theme.space8 * 2 + Theme.space8 - Theme.space2
    }

    readonly property int mediumX: root.margin + root.largeSize + Theme.space7
    readonly property int smallX: root.mediumX + root.mediumSize + Theme.space7
    readonly property int barX: root.smallX + root.smallSize + Theme.space8 + Theme.space2 + Theme.hairlineWidth
    readonly property int lightX: root.smallX + root.smallSize + Theme.space8 * 2 + Theme.space6 + Theme.space1
    readonly property int labelY: root.rowTop + root.mediumSize + Theme.space4

    Image {
        height: root.largeSize
        source: root.light ? "qrc:/qt/qml/Dettivo/icons/dettivo-light.svg" : "qrc:/qt/qml/Dettivo/icons/dettivo.svg"
        sourceSize.height: root.largeSize
        sourceSize.width: root.largeSize
        width: root.largeSize
        x: root.margin
        y: root.markTop
        Accessible.role: Accessible.Graphic
        Accessible.name: qsTr("Desktop icon at 128")

        Rectangle {
            anchors.fill: parent
            border.color: Qt.alpha(Theme.roleAccent, 0.5)
            border.width: Theme.hairlineWidth
            color: "transparent"
        }
    }

    Image {
        height: root.mediumSize
        source: root.light ? "qrc:/qt/qml/Dettivo/icons/dettivo-light.svg" : "qrc:/qt/qml/Dettivo/icons/dettivo.svg"
        sourceSize.height: root.mediumSize
        sourceSize.width: root.mediumSize
        width: root.mediumSize
        x: root.mediumX
        y: root.rowTop
        Accessible.role: Accessible.Graphic
        Accessible.name: qsTr("Desktop icon at 64")

        Rectangle {
            anchors.fill: parent
            border.color: Qt.alpha(Theme.roleAccent, 0.5)
            border.width: Theme.hairlineWidth
            color: "transparent"
        }
    }

    Icon {
        accessibleName: qsTr("Mark at 32")
        color: Theme.roleAccent
        size: root.smallSize
        source: "sixbar"
        x: root.smallX
        y: root.rowTop + root.mediumSize - root.smallSize
    }

    Icon {
        accessibleName: qsTr("Mark at 16")
        color: Theme.roleAccent
        size: root.barSize
        source: "sixbar"
        x: root.barX
        y: root.rowTop + root.mediumSize - root.barSize
    }

    Image {
        height: root.mediumSize
        source: "qrc:/qt/qml/Dettivo/icons/dettivo-light.svg"
        sourceSize.height: root.mediumSize
        sourceSize.width: root.mediumSize
        width: root.mediumSize
        x: root.lightX
        y: root.rowTop
        Accessible.role: Accessible.Graphic
        Accessible.name: qsTr("Light desktop icon at 64")
    }

    Repeater {
        model: [[root.margin + root.largeSize / 2, qsTr("128")], [root.mediumX + root.mediumSize / 2, qsTr("64")], [root.smallX + root.smallSize / 2, qsTr("32")], [root.barX + root.barSize / 2, qsTr("16 · bar")], [root.lightX + root.mediumSize / 2, qsTr("light")]]

        delegate: SectionLabel {
            id: sizeLabel
            required property var modelData
            text: sizeLabel.modelData[1]
            x: sizeLabel.modelData[0] - width / 2
            y: root.labelY
        }
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: root.margin
        color: Theme.roleMutedText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.typeBodySize
        text: qsTr("Same six bars as the bar glyph and the in-app logo. The icon ships as SVG in the hicolor theme; the compositor or icon theme applies rounding if it wants any. The bars recolour to the theme accent at runtime in the bar and the app; the desktop icon stays gold on black and blue on cream for light.")
        width: Theme.space8 * 10 + Theme.space8 + Theme.space6
        wrapMode: Text.WordWrap
        y: root.rowTop + root.mediumSize + Theme.space8 + Theme.space5
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Icon sheet")
}
