pragma ComponentBehavior: Bound
import QtQuick
import QtTest
import Dettivo

// The media row shows what the daemon does: `[hotkeys] pause_media = true`
// with `nothing` reads as pause (ADR 0091), and a choice clears the flag.
TestCase {
    id: root
    name: "MediaSetting"
    width: 1080
    height: 820
    visible: true
    when: windowShown

    function test_legacy_pause_shows_as_pause_and_nothing_clears_it() {
        const settings = createTemporaryObject(settingsC, root);
        const page = createTemporaryObject(hotkeysC, root, {
            settings: settings,
            width: 1080,
            height: 820
        });
        waitForRendering(page);
        const media = page.rowFor(page, "audio.while_dictating");
        compare(media.selection, "pause");
        media.controlItem.chosen("nothing");
        compare(settings.value("hotkeys.pause_media"), false);
        compare(settings.value("audio.while_dictating"), "nothing");
        compare(media.selection, "nothing");
        media.controlItem.chosen("duck");
        compare(settings.writes, 3);
        compare(media.selection, "duck");
    }

    Component {
        id: hotkeysC
        HotkeysSection {}
    }
    Component {
        id: settingsC
        QtObject {
            property int revision: 0
            property string notice: ""
            property bool registryLoaded: true
            property int writes: 0
            property var values: ({
                    "hotkeys.pause_media": true,
                    "audio.while_dictating": "nothing"
                })
            signal errorChanged(string key)
            function pending(key) {
                return false;
            }
            function value(key) {
                return values[key] === undefined ? "" : values[key];
            }
            function text(key) {
                return String(value(key));
            }
            function lockedBy(key) {
                return "";
            }
            function error(key) {
                return "";
            }
            function writesBlock(section) {
                return "";
            }
            function defaultText(key) {
                return '""';
            }
            function set(key, value) {
                const next = Object.assign({}, values);
                next[key] = value;
                values = next;
                writes++;
                revision++;
            }
            function unset(key) {
                set(key, "");
            }
        }
    }
}
