import QtQuick
import Dettivo

// The focus ring (docs/design/checklist.md, C-09): the shell's focus
// border drawn over whichever custom control holds keyboard focus, so a
// row, a chip or a sidebar item shows the same ring a styled control
// does. Place it last inside the control; it fills its parent.
Rectangle {
    id: root

    anchors.fill: parent
    border.color: StyleHelpers.borderColor("Focus")
    border.width: StyleHelpers.borderWidth("Focus")
    color: "transparent"
    radius: Theme.radius
    visible: parent ? parent.activeFocus : false
    z: 1

    // Decoration: the control it rings carries the role and the name.
    Accessible.ignored: true
    Accessible.role: Accessible.NoRole
    Accessible.name: ""
}
