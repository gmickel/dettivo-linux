import QtQuick
import QtTest
import Quickshell
import "DettivoText.js" as DettivoText
import "plugin_support.js" as Support

// The Omarchy plugin under the shell shim (fn-29 R2, R5): a missing
// module shows the install hint and an old daemon the upgrade hint, and
// the pill host claims its bus name in panel mode with the module present
// only, showing its window once the claim is answered. The staging
// script copies this file beside the plugin, so the sibling types
// resolve as they do in the shell.
TestCase {
    id: root
    name: "OmarchyPluginHints"
    width: 480
    height: 520
    visible: true
    when: windowShown

    Component {
        id: widgetC
        BarWidget {}
    }

    Component {
        id: hostC
        Panel {}
    }

    function test_1_install_hint_without_the_module() {
        Quickshell.environment = {
            "DETTIVO_QML_DIR": "/nonexistent/Dettivo"
        };
        const widget = createTemporaryObject(widgetC, root);
        verify(widget);
        Support.processFor(Quickshell, ["test", "-f"]).exit(1);
        Support.answerVersion(Quickshell, "0.1.0");
        compare(DettivoState.moduleAvailable, false);
        compare(DettivoState.hint, "install");
        compare(DettivoState.panelState, "hint");
        const glyph = Support.glyphLoader(widget);
        verify(glyph);
        compare(glyph.active, false);
        verify(widget.tooltip.indexOf("install dettivo") > 0, widget.tooltip);
        const content = Support.contentLoader(widget);
        verify(content);
        compare(content.active, false);
        const host = createTemporaryObject(hostC, root);
        compare(host.hosting, false);
        // No module to draw with: the name stays free for dettivo-osd.
        compare(host.claiming, false);
    }

    function test_2_upgrade_hint_against_an_old_daemon() {
        Quickshell.environment = {};
        DettivoState.refresh();
        Support.processFor(Quickshell, ["test", "-f"]).exit(0);
        Support.answerVersion(Quickshell, "0.0.1");
        compare(DettivoState.moduleAvailable, true);
        compare(DettivoState.hint, "upgrade");
        verify(DettivoState.hintDetail.indexOf("daemon 0.0.1") === 0, DettivoState.hintDetail);
        const widget = createTemporaryObject(widgetC, root);
        const glyph = Support.glyphLoader(widget);
        compare(glyph.active, true);
        tryCompare(glyph, "status", Loader.Ready);
        compare(glyph.item.dimmed, true);
        verify(Support.button(widget).visible, "the loaded glyph must keep the button visible");
        const panel = Support.contentLoader(widget);
        tryCompare(panel, "status", Loader.Ready);
        compare(panel.item.state, "hint");
        compare(panel.item.sentence, "Upgrade Dettivo");
        verify(panel.item.reason.indexOf("plugin needs " + DettivoState.minDettivo) > 0, panel.item.reason);
    }

    function test_3_the_pill_host_claims_the_name_in_panel_mode_only() {
        // The singleton is created on first use; touch it before its
        // processes are answered.
        compare(DettivoState.binary, "dettivo");
        Support.readyDaemon(Quickshell);
        compare(DettivoState.hint, "");
        const host = createTemporaryObject(hostC, root);
        compare(host.hosting, true);
        compare(host.claiming, true);
        const claim = Support.processFor(Quickshell, ["osd", "host-panel"]);
        verify(claim);
        compare(claim.command, ["dettivo", "--json", "osd", "host-panel"]);
        compare(host.pillWindow.visible, false);
        compare(host.pillWindow.anchors.bottom, true);
        compare(host.pillWindow.anchors.right, true);
        compare(host.pillWindow.margins.bottom, 32);
        // The window shows once the claim has answered, not before.
        DettivoState.pillState = "listening";
        compare(host.claimed, false);
        compare(host.pillWindow.visible, false);
        claim.feed(JSON.stringify({
            "claimed": true,
            "name": "dev.dettivo.OmarchyPanel"
        }));
        compare(host.claimed, true);
        compare(host.pillWindow.visible, true);
        DettivoState.pillState = "hidden";
        compare(host.pillWindow.visible, false);
        DettivoState.osdMode = "service";
        compare(host.claiming, false);
        compare(host.hosting, false);
        DettivoState.osdMode = "panel";
        compare(host.claiming, true);
        // The claimer going away releases the window with it.
        DettivoState.pillState = "listening";
        claim.exit(1);
        compare(host.claimed, false);
        compare(host.pillWindow.visible, false);
        DettivoState.pillState = "hidden";
    }

    // `dettivo dictation stop --print` completes as done, into no window.
    function test_4_a_printed_take_shows_as_done_without_a_window() {
        const done = DettivoText.completion({
            "outcome": "printed",
            "target_app": {}
        }, "Hello");
        compare(done.pill, "inserted");
        compare(done.texts.target, "");
        compare(done.texts.words, "Hello");
    }

    // A double tap of the hold key locks the take; the pill says how it ends.
    function test_5_a_locked_take_names_the_press_that_finishes_it() {
        DettivoState.handleDictation({
            "state": "recording"
        });
        compare(DettivoState.pillHint, "release to insert");
        DettivoState.handleDictation({
            "state": "recording",
            "previous_state": "recording",
            "reason": "locked"
        });
        compare(DettivoState.pillState, "listening");
        compare(DettivoState.pillHint, "locked · press to finish");
        DettivoState.pillState = "hidden";
    }
}
