import QtQuick

// The shell's key catcher: return, escape and tab signals.
Item {
    id: root

    property bool blocked: false

    signal returnRequested
    signal closeRequested
    signal tabRequested(int direction)
}
