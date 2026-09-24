// What Main.qml receives from the host (ADR 0020): the router, the models,
// the config binding and the saved window state, as one QML singleton the
// dettivo-app module compiles in. The shared `Dettivo.AppWindow` takes
// these as plain object properties, so the Quick Tests hand it fakes.
#pragma once

#include "app_state.h"

#include <QJsonObject>
#include <QObject>
#include <QQmlEngine>
#include <QString>

namespace dettivo {

class ConfigBinding;
class EnginesModel;
class FirstRunModel;
class HistoryActions;
class HistoryDetailModel;
class HistoryFilterModel;
class HistoryModel;
class HistoryPlayer;
class MeetingDetailModel;
class MeetingLiveModel;
class MeetingsActions;
class MeetingsModel;
class NewestDayModel;
class Router;
class StatusModel;

class AgentsModel;
class DoctorModel;
class HotkeysSetupModel;
class ModelsTable;
class SettingsModel;

class AppHost : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(QObject *router READ router CONSTANT)
    Q_PROPERTY(QObject *status READ status CONSTANT)
    Q_PROPERTY(QObject *engines READ engines CONSTANT)
    Q_PROPERTY(QObject *history READ history CONSTANT)
    Q_PROPERTY(QObject *today READ today CONSTANT)
    Q_PROPERTY(QObject *historyFiltered READ historyFiltered CONSTANT)
    Q_PROPERTY(QObject *detail READ detail CONSTANT)
    Q_PROPERTY(QObject *actions READ actions CONSTANT)
    Q_PROPERTY(QObject *player READ player CONSTANT)
    Q_PROPERTY(QObject *config READ config CONSTANT)
    Q_PROPERTY(QObject *firstRun READ firstRun CONSTANT)
    Q_PROPERTY(QObject *settings READ settings CONSTANT)
    Q_PROPERTY(QObject *modelsTable READ modelsTable CONSTANT)
    Q_PROPERTY(QObject *hotkeysSetup READ hotkeysSetup CONSTANT)
    Q_PROPERTY(QObject *agents READ agents CONSTANT)
    Q_PROPERTY(QObject *doctor READ doctor CONSTANT)
    Q_PROPERTY(QObject *meetings READ meetings CONSTANT)
    Q_PROPERTY(QObject *meetingLive READ meetingLive CONSTANT)
    Q_PROPERTY(QObject *meetingDetail READ meetingDetail CONSTANT)
    Q_PROPERTY(QObject *meetingsActions READ meetingsActions CONSTANT)
    Q_PROPERTY(int savedWidth READ savedWidth CONSTANT)
    Q_PROPERTY(int savedHeight READ savedHeight CONSTANT)
    Q_PROPERTY(int savedX READ savedX CONSTANT)
    Q_PROPERTY(int savedY READ savedY CONSTANT)
    Q_PROPERTY(bool savedMaximized READ savedMaximized CONSTANT)
    Q_PROPERTY(QString qaState READ qaState CONSTANT)

public:
    struct Parts {
        Router *router = nullptr;
        StatusModel *status = nullptr;
        EnginesModel *engines = nullptr;
        HistoryModel *history = nullptr;
        NewestDayModel *today = nullptr;
        ConfigBinding *config = nullptr;
        FirstRunModel *firstRun = nullptr;
        HistoryFilterModel *historyFiltered = nullptr;
        HistoryDetailModel *detail = nullptr;
        HistoryActions *actions = nullptr;
        HistoryPlayer *player = nullptr;
        SettingsModel *settings = nullptr;
        ModelsTable *modelsTable = nullptr;
        HotkeysSetupModel *hotkeysSetup = nullptr;
        AgentsModel *agents = nullptr;
        DoctorModel *doctor = nullptr;
        MeetingsModel *meetings = nullptr;
        MeetingLiveModel *meetingLive = nullptr;
        MeetingDetailModel *meetingDetail = nullptr;
        MeetingsActions *meetingsActions = nullptr;
    };

    AppHost(Parts parts, AppState state, QObject *parent = nullptr);

    /// The QML singleton hands out the instance the host created.
    static void setInstance(AppHost *host);
    static AppHost *create(QQmlEngine *, QJSEngine *);

    QObject *router() const;
    QObject *status() const;
    QObject *engines() const;
    QObject *history() const;
    QObject *today() const;
    QObject *historyFiltered() const;
    QObject *detail() const;
    QObject *actions() const;
    QObject *player() const;
    QObject *config() const;
    QObject *firstRun() const;
    QObject *settings() const;
    QObject *modelsTable() const;
    QObject *hotkeysSetup() const;
    QObject *agents() const;
    QObject *doctor() const;
    QObject *meetings() const;
    QObject *meetingLive() const;
    QObject *meetingDetail() const;
    QObject *meetingsActions() const;
    int savedWidth() const { return m_state.width; }
    int savedHeight() const { return m_state.height; }
    int savedX() const { return m_state.x; }
    int savedY() const { return m_state.y; }
    bool savedMaximized() const { return m_state.maximized; }
    /// The designed state `DETTIVO_E2E_STATE` asked for, empty otherwise.
    QString qaState() const { return m_qaState; }
    void setQaState(const QString &state) { m_qaState = state; }

    /// The window reports its geometry so the state file follows it.
    Q_INVOKABLE void rememberGeometry(int width, int height, int x, int y, bool maximized);
    /// First run finished (or was found not needed): the moment is kept.
    void rememberFirstRun(const QString &completedAt);

    /// The state as it will be saved (the last route is read at save time).
    AppState state() const;
    const Parts &parts() const { return m_parts; }

private:
    Parts m_parts;
    AppState m_state;
    QString m_qaState;
};

}  // namespace dettivo
