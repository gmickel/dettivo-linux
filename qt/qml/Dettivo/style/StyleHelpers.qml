// Shared state arithmetic for the DettivoStyle controls and any component
// that draws control states: resolve one fill state and one border state,
// then read the shell's colour and alpha for it here, so no file carries a
// literal.
pragma Singleton
import QtQuick
import Dettivo

QtObject {
    id: helpers

    // The state's colour without its alpha, for one of Normal, Hover,
    // Focus, Selected, Pressed.
    function fillBase(stateKey) {
        return Theme["state" + stateKey + "Color"];
    }

    function fillAlpha(stateKey) {
        return Theme["state" + stateKey + "FillAlpha"];
    }

    // The fill as one colour with the state's alpha baked in; laid over a
    // surface it produces the shell's translucent control fills.
    function fillColor(stateKey) {
        return Qt.alpha(fillBase(stateKey), fillAlpha(stateKey));
    }

    // Border colour with its alpha for one of Normal, Hover, Focus, Selected.
    function borderColor(stateKey) {
        return Qt.alpha(Theme["state" + stateKey + "Color"], Theme["state" + stateKey + "BorderAlpha"]);
    }

    function borderWidth(stateKey) {
        return Theme["state" + stateKey + "BorderWidth"];
    }

    // The selection wash lists and text fields use.
    function selectionFillColor() {
        return Qt.alpha(Theme.stateSelectionColor, Theme.stateSelectionFillAlpha);
    }

    // Priority order for fills: pressed, selected, focus, hover, normal.
    function pickFillState(pressed, selected, focused, hovered) {
        if (pressed)
            return "Pressed";
        if (selected)
            return "Selected";
        if (focused)
            return "Focus";
        if (hovered)
            return "Hover";
        return "Normal";
    }

    // Pressed has no border tokens, so borders stop at selected.
    function pickBorderState(selected, focused, hovered) {
        if (selected)
            return "Selected";
        if (focused)
            return "Focus";
        if (hovered)
            return "Hover";
        return "Normal";
    }

    // Kept for the controls that tint a base colour by a state fill.
    function withAlpha(color, alpha) {
        return Qt.alpha(color, alpha);
    }
}
