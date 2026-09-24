import QtQuick
import QtTest
import Dettivo

// Every component instantiates, carries an accessible role and name, and
// changes state the way its properties say.
TestCase {
    id: root
    name: "Components"
    width: 400
    height: 300
    visible: true
    when: windowShown

    Component {
        id: chipC
        Chip {
            text: "Enhanced"
        }
    }
    Component {
        id: iconC
        Icon {
            source: "mic"
            accessibleName: "Microphone"
        }
    }
    Component {
        id: keyCapC
        KeyCap {
            text: "F9"
        }
    }
    Component {
        id: keyValueC
        KeyValueRow {
            label: "Backend"
            value: "virtual keyboard"
        }
    }
    Component {
        id: listRowC
        ListRow {
            leading: "13:12"
            text: "Add a regression test"
            trailing: "4 s"
        }
    }
    Component {
        id: sectionC
        SectionLabel {
            text: "Workspace"
        }
    }
    Component {
        id: segmentedC
        SegmentedControl {
            model: ["Raw", "Polish", "Enhanced"]
            currentIndex: 2
            label: "Mode"
        }
    }
    Component {
        id: statusC
        StatusDot {
            label: "Recording"
            status: StatusDot.Accent
        }
    }
    Component {
        id: waveformC
        Waveform {
            levels: [0.2, 0.8, 0.5]
        }
    }
    Component {
        id: meterC
        LevelMeter {
            level: 0.6
            peak: 0.9
        }
    }

    function test_every_component_has_role_and_name() {
        const cases = [[chipC, Accessible.StaticText, "Enhanced"], [iconC, Accessible.Graphic, "Microphone"], [keyCapC, Accessible.StaticText, "F9"], [keyValueC, Accessible.Row, "Backend: virtual keyboard"], [listRowC, Accessible.ListItem, "Add a regression test"], [sectionC, Accessible.Heading, "Workspace"], [segmentedC, Accessible.PageTabList, "Mode"], [statusC, Accessible.Indicator, "Recording"], [waveformC, Accessible.Graphic, "Audio level"], [meterC, Accessible.ProgressBar, "Input level"],];
        for (const [component, role, name] of cases) {
            const item = createTemporaryObject(component, root);
            verify(item, "component instantiates");
            compare(item.Accessible.role, role);
            compare(item.Accessible.name, name);
            verify(item.implicitHeight > 0);
        }
    }

    function test_chip_states() {
        const chip = createTemporaryObject(chipC, root, {
            accent: true,
            closable: true
        });
        compare(chip.Accessible.role, Accessible.Button);
        compare(chip.radius, Theme.radius);
        compare(chip.border.width, Theme.hairlineWidth);
        fuzzyCompare(chip.border.color.a, 0.5, 0.02);
    }

    function test_list_row_selection_and_hover() {
        const row = createTemporaryObject(listRowC, root);
        compare(row.Accessible.selected, false);
        compare(row.color, Qt.color("transparent"));
        row.selected = true;
        compare(row.Accessible.selected, true);
        tryCompare(row, "color", Theme.roleSelectedFill);
    }

    function test_segmented_control_selects_on_tap() {
        const seg = createTemporaryObject(segmentedC, root);
        let picked = -1;
        seg.selected.connect(function (index) {
            picked = index;
            seg.currentIndex = index;
        });
        waitForRendering(seg);
        const first = seg.segmentAt(0);
        verify(first);
        mouseClick(first);
        compare(seg.currentIndex, 0);
        compare(picked, 0);
    }

    function test_waveform_bars_are_scene_graph_rectangles() {
        const wave = createTemporaryObject(waveformC, root, {
            barCount: 3
        });
        compare(wave.implicitWidth, 3 * (Theme.waveformBarWidth + Theme.waveformBarGap) - Theme.waveformBarGap);
        compare(wave.levelAt(1), 0.8);
        wave.levels = [1.0, 0.0, 0.5];
        compare(wave.levelAt(0), 1.0);
    }

    function test_level_meter_clips_in_urgent() {
        const meter = createTemporaryObject(meterC, root);
        compare(meter.clipping, false);
        meter.level = 0.99;
        compare(meter.clipping, true);
    }

    function test_status_dot_colours_by_status() {
        const dot = createTemporaryObject(statusC, root);
        compare(dot.dotColor, Theme.roleAccent);
        dot.status = StatusDot.Urgent;
        compare(dot.dotColor, Theme.roleUrgent);
    }
}
