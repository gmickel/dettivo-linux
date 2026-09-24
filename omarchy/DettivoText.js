// Text helpers shared by DettivoState and DettivoFacts: version order,
// the shortcut label and the meeting timer.
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
