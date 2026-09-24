import QtQuick
import QtTest
import Dettivo

TestCase {
    id: root
    name: "BarControls"
    width: 400
    height: 520
    visible: true
    when: windowShown

    Component {
        id: panelC
        BarPanel {
            reducedMotion: true
        }
    }

    function findAll(item, type, out) {
        for (const child of item.children) {
            if (child.toString().indexOf(type + "(") === 0 || child.toString().indexOf(type + "_QMLTYPE") === 0)
                out.push(child);
            findAll(child, type, out);
        }
        return out;
    }

    function test_panel_mode_track_and_keyboard() {
        const panel = createTemporaryObject(panelC, root, {
            "width": Theme.barPanelWidth
        });
        const modes = findAll(panel, "SegmentedControl", [])[0];
        waitForRendering(panel);
        const first = modes.segmentAt(0);
        const last = modes.segmentAt(2);
        fuzzyCompare(first.width, last.width, 0.01);
        fuzzyCompare(last.mapToItem(modes, last.width, 0).x, modes.width - Theme.space1, 0.01);
        verify(first.height < modes.height);
        const selected = [];
        panel.modeSelected.connect(index => {
            return selected.push(index);
        });
        first.forceActiveFocus(Qt.TabFocusReason);
        keyClick(Qt.Key_Right);
        verify(modes.segmentAt(1).activeFocus);
        keyClick(Qt.Key_Return);
        compare(selected, [1]);
        compare(modes.currentIndex, 0);
        panel.modeIndex = 1;
        verify(modes.segmentAt(1).Accessible.selected);
        verify(!first.Accessible.selected);
        for (const button of findAll(panel, "Button", [])) {
            compare(button.font.family, Theme.fontFamily);
            compare(button.background.radius, Theme.radius);
        }
    }
}
