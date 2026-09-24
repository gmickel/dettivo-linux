pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Dettivo

// The dictation detail (history.png): the title with Copy transcript, Re-run
// and Export, the when line, Enhanced first with the mode that produced
// it, Raw in the light weight, the audio strip with the played portion
// in accent, the facts grid, and delete as text in the urgent colour.
// Without an item the designed state says what to pick; a refused action
// names the daemon's reason where the action sits.
Item {
    id: root

    property string itemId: ""
    property var detail: null
    property var actions: null
    property var player: null

    readonly property string transcript: root.detail ? (root.detail.enhancedText || root.detail.rawText || "") : ""
    readonly property bool hasItem: root.itemId.length > 0
    readonly property bool loaded: root.detail ? root.detail.loaded : false
    readonly property string failure: root.detail ? root.detail.error : ""
    property string actionError: ""
    property string actionNote: ""

    function rerun() {
        if (root.loaded)
            rerunDialog.openFor(root.itemId);
    }

    function exportSheet() {
        if (root.loaded)
            sheet.openFor(root.itemId);
    }

    function confirmDelete() {
        if (root.loaded)
            deleteConfirm.openFor(root.itemId);
    }

    onItemIdChanged: {
        root.actionError = "";
        root.actionNote = "";
    }

    // The player follows the detail: the take when there is one, nothing
    // when the item has none or the detail is cleared.
    Connections {
        target: root.detail
        function onChanged() {
            if (root.player && root.detail)
                root.player.source = root.detail.audioRetained ? root.detail.audioPath : "";
        }
    }

    Connections {
        target: root.actions
        function onFailed(action, reason) {
            root.actionError = reason;
        }
        function onExported(path, bytes) {
            root.actionNote = qsTr("Wrote %1.").arg(path);
        }
        function onRerunStarted(itemId, newItemId, jobId) {
            root.actionNote = qsTr("Re-run started as %1.").arg(newItemId.slice(0, 8));
        }
    }

    RouteScaffold {
        anchors.fill: parent
        stateReason: root.hasItem ? (root.failure.length > 0 ? root.failure : qsTr("Reading the dictation.")) : qsTr("Pick a dictation in History to see it here.")
        stateTitle: root.hasItem ? (root.failure.length > 0 ? qsTr("Dictation unavailable.") : qsTr("Dictation selected.")) : qsTr("No dictation selected.")
        subtitle: qsTr("Raw and final text, the take and the insertion facts.")
        title: qsTr("Dictation")
        urgent: root.failure.length > 0
        visible: !root.loaded
    }

    Flickable {
        id: scroll
        anchors.bottom: footer.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        clip: true
        contentHeight: column.implicitHeight + Theme.pagePaddingY * 2
        visible: root.loaded

        Column {
            id: column
            anchors.left: parent.left
            anchors.leftMargin: Theme.pagePaddingX - Theme.space2
            anchors.right: parent.right
            anchors.rightMargin: Theme.pagePaddingX
            anchors.top: parent.top
            anchors.topMargin: Theme.pagePaddingY
            spacing: Theme.space5

            Item {
                height: title.implicitHeight
                width: parent.width

                Text {
                    id: title
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    color: Theme.roleText
                    font.family: Theme.fontFamily
                    font.letterSpacing: Theme.typeTitleSize * Theme.typeTitleTracking
                    font.pixelSize: Theme.typeTitleSize
                    font.weight: Theme.typeEmphasisWeight
                    text: qsTr("Dictation")
                    Accessible.role: Accessible.Heading
                    Accessible.name: qsTr("Dictation")
                }

                Row {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.space2

                    Button {
                        enabled: root.actions !== null && root.transcript.length > 0
                        icon.name: "copy"
                        text: qsTr("Copy transcript")
                        onClicked: {
                            root.actionError = "";
                            if (root.actions.copyText(root.transcript))
                                root.actionNote = qsTr("Copied. Switch to your app and paste (Ctrl+V; Ctrl+Shift+V in terminals).");
                            else
                                root.actionError = qsTr("Clipboard unavailable.");
                        }
                    }

                    Button {
                        enabled: root.detail !== null && root.detail.canRerun && root.actions !== null && !root.actions.busy
                        icon.name: "rerun"
                        text: qsTr("Re-run")
                        onClicked: root.rerun()
                    }

                    Button {
                        enabled: root.actions !== null && !root.actions.busy
                        icon.name: "export"
                        text: qsTr("Export")
                        onClicked: root.exportSheet()
                    }
                }
            }

            Text {
                color: Theme.roleMutedText
                elide: Text.ElideRight
                font.family: Theme.fontFamily
                font.features: Theme.typeTabularNumerals
                font.pixelSize: Theme.typeBodySize
                text: root.detail ? root.detail.whenLine : ""
                width: parent.width
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("When")
            }

            Text {
                color: root.actionError.length > 0 ? Theme.roleUrgent : Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                text: root.actionError.length > 0 ? root.actionError : root.actionNote
                visible: text.length > 0
                width: parent.width
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Action outcome")
            }

            DetailBlock {
                body: root.detail ? root.detail.enhancedText : ""
                chip: root.detail ? root.detail.outcomeLabel : ""
                label: qsTr("Enhanced")
                meta: root.detail ? root.detail.enhancedMeta : ""
                width: parent.width
            }

            DetailBlock {
                body: root.detail ? root.detail.rawText : ""
                label: qsTr("Raw")
                light: true
                meta: root.detail ? root.detail.rawMeta : ""
                width: parent.width
            }

            AudioStrip {
                meta: root.detail ? root.detail.audioMeta : ""
                player: root.player
                reason: root.detail ? root.detail.audioReason : ""
                retained: root.detail ? root.detail.audioRetained : false
                width: parent.width
            }

            FactsGrid {
                facts: root.detail ? root.detail.facts : []
                width: parent.width
            }

            Text {
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeCaptionSize
                text: root.detail && root.detail.progress >= 0 ? qsTr("Re-run %1 · %2 %").arg(root.detail.stage).arg(Math.round(root.detail.progress * 100)) : ""
                visible: text.length > 0
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Re-run progress")
            }
        }
    }

    Item {
        id: footer
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.pagePaddingY
        anchors.left: parent.left
        anchors.leftMargin: Theme.pagePaddingX - Theme.space2
        anchors.right: parent.right
        anchors.rightMargin: Theme.pagePaddingX
        height: deleteText.implicitHeight
        visible: root.loaded

        Text {
            id: deleteText
            anchors.right: parent.right
            color: Theme.roleUrgent
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: qsTr("delete")

            TapHandler {
                onTapped: root.confirmDelete()
            }

            activeFocusOnTab: true
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                    root.confirmDelete();
                    event.accepted = true;
                }
            }

            FocusRing {}

            Accessible.role: Accessible.Button
            Accessible.name: qsTr("Delete")
            Accessible.onPressAction: root.confirmDelete()
        }
    }

    RerunDialog {
        id: rerunDialog
        actions: root.actions
        anchors.centerIn: parent
        detail: root.detail
    }

    ExportSheet {
        id: sheet
        actions: root.actions
        anchors.centerIn: parent
    }

    DeleteConfirm {
        id: deleteConfirm
        actions: root.actions
        anchors.centerIn: parent
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Dictation detail")
}
