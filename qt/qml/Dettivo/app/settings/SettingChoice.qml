pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// A key with a fixed set of values: the segments for a short set (the
// mode segment of Home), the styled combo box for a long one (the
// insertion backends, the models). Both are named by the key; a segment
// is a tab named by its label, so a drive picks a value by name.
Item {
    id: root

    property var choices: []
    property var choiceLabels: []
    property string current: ""
    property string name: ""
    // Segments up to this many choices; a combo box beyond.
    property int segmentLimit: 5

    signal chosen(string choice)

    readonly property var effectiveChoices: root.choices.indexOf(root.current) >= 0 ? root.choices : root.choices.concat([root.current])
    readonly property bool segmented: root.effectiveChoices.length <= root.segmentLimit
    readonly property int currentIndex: root.effectiveChoices.indexOf(root.current)
    readonly property var controlItem: loader.item

    function labelAt(index) {
        if (index >= root.choices.length)
            return root.current.length > 0 ? qsTr("Custom / unavailable: %1").arg(root.current) : qsTr("Not selected");
        return root.choiceLabels.length > index ? root.choiceLabels[index] : String(root.effectiveChoices[index]);
    }

    implicitHeight: Theme.controlHeight
    implicitWidth: root.controlItem ? root.controlItem.implicitWidth : Theme.settingsControlWidth

    Loader {
        id: loader
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        sourceComponent: root.segmented ? segments : combo
    }

    Component {
        id: segments
        SegmentedControl {
            currentIndex: root.currentIndex
            enabled: root.enabled
            label: root.name
            model: root.effectiveChoices.map((c, i) => root.labelAt(i))
            opacity: root.enabled ? 1.0 : 0.5
            onSelected: index => root.chosen(String(root.effectiveChoices[index]))
        }
    }

    Component {
        id: combo
        ComboBox {
            id: box
            enabled: root.enabled
            model: root.effectiveChoices.map((c, i) => root.labelAt(i))
            width: root.width > 0 ? root.width : Theme.settingsControlWidth
            onActivated: index => root.chosen(String(root.effectiveChoices[index]))
            Accessible.name: root.name

            // A new model resets the box's index, so the key's value is
            // applied after every change rather than bound once.
            Component.onCompleted: box.currentIndex = root.currentIndex
            onModelChanged: box.currentIndex = root.currentIndex

            Connections {
                target: root
                function onCurrentIndexChanged() {
                    box.currentIndex = root.currentIndex;
                }
            }
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("%1 choice").arg(root.name)
}
