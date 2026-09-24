// Rich text is assembled from daemon facts (a hotkey chord, the focused
// app's id) and the theme's colours; a fact goes through `escaped` before it
// enters a tag, so a `<` or an `&` in an app id is drawn, never parsed.
pragma Singleton
import QtQuick

QtObject {
    function escaped(text) {
        return String(text).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
    }

    // The text with every `[start, end]` character range painted in
    // `color` (a search hit's matches), everything else escaped as is.
    function highlighted(text, ranges, color) {
        const s = String(text);
        let out = "";
        let at = 0;
        for (const range of (ranges || [])) {
            const start = Math.max(at, Number(range[0]));
            const end = Math.min(s.length, Number(range[1]));
            if (end <= start)
                continue;

            out += escaped(s.slice(at, start));
            out += "<span style=\"background-color:" + String(color) + ";\">" + escaped(s.slice(start, end)) + "</span>";
            at = end;
        }
        return out + escaped(s.slice(at));
    }
}
