import Dettivo
import QtQuick
import QtTest

// The text Home assembles from daemon facts (fn-17): a chord or an app id
// is escaped before it enters the rich subtitle's markup, so a `<` or an
// `&` in a fact is drawn rather than parsed.
TestCase {
    id: root

    function test_html_escape_keeps_facts_out_of_markup() {
        compare(Html.escaped("<b>"), "&lt;b&gt;");
        compare(Html.escaped("a & b"), "a &amp; b");
        compare(Html.escaped("say \"hi\" 'now'"), "say &quot;hi&quot; &#39;now&#39;");
        compare(Html.escaped("&lt;"), "&amp;lt;");
        compare(Html.escaped("Super+Ctrl+X"), "Super+Ctrl+X");
        compare(Html.escaped(""), "");
    }

    function test_home_sentence_escapes_the_daemon_facts() {
        const status = createTemporaryObject(statusC, root, {
            "holdChord": "<F9>",
            "targetApp": "a&b",
            "toggleChord": "Super+\"X\""
        });
        const home = createTemporaryObject(homeC, root, {
            "status": status,
            "width": 1000,
            "height": 700
        });
        waitForRendering(home);
        compare(home.detailPlain, "Hold <F9> to dictate into a&b. Super+\"X\" toggles.");
        verify(home.detailRich.indexOf("&lt;F9&gt;") >= 0);
        verify(home.detailRich.indexOf("a&amp;b") >= 0);
        verify(home.detailRich.indexOf("Super+&quot;X&quot;") >= 0);
        verify(home.detailRich.indexOf("<F9>") < 0);
    }

    function test_home_names_a_failed_start_and_escapes_its_reason() {
        const status = createTemporaryObject(statusC, root);
        const home = createTemporaryObject(homeC, root, {
            "status": status,
            "width": 1000,
            "height": 700
        });
        status.dictationError = "Model <tiny> is not downloaded.";
        compare(home.sentence, "Dictation unavailable.");
        compare(home.detailPlain, status.dictationError);
        verify(home.detailRich.indexOf("&lt;tiny&gt;") >= 0);
        status.dictationError = "";
        compare(home.sentence, "Ready.");
    }

    Component {
        id: statusC

        QtObject {
            property bool daemonConnected: true
            property string daemonState: "connected"
            property bool healthOk: true
            property string dictationState: "idle"
            property string dictationError: ""
            property string holdChord: "F9"
            property string toggleChord: "Super+Ctrl+X"
            property string targetApp: "ghostty"
            property string socketMode: "peer"
            property string gpu: "vulkan"
            property string inputName: "Arctis Nova"
            property string inputRate: "16 kHz"
            property string restState: "off"
            property string mcpHosts: ""
            property string lastCall: ""
            property string clock: "Wed 3 Sep · 13:42"
            property var levels: [0.1, 0.5, 0.9]

            function toggleDictation() {
            }
        }
    }

    Component {
        id: homeC

        HomeRoute {}
    }

    name: "AppText"
    width: 1280
    height: 820
    visible: true
    when: windowShown
}
