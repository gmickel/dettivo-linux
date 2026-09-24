import QtQuick

// The keys of the meetings routes (docs/app.md): `n` new meeting, `i`
// import, `j` and `k` move, Enter opens on the list; `s` stops the live
// meeting after the urgent confirmation; `1`, `2`, `3` pick the detail
// tabs, `p` toggles Raw and Polished, `r` renames the focused speaker, `t`
// renames the meeting from its title, `e` exports and `d` deletes. A
// focused field keeps its letters.
Item {
    id: root

    // The route (functions) and the page it shows (its own functions).
    property var route: null
    property var page: null

    readonly property string sub: root.route && root.route.sub !== undefined ? root.route.sub : ""
    readonly property bool typing: root.page && root.page.typing !== undefined ? root.page.typing : false
    readonly property bool onList: root.sub.length === 0 && !root.typing
    readonly property bool onLive: root.sub === "live" && !root.typing
    readonly property bool onDetail: root.sub === "detail" && !root.typing

    Shortcut {
        enabled: !root.typing && root.sub !== "live"
        sequence: "N"
        onActivated: root.route.newMeeting()
    }

    Shortcut {
        enabled: root.onList
        sequence: "I"
        onActivated: root.route.importAudio()
    }

    Shortcut {
        enabled: root.onList || root.onDetail
        sequence: "J"
        onActivated: {
            if (root.page && root.page.move)
                root.page.move(1);
        }
    }

    Shortcut {
        enabled: root.onList || root.onDetail
        sequence: "K"
        onActivated: {
            if (root.page && root.page.move)
                root.page.move(-1);
        }
    }

    Shortcut {
        enabled: root.onList
        sequence: "Return"
        onActivated: {
            if (root.page && root.page.openCurrent)
                root.page.openCurrent();
        }
    }

    Shortcut {
        enabled: root.onLive
        sequence: "S"
        onActivated: {
            if (root.page && root.page.confirmStop)
                root.page.confirmStop();
        }
    }

    Shortcut {
        enabled: root.onDetail
        sequence: "1"
        onActivated: root.page.pickTab(0)
    }

    Shortcut {
        enabled: root.onDetail
        sequence: "2"
        onActivated: root.page.pickTab(1)
    }

    Shortcut {
        enabled: root.onDetail
        sequence: "3"
        onActivated: root.page.pickTab(2)
    }

    Shortcut {
        enabled: root.onDetail
        sequence: "P"
        onActivated: root.page.togglePolished()
    }

    Shortcut {
        enabled: root.onDetail
        sequence: "R"
        onActivated: root.page.renameSpeaker()
    }

    Shortcut {
        enabled: root.onDetail
        sequence: "T"
        onActivated: {
            if (root.page && root.page.renameTitle)
                root.page.renameTitle();
        }
    }

    Shortcut {
        enabled: root.onDetail
        sequence: "E"
        onActivated: root.page.exportSheet()
    }

    Shortcut {
        enabled: root.onDetail
        sequence: "D"
        onActivated: root.page.confirmDelete()
    }

    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Meetings keys")
}
