import QtQuick
import QtTest
import Dettivo

// Dettivo.Osd (fn-12 R1): the six states and hidden map to the dot colour,
// the bars, the border and the icon; every text is one line that elides;
// the hide timers run; reduced motion holds the bars; every element has an
// accessible name; and an unknown state warns and hides.
TestCase {
    id: root
    name: "Osd"
    width: 720
    height: 120
    visible: true
    when: windowShown

    Component {
        id: osdC
        Osd {
            hideAfterMs: 60
            errorHideAfterMs: 90
        }
    }

    function find(item, type) {
        for (const child of item.children) {
            if (child.toString().indexOf(type + "(") === 0 || child.toString().indexOf(type + "_QMLTYPE") === 0 || child.toString().indexOf("dettivo::" + type + "(") === 0)
                return child;
            const nested = find(child, type);
            if (nested)
                return nested;
        }
        return null;
    }

    function content(pill) {
        return find(pill, "OsdContent");
    }

    function test_states_map_to_dot_bars_border_and_icon() {
        const pill = createTemporaryObject(osdC, root, {
            hideAfterMs: 0,
            errorHideAfterMs: 0,
            reducedMotion: true
        });
        compare(pill.effectiveState, "hidden");
        compare(pill.shown, false);
        compare(pill.opacity, 0);

        pill.state = "listening";
        pill.hint = "release to insert";
        waitForRendering(pill);
        compare(pill.shown, true);
        compare(pill.effectiveTitle, "Listening");
        const c = content(pill);
        verify(c);
        compare(c.dotColor, Theme.roleAccent);
        compare(c.hasBars, true);
        compare(c.barColor, Theme.roleAccent);
        compare(c.detailText, "release to insert");
        const frame = find(pill, "QQuickRectangle");
        compare(frame.border.color, Theme.roleBorder);

        pill.state = "transcribing";
        pill.engine = "whisper tiny.en";
        pill.elapsed = "0.4 s";
        compare(pill.effectiveTitle, "Transcribing");
        compare(c.dotColor, Theme.roleSelected);
        compare(c.hasBars, true);
        fuzzyCompare(c.barColor.a, 0.45, 0.02);
        compare(c.detailText, "whisper tiny.en · 0.4 s");

        pill.state = "enhancing";
        pill.words = "add a regression test for the merger";
        compare(pill.effectiveTitle, "");
        compare(c.hasBars, false);
        compare(find(pill, "OsdThread").visible, true);
        compare(c.detailText, "add a regression test for the merger");

        pill.state = "inserted";
        pill.target = "ghostty";
        pill.words = "Add a regression test for the merger. Then ship.";
        compare(pill.effectiveTitle, "Inserted into");
        compare(c.hasDot, false);
        fuzzyCompare(frame.border.color.r, Theme.roleAccent.r, 0.01);
        fuzzyCompare(frame.border.color.a, 0.75, 0.02);
        compare(c.detailText, "Add a regression test for the merger.…");
        // A decimal is not a sentence boundary (matches the daemon's first_words).
        compare(pill.firstSentence("Version 2.0 is out. Next one soon"), "Version 2.0 is out.…");
        compare(pill.firstSentence("Version 2.0 is out"), "Version 2.0 is out");
        const icon = find(pill, "Icon");
        compare(icon.visible, true);
        compare(icon.source, "insert");
        compare(pill.sentence, "Inserted into ghostty Add a regression test for the merger.…");

        pill.state = "copied";
        pill.reason = "no text input focused";
        pill.action = "Ctrl+V";
        compare(pill.effectiveTitle, "Copied to clipboard");
        compare(icon.source, "copy");
        compare(c.detailText, "no text input focused · Ctrl+V");

        pill.state = "error";
        pill.title = "No microphone";
        pill.reason = "Arctis Nova disconnected";
        pill.action = "choose input";
        compare(pill.effectiveTitle, "No microphone");
        compare(c.dotColor, Theme.roleUrgent);
        fuzzyCompare(frame.border.color.r, Theme.roleUrgent.r, 0.01);
        compare(c.detailText, "Arctis Nova disconnected");

        pill.state = "hidden";
        compare(pill.shown, false);
        tryCompare(pill, "opacity", 0);
    }

    // A meeting's Listening carries the meeting clock as the hint
    // (fn-28 R5); the state renders the same, the detail is the clock.
    function test_listening_shows_the_meeting_clock_as_the_hint() {
        const pill = createTemporaryObject(osdC, root, {
            state: "listening",
            hint: "12:04",
            reducedMotion: true
        });
        waitForRendering(pill);
        compare(pill.effectiveTitle, "Listening");
        const c = content(pill);
        verify(c);
        compare(c.detailText, "12:04");
        compare(c.hasBars, true);
        pill.hint = "12:05";
        compare(c.detailText, "12:05");
    }

    function test_every_text_is_one_line_and_elides() {
        const pill = createTemporaryObject(osdC, root, {
            state: "inserted",
            target: "ghostty",
            maxTextWidth: 80,
            words: "A sentence long enough to run past the width the pill allows for one line"
        });
        waitForRendering(pill);
        const texts = [];
        const collect = function (item) {
            for (const child of item.children) {
                if (child.toString().indexOf("QQuickText(") === 0 && child.visible)
                    texts.push(child);
                collect(child);
            }
        };
        collect(pill);
        verify(texts.length >= 2);
        let truncated = 0;
        for (const t of texts) {
            compare(t.lineCount, 1);
            compare(t.elide, Text.ElideRight);
            verify(t.width <= 80);
            if (t.truncated)
                truncated += 1;
        }
        verify(truncated >= 1, "the long words elide");
        verify(pill.implicitWidth < 80 * 3 + Theme.rowPaddingX * 2 + Theme.space4 * 4 + Theme.iconSize + 1);
    }

    function test_hide_timers_run_for_inserted_copied_and_error() {
        const pill = createTemporaryObject(osdC, root);
        let hiddenCount = 0;
        pill.hidden.connect(function () {
            hiddenCount += 1;
        });
        pill.state = "inserted";
        compare(pill.shown, true);
        tryCompare(pill, "effectiveState", "hidden", 1000);
        compare(hiddenCount, 1);

        pill.state = "copied";
        compare(pill.effectiveState, "copied");
        tryCompare(pill, "effectiveState", "hidden", 1000);

        pill.state = "error";
        compare(pill.effectiveState, "error");
        wait(40);
        compare(pill.effectiveState, "error", "the error delay is the longer one");
        tryCompare(pill, "effectiveState", "hidden", 1000);

        // Listening never times out.
        pill.state = "listening";
        wait(150);
        compare(pill.effectiveState, "listening");
        compare(hiddenCount, 3);
    }

    function test_reduced_motion_holds_the_bars_and_cuts_transitions() {
        const pill = createTemporaryObject(osdC, root, {
            state: "listening",
            level: 0.6,
            reducedMotion: true
        });
        waitForRendering(pill);
        const bars = find(pill, "OsdBars");
        verify(bars);
        compare(bars.reducedMotion, true);
        compare(bars.live, true);
        const before = bars.frames;
        wait(120);
        compare(bars.frames, before, "reduced motion renders no extra frames");
        fuzzyCompare(bars.barHeightAt(Math.floor(bars.barCount / 2)), bars.targetAt(Math.floor(bars.barCount / 2)), 0.001);
        compare(find(pill, "OsdDot").pulsing, false);

        pill.reducedMotion = false;
        ThemeBackend.setReducedMotionForTesting(false);
        waitForRendering(pill);
        const live = bars.frames;
        tryVerify(function () {
            return bars.frames > live + 2;
        }, 2000, "live bars render every frame");
        compare(find(pill, "OsdDot").pulsing, true);
        ThemeBackend.setReducedMotionForTesting(true);
        compare(pill.reduced, true);
        ThemeBackend.setReducedMotionForTesting(false);
    }

    function test_level_drives_the_bar_targets() {
        const pill = createTemporaryObject(osdC, root, {
            state: "listening",
            reducedMotion: true
        });
        const bars = find(pill, "OsdBars");
        pill.level = 0;
        const centre = Math.floor(bars.barCount / 2);
        fuzzyCompare(bars.targetAt(centre), bars.floorLevel, 0.001);
        pill.level = 1;
        verify(bars.targetAt(centre) > 0.9);
        verify(bars.targetAt(0) < bars.targetAt(centre), "the envelope is centre weighted");
        pill.showLevel = false;
        fuzzyCompare(bars.targetAt(centre), bars.floorLevel, 0.001);
        compare(bars.implicitWidth, bars.barCount * Theme.waveformBarWidth + (bars.barCount - 1) * Theme.waveformBarGap);
    }

    function test_every_element_has_an_accessible_role_and_name() {
        const pill = createTemporaryObject(osdC, root, {
            state: "error",
            title: "No microphone",
            reason: "Arctis Nova disconnected",
            action: "choose input",
            errorHideAfterMs: 0
        });
        waitForRendering(pill);
        compare(pill.Accessible.role, Accessible.StaticText);
        compare(pill.Accessible.name, "No microphone Arctis Nova disconnected");
        compare(find(pill, "OsdDot").Accessible.name, "Status dot");
        compare(find(pill, "OsdThread").Accessible.name, "Enhancing");
        compare(find(pill, "OsdBars").Accessible.name, "Audio level");
        pill.state = "inserted";
        pill.target = "ghostty";
        compare(find(pill, "Icon").Accessible.name, "Inserted");
        const texts = [];
        const collect = function (item) {
            for (const child of item.children) {
                if (child.toString().indexOf("QQuickText(") === 0 && child.visible)
                    texts.push(child);
                collect(child);
            }
        };
        collect(pill);
        for (const t of texts) {
            compare(t.Accessible.role, Accessible.StaticText);
            verify(t.Accessible.name.length > 0);
        }
        verify(texts.some(t => t.Accessible.name === "Inserted into"));
    }

    function test_unknown_state_warns_and_hides() {
        const pill = createTemporaryObject(osdC, root, {
            state: "listening"
        });
        compare(pill.shown, true);
        ignoreWarning(new RegExp(".*Dettivo.Osd: unknown state \"sleeping\"; hiding the pill.*"));
        pill.state = "sleeping";
        compare(pill.valid, false);
        compare(pill.effectiveState, "hidden");
        compare(pill.shown, false);
        pill.state = "listening";
        compare(pill.effectiveState, "listening");
    }
}
