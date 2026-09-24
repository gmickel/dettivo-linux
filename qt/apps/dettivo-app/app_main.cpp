#include "app_main.h"

#include "app_control.h"
#include "app_environment.h"
#include "app_host.h"
#include "app_journal.h"
#include "agents_model.h"
#include "app_state.h"
#include "cli_runner.h"
#include "config_binding.h"
#include "doctor_model.h"
#include "daemon_client.h"
#include "daemon_paths.h"
#include "engines_model.h"
#include "first_run_model.h"
#include "first_frame_timer.h"
#include "history_actions.h"
#include "history_detail_model.h"
#include "history_model.h"
#include "history_player.h"
#include "hotkeys_setup_model.h"
#include "meeting_detail_model.h"
#include "meeting_live_model.h"
#include "meetings_actions.h"
#include "meetings_model.h"
#include "models_table.h"
#include "pacing_collector.h"
#include "qa_environment.h"
#include "render.h"
#include "router.h"
#include "sample_data.h"
#include "settings_model.h"
#include "status_model.h"
#include "style_check.h"
#include "theme_backend.h"
#include "unix_signals.h"

#include <QElapsedTimer>
#include <QFile>
#include <QDeadlineTimer>
#include <QThread>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>

#include <cstdio>
#include <cstring>

namespace dettivo {

namespace {

constexpr auto kStyle = "DettivoStyle";

/// The route a `DETTIVO_E2E_MEETING_STATE` shows: the list for the list
/// states and the two dialogs, the live screen, the detail for the three
/// tabs and the rename popover.
QString meetingStateRoute(const QString &state)
{
    if (state == QStringLiteral("live"))
        return QStringLiteral("meetings.live");
    if (state.startsWith(QStringLiteral("detail")) || state == QStringLiteral("rename"))
        return QStringLiteral("meetings.detail");
    return QStringLiteral("meetings");
}

/// The route the process starts on: `--open` first, then the QA
/// variables; `variable` names which one refused a bad name.
bool firstRoute(const AppArgs &args, const QaEnvironment &qa, QString *page, QString *error, QString *variable)
{
    QString name = args.open;
    *variable = QStringLiteral("--open");
    if (name.isEmpty() && !qa.e2eOpen.isEmpty()) {
        name = qa.e2eOpen;
        *variable = QStringLiteral("DETTIVO_E2E_OPEN");
        if (name == QStringLiteral("settings") && !qa.e2eRoute.isEmpty()) {
            name = QStringLiteral("settings.") + qa.e2eRoute;
            *variable = QStringLiteral("DETTIVO_E2E_ROUTE");
        }
    }
    if (name.isEmpty() && !qa.e2eMeetingState.isEmpty()) {
        name = meetingStateRoute(qa.e2eMeetingState);
        *variable = QStringLiteral("DETTIVO_E2E_MEETING_STATE");
    }
    if (name.isEmpty()) {
        page->clear();
        return true;
    }
    QString route, sub;
    if (!Router::parse(name, &route, &sub, error))
        return false;
    *page = Router::pageFor(route, sub);
    return true;
}

QJsonObject statusOf(const Router &router, const StatusModel &status, const QQuickWindow *window, qint64 firstFrameMs,
                     const ThemeBackend *theme)
{
    return {{QStringLiteral("route"), router.page()},
            {QStringLiteral("title"), router.title()},
            {QStringLiteral("visible"), window != nullptr && window->isVisible()},
            {QStringLiteral("active"), window != nullptr && window->isActive()},
            {QStringLiteral("fullscreen"), window != nullptr && window->visibility() == QWindow::FullScreen},
            {QStringLiteral("daemon_connected"), status.daemonConnected()},
            {QStringLiteral("daemon_state"), status.daemonState()},
            {QStringLiteral("dictation_state"), status.dictationState()},
            {QStringLiteral("first_frame_ms"), firstFrameMs},
            {QStringLiteral("rss_kb"), AppJournal::residentKb()},
            {QStringLiteral("environment"), appEnvironment(window)},
            {QStringLiteral("pid"), qint64(QCoreApplication::applicationPid())},
            {QStringLiteral("theme"), theme != nullptr ? theme->status() : QJsonObject()}};
}

}  // namespace

AppArgs parseAppArgs(int argc, char **argv)
{
    AppArgs a;
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--version") == 0)
            a.version = true;
        else if (std::strcmp(argv[i], "--smoke") == 0)
            a.smoke = true;
        else if (std::strcmp(argv[i], "--sample") == 0)
            a.sample = true;
        else if (std::strcmp(argv[i], "--render") == 0 && i + 1 < argc)
            a.render = QString::fromLocal8Bit(argv[++i]);
        else if (std::strcmp(argv[i], "--open") == 0 && i + 1 < argc)
            a.open = QString::fromLocal8Bit(argv[++i]);
        else if (std::strcmp(argv[i], "--id") == 0 && i + 1 < argc)
            a.id = QString::fromLocal8Bit(argv[++i]);
    }
    return a;
}

