import QtQuick
import Dettivo
import DettivoApp

// The app window over the host's objects (fn-17, ADR 0020): the saved
// geometry is restored before the first frame and reported back when the
// window closes.
AppWindow {
    id: root

    actions: AppHost.actions
    agents: AppHost.agents
    config: AppHost.config
    detail: AppHost.detail
    doctor: AppHost.doctor
    engines: AppHost.engines
    firstRun: AppHost.firstRun
    history: AppHost.history
    historyFiltered: AppHost.historyFiltered
    hotkeysSetup: AppHost.hotkeysSetup
    meetingDetail: AppHost.meetingDetail
    meetingLive: AppHost.meetingLive
    meetings: AppHost.meetings
    meetingsActions: AppHost.meetingsActions
    modelsTable: AppHost.modelsTable
    player: AppHost.player
    qaState: AppHost.qaState
    router: AppHost.router
    settings: AppHost.settings
    status: AppHost.status
    today: AppHost.today

    Component.onCompleted: {
        if (AppHost.savedWidth > 0 && AppHost.savedHeight > 0) {
            root.width = AppHost.savedWidth;
            root.height = AppHost.savedHeight;
        }
        if (AppHost.savedX >= 0 && AppHost.savedY >= 0) {
            root.x = AppHost.savedX;
            root.y = AppHost.savedY;
        }
        if (AppHost.savedMaximized)
            root.visibility = Window.Maximized;
    }

    onClosing: AppHost.rememberGeometry(root.width, root.height, root.x, root.y, root.visibility === Window.Maximized)
}
