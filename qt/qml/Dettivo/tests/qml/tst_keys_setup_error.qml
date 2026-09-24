pragma ComponentBehavior: Bound
import Dettivo
import QtQuick
import QtTest

TestCase {
    id: root
    name: "KeysSetupError"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    function findByName(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;
            const nested = findByName(child, name);
            if (nested)
                return nested;
        }
        return null;
    }

    function test_keys_setup_error_is_visible_and_retryable_data() {
        return [
            {
                tag: "snippet",
                supported: true,
                written: false
            },
            {
                tag: "written-but-inactive",
                supported: true,
                written: true
            },
            {
                tag: "portal",
                supported: false,
                written: false
            }
        ];
    }

    function test_keys_setup_error_is_visible_and_retryable(data) {
        const firstRun = createTemporaryObject(firstRunC, root, {
            "snippetSupported": data.supported,
            "snippetWritten": data.written
        });
        const page = createTemporaryObject(keysC, root, {
            "firstRun": firstRun,
            "width": 1280,
            "height": 820
        });
        waitForRendering(page);
        const failure = "Connect to Dettivo, then retry shortcut setup.";
        firstRun.keysError = failure;
        const error = findByName(page, failure);
        verify(error);
        verify(error.visible);
        const action = findByName(page, data.supported ? "Retry shortcut setup" : "Continue");
        verify(action);
        verify(action.enabled);
        action.clicked();
        compare(firstRun.nexts, 1);
        firstRun.keysError = "";
        verify(!findByName(page, failure));
        verify(!findByName(page, "Retry shortcut setup"));
        verify(findByName(page, "Continue"));
    }

    Component {
        id: keysC
        KeysStep {}
    }

    Component {
        id: firstRunC
        QtObject {
            property bool snippetSupported: true
            property bool snippetWritten: false
            property bool snippetSourced: false
            property string compositor: "Hyprland"
            property string configPath: "~/.config/dettivo/config.toml"
            property string snippetPath: "~/.config/hypr/dettivo.conf"
            property string snippetText: "bind = , F9, exec, dettivo --quiet dictation start"
            property string includeLine: "source = ~/.config/hypr/dettivo.conf"
            property string mainConfigPath: "~/.config/hypr/hyprland.conf"
            property string keysError: ""
            property string portalLine: "Portal bindings"
            property string holdKey: "F9"
            property bool pressed: false
            property string pressLine: ""
            property var bindings: []
            property int nexts: 0
            function next() {
                nexts++;
            }
            function skip() {
            }
        }
    }
}
