// Shared by the plugin's Qt Quick Tests: finds the shim processes the
// plugin started through the Quickshell shim's registry, answers them
// the way a daemon at 0.1.0 would, and locates the widget's loaders.
.pragma library

function processFor(shell, words) {
    return shell.process(...words);
}

function answerVersion(shell, version) {
    processFor(shell, ["status", "version"]).finish(JSON.stringify({
        "app_version": version
    }));
}

function readyDaemon(shell) {
    processFor(shell, ["test", "-f"]).exit(0);
    answerVersion(shell, "0.1.0");
    processFor(shell, ["dictation", "status"]).finish(JSON.stringify({
        "is_active": false
    }));
    processFor(shell, ["config", "get"]).finish(JSON.stringify({
        "entries": [
            {
                "key": "dictation.mode",
                "value": "enhanced"
            },
            {
                "key": "omarchy.open_shortcut",
                "value": "SUPER SHIFT, D"
            },
            {
                "key": "omarchy.osd",
                "value": "panel"
            },
            {
                "key": "osd.position",
                "value": "bottom_right"
            },
            {
                "key": "osd.margin",
                "value": 32
            }
        ]
    }));
    processFor(shell, ["speech", "selection"]).finish(JSON.stringify({
        "dictation_model_id": "parakeet-v3",
        "dictation_provider_id": "local"
    }));
    processFor(shell, ["llm", "providers"]).finish(JSON.stringify({
        "selected": "local",
        "providers": [
            {
                "id": "local",
                "model": "qwen3-4b",
                "available": true
            }
        ]
    }));
    processFor(shell, ["insert", "target"]).finish(JSON.stringify({
        "chosen": "virtual_keyboard",
        "target": {
            "app_id": "ghostty"
        }
    }));
    processFor(shell, ["history", "list"]).finish(JSON.stringify({
        "items": [
            {
                "title": "Add a regression test",
                "started_at": "2026-09-05T13:12:00Z",
                "app_id": "ghostty"
            },
            {
                "title": "Reply to Mara",
                "started_at": "2026-09-05T12:58:00Z"
            }
        ]
    }));
}

function isLoader(item) {
    return item.toString().indexOf("QQuickLoader") === 0;
}

// The Loader of the panel content inside the widget's popup, or null.
function contentLoader(widget) {
    const popup = widget.children.find(c => isLoader(c) && c.item && ("opened" in c.item));
    if (!popup)
        return null;
    const loaders = [];
    const walk = item => {
        for (const child of item.children) {
            if (isLoader(child))
                loaders.push(child);
            walk(child);
        }
    };
    walk(popup.item);
    return loaders.find(l => String(l.source).indexOf("DettivoPanelContent") >= 0) || null;
}

// The widget's button and the glyph Loader inside it.
function button(widget) {
    return widget.children.find(c => "tooltipText" in c) || null;
}

function glyphLoader(widget) {
    const b = button(widget);
    return b ? b.children.find(isLoader) || null : null;
}
