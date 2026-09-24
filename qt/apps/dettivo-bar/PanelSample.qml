import QtQuick
import Dettivo

// The panel with the baseline's sample data (omarchy-bar-panel.png): the
// engine, Enhanced and insertion facts, the three recent items and the
// shortcut. The item's `state` is the panel's; `hint` renders the
// upgrade hint the plugin shows against an old daemon.
BarPanel {
    id: root

    elapsed: "00:23:41"
    engine: "Parakeet v3 · Vulkan · warm"
    enhanced: "Qwen3 4B · loaded"
    hint: "upgrade"
    hintDetail: "daemon 0.1.0, plugin needs 0.2.0"
    insertTarget: "ghostty · virtual keyboard"
    modeIndex: 2
    recent: [
        {
            "time": "13:12",
            "title": "Add a regression test for the merger. Then ship.",
            "app": "ghostty"
        },
        {
            "time": "12:58",
            "title": "Reply to Mara: yes, keep the diarization pass.",
            "app": "chromium"
        },
        {
            "time": "12:40",
            "title": "Summarise the diarization thread for the standup.",
            "app": "cursor"
        }
    ]
    reducedMotion: true
    shortcut: "SUPER+SHIFT+D"
    target: "ghostty"
}
