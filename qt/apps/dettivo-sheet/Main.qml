import QtQuick
import QtQuick.Layouts
import Dettivo

// The component sheet: palette, type, spacing, controls, components, icons
// and motion on one 1280 by 1700 canvas, laid out like the approved
// design-system baseline so the two can be compared side by side. With
// `--icon` the window is the icon page instead (icon-and-osd-elsewhere.png,
// 1280 by 620): the mark at four sizes and the light variant.
Window {
    id: sheet

    property bool icon: false

    color: Theme.roleSurface
    height: sheet.icon ? 620 : 1700
    title: sheet.icon ? qsTr("Dettivo icon") : qsTr("Dettivo design system")
    visible: true
    width: 1280

    IconSheet {
        anchors.fill: parent
        visible: sheet.icon
    }

    Flickable {
        anchors.fill: parent
        visible: !sheet.icon
        contentHeight: page.implicitHeight + Theme.space8 * 2
        contentWidth: width

        ColumnLayout {
            id: page
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: Theme.space8
            anchors.top: parent.top
            spacing: Theme.space7

            SheetHeader {
                Layout.fillWidth: true
            }
            SheetPalette {
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.space8

                SheetType {
                    Layout.fillWidth: true
                }
                SheetSpacing {
                    Layout.fillWidth: true
                }
            }
            SheetControls {
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.space8

                SheetIcons {
                    Layout.fillWidth: true
                }
                SheetMotion {
                    Layout.fillWidth: true
                }
            }
        }
    }
}
