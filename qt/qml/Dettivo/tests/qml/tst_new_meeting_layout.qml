import Dettivo
import QtQuick
import QtTest

TestCase {
    id: root
    name: "NewMeetingLayout"
    width: 1280
    height: 820
    visible: true
    when: windowShown

    function named(item, name) {
        for (const child of item.children) {
            if (child.Accessible && child.Accessible.name === name)
                return child;
            const found = named(child, name);
            if (found)
                return found;
        }
        return null;
    }

    function test_short_window_controls_clear_footer_data() {
        return [
            {
                tag: "normal",
                width: 1280,
                height: 820
            },
            {
                tag: "short",
                width: 1280,
                height: 410
            },
            {
                tag: "minimum",
                width: Theme.appWindowMinWidth,
                height: 410
            }
        ];
    }

    function test_short_window_controls_clear_footer(data) {
        const window = createTemporaryObject(windowC, root, {
            width: data.width,
            height: data.height
        });
        verify(window);
        waitForRendering(window.contentItem);
        const rail = named(window.contentItem, "New meeting rail");
        const start = named(rail, "Start meeting");
        const footer = named(rail, "Configuration keys");
        verify(start && footer);
        start.forceActiveFocus(Qt.TabFocusReason);
        tryVerify(() => start.mapToItem(rail, 0, 0).y >= 0 && start.mapToItem(rail, 0, start.height).y < footer.y);
        const source = named(rail, "System audio");
        verify(source.activeFocusOnTab, "System audio must be keyboard reachable");
        compare(named(rail, "Microphone").activeFocusOnTab, false);
        compare(named(rail, "Microphone").enabled, false);
        for (let i = 0; i < 12 && !source.activeFocus; i++)
            keyClick(Qt.Key_Backtab);
        verify(source.activeFocus);
        tryVerify(() => source.mapToItem(rail, 0, 0).y >= 0);
        const checked = rail.systemAudio;
        keyClick(Qt.Key_Space);
        compare(rail.systemAudio, !checked);
        source.parent.enabled = false;
        compare(source.activeFocusOnTab, false);
        source.parent.toggle();
        compare(rail.systemAudio, !checked);
        source.parent.enabled = true;
        const analysis = named(rail, "Analysis after stop");
        verify(analysis.activeFocusOnTab);
        analysis.forceActiveFocus(Qt.TabFocusReason);
        const analyze = rail.analyze;
        keyClick(Qt.Key_Return);
        compare(rail.analyze, !analyze);
        const speakers = named(rail, "Expected speakers");
        speakers.forceActiveFocus(Qt.TabFocusReason);
        keyClick(Qt.Key_Space);
        verify(speakers.editing);
        keyClick(Qt.Key_3);
        keyClick(Qt.Key_Return);
        compare(rail.expectedSpeakers, 3);
        const disclosure = named(rail, "Disclosure state");
        verify(disclosure.activeFocusOnTab);
        verify(named(rail, "Meeting engine").activeFocusOnTab);
        let reachedStart = false;
        for (let i = 0; i < 12; i++) {
            keyClick(Qt.Key_Tab);
            if (start.activeFocus) {
                reachedStart = true;
                break;
            }
        }
        verify(reachedStart);
        tryVerify(() => start.mapToItem(rail, 0, start.height).y < footer.y);
    }

    Component {
        id: windowC
        AppWindow {
            visible: true
            router: FakeMeetingsRouter {}
        }
    }
}
