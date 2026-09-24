import QtQuick
import qs.Commons
import qs.Ui

// The popup the bar widget opens: the shell's keyboard panel anchored to
// the widget, hosting the module's 340 px panel. Not a manifest panel
// kind; BarWidget.qml loads it, so the shell's toggle IPC stays with the
// pill host in Panel.qml.
Panel {
    id: root

    moduleName: "gmickel.dettivo"
    ipcTarget: ""
    manageIpc: false

    property var anchorItem: null
    property var hostWidget: null

    readonly property var barIdentity: hostWidget || root
    // The bar as a plain object: switchPanelFrom is the bar's own method.
    readonly property var host: root.bar

    function open() {
        DettivoState.refresh();
        root.controller.show();
    }
    function close() {
        root.controller.hide();
    }
    function toggle() {
        if (root.opened)
            root.close();
        else
            root.open();
    }
    function switchPanel(direction) {
        if (root.host && typeof root.host.switchPanelFrom === "function")
            return root.host.switchPanelFrom(root.barIdentity, direction);
        return false;
    }

    KeyboardPanel {
        id: panel
        anchorItem: root.anchorItem
        owner: root.barIdentity
        bar: root.bar
        open: root.opened
        focusTarget: keyCatcher
        padding: 0
        contentWidth: panel.fittedContentWidth(content.active ? content.implicitWidth : Style.space(340))
        contentHeight: panel.fittedContentHeight(content.active ? content.implicitHeight : hintText.implicitHeight + Style.space(28))

        PanelKeyCatcher {
            id: keyCatcher
            anchors.fill: parent
            onReturnRequested: DettivoState.dictate()
            onCloseRequested: root.close()
            onTabRequested: function (direction) {
                root.switchPanel(direction);
            }

            Loader {
                id: content
                anchors.fill: parent
                active: DettivoState.moduleAvailable
                source: Qt.resolvedUrl("DettivoPanelContent.qml")
            }

            // The panel content asks to close after opening the app.
            Connections {
                target: content.item
                ignoreUnknownSignals: true
                function onOpenRequested() {
                    root.close();
                }
            }

            // Without the module there is nothing of Dettivo to draw with,
            // so the hint is the shell's own text.
            Text {
                id: hintText
                visible: !content.active
                anchors.fill: parent
                anchors.margins: Style.space(14)
                text: "Install Dettivo\n\nyay -S dettivo-bin\nthe shared module at " + DettivoState.moduleDir + " is missing"
                color: Color.popups.text
                font.family: Style.font.family
                font.pixelSize: Style.font.body
                wrapMode: Text.WordWrap
            }
        }
    }
}
