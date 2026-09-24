pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// First run (first-run-*.png, ADR 0024): one centred column with the
// mark and the three-step marker on top, the step the model names in
// the middle (Keys, Models, Try it) and the step's footer at the bottom.
// No welcome screen, no sidebar: the page is the window while it lasts.
FocusFlickable {
    id: root

    property var router: null
    property var firstRun: null
    property var status: null
    contentWidth: width
    contentHeight: Math.max(height, Theme.appWindowHeight)

    readonly property string step: root.firstRun ? root.firstRun.step : "keys"
    readonly property int stepIndex: root.firstRun ? root.firstRun.stepIndex : 0
    readonly property string stepLabel: qsTr("Step %1 of 3").arg(root.stepIndex + 1)
    readonly property int columnX: Math.max(Theme.pagePaddingX, Math.round((root.width - Theme.firstRunContentWidth) / 2))
    readonly property int columnWidth: Math.min(Theme.firstRunContentWidth, root.width - 2 * Theme.pagePaddingX)

    Item {
        id: header
        anchors.left: parent.left
        anchors.leftMargin: root.columnX
        anchors.top: parent.top
        anchors.topMargin: Theme.firstRunPaddingY
        height: Theme.typeBodySize + Theme.space3
        width: root.columnWidth

        Row {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.space3 + Theme.space1

            Icon {
                accessibleName: ""
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleAccent
                size: Theme.iconSize
                source: "sixbar"
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                font.weight: Theme.typeEmphasisWeight
                text: qsTr("dettivo")
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Dettivo")
            }
        }

        Row {
            id: stepper
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.space3

            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.space3

                Repeater {
                    model: 3

                    delegate: Rectangle {
                        required property int index
                        color: index <= root.stepIndex ? Theme.roleAccent : Theme.roleFaintText
                        height: Theme.space2 + Theme.space1
                        radius: Theme.radius
                        width: height
                    }
                }
            }

            SectionLabel {
                anchors.verticalCenter: parent.verticalCenter
                bottomPadding: 0
                leftPadding: 0
                text: qsTr("Keys · Models · Try it")
                topPadding: 0
            }
        }

        Accessible.role: Accessible.Pane
        Accessible.name: root.stepLabel
    }

    Loader {
        id: stepLoader
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.space8 - Theme.space3
        anchors.left: parent.left
        anchors.leftMargin: root.columnX
        anchors.top: header.bottom
        anchors.topMargin: Theme.space6 + Theme.space2
        width: root.columnWidth
        sourceComponent: {
            switch (root.step) {
            case "models":
                return modelsStep;
            case "try":
                return tryStep;
            default:
                return keysStep;
            }
        }
    }

    Component {
        id: keysStep
        KeysStep {
            firstRun: root.firstRun
            status: root.status
        }
    }

    Component {
        id: modelsStep
        ModelsStep {
            firstRun: root.firstRun
        }
    }

    Component {
        id: tryStep
        TryItStep {
            firstRun: root.firstRun
            status: root.status
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("First run")
}
