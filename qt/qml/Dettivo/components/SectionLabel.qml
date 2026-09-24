import QtQuick
import Dettivo

// Small tracked-caps section header (e.g. above a list group). Consumes
// only Theme tokens — no literal color/px sizes.
Text {
    id: root

    // Uses Text's own built-in `text` property directly (the contract
    // asks for a `text` property, which Text already provides — no need
    // to shadow it with a duplicate declaration).
    color: Theme.roleMutedText
    font.family: Theme.fontFamily
    font.pixelSize: Theme.typeLabelSize
    font.letterSpacing: Theme.typeLabelSize * Theme.typeLabelTracking
    font.capitalization: Font.AllUppercase
    leftPadding: Theme.spacingXs
    topPadding: Theme.spacingXs
    bottomPadding: Theme.spacingXs

    Accessible.role: Accessible.Heading
    Accessible.name: text
}
