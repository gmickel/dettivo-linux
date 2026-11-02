// Text helpers shared by DettivoState and DettivoFacts: version order,
// the shortcut label, the meeting timer and the pills that end a take.
.pragma library

function compareVersions(a, b) {
    const pa = String(a).split(".").map(n => parseInt(n, 10) || 0);
    const pb = String(b).split(".").map(n => parseInt(n, 10) || 0);
    for (let i = 0; i < Math.max(pa.length, pb.length); ++i) {
        const d = (pa[i] || 0) - (pb[i] || 0);
        if (d !== 0)
            return d;
    }
    return 0;
}

// "SUPER SHIFT, D" reads "SUPER+SHIFT+D" beside Open Dettivo.
function shortcutLabel(chord) {
    return String(chord || "").replace(",", " ").split(/\s+/).filter(p => p.length > 0).join("+");
}

// Milliseconds as "hh:mm:ss".
function formatElapsed(ms) {
    const total = Math.max(0, Math.floor(ms / 1000));
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    const two = n => (n < 10 ? "0" : "") + n;
    return two(h) + ":" + two(m) + ":" + two(s);
}

// The pill for a take the speech gate found silent (`cancelled`, reason
// `silence: <stage>`) or that completed with no text to insert, as the OSD shows it.
function nothingHeard() {
    return {
        "title": "Nothing heard",
        "reason": "the take was silent",
        "action": "try again"
    };
}

// What a completed take leaves: the dictation state, the pill and its texts.
// A take with no text carries no insertion, so nothing was heard.
function completion(insertion, words) {
    if (!insertion)
        return { "state": "idle", "pill": "error", "texts": nothingHeard() };
    if (insertion.outcome === "inserted") {
        const app = insertion.target_app || {};
        return {
            "state": "inserted",
            "pill": "inserted",
            "texts": { "target": app.name || app.bundle_id || "", "words": words }
        };
    }
    // `dettivo dictation stop --print` took the text: done, into no window.
    if (insertion.outcome === "printed")
        return { "state": "inserted", "pill": "inserted", "texts": { "target": "", "words": words } };
    if (insertion.outcome === "copied_to_clipboard")
        return {
            "state": "idle",
            "pill": "copied",
            "texts": { "reason": insertion.reason || "", "action": "Ctrl+V", "words": words }
        };
    return {
        "state": "failed",
        "pill": "error",
        "texts": { "title": "Not inserted", "reason": insertion.reason || "the insertion failed", "action": "copy from history" }
    };
}