int runApp(int argc, char **argv)
{
    QElapsedTimer sinceStart;
    sinceStart.start();
    const AppArgs args = parseAppArgs(argc, argv);
    if (args.version) {
        std::printf("dettivo-app %s\n", DETTIVO_VERSION);
        return 0;
    }
    QString qaError;
    const auto qa = QaEnvironment::fromProcess(&qaError);
    if (!qa.has_value()) {
        std::fprintf(stderr, "dettivo-app: %s\n", qPrintable(qaError));
        return 2;
    }
    QString page, routeError, routeVariable;
    if (!firstRoute(args, *qa, &page, &routeError, &routeVariable)) {
        std::fprintf(stderr, "dettivo-app: %s: %s\n", qPrintable(routeVariable), qPrintable(routeError));
        return 2;
    }
    const bool headless = args.smoke || !args.render.isEmpty();
    const QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    const QString socketDir = paths::socketDir(env);
    const QString stateDir = paths::stateDir(env);

    // One window per session (R5): a running app takes the request and
    // this process is done. The forward is tried once before the lock and
    // again, for a while, when the lock belongs to a sibling still
    // starting up; -1 means nobody answered.
    QJsonObject command{{QStringLiteral("cmd"), page.isEmpty() ? QStringLiteral("raise") : QStringLiteral("open")}};
    if (!page.isEmpty()) {
        command.insert(QStringLiteral("route"), page);
        command.insert(QStringLiteral("arg"), args.id);
    }
    const auto forwardOnce = [&](int timeoutMs) -> int {
        QJsonObject reply;
        if (!AppControl::forward(socketDir, command, &reply, timeoutMs))
            return -1;
        if (reply.value(QStringLiteral("ok")).toBool(false)) {
            std::printf("dettivo-app: raised the running window%s\n",
                        page.isEmpty() ? "" : qPrintable(QStringLiteral(" on ") + page));
            return 0;
        }
        std::fprintf(stderr, "dettivo-app: the running app refused: %s\n",
                     qPrintable(reply.value(QStringLiteral("error")).toString()));
        return 2;
    };
    if (!headless) {
        const int forwarded = forwardOnce(1000);
        if (forwarded >= 0)
            return forwarded;
    }
    // Ownership before any window exists: the launch holding `app.lock`
    // is the instance. A taken lock means a sibling is between its check
    // and its socket; this launch waits for it to answer instead of
    // opening a second window or unlinking its socket.
    InstanceLock instanceLock;
    QString lockError;
    if (!instanceLock.acquire(socketDir, &lockError) && !headless) {
        QDeadlineTimer sibling(5000);
        while (!sibling.hasExpired()) {
            const int forwarded = forwardOnce(500);
            if (forwarded >= 0)
                return forwarded;
            QThread::msleep(100);
        }
        std::fprintf(stderr, "dettivo-app: another dettivo-app is starting and did not answer (%s)\n",
                     qPrintable(lockError));
        return 2;
    }

    QGuiApplication::setHighDpiScaleFactorRoundingPolicy(Qt::HighDpiScaleFactorRoundingPolicy::PassThrough);
    QGuiApplication app(argc, argv);
    const qint64 appMs = sinceStart.elapsed();
    QGuiApplication::setApplicationName(QStringLiteral("dettivo-app"));
    QGuiApplication::setApplicationVersion(QStringLiteral(DETTIVO_VERSION));
    QGuiApplication::setOrganizationName(QStringLiteral("dettivo"));
    QGuiApplication::setDesktopFileName(QStringLiteral("dettivo"));
    QQuickStyle::setStyle(QString::fromLatin1(kStyle));
    QQuickStyle::setFallbackStyle(QStringLiteral("Basic"));

    QString stateWarning;
    AppState state = headless ? AppState() : AppState::load(AppState::path(env), &stateWarning);
    if (!stateWarning.isEmpty())
        std::fprintf(stderr, "dettivo-app: %s\n", qUtf8Printable(stateWarning));
    AppJournal journal(stateDir, qa->enabled && !headless);

    DaemonClient client(paths::daemonSocket(env), paths::ipcToken(env));
    ConfigBinding config(&client);
    StatusModel status(&client, &config);
    EnginesModel engines(&client);
    HistoryModel history(&client);
    NewestDayModel today(&history);
    FirstRunModel firstRun(&client, &config);
    HistoryFilterModel historyFiltered(&history);
    HistoryDetailModel detail(&client);
    HistoryActions actions(&client, qa->e2eExportDir);
    HistoryPlayer player;
    // The settings routes (ADR 0033): the editor over the same binding,
    // the models table, the hotkeys snippet panel, and the two routes
    // that run the command line (`dettivo mcp config`, `dettivo doctor`).
    SettingsModel settingsModel(&client, &config);
    ModelsTable modelsTable(&client);
    HotkeysSetupModel hotkeysSetup(&client, &config);
    ProcessRunner runner;
    AgentsModel agents(&runner);
    QObject::connect(&agents, &AgentsModel::hostsChanged, &status, [&]() {
        QStringList configured;
        for (const QVariant &entry : agents.hosts()) {
            const QVariantMap host = entry.toMap();
            if (host.value(QStringLiteral("path")).toString().isEmpty())
                return;
            if (host.value(QStringLiteral("configured")).toBool())
                configured.append(host.value(QStringLiteral("id")).toString());
        }
        status.setMcpHosts(configured);
    });
    DoctorModel doctor(&runner);
    // Meetings (ADR 0038): the list, the live meeting, the detail and the
    // actions, all clients of `meetings.*` over the same link.
    MeetingsModel meetings(&client);
    MeetingLiveModel meetingLive(&client);
    MeetingDetailModel meetingDetail(&client);
    MeetingsActions meetingsActions(&client, &config);
    meetings.setQaState(qa->e2eMeetingState);
    meetingDetail.setLastTab(state.lastMeetingTab);
    Router router;
    // First run (ADR 0024): `DETTIVO_E2E_COMPLETE` marks it done,
    // `DETTIVO_E2E_STEP` opens a step by name, a finished flow in the
    // state file never shows again, and an explicit `onboarding` route
    // reopens it on the remembered step.
    if (qa->e2eComplete || state.firstRunComplete())
        firstRun.markComplete();
    if (!qa->e2eStep.isEmpty()) {
        if (page.isEmpty())
            page = QStringLiteral("onboarding");
        firstRun.openAt(qa->e2eStep);
    } else if (page == QStringLiteral("onboarding")) {
        firstRun.openAt(state.firstRunStep);
    } else if (!headless && !state.firstRunComplete() && !state.firstRunStep.isEmpty() && page.isEmpty()) {
        // The window closed mid-flow: it reopens where it was.
        page = QStringLiteral("onboarding");
        firstRun.openAt(state.firstRunStep);
    }
    // A meeting state names the detail's meeting when the route is a
    // detail and no id was given; the sample fills it under a render.
    QString firstArg = args.id;
    if (firstArg.isEmpty() && page == QStringLiteral("meetings.detail")) {
        if (!qa->e2eRoute.isEmpty())
            firstArg = qa->e2eRoute;
        else if (!qa->e2eMeetingState.isEmpty())
            firstArg = sample::meetingDetailId();
    }
    if (!page.isEmpty())
        router.open(page, firstArg);
    else if (!headless && state.lastRoute != QStringLiteral("home"))
        router.open(state.lastRoute);
    const bool routeChosen = !page.isEmpty();
    AppHost host({&router, &status, &engines, &history, &today, &config, &firstRun, &historyFiltered, &detail, &actions, &player,
                  &settingsModel, &modelsTable, &hotkeysSetup, &agents, &doctor, &meetings, &meetingLive, &meetingDetail,
                  &meetingsActions},
                 state);
    host.setQaState(qa->e2eState);
    AppHost::setInstance(&host);
    QObject::connect(&firstRun, &FirstRunModel::completed, &host, [&](const QString &at) {
        host.rememberFirstRun(at);
        if (router.route() == QStringLiteral("onboarding"))
            router.open(QStringLiteral("home"));
    });
    QObject::connect(&firstRun, &FirstRunModel::decisionChanged, &router, [&]() {
        if (firstRun.decided() && firstRun.required() && !routeChosen && router.route() != QStringLiteral("onboarding"))
            router.open(QStringLiteral("onboarding"));
    });

    const bool sampled = args.sample || (!args.render.isEmpty() && qa->e2eSeed);
    const auto activateRoute = [&]() {
        if (sampled)
            return;
        const QString route = router.route();
        const QString section = route == QStringLiteral("settings") ? router.sub() : QString();
        settingsModel.setActive(route == QStringLiteral("settings"));
        modelsTable.setActive(section == QStringLiteral("models") || section == QStringLiteral("meetings"));
        hotkeysSetup.setActive(section == QStringLiteral("hotkeys"));
        if (route == QStringLiteral("onboarding"))
            firstRun.openAt(firstRun.step());
        else
            firstRun.leave();
        if (client.connected() && section == QStringLiteral("agents"))
            agents.refresh();
        if (client.connected() && section == QStringLiteral("diagnostics"))
            doctor.run();
    };
    QObject::connect(&router, &Router::routeChanged, &app, activateRoute);
    QObject::connect(&client, &DaemonClient::connectedChanged, &app, [&](bool connected) {
        if (connected)
            activateRoute();
    });
    activateRoute();
    if (sampled) {
        sample::apply(&status, &engines, &history, &config, &detail, &player, &actions);
        sample::applySettings(&config, &settingsModel, &modelsTable, &hotkeysSetup, &agents, &doctor);
    }
    // TryIt's sample text is assigned once when its field is created, so
    // the sample must exist before loading the QML page. Apply it after
    // shared settings so their change signal cannot replace its key facts.
    if (sampled && router.route() == QStringLiteral("onboarding"))
        firstRun.applySample(qa->e2eStep.isEmpty() ? QStringLiteral("keys") : qa->e2eStep);

    QQmlApplicationEngine engine;
    engine.addImportPath(QStringLiteral(DETTIVO_QML_INSTALL_DIR));
    engine.loadFromModule(QStringLiteral("DettivoApp"), QStringLiteral("Main"));
    if (engine.rootObjects().isEmpty()) {
        std::fprintf(stderr, "dettivo-app: failed to load DettivoApp/Main.qml\n");
        return 1;
    }
    const qint64 qmlMs = sinceStart.elapsed();
    if (QQuickStyle::name() != QLatin1String(kStyle)) {
        std::fprintf(stderr, "dettivo-app: controls resolved to the %s style instead of %s; refusing to run\n",
                     qUtf8Printable(QQuickStyle::name()), kStyle);
        return 2;
    }
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    if (window == nullptr) {
        std::fprintf(stderr, "dettivo-app: Main.qml has no window\n");
        return 1;
    }
    if (args.smoke) {
        std::printf("dettivo-app: route %s title \"%s\"\n", qUtf8Printable(router.page()), qUtf8Printable(router.title()));
        return 0;
    }

    // The visual renders carry the baselines' facts with no daemon: with
    // `--sample`, and under `--render` with `DETTIVO_E2E_SEED=1`.
    if (sampled) {
        sample::applyMeetings(qa->e2eMeetingState.isEmpty() ? QStringLiteral("list") : qa->e2eMeetingState, &meetings, &meetingLive,
                              &meetingDetail, &meetingsActions);
        if (router.route() == QStringLiteral("meetings") && router.sub() == QStringLiteral("detail") && router.arg().isEmpty())
            router.open(QStringLiteral("meetings.detail"), sample::meetingDetailId());
        if (router.route() == QStringLiteral("onboarding")) {
            QJsonObject facts = sample::status();
            facts.insert(QStringLiteral("levels"), true);
            status.applySample(facts);
        }
        // History renders with its newest row open, the way history.png
        // shows it.
        if (router.route() == QStringLiteral("history"))
            router.open(QStringLiteral("history.detail"), sample::detailId());
    } else {
        client.start();
    }
    status.start();
    firstRun.start();

    // Frame pacing evidence for the drives (fn-20 R4): DETTIVO_QA_PACING
    // names the file the summary lands in, theme changes included, the
    // way every other Qt host attaches it.
    auto *pacing = PacingCollector::attachFromEnvironment(window, &engine);
    if (pacing == nullptr) {
        pacing = new PacingCollector(window, 0, window, false);
        pacing->watchTheme(&engine);
        pacing->start();
    }
    QObject::connect(pacing, &PacingCollector::themeApplied, &app, [&](const QJsonObject &change) {
        const qint64 ms = qint64(change.value(QStringLiteral("frame_after_ms")).toDouble());
        const QString source = change.value(QStringLiteral("source")).toString();
        const QString accent = change.value(QStringLiteral("accent")).toString();
        std::fprintf(stderr, "dettivo-app: theme applied (%s, accent %s) %lld ms after the change\n",
                     qUtf8Printable(source), qUtf8Printable(accent), static_cast<long long>(ms));
        journal.record(QStringLiteral("theme_applied"),
                       {{QStringLiteral("ms"), ms}, {QStringLiteral("source"), source}, {QStringLiteral("accent"), accent}});
    });

    qint64 firstFrameMs = -1;
    auto *theme = engine.singletonInstance<ThemeBackend *>(QStringLiteral("Dettivo"), QStringLiteral("ThemeBackend"));
    FirstFrameTimer firstFrame(window, sinceStart);
    QObject::connect(&firstFrame, &FirstFrameTimer::captured, &app, [&](qint64 capturedMs) {
        if (firstFrameMs < 0) {
            firstFrameMs = capturedMs;
            const qint64 rss = AppJournal::residentKb();
            std::fprintf(stderr,
                         "dettivo-app: first frame %lld ms (application %lld ms, qml %lld ms), %lld kB resident, theme %s\n",
                         static_cast<long long>(firstFrameMs), static_cast<long long>(appMs),
                         static_cast<long long>(qmlMs), static_cast<long long>(rss),
                         theme != nullptr ? qUtf8Printable(theme->source()) : "unknown");
            journal.record(QStringLiteral("first_frame"),
                           {{QStringLiteral("ms"), firstFrameMs},
                            {QStringLiteral("application_ms"), appMs},
                            {QStringLiteral("qml_ms"), qmlMs},
                            {QStringLiteral("rss_kb"), rss},
                            {QStringLiteral("theme"), theme != nullptr ? theme->source() : QString()}});
        }
    });

    int result = 0;
    if (!args.render.isEmpty()) {
        // The shared render of every host (qt/host/render.cpp): the grab
        // after the first frame, then the negative style check over the
        // tree, with the QA plant when one is asked for (ADR 0021).
        renderAndQuit(&app, &engine, window, args.render, qa->qaPlant, &result);
        const int code = QGuiApplication::exec();
        return result != 0 ? result : code;
    }

    AppControl control;
    control.setStatusProvider([&]() {
        QJsonObject statusReport = statusOf(router, status, window, firstFrameMs, theme);
        statusReport.insert(QStringLiteral("playback_active"), player.playing());
        statusReport.insert(QStringLiteral("meeting_active"), meetingLive.active());
        statusReport.insert(QStringLiteral("analysis_status"), meetingDetail.analysisStatus());
        modelsTable.ensureDownloadStatus();
        statusReport.insert(QStringLiteral("active_downloads"), modelsTable.activeDownloads());
        return statusReport;
    });
    QString controlError;
    if (!control.listen(socketDir, instanceLock, &controlError))
        std::fprintf(stderr, "dettivo-app: instance socket unavailable: %s\n", qUtf8Printable(controlError));
    QObject::connect(&control, &AppControl::raiseRequested, window, [window]() {
        window->show();
        window->raise();
        window->requestActivate();
    });
    QObject::connect(&control, &AppControl::openRequested, &router,
                     [&router](const QString &route, const QString &arg) { router.open(route, arg); });
    quitOnTerminationSignals();

    if (journal.enabled()) {
        QTimer::singleShot(5000, &app, [&]() {
            journal.record(QStringLiteral("idle"),
                           {{QStringLiteral("ms"), sinceStart.elapsed()}, {QStringLiteral("rss_kb"), AppJournal::residentKb()}});
        });
    }
    QObject::connect(&app, &QCoreApplication::aboutToQuit, &app, [&]() {
        // Notes still waiting for their debounce go out before the socket
        // closes (ADR 0048).
        meetingLive.flushNotes();
        meetingDetail.flushNotes();
        client.finish();
        QString error;
        if (!host.state().save(AppState::path(env), &error))
            std::fprintf(stderr, "dettivo-app: state not saved: %s\n", qUtf8Printable(error));
    });
    return QGuiApplication::exec();
}

}  // namespace dettivo
