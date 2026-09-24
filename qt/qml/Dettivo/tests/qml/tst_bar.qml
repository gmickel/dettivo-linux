import QtQuick
import QtTest
import Dettivo

// Dettivo.BarGlyph and Dettivo.BarPanel (fn-29 R2): the glyph's five
// states map to the mark colour, the meeting timer and the accessible
// sentence, the dot glyph and the dimmed hint; the panel's states drive
// the header, every row and action fires its signal, the hint and
// unavailable states hide the body, and every element carries a name.
TestCase {
    id: root
    name: "Bar"
    width: 400
    height: 520
    visible: true
    when: windowShown

    Component {
        id: glyphC
        BarGlyph {
            reducedMotion: true
        }
    }

    Component {
        id: panelC
        BarPanel {
            reducedMotion: true
        }
    }

    function find(item, type) {
        for (const child of item.children) {
            if (child.toString().indexOf(type + "(") === 0 || child.toString().indexOf(type + "_QMLTYPE") === 0)
                return child;
            const nested = find(child, type);
            if (nested)
                return nested;
        }
        return null;
    }

    function findAll(item, type, out) {
        for (const child of item.children) {
            if (child.toString().indexOf(type + "(") === 0 || child.toString().indexOf(type + "_QMLTYPE") === 0)
                out.push(child);
            findAll(child, type, out);
        }
        return out;
    }

    function buttons(panel) {
        return findAll(panel, "Button", []);
    }

    function buttonNamed(panel, text) {
        const found = buttons(panel).filter(b => b.text === text);
        verify(found.length === 1, "one button named " + text + ", found " + found.length);
        return found[0];
    }

    function test_glyph_states() {
        const glyph = createTemporaryObject(glyphC, root);
        compare(glyph.effectiveState, "idle");
        compare(glyph.markColor, Theme.roleMutedText);
        compare(glyph.Accessible.name, "Dettivo idle");
        compare(glyph.implicitHeight, Theme.iconSize);

        glyph.state = "listening";
        glyph.level = 1;
        compare(glyph.markColor, Theme.roleAccent);
        compare(glyph.Accessible.name, "Dettivo listening");
        compare(glyph.barHeight(2), Theme.iconSize);
        glyph.level = 0;
        fuzzyCompare(glyph.barHeight(2), Theme.iconSize * 0.3, 0.01);
        glyph.levelMeter = false;
        compare(glyph.barHeight(2), Theme.iconSize);

        glyph.state = "transcribing";
        fuzzyCompare(glyph.markColor.a, 0.45, 0.02);
        compare(glyph.Accessible.name, "Dettivo transcribing");

        glyph.state = "meeting";
        glyph.elapsed = "23:41";
        compare(glyph.markColor, Theme.roleAccent);
        compare(glyph.Accessible.name, "Meeting 23:41");
        const timer = find(glyph, "QQuickText");
        verify(timer);
        compare(timer.visible, true);
        compare(timer.text, "23:41");
        glyph.elapsed = "";
        compare(glyph.Accessible.name, "Meeting recording");
        compare(timer.visible, false);

        glyph.state = "error";
        compare(glyph.markColor, Theme.roleUrgent);
        compare(glyph.Accessible.name, "Dettivo error");

        glyph.state = "asleep";
        compare(glyph.effectiveState, "idle");

        glyph.dimmed = true;
        compare(glyph.markColor, Theme.roleFaintText);
        compare(glyph.Accessible.name, "Dettivo not ready");

        glyph.dimmed = false;
        glyph.glyph = "dot";
        const rects = findAll(glyph, "QQuickRectangle", []);
        const dot = rects.filter(r => r.visible);
        compare(dot.length, 1);
        compare(dot[0].width, Theme.iconSize / 2);
    }

    function test_panel_states_and_header() {
        const panel = createTemporaryObject(panelC, root, {
            width: Theme.barPanelWidth
        });
        compare(panel.implicitWidth, Theme.barPanelWidth);
        compare(panel.sentence, "Ready.");
        compare(panel.Accessible.name, "Dettivo panel");
        compare(panel.live, true);
        compare(buttonNamed(panel, "Dictate").enabled, true);
        verify(buttonNamed(panel, "Start meeting"));

        panel.state = "recording";
        compare(panel.sentence, "Listening · release to insert");
        compare(panel.glyphState, "listening");
        const stop = buttonNamed(panel, "Stop");
        compare(stop.highlighted, true);
        verify(buttonNamed(panel, "Cancel"));
        const chip = find(panel, "Chip");
        verify(chip);
        compare(chip.visible, true);
        compare(chip.text, "REC");

        panel.state = "transcribing";
        compare(panel.sentence, "Transcribing");
        compare(buttonNamed(panel, "Dictate").enabled, false);
        compare(chip.visible, false);

        panel.state = "inserted";
        panel.target = "ghostty";
        compare(panel.sentence, "Inserted into ghostty");
        panel.target = "";
        compare(panel.sentence, "Inserted");

        panel.state = "meeting";
        panel.elapsed = "00:23:41";
        compare(panel.sentence, "Meeting recording · 00:23:41");
        compare(panel.glyphState, "meeting");
        compare(buttonNamed(panel, "Stop meeting").highlighted, true);
        compare(chip.visible, true);
        const glyph = find(panel, "BarGlyph");
        verify(glyph);
        compare(glyph.state, "meeting");

        panel.state = "unavailable";
        compare(panel.sentence, "Daemon unavailable.");
        compare(panel.reason, "systemctl --user start dettivod.socket");
        compare(panel.live, false);
        compare(buttons(panel).filter(b => b.visible).length, 0);
        compare(glyph.dimmed, true);

        panel.state = "hint";
        panel.hint = "install";
        compare(panel.sentence, "Install Dettivo");
        verify(panel.reason.indexOf("yay -S dettivo-bin") === 0);
        verify(panel.reason.indexOf("shared Dettivo module") > 0);
        panel.hint = "upgrade";
        panel.hintDetail = "daemon 0.1.0, plugin needs 0.2.0";
        compare(panel.sentence, "Upgrade Dettivo");
        verify(panel.reason.indexOf("plugin needs 0.2.0") > 0);

        panel.state = "asleep";
        compare(panel.effectiveState, "idle");
    }

    function test_panel_rows_actions_and_signals() {
        const panel = createTemporaryObject(panelC, root, {
            width: Theme.barPanelWidth,
            engine: "Parakeet v3 · Vulkan · warm",
            enhanced: "Qwen3 4B · loaded",
            insertTarget: "ghostty · virtual keyboard",
            modeIndex: 2,
            shortcut: "SUPER+SHIFT+D",
            recent: [
                {
                    "time": "13:12",
                    "title": "Add a regression test",
                    "app": "ghostty"
                },
                {
                    "time": "12:58",
                    "title": "Reply to Mara",
                    "app": "chromium"
                },
                {
                    "time": "12:40",
                    "title": "Summarise the thread",
                    "app": "cursor"
                }
            ]
        });
        waitForRendering(panel);

        const facts = find(panel, "BarPanelFacts");
        verify(facts);
        const factRows = facts.children.filter(c => c.Accessible.role === Accessible.Row);
        compare(factRows.length, 3);
        compare(factRows[0].Accessible.name, "Engine: Parakeet v3 · Vulkan · warm");
        compare(factRows[1].Accessible.name, "Enhanced: Qwen3 4B · loaded");
        compare(factRows[2].Accessible.name, "Insert into: ghostty · virtual keyboard");

        const rows = findAll(panel, "ListRow", []);
        compare(rows.length, 3);
        compare(rows[0].leading, "13:12");
        compare(rows[0].text, "Add a regression test");
        compare(rows[0].trailing, "GHOSTTY");
        compare(rows[2].trailing, "CURSOR");

        const segment = find(panel, "SegmentedControl");
        verify(segment);
        compare(segment.currentIndex, 2);
        compare(segment.Accessible.name, "Dictation mode");

        const fired = [];
        panel.dictate.connect(() => fired.push("dictate"));
        panel.stopDictation.connect(() => fired.push("stop"));
        panel.cancelDictation.connect(() => fired.push("cancel"));
        panel.startMeeting.connect(() => fired.push("meeting.start"));
        panel.stopMeeting.connect(() => fired.push("meeting.stop"));
        panel.modeSelected.connect(index => fired.push("mode:" + index));
        panel.openDettivo.connect(() => fired.push("open"));
        panel.openItem.connect(index => fired.push("item:" + index));

        buttonNamed(panel, "Dictate").clicked();
        buttonNamed(panel, "Start meeting").clicked();
        panel.state = "recording";
        buttonNamed(panel, "Stop").clicked();
        buttonNamed(panel, "Cancel").clicked();
        panel.state = "meeting";
        buttonNamed(panel, "Stop meeting").clicked();
        mouseClick(segment.segmentAt(0));
        rows[1].activated();
        const footer = panel.children[0].children.find(c => c.Accessible.name === "Open Dettivo");
        verify(footer);
        mouseClick(footer);
        compare(fired, ["dictate", "meeting.start", "stop", "cancel", "meeting.stop", "mode:0", "item:1", "open"]);

        const shortcut = findAll(panel, "QQuickText", []).find(t => t.Accessible.name === "Shortcut");
        verify(shortcut);
        compare(shortcut.text, "SUPER+SHIFT+D");
        compare(shortcut.visible, true);

        panel.recent = [];
        compare(findAll(panel, "ListRow", []).length, 0);
        const empty = findAll(panel, "QQuickText", []).find(t => t.text === "Nothing dictated yet.");
        verify(empty);
        compare(empty.visible, true);
    }

    function test_every_element_has_an_accessible_name() {
        const panel = createTemporaryObject(panelC, root, {
            width: Theme.barPanelWidth,
            recent: [
                {
                    "time": "13:12",
                    "title": "Add a regression test",
                    "app": "ghostty"
                }
            ]
        });
        waitForRendering(panel);
        const named = [];
        const walk = item => {
            for (const child of item.children) {
                // A button's icon is decorative: the button carries the name.
                const decorative = child.toString().indexOf("Icon_QMLTYPE") === 0;
                if (!decorative && child.Accessible.role !== Accessible.NoRole && child.Accessible.name.length === 0)
                    named.push(child.toString());
                walk(child);
            }
        };
        walk(panel);
        compare(named, []);
        const heading = findAll(panel, "QQuickText", []).find(t => t.Accessible.role === Accessible.Heading);
        verify(heading);
        compare(heading.text, "Dettivo");
        verify(findAll(panel, "SectionLabel", []).some(l => l.text === "Recent"));
        const list = find(panel, "BarPanelRecent");
        compare(list.Accessible.name, "Recent dictations");
    }
}
