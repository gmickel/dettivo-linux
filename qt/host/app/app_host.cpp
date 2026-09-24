#include "app_host.h"
#include "agents_model.h"
#include "config_binding.h"
#include "doctor_model.h"
#include "hotkeys_setup_model.h"
#include "meeting_detail_model.h"
#include "meeting_live_model.h"
#include "meetings_actions.h"
#include "meetings_model.h"
#include "models_table.h"
#include "settings_model.h"
#include "engines_model.h"
#include "first_run_model.h"
#include "history_actions.h"
#include "history_detail_model.h"
#include "history_model.h"
#include "history_player.h"
#include "router.h"
#include "status_model.h"

namespace dettivo {

namespace {
AppHost *g_instance = nullptr;
}

AppHost::AppHost(Parts parts, AppState state, QObject *parent)
    : QObject(parent), m_parts(parts), m_state(std::move(state))
{
}

void AppHost::setInstance(AppHost *host)
{
    g_instance = host;
}

AppHost *AppHost::create(QQmlEngine *engine, QJSEngine *)
{
    if (g_instance == nullptr)
        return nullptr;
    // The host owns the object; the engine must not delete it.
    QQmlEngine::setObjectOwnership(g_instance, QQmlEngine::CppOwnership);
    Q_UNUSED(engine);
    return g_instance;
}

QObject *AppHost::router() const
{
    return m_parts.router;
}

QObject *AppHost::status() const
{
    return m_parts.status;
}

QObject *AppHost::engines() const
{
    return m_parts.engines;
}

QObject *AppHost::history() const
{
    return m_parts.history;
}

QObject *AppHost::today() const
{
    return m_parts.today;
}

QObject *AppHost::historyFiltered() const
{
    return m_parts.historyFiltered;
}

QObject *AppHost::detail() const
{
    return m_parts.detail;
}

QObject *AppHost::actions() const
{
    return m_parts.actions;
}

QObject *AppHost::player() const
{
    return m_parts.player;
}

QObject *AppHost::config() const
{
    return m_parts.config;
}

QObject *AppHost::firstRun() const
{
    return m_parts.firstRun;
}

QObject *AppHost::settings() const
{
    return m_parts.settings;
}

QObject *AppHost::modelsTable() const
{
    return m_parts.modelsTable;
}

QObject *AppHost::hotkeysSetup() const
{
    return m_parts.hotkeysSetup;
}

QObject *AppHost::agents() const
{
    return m_parts.agents;
}

QObject *AppHost::doctor() const
{
    return m_parts.doctor;
}

QObject *AppHost::meetings() const
{
    return m_parts.meetings;
}

QObject *AppHost::meetingLive() const
{
    return m_parts.meetingLive;
}

QObject *AppHost::meetingDetail() const
{
    return m_parts.meetingDetail;
}

QObject *AppHost::meetingsActions() const
{
    return m_parts.meetingsActions;
}

void AppHost::rememberFirstRun(const QString &completedAt)
{
    m_state.firstRunCompletedAt = completedAt;
    m_state.firstRunStep.clear();
}

void AppHost::rememberGeometry(int width, int height, int x, int y, bool maximized)
{
    if (!maximized) {
        m_state.width = width;
        m_state.height = height;
        m_state.x = x;
        m_state.y = y;
    }
    m_state.maximized = maximized;
}

AppState AppHost::state() const
{
    AppState s = m_state;
    if (m_parts.router != nullptr)
        s.lastRoute = m_parts.router->page();
    if (m_parts.meetingDetail != nullptr)
        s.lastMeetingTab = m_parts.meetingDetail->lastTab();
    // A flow closed mid-way reopens on its step; a finished one never does.
    if (m_parts.firstRun != nullptr && !s.firstRunComplete() && s.lastRoute == QStringLiteral("onboarding"))
        s.firstRunStep = m_parts.firstRun->step();
    if (s.lastRoute == QStringLiteral("onboarding"))
        s.lastRoute = QStringLiteral("home");
    return s;
}

}  // namespace dettivo
