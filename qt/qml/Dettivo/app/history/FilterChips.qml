pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The kind filters of the History list: All, Dictation, Meeting, Import
// as tracked-caps chips, the chosen one on the accent.
Row {
    id: root

    property string current: "all"

    signal chosen(string filter)

    readonly property var filters: [
        {
            "id": "all",
            "label": qsTr("All")
        },
        {
            "id": "dictation",
            "label": qsTr("Dictation")
        },
        {
            "id": "meeting",
            "label": qsTr("Meeting")
        },
        {
            "id": "import",
            "label": qsTr("Import")
        }
    ]

    spacing: Theme.space2

    Repeater {
        model: root.filters

        delegate: Chip {
            id: chip
            required property var modelData
            accent: root.current === chip.modelData.id
            text: chip.modelData.label
            onClicked: root.chosen(chip.modelData.id)

            activeFocusOnTab: true
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                    root.chosen(chip.modelData.id);
                    event.accepted = true;
                }
            }

            FocusRing {}

            Accessible.role: Accessible.RadioButton
            Accessible.name: chip.modelData.label
            Accessible.checkable: true
            Accessible.checked: chip.accent
            Accessible.onPressAction: root.chosen(chip.modelData.id)
        }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Kind filters")
}
