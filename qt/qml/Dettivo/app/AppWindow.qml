pragma ComponentBehavior: Bound
import QtQuick
import Dettivo

// The app's one window (fn-17, ADR 0020): compositor-native chrome, the
// sidebar, the page the router names, the daemon banner over every page
// and the four keyboard conventions (Super+F fullscreen, Escape closes a
// detail, / goes to search, ? opens the hint sheet). The host hands the
// router, the models and the config binding in as plain objects; the
// tests hand in fakes. `qaState` names a designed state to render instead
// of a route (DETTIVO_E2E_STATE, docs/app.md).
Window {
    id: root

    required property var router
    property var status: null
    property var engines: null
    property var history: null
    property var historyFiltered: null
    property var detail: null
    property var actions: null
    property var player: null
    property var today: null
    property var config: null
    property var firstRun: null
    property var settings: null
    property var modelsTable: null
    property var hotkeysSetup: null
    property var agents: null
    property var doctor: null
    property var meetings: null
    property var meetingLive: null
    property var meetingDetail: null
    property var meetingsActions: null
    property string qaState: ""

    // `/` from a page other than History: the window goes there first and
    // the search field takes focus once the page is up.
    signal searchRequested

    readonly property string page: root.router ? root.router.page : "home"
    readonly property string route: root.router ? root.router.route : "home"
    // First run is the whole window (first-run-*.png): no sidebar, no hints.
    readonly property bool firstRunShown: root.route === "onboarding" || root.qaState.length > 0
    readonly property bool hintSheetShown: hintSheet.visible
    readonly property var pageItem: pageLoader.item
    readonly property string keyHints: {
        if (root.route === "history")
            return qsTr("j / k move · enter open · / search · r re-run · e export · d delete");
        if (root.page === "meetings.live")
            return qsTr("s stop · escape back to meetings · notes save as you type");
        if (root.page === "meetings.detail")
            return qsTr("j / k move · 1 2 3 tabs · p raw / polished · r rename · e export · d delete");
        if (root.route === "meetings")
            return qsTr("j / k move · enter open · n new meeting · i import · / search");
        if (root.route === "home")
            return qsTr("j / k move · enter open · / search · m meetings");
        return qsTr("tab / shift+tab move · space toggle · enter apply · / search");
    }

    color: Theme.roleSurface
    height: Theme.appWindowHeight
    minimumHeight: Theme.appWindowHeight / 2
    minimumWidth: Theme.appWindowMinWidth
    title: root.router ? qsTr("Dettivo · %1").arg(root.router.title) : qsTr("Dettivo")
    visible: true
    width: Theme.appWindowWidth

    function toggleFullscreen() {
        root.visibility = root.visibility === Window.FullScreen ? Window.Windowed : Window.FullScreen;
    }

    function beginMeeting() {
        if (root.route === "meetings" && root.pageItem && root.pageItem.newMeeting)
            root.pageItem.newMeeting();
    }

    function startMeetingFromHome() {
        if (!root.router)
            return;
        if (root.meetingLive && root.meetingLive.active) {
            root.router.open("meetings.live");
            return;
        }
        root.router.open("meetings");
        // The Home page is destroyed by navigation. Schedule on the window,
        // whose context survives until the new page is ready.
        Qt.callLater(root.beginMeeting);
    }

    Shortcut {
        sequence: "Meta+F"
        onActivated: root.toggleFullscreen()
    }

    Shortcut {
        sequence: "?"
        onActivated: root.toggleHintSheet()
    }

    function toggleHintSheet() {
        hintSheet.visible = !hintSheet.visible;
        if (hintSheet.visible)
            hintSheet.forceActiveFocus();
    }

    Shortcut {
        sequence: "Escape"
        onActivated: {
            if (hintSheet.visible)
                hintSheet.visible = false;
            else if (root.visibility === Window.FullScreen)
                root.toggleFullscreen();
            else if (root.pageItem && root.pageItem.dismiss && root.pageItem.dismiss())
                return;
            else if (root.router && root.router.detail)
                root.router.back();
        }
    }

    Shortcut {
        sequence: "/"
        onActivated: {
            // On Meetings the slash searches meetings; everywhere else it
            // goes to History.
            if (root.router && root.route !== "history" && root.route !== "meetings")
                root.router.open("history");
            if (root.pageItem && root.pageItem.focusSearch)
                root.pageItem.focusSearch();
            root.searchRequested();
        }
    }

    Shortcut {
        // From Home alone: the other pages have fields that take letters.
        enabled: root.route === "home"
        sequence: "M"
        onActivated: {
            if (root.router)
                root.router.open("meetings");
        }
    }

    Sidebar {
        id: sidebar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.top: parent.top
        engines: root.engines
        router: root.router
        status: root.status
        visible: !root.firstRunShown
        width: root.firstRunShown ? 0 : Theme.sidebarWidth
    }

    Item {
        id: content
        anchors.bottom: parent.bottom
        anchors.left: sidebar.right
        anchors.right: parent.right
        anchors.top: parent.top

        DaemonBanner {
            id: banner
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            status: root.status
        }

        Loader {
            id: pageLoader
            anchors.bottom: root.firstRunShown || root.route === "meetings" ? parent.bottom : hints.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: banner.bottom
            sourceComponent: {
                if (root.qaState.length > 0)
                    return statesPage;
                switch (root.route) {
                case "history":
                    return historyPage;
                case "meetings":
                    return meetingsPage;
                case "settings":
                    return settingsPage;
                case "onboarding":
                    return onboardingPage;
                default:
                    return homePage;
                }
            }
        }

        Text {
            id: hints
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.pagePaddingY
            anchors.left: parent.left
            // On History the hints sit under the detail, beside the list column.
            anchors.leftMargin: root.route === "history" ? Theme.historyListWidth + Theme.pagePaddingX : Theme.pagePaddingX
            color: Theme.roleFaintText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.typeCaptionSize
            text: root.keyHints
            // The meetings routes draw the hints in their own footer.
            visible: !root.firstRunShown && root.route !== "meetings"
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Keyboard hints")
        }
    }

    HintSheet {
        id: hintSheet
        anchors.centerIn: parent
        visible: false
        onClosed: hintSheet.visible = false
    }

    Component {
        id: statesPage
        StatesPage {
            stateName: root.qaState
        }
    }

    Component {
        id: homePage
        HomeRoute {
            meetingActive: root.meetingLive ? root.meetingLive.active : false
            config: root.config
            engines: root.engines
            router: root.router
            status: root.status
            today: root.today
            onMeetingRequested: root.startMeetingFromHome()
        }
    }

    Component {
        id: historyPage
        HistoryRoute {
            actions: root.actions
            detail: root.detail
            history: root.history
            historyFiltered: root.historyFiltered
            player: root.player
            router: root.router
            status: root.status
        }
    }

    Component {
        id: meetingsPage
        MeetingsRoute {
            actions: root.actions
            config: root.config
            detail: root.meetingDetail
            live: root.meetingLive
            meetings: root.meetings
            meetingsActions: root.meetingsActions
            router: root.router
            status: root.status
        }
    }

    Component {
        id: settingsPage
        SettingsRoute {
            agents: root.agents
            config: root.config
            doctor: root.doctor
            hotkeysSetup: root.hotkeysSetup
            modelsTable: root.modelsTable
            router: root.router
            settings: root.settings
            status: root.status
        }
    }

    Component {
        id: onboardingPage
        OnboardingRoute {
            firstRun: root.firstRun
            router: root.router
            status: root.status
        }
    }
}
