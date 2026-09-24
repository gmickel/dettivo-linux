import Dettivo
import QtQuick
import QtTest

// The new-meeting rail (fn-34, ADR 0038; fn-47, ADR 0049): it starts a
// meeting with the choices made on it, the analysis toggle starts on
// `[meetings.analysis] auto` and follows a configuration change while the
// rail is open, and an untouched choice is left off the start request so
// the daemon applies its defaults. The fakes are the Fake*.qml files
// beside this one; the config fake answers `value` and carries a
// `revision` the rail's binding reads.
TestCase {
    id: root

    name: "AppMeetingsRail"
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

    function test_rail_starts_a_meeting_with_its_choices() {
        const live = createTemporaryObject(liveC, root);
        const actions = createTemporaryObject(actionsC, root);
        // Already-loaded choices must select the saved model, not the first.
        actions.providers = [
            {
                id: "whisper",
                label: "Whisper",
                models: [
                    {
                        id: "tiny",
                        label: "Tiny",
                        downloaded: true
                    },
                    {
                        id: "small",
                        label: "Small",
                        downloaded: true
                    }
                ]
            }
        ];
        const status = createTemporaryObject(statusC, root);
        const rail = createTemporaryObject(railC, root, {
            "width": 320,
            "height": 800,
            "live": live,
            "actions": actions,
            "status": status
        });
        waitForRendering(rail);
        compare(rail.engineLabel, "whisper small · timestamps");
        verify(findByName(rail, "Meeting engine"));
        verify(findByName(rail, "Expected speakers"));
        verify(findByName(rail, "Disclosure state"));
        // Untouched, the analysis choice follows the configuration and is
        // left off the request so the daemon applies its defaults.
        compare(rail.analyze, true);
        mouseClick(findByName(rail, "Start meeting"));
        compare(live.starts.length, 1);
        compare(live.starts[0].analyze, undefined);
        compare(live.starts[0].diarize, undefined);
        mouseClick(findByName(rail, "Analysis after stop"));
        compare(rail.analyze, false);
        mouseClick(findByName(rail, "System audio"));
        compare(rail.systemAudio, false);
        rail.expectedSpeakers = 3;
        mouseClick(findByName(rail, "Start meeting"));
        compare(live.starts.length, 2);
        compare(live.starts[1].systemAudio, false);
        compare(live.starts[1].provider, "whisper");
        compare(live.starts[1].model, "small");
        compare(live.starts[1].speakers, 3);
        compare(live.starts[1].analyze, false);
        compare(live.starts[1].diarize, undefined);
        live.active = true;
        const router = createTemporaryObject(routerC, root);
        rail.router = router;
        const resume = findByName(rail, "Return to meeting");
        verify(resume.enabled);
        mouseClick(resume);
        compare(router.page, "meetings.live");
        compare(live.starts.length, 2);
    }

    function test_rail_reads_the_analysis_default_from_the_configuration() {
        const live = createTemporaryObject(liveC, root);
        const actions = createTemporaryObject(actionsC, root);
        const config = createTemporaryObject(configC, root);
        const rail = createTemporaryObject(railC, root, {
            "width": 320,
            "height": 800,
            "live": live,
            "actions": actions,
            "config": config
        });
        waitForRendering(rail);
        compare(rail.analyze, false, "[meetings.analysis] auto = false shows as off");
        mouseClick(findByName(rail, "Start meeting"));
        compare(live.starts[0].analyze, undefined, "untouched, the daemon applies the default");
        config.analysisAuto = true;
        config.revision += 1;
        compare(rail.analyze, true, "a configuration change while the rail is open");
        mouseClick(findByName(rail, "Analysis after stop"));
        compare(rail.analyze, false);
        mouseClick(findByName(rail, "Start meeting"));
        compare(live.starts[1].analyze, false, "an explicit change is sent");
    }

    Component {
        id: liveC
        FakeMeetingLive {}
    }

    Component {
        id: actionsC
        FakeMeetingsActions {}
    }

    Component {
        id: statusC

        QtObject {
            property bool daemonConnected: true
            property string inputName: "Arctis Nova"
        }
    }

    Component {
        id: routerC
        QtObject {
            id: router
            property string page: "meetings"
            function open(page) {
                router.page = page;
            }
        }
    }

    Component {
        id: railC
        NewMeetingRail {}
    }

    Component {
        id: configC

        QtObject {
            property int revision: 1
            property bool analysisAuto: false

            function value(key) {
                return key === "meetings.analysis.auto" ? analysisAuto : undefined;
            }
        }
    }
}
