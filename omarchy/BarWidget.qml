import QtQuick
import qs.Commons
import qs.Ui

// The Dettivo bar widget: the 16 px mark from the shared Dettivo module
// (idle, listening, transcribing, meeting with its timer, error), the
// shell's active underline while a take runs, and the panel on click.
// The module is loaded through a Loader so a machine without
// /usr/lib/dettivo/qml shows a dimmed glyph and the install hint
// instead of a QML error.
BarWidget {
    id: root

    moduleName: "gmickel.dettivo"

    readonly property bool recording: DettivoState.dictationState === "recording"
    // The popup and the bar as plain objects: their methods are theirs.
    readonly property var popup: panelLoader.item
    readonly property var host: root.bar
    readonly property bool opened: root.popup ? root.popup.opened === true : false
    readonly property bool popoutSwitchClosing: root.popup ? root.popup.popoutSwitchClosing === true : false
    readonly property string tooltip: {
        if (DettivoState.hint === "install")
            return "Dettivo — install dettivo (yay -S dettivo-bin)";
        if (DettivoState.hint === "upgrade")
            return "Dettivo — upgrade dettivo to " + DettivoState.minDettivo;
        if (!DettivoState.daemonAvailable)
            return "Dettivo — daemon unavailable";
        if (root.recording)
            return "Dettivo — listening";
        if (DettivoState.meetingFinalising)
            return "Dettivo — meeting transcribing";
        if (DettivoState.meetingActive)
            return "Dettivo — meeting" + (DettivoState.meetingElapsed.length > 0 ? " " + DettivoState.meetingElapsed : "");
        return "Dettivo — ready";
    }

    function injectPanel() {
        const target = root.popup;
        if (!target)
            return;
        if ("bar" in target)
            target.bar = root.bar;
        if ("settings" in target)
            target.settings = root.settings;
        if ("anchorItem" in target)
            target.anchorItem = button;
        if ("hostWidget" in target)
            target.hostWidget = root;
    }

    // The bar's summon, hide and toggle routing needs open, close and
    // opened on the widget root.
    function open() {
        if (root.popup)
            root.popup.open();
    }
    function close() {
        if (root.popup)
            root.popup.close();
    }
    function toggle() {
        if (root.popup)
            root.popup.toggle();
    }
    function closeForPopoutSwitch() {
        if (root.popup)
            root.popup.closeForPopoutSwitch();
    }

    // The shell's settings sheet writes the three keys here; the file is
    // the source of truth, so each one goes through `dettivo config set`.
    function pushSettings() {
        const glyph = root.setting("glyph", "");
        if (glyph !== "" && glyph !== DettivoState.glyph)
            DettivoState.setSetting("glyph", glyph);
        const meter = root.setting("levelMeter", null);
        if (meter !== null && meter !== DettivoState.levelMeter)
            DettivoState.setSetting("level_meter", meter ? "true" : "false");
        const osd = root.setting("osd", "");
        if (osd !== "" && osd !== DettivoState.osdMode)
            DettivoState.setSetting("osd", osd);
    }

    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight

    onBarChanged: injectPanel()
    onSettingsChanged: {
        pushSettings();
        injectPanel();
    }
    Component.onCompleted: injectPanel()

    Loader {
        id: panelLoader
        active: true
        source: Qt.resolvedUrl("PanelPopup.qml")
        visible: false
        onLoaded: {
            root.injectPanel();
            Qt.callLater(root.injectPanel);
        }
    }

    WidgetButton {
        id: button
        anchors.fill: parent
        bar: root.bar
        // The mark is drawn by the module; the text keeps the slot sized
        // and stands in as a plain glyph when the module is missing.
        text: glyphLoader.status === Loader.Ready ? "" : ""
        hasVisualContent: glyphLoader.status === Loader.Ready || text !== ""
        labelVisible: glyphLoader.status !== Loader.Ready
        fixedWidth: glyphLoader.status === Loader.Ready ? glyphLoader.width + Style.spaceReal(horizontalMargin) * 2 : -1
        foreground: root.host ? root.host.barForeground : Color.foreground
        dimmed: DettivoState.dimmed
        active: root.recording
        useActiveColor: false
        tooltipText: root.tooltip

        onPressed: function (b) {
            if (!root.host)
                return;
            if (b === Qt.LeftButton)
                root.toggle();
            else if (b === Qt.MiddleButton)
                DettivoState.dictate();
        }

        Loader {
            id: glyphLoader
            anchors.centerIn: parent
            active: DettivoState.moduleAvailable
            source: Qt.resolvedUrl("DettivoGlyph.qml")
        }
    }
}
