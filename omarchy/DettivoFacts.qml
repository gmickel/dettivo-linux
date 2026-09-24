import QtQuick
import Quickshell.Io
import "DettivoText.js" as DettivoText

// The facts the panel shows, each one short `dettivo --json` call:
// the daemon's version, the configuration, the speech selection, the
// language model providers, the insertion target and the recent
// dictations. Owned by DettivoState, which is `state` here and takes
// every answer; refresh() re-reads them all.
QtObject {
    id: root

    required property var state

    function refresh() {
        version.running = true;
        config.running = true;
        selection.running = true;
        providers.running = true;
        target.running = true;
        history.running = true;
    }

    function refreshHistory() {
        history.running = true;
    }

    function applyConfig(entries) {
        const modes = ["raw", "polish", "enhanced"];
        for (const entry of entries || []) {
            const value = entry.value;
            switch (entry.key) {
            case "dictation.mode":
                root.state.modeIndex = Math.max(0, modes.indexOf(String(value)));
                break;
            case "omarchy.glyph":
                root.state.glyph = String(value);
                break;
            case "omarchy.level_meter":
                root.state.levelMeter = value === true || value === "true";
                break;
            case "omarchy.osd":
                root.state.osdMode = String(value);
                break;
            case "omarchy.history_items":
                root.state.historyItems = parseInt(value, 10) || 3;
                break;
            case "omarchy.open_shortcut":
                root.state.openShortcut = DettivoText.shortcutLabel(value);
                break;
            case "osd.position":
                root.state.osdPosition = String(value);
                break;
            case "osd.margin":
                root.state.osdMargin = parseInt(value, 10) || 24;
                break;
            }
        }
    }

    function applyRecent(result) {
        const rows = [];
        for (const item of (result && result.items) || []) {
            const when = item.started_at ? new Date(item.started_at) : null;
            rows.push({
                "time": when && !isNaN(when.getTime()) ? Qt.formatTime(when, "HH:mm") : "",
                "title": item.title || "",
                "app": item.app_id || (item.insertion ? item.insertion.target_app : "") || ""
            });
        }
        root.state.recent = rows;
    }

    function parsed(text) {
        try {
            return JSON.parse(text);
        } catch (e) {
            return null;
        }
    }

    readonly property Process version: Process {
        command: [root.state.binary, "--json", "status", "version"]
        stdout: StdioCollector {
            onStreamFinished: {
                const v = root.parsed(text);
                if (v)
                    root.state.daemonVersion = String(v.app_version || "");
            }
        }
        onExited: code => {
            if (code === 127)
                root.state.binaryAvailable = false;
        }
    }

    readonly property Process config: Process {
        command: [root.state.binary, "--json", "config", "get"]
        stdout: StdioCollector {
            onStreamFinished: {
                const v = root.parsed(text);
                if (v)
                    root.applyConfig(v.entries);
            }
        }
    }

    readonly property Process selection: Process {
        command: [root.state.binary, "--json", "speech", "selection", "get"]
        stdout: StdioCollector {
            onStreamFinished: {
                const s = root.parsed(text);
                if (s)
                    root.state.engineFact = [s.dictation_model_id, s.dictation_provider_id].filter(p => p).join(" · ");
            }
        }
    }

    readonly property Process providers: Process {
        command: [root.state.binary, "--json", "llm", "providers", "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                const p = root.parsed(text);
                if (!p)
                    return;
                const chosen = (p.providers || []).find(row => row.id === p.selected) || (p.providers || [])[0];
                root.state.enhancedFact = chosen ? [chosen.model, chosen.available ? "loaded" : "not loaded"].filter(x => x).join(" · ") : "";
            }
        }
    }

    readonly property Process target: Process {
        command: [root.state.binary, "--json", "insert", "target"]
        stdout: StdioCollector {
            onStreamFinished: {
                const t = root.parsed(text);
                if (t)
                    root.state.insertFact = [t.target ? t.target.app_id : "", t.chosen].filter(p => p).join(" · ");
            }
        }
    }

    readonly property Process history: Process {
        command: [root.state.binary, "--json", "history", "list", "--limit", String(root.state.historyItems)]
        stdout: StdioCollector {
            onStreamFinished: {
                const h = root.parsed(text);
                if (h)
                    root.applyRecent(h);
            }
        }
    }
}
