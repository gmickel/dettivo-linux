import QtQuick
import QtQuick.Controls.impl as CImpl
import Dettivo

// Reusable recolorable icon (fn-5.1 contract, "Icon set (R6)").
//
// Recolor technique: QtQuick.Controls.impl's ColorImage (QQuickColorImage,
// "private/qquickcolorimage_p.h"), NOT a `ColorImage` under plain
// `QtQuick`, and NOT MultiEffect.colorization. Verified against the
// installed Qt 6.11 tree: there is no `ColorImage` type under plain
// `QtQuick` in this Qt version (only under `QtQuick.Controls.impl`), and
// it is exactly what QQC2's own Basic/Fusion/FluentWinUI3 styles use for
// their check/radio/chevron glyphs (e.g.
// QtQuick/Controls/Basic/CheckBox.qml ships a ColorImage from this same
// module). It recolors by pixel color-key replacement: pixels matching
// `defaultColor` are swapped to `color`, alpha (and anti-aliased edge
// blending) preserved — which is why every Dettivo icon SVG is drawn in
// solid `#000000` (see icons/check.svg's header comment) rather than
// `currentColor`, which ColorImage does not understand.
//
// `QtQuick.Controls.impl` is a QQC2-internal styling module (it ships
// with every Qt install and is what Qt's own built-in styles use) rather
// than a documented-stable public API surface — flagged in the fn-5
// report as a dependency worth the coordinator's awareness, not a
// blocker: it is exactly the mechanism QQC2's shipped styles rely on for
// this same job, so a custom "Dettivo" QQC2 style depending on it too is
// consistent with how Qt itself does this.
//
// Wrapped (rather than subclassing ColorImage directly) so the public
// `source` property can stay a plain string ("check", or an explicit
// path/URL) without colliding with ColorImage's own `source: url`.
Item {
    id: root

    property string source: ""
    property color color: Theme.roleText
    property int size: Theme.iconSize
    property string accessibleName: ""

    readonly property url resolvedSource: source.length === 0 ? "" : (source.includes("/") || source.includes(":") ? source : Qt.resolvedUrl("../icons/" + source + ".svg"))

    implicitWidth: size
    implicitHeight: size
    width: size
    height: size

    Accessible.role: Accessible.Graphic
    Accessible.name: root.accessibleName

    CImpl.ColorImage {
        anchors.fill: parent
        source: root.resolvedSource
        // Every Dettivo icon SVG is drawn solid #000000 — this is the
        // pixel value ColorImage looks for and replaces with `color`.
        defaultColor: "#000000"
        color: root.color
        sourceSize.width: root.size
        sourceSize.height: root.size
        fillMode: Image.PreserveAspectFit
        smooth: true
        antialiasing: true
    }
}
