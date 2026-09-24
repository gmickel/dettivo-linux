import QtQuick
import QtQuick.Controls
import DettivoStyle as Style
import Dettivo

// The search input of History: the glyph, the field with the shell's
// focus border, the hit count while a query is set and the clear glyph.
// The query reaches the model after a short pause so a word typed at
// speed is one search, not one per key.
Item {
    id: root

    property string placeholder: qsTr("Search")
    property string fieldName: qsTr("Search history")
    property string clearName: qsTr("Clear search")
    property string containerName: qsTr("Search")
    property int hits: 0
    property bool searching: false
    readonly property bool focused: input.activeFocus
    readonly property string text: input.text

    signal queryChanged(string query)

    implicitHeight: input.implicitHeight

    function takeFocus() {
        input.forceActiveFocus();
        input.selectAll();
    }

    function dropFocus() {
        input.focus = false;
        root.forceActiveFocus();
    }

    function clear() {
        input.clear();
        debounce.stop();
        root.queryChanged("");
    }

    TextField {
        id: input
        anchors.fill: parent
        leftPadding: Theme.space2 + Theme.iconSize + Theme.space2
        placeholderText: root.placeholder
        rightPadding: Theme.space2 + trailing.implicitWidth + Theme.space2
        onTextEdited: debounce.restart()
        onAccepted: {
            debounce.stop();
            root.queryChanged(input.text);
        }

        Accessible.role: Accessible.EditableText
        Accessible.name: root.fieldName
    }

    Icon {
        accessibleName: ""
        anchors.left: parent.left
        anchors.leftMargin: Theme.space2
        anchors.verticalCenter: parent.verticalCenter
        color: input.activeFocus ? Theme.roleText : Theme.roleMutedText
        size: Theme.iconSize
        source: "search"
    }

    Row {
        id: trailing
        anchors.right: parent.right
        anchors.rightMargin: Theme.space2
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.space2

        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.roleMutedText
            font.capitalization: Font.AllUppercase
            font.family: Theme.fontFamily
            font.letterSpacing: Theme.typeLabelSize * Theme.typeLabelTracking
            font.pixelSize: Theme.typeLabelSize
            text: root.hits === 1 ? qsTr("1 hit") : qsTr("%1 hits").arg(root.hits)
            visible: root.searching
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        Style.SearchClearButton {
            anchors.verticalCenter: parent.verticalCenter
            visible: input.text.length > 0
            Accessible.name: root.clearName
            onClicked: root.clear()
        }
    }

    Timer {
        id: debounce
        interval: Motion.durationReveal
        onTriggered: root.queryChanged(input.text)
    }

    Accessible.role: Accessible.Pane
    Accessible.name: root.containerName
}
