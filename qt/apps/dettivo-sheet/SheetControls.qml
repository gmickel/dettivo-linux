import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Dettivo

// Every styled control in every state, the inputs and chips, and the row
// states with the components that sit beside them.
ColumnLayout {
    spacing: Theme.space3

    SectionLabel {
        text: qsTr("Controls · states use the shell's fill and border alphas")
    }
    Rectangle {
        Layout.fillWidth: true
        border.color: Theme.roleHairline
        border.width: Theme.hairlineWidth
        color: "transparent"
        implicitHeight: column.implicitHeight + Theme.space6 * 2

        ColumnLayout {
            id: column
            anchors.fill: parent
            anchors.margins: Theme.space6
            spacing: Theme.space6

            Flow {
                Layout.fillWidth: true
                spacing: Theme.space5

                Button {
                    text: qsTr("Normal")
                }
                Button {
                    hoverEnabled: true
                    text: qsTr("Hover")
                }
                Button {
                    focusPolicy: Qt.StrongFocus
                    text: qsTr("Focus")
                }
                Button {
                    checkable: true
                    checked: true
                    text: qsTr("Selected")
                }
                Button {
                    down: true
                    text: qsTr("Pressed")
                }
                Button {
                    highlighted: true
                    icon.name: "mic"
                    text: qsTr("Start dictation")
                }
                Button {
                    danger: true // qmllint disable missing-property
                    text: qsTr("Delete meeting")
                }
                Button {
                    enabled: false
                    text: qsTr("Disabled")
                }
            }
            Flow {
                Layout.fillWidth: true
                spacing: Theme.space5

                TextField {
                    placeholderText: qsTr("Search transcripts")
                    width: Theme.controlHeight * 10
                }
                TextField {
                    focus: true
                    text: qsTr("merger overlap")
                    width: Theme.controlHeight * 10
                }
                SegmentedControl {
                    currentIndex: 2
                    label: qsTr("Mode")
                    model: [qsTr("Raw"), qsTr("Polish"), qsTr("Enhanced")]
                }
                Row {
                    spacing: Theme.space3

                    Chip {
                        text: qsTr("Raw")
                    }
                    Chip {
                        text: qsTr("Polish")
                    }
                    Chip {
                        accent: true
                        text: qsTr("Enhanced")
                    }
                    Chip {
                        dotColor: Theme.roleUrgent
                        text: qsTr("Failed")
                    }
                }
            }
            Flow {
                Layout.fillWidth: true
                spacing: Theme.space5

                CheckBox {
                    checked: true
                    text: qsTr("Microphone")
                }
                RadioButton {
                    checked: true
                    text: qsTr("Large v3 Turbo")
                }
                Switch {
                    checked: true
                    text: qsTr("Sounds")
                }
                ComboBox {
                    model: [qsTr("Whisper (local)"), qsTr("Parakeet")]
                    width: Theme.controlHeight * 8
                }
                ProgressBar {
                    value: 0.42
                    width: Theme.controlHeight * 6
                }
                ProgressBar {
                    indeterminate: true
                    width: Theme.controlHeight * 6
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.space6

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredWidth: Theme.controlHeight * 20
                    border.color: Theme.roleHairline
                    border.width: Theme.hairlineWidth
                    color: "transparent"
                    implicitHeight: rows.implicitHeight

                    Column {
                        id: rows
                        anchors.left: parent.left
                        anchors.right: parent.right

                        ListRow {
                            width: rows.width
                            leading: "13:12"
                            text: qsTr("Add a regression test for the merger overlap case before we ship.")
                            trailing: "4 s"
                            trailingItem: Chip {
                                text: "ghostty"
                            }
                        }
                        ListRow {
                            width: rows.width
                            leading: "12:58"
                            text: qsTr("Reply to Mara: yes, keep the Vulkan build as the default package.")
                            trailing: "6 s"
                            trailingItem: Chip {
                                text: "chromium"
                            }
                        }
                        ListRow {
                            width: rows.width
                            leading: "12:40"
                            selected: true
                            separator: false
                            text: qsTr("Summarise the diarization thresholds in the spec.")
                            trailing: "3 s"
                            trailingItem: Chip {
                                text: "cursor"
                            }
                        }
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.preferredWidth: Theme.controlHeight * 20
                    spacing: Theme.space3

                    SectionLabel {
                        text: qsTr("Row states · normal, hover, selected")
                    }
                    Text {
                        Layout.fillWidth: true
                        color: Theme.roleMutedText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.typeBodySize
                        text: qsTr("Selection is an 18% accent fill with a 2 px accent rail on the leading edge. Hover is 8% hover colour. No shadows, no rounded chips, no zebra stripes.")
                        wrapMode: Text.WordWrap
                    }
                    Row {
                        spacing: Theme.space3

                        KeyCap {
                            text: "F9"
                        }
                        KeyCap {
                            text: "Ctrl"
                        }
                        KeyCap {
                            text: "K"
                        }
                        StatusDot {
                            active: true
                            label: qsTr("Recording")
                            status: StatusDot.Accent
                        }
                        StatusDot {
                            label: qsTr("Error")
                            status: StatusDot.Urgent
                        }
                    }
                }
            }
        }
    }
}
