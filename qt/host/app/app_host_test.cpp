// Tests the app routes, state, models, instance socket and theme events.
#include "app_control.h"
#include "app_state.h"
#include "config_binding.h"
#include "daemon_link.h"
#include "engines_model.h"
#include "fake_link.h"
#include "history_model.h"
#include "router.h"
#include "sample_data.h"
#include "status_format.h"
#include "status_model.h"
#include "theme_backend.h"

#include <QCoreApplication>
#include <QDBusVariant>
#include <QJsonArray>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

#include <thread>

using namespace dettivo;
using dettivo::test::FakeLink;


class AppHostTest : public QObject {
    Q_OBJECT

private slots:
    void routerAcceptsEveryRouteAndRefusesTheRest();
    void routerBackLeavesADetailForItsList();
    void stateFileRoundTrips();
    void formattingMatchesTheBaseline();
    void statusModelReadsFactsAndFollowsTheStream();
    void enginesModelOrdersTheRailAndFollowsTheStream();
    void historyModelPagesAndGroupsByDay();
    void configBindingReadsAndWrites();
    void instanceSocketAnswersOneLine();
    void instanceLockAdmitsOneOwner();
    void portalColourSchemeSignalFlipsThePalette();
};

void AppHostTest::routerAcceptsEveryRouteAndRefusesTheRest()
{
    Router router;
    QCOMPARE(router.page(), QStringLiteral("home"));
    QCOMPARE(router.title(), QStringLiteral("Home"));
    const QStringList names = Router::routeNames();
    QCOMPARE(names.size(), 18);
    QVERIFY(names.contains(QStringLiteral("settings.vocabulary")));
    for (const QString &name : names) {
        QVERIFY2(router.open(name), qPrintable(name));
        QVERIFY2(!router.title().isEmpty(), qPrintable(name));
    }
    QCOMPARE(router.page(), QStringLiteral("settings.agents"));
    QCOMPARE(router.title(), QStringLiteral("Settings / Agents"));
    QString route, sub, error;
    QVERIFY(!Router::parse(QStringLiteral("garage"), &route, &sub, &error));
    QVERIFY(error.contains(QStringLiteral("settings.hotkeys")));
    QVERIFY(error.contains(QStringLiteral("meetings.live")));
    QVERIFY(!Router::parse(QStringLiteral("settings.audio"), &route, &sub, &error));
    QVERIFY(!Router::parse(QStringLiteral("home.detail"), &route, &sub, &error));

    Router stack;
    QVERIFY(stack.open(QStringLiteral("history")));
    QVERIFY(stack.open(QStringLiteral("history.detail"), QStringLiteral("abc")));
    QVERIFY(stack.isDetail());
    QVERIFY(stack.canGoBack());
    QCOMPARE(stack.arg(), QStringLiteral("abc"));
    QCOMPARE(stack.title(), QStringLiteral("Dictation"));
    stack.back();
    QCOMPARE(stack.page(), QStringLiteral("history"));
    QCOMPARE(stack.arg(), QString());
    QVERIFY(!stack.isDetail());
    stack.back();
    QCOMPARE(stack.page(), QStringLiteral("home"));
    QVERIFY(!stack.canGoBack());
    stack.back();
    QCOMPARE(stack.page(), QStringLiteral("home"));
}

void AppHostTest::routerBackLeavesADetailForItsList()
{
    // A detail opened from another page (a launcher action, `dettivo app
    // open`) still returns to its list, and only then to the page before.
    Router router;
    QVERIFY(router.open(QStringLiteral("settings.hotkeys")));
    QVERIFY(router.open(QStringLiteral("meetings.detail"), QStringLiteral("m1")));
    QVERIFY(router.isDetail());
    router.back();
    QCOMPARE(router.page(), QStringLiteral("meetings"));
    QCOMPARE(router.arg(), QString());
    QVERIFY(router.canGoBack());
    router.back();
    QCOMPARE(router.page(), QStringLiteral("settings.hotkeys"));
    router.back();
    QCOMPARE(router.page(), QStringLiteral("home"));

    // A detail with nothing underneath still has its list to return to,
    // and a chain of details of one route collapses onto the list.
    Router fresh;
    QVERIFY(fresh.open(QStringLiteral("history.detail"), QStringLiteral("a")));
    QVERIFY(fresh.canGoBack());
    QVERIFY(fresh.open(QStringLiteral("history.detail"), QStringLiteral("b")));
    fresh.back();
    QCOMPARE(fresh.page(), QStringLiteral("history"));
    fresh.back();
    QCOMPARE(fresh.page(), QStringLiteral("home"));

    // Off a detail, Escape keeps the plain stack semantics.
    Router plain;
    QVERIFY(plain.open(QStringLiteral("meetings")));
    QVERIFY(plain.open(QStringLiteral("settings.models")));
    plain.back();
    QCOMPARE(plain.page(), QStringLiteral("meetings"));
    plain.back();
    QCOMPARE(plain.page(), QStringLiteral("home"));
}

void AppHostTest::stateFileRoundTrips()
{
    AppState state;
    state.width = 1280;
    state.height = 820;
    state.x = 10;
    state.y = 20;
    state.lastRoute = QStringLiteral("settings.models");
    state.firstRunCompletedAt = QStringLiteral("2026-09-04T10:00:00Z");
    state.firstRunStep = QStringLiteral("try");
    state.lastMeetingTab = QStringLiteral("notes");
    QString warning;
    const AppState back = AppState::fromToml(state.toToml(), &warning);
    QVERIFY(warning.isEmpty());
    QCOMPARE(back.width, 1280);
    QCOMPARE(back.y, 20);
    QCOMPARE(back.lastRoute, QStringLiteral("settings.models"));
    QCOMPARE(back.firstRunCompletedAt, QStringLiteral("2026-09-04T10:00:00Z"));
    QVERIFY(back.firstRunComplete());
    QCOMPARE(back.firstRunStep, QStringLiteral("try"));
    QCOMPARE(back.lastMeetingTab, QStringLiteral("notes"));
    QCOMPARE(AppState().lastMeetingTab, QStringLiteral("transcript"));
    QVERIFY(!AppState().firstRunComplete());

    // The daemon's tables in the same file are read fresh at every save
    // (qt-hosts/F1, fn-43): an acknowledgement the daemon records while
    // the app is open survives the app's save on close, and a load-time
    // snapshot is never written back.
    const AppState shared = AppState::fromToml(
        QStringLiteral("[daemon]\nlast_started_unix = 42\nstart_count = 3\n\n[acknowledgements]\nmeeting_disclosure = false\n\n[first_run]\nstep = \"models\"\n"),
        &warning);
    QVERIFY(warning.isEmpty());
    QCOMPARE(shared.firstRunStep, QStringLiteral("models"));
    QVERIFY(!shared.firstRunComplete());
    QVERIFY2(!shared.toToml().contains(QStringLiteral("[daemon]")), qPrintable(shared.toToml()));
    QTemporaryDir sharedDir;
    QVERIFY(sharedDir.isValid());
    const QString sharedPath = sharedDir.filePath("state.toml");
    {
        QFile seed(sharedPath);
        QVERIFY(seed.open(QIODevice::WriteOnly));
        seed.write("[daemon]\nlast_started_unix = 42\nstart_count = 3\n\n[acknowledgements]\nmeeting_disclosure = false\n");
    }
    QString sharedWarning;
    AppState open = AppState::load(sharedPath, &sharedWarning);
    QVERIFY(sharedWarning.isEmpty());
    {
        // The daemon records the acknowledgement while the app is open.
        QFile daemon(sharedPath);
        QVERIFY(daemon.open(QIODevice::WriteOnly | QIODevice::Truncate));
        daemon.write("[daemon]\nlast_started_unix = 42\nstart_count = 4\n\n[acknowledgements]\nmeeting_disclosure = true\nmeeting_disclosure_at = \"2026-09-06T10:00:00Z\"\n");
    }
    open.lastRoute = QStringLiteral("meetings");
    QString sharedError;
    QVERIFY2(open.save(sharedPath, &sharedError), qPrintable(sharedError));
    QFile saved(sharedPath);
    QVERIFY(saved.open(QIODevice::ReadOnly));
    const QString written = QString::fromUtf8(saved.readAll());
    QVERIFY2(written.contains(QStringLiteral("meeting_disclosure = true")), qPrintable(written));
    QVERIFY2(written.contains(QStringLiteral("start_count = 4")), qPrintable(written));
    QVERIFY2(written.contains(QStringLiteral("last_route = \"meetings\"")), qPrintable(written));
    QCOMPARE(written.count(QStringLiteral("[first_run]")), 1);
    QCOMPARE(written.count(QStringLiteral("[acknowledgements]")), 1);

    const AppState bad = AppState::fromToml(QStringLiteral("this is not [toml"), &warning);
    QVERIFY(!warning.isEmpty());
    QCOMPARE(bad.lastRoute, QStringLiteral("home"));

    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    const QString path = dir.filePath("nested/state.toml");
    QString error;
    QVERIFY2(state.save(path, &error), qPrintable(error));
    QString loadWarning;
    QCOMPARE(AppState::load(path, &loadWarning).lastRoute, QStringLiteral("settings.models"));
    QVERIFY(loadWarning.isEmpty());
    QCOMPARE(AppState::load(dir.filePath("absent.toml"), &loadWarning).width, 0);

    QProcessEnvironment env;
    env.insert(QStringLiteral("XDG_STATE_HOME"), QStringLiteral("/tmp/s"));
    QCOMPARE(AppState::path(env), QStringLiteral("/tmp/s/dettivo/state.toml"));
}

void AppHostTest::formattingMatchesTheBaseline()
{
    QCOMPARE(format::chord(QStringLiteral("F9")), QStringLiteral("F9"));
    QCOMPARE(format::chord(QStringLiteral("SUPER CTRL, X")), QStringLiteral("Super+Ctrl+X"));
    QCOMPARE(format::chord(QStringLiteral("SUPER + CTRL + ESCAPE")), QStringLiteral("Super+Ctrl+Esc"));
    QCOMPARE(format::chord(QStringLiteral("SUPER CTRL SHIFT, X")), QStringLiteral("Super+Ctrl+Shift+X"));
    QCOMPARE(format::chord(QString()), QString());
    QCOMPARE(format::appName(QStringLiteral("com.mitchellh.ghostty")), QStringLiteral("ghostty"));
    QCOMPARE(format::appName(QStringLiteral("org.gnome.TextEditor")), QStringLiteral("TextEditor"));
    QCOMPARE(format::appName(QStringLiteral("Alacritty")), QStringLiteral("Alacritty"));
    QCOMPARE(format::duration(4.4), QStringLiteral("4 s"));
    QCOMPARE(format::duration(41 * 60), QStringLiteral("41 min"));
    QCOMPARE(format::duration(72 * 60), QStringLiteral("1 h 12 min"));
    QCOMPARE(format::modelName(QStringLiteral("/home/u/.local/share/dettivo/models/whisper/large-v3-turbo/ggml-large-v3-turbo.bin")),
             QStringLiteral("large-v3-turbo"));
    QCOMPARE(format::modelName(QStringLiteral("ggml-tiny.en")), QStringLiteral("tiny.en"));
    QCOMPARE(format::modelName(QStringLiteral("parakeet-v3-int8")), QStringLiteral("parakeet-v3-int8"));
    const QDate today(2026, 2, 13);
    QCOMPARE(format::dayLabel(QStringLiteral("2026-02-13T09:00:00Z"), today), QStringLiteral("Today"));
    QCOMPARE(format::dayLabel(QStringLiteral("2026-02-12T09:00:00Z"), today), QStringLiteral("Yesterday"));
    QCOMPARE(format::dayLabel(QStringLiteral("2026-02-11T09:00:00Z"), today), QStringLiteral("Wed 11 Feb"));
    QCOMPARE(format::timeOfDay(QStringLiteral("not a time")), QString());
    QCOMPARE(format::clock(QDateTime(QDate(2026, 9, 3), QTime(13, 42))), QStringLiteral("Thu 3 Sep · 13:42"));
    QProcessEnvironment env;
    env.insert(QStringLiteral("HOME"), QStringLiteral("/home/u"));
    env.insert(QStringLiteral("XDG_DATA_HOME"), QStringLiteral("/tmp/dq1/data"));
    QCOMPARE(format::homePath(QStringLiteral("/home/u/.config/dettivo/config.toml"), env), QStringLiteral("~/.config/dettivo/config.toml"));
    QCOMPARE(format::homePath(QStringLiteral("/tmp/dq1/data/dettivo/models"), env), QStringLiteral("$XDG_DATA_HOME/dettivo/models"));
    QCOMPARE(format::homePath(QStringLiteral("/etc/dettivo.toml"), env), QStringLiteral("/etc/dettivo.toml"));
    QCOMPARE(format::homePath(QString(), env), QString());
}

void AppHostTest::statusModelReadsFactsAndFollowsTheStream()
{
    FakeLink link;
    link.answers.insert(QStringLiteral("system.health"), {{QStringLiteral("ok"), true}, {QStringLiteral("recording_state"), QStringLiteral("idle")}});
    link.answers.insert(QStringLiteral("system.version"), {{QStringLiteral("version"), QStringLiteral("0.1.0")}});
    link.answers.insert(QStringLiteral("system.capabilities"),
                        {{QStringLiteral("auth"), QJsonObject{{QStringLiteral("ipc_mode"), QStringLiteral("peer")}}},
                         {QStringLiteral("platform"), QJsonObject{{QStringLiteral("gpu"), QStringLiteral("vulkan")}, {QStringLiteral("compositor"), QStringLiteral("Hyprland")}}}});
    link.answers.insert(QStringLiteral("insert.target"),
                        {{QStringLiteral("target"), QJsonObject{{QStringLiteral("app_id"), QStringLiteral("com.mitchellh.ghostty")}, {QStringLiteral("is_dettivo"), false}}}});
    link.answers.insert(QStringLiteral("audio.devices"),
                        {{QStringLiteral("default_source"), QStringLiteral("alsa_input.usb")},
                         {QStringLiteral("pinned"), QString()},
                         {QStringLiteral("pipewire"), true},
                         {QStringLiteral("devices"), QJsonArray{QJsonObject{{QStringLiteral("name"), QStringLiteral("alsa_input.usb")}, {QStringLiteral("description"), QStringLiteral("Arctis Nova")}}}}});
    link.answers.insert(QStringLiteral("dictation.status"), {{QStringLiteral("is_active"), false}});
    link.answers.insert(QStringLiteral("config.get"),
                        {{QStringLiteral("entries"), QJsonArray{QJsonObject{{QStringLiteral("key"), QStringLiteral("hotkeys.hold")}, {QStringLiteral("value"), QStringLiteral("F9")}, {QStringLiteral("source"), QStringLiteral("default")}},
                                                             QJsonObject{{QStringLiteral("key"), QStringLiteral("hotkeys.toggle")}, {QStringLiteral("value"), QStringLiteral("SUPER CTRL, X")}, {QStringLiteral("source"), QStringLiteral("file")}}}}});
    ConfigBinding config(&link);
    StatusModel status(&link, &config);
    status.start();
    QCOMPARE(link.subscriptions.size(), 1);
    QVERIFY(link.subscriptions.first().contains(QStringLiteral("audio.level")));
    QCOMPARE(status.daemonState(), QStringLiteral("connected"));
    QCOMPARE(status.holdChord(), QStringLiteral("F9"));
    QCOMPARE(status.toggleChord(), QStringLiteral("Super+Ctrl+X"));
    QCOMPARE(status.targetApp(), QStringLiteral("ghostty"));
    QCOMPARE(status.socketMode(), QStringLiteral("peer"));
    QCOMPARE(status.gpu(), QStringLiteral("vulkan"));
    QCOMPARE(status.inputName(), QStringLiteral("Arctis Nova"));
    QCOMPARE(status.dictationState(), QStringLiteral("idle"));
    QVERIFY(!status.clock().isEmpty());

    QSignalSpy dictation(&status, &StatusModel::dictationChanged);
    link.notify(QStringLiteral("dictation.state"), {{QStringLiteral("state"), QStringLiteral("recording")}, {QStringLiteral("previous_state"), QStringLiteral("idle")}});
    QCOMPARE(status.dictationState(), QStringLiteral("recording"));
    link.notify(QStringLiteral("audio.level"), {{QStringLiteral("rms"), 0.2}, {QStringLiteral("peak"), 0.5}, {QStringLiteral("source"), QStringLiteral("microphone")}});
    QCOMPARE(status.levels().size(), StatusModel::kLevelSamples);
    QVERIFY(status.levels().last().toDouble() > 0.5);
    link.notify(QStringLiteral("dictation.state"), {{QStringLiteral("state"), QStringLiteral("failed")}, {QStringLiteral("previous_state"), QStringLiteral("recording")}});
    QCOMPARE(status.dictationState(), QStringLiteral("idle"));
    QCOMPARE(status.levels().last().toDouble(), 0.0);
    QCOMPARE(dictation.count(), 2);

    status.toggleDictation();
    QVERIFY(link.calls.contains(QStringLiteral("dictation.start")));
    // The request names no mode and no language: the daemon runs the
    // configured ones, the mode the strip shows.
    const QJsonObject start = link.lastParams.value(QStringLiteral("dictation.start"));
    QVERIFY2(!start.contains(QStringLiteral("mode")), "Start never overrides the configured mode");
    QVERIFY2(!start.contains(QStringLiteral("language")), "Start never overrides the configured language");

    // The daemon goes away: the state follows after the grace period,
    // never at once, so a restart under the grace period shows nothing.
    link.setConnected(false);
    QCOMPARE(status.daemonState(), QStringLiteral("connected"));
    QTRY_COMPARE_WITH_TIMEOUT(status.daemonState(), QStringLiteral("away"), StatusModel::kAwayGraceMs + 500);
    link.setConnected(true);
    QCOMPARE(status.daemonState(), QStringLiteral("connected"));
    QCOMPARE(link.calls.count(QStringLiteral("system.health")), 2);
}

void AppHostTest::enginesModelOrdersTheRailAndFollowsTheStream()
{
    FakeLink link;
    EnginesModel engines(&link);
    QCOMPARE(engines.rowCount(), 4);
    engines.applySelection(sample::selection());
    engines.applyEngines(sample::engines());
    const QList<EnginesModel::Engine> rows = engines.engines();
    QCOMPARE(rows[0].key, QStringLiteral("parakeet"));
    QCOMPARE(rows[0].name, QStringLiteral("parakeet v3-int8"));
    QCOMPARE(rows[0].state, QStringLiteral("warm"));
    QCOMPARE(rows[1].name, QStringLiteral("whisper small"));
    QCOMPARE(rows[1].state, QStringLiteral("idle"));
    QCOMPARE(rows[2].name, QStringLiteral("qwen3-4b-instruct"));
    QCOMPARE(rows[3].name, QStringLiteral("diarization"));
    QCOMPARE(engines.speechName(), QStringLiteral("parakeet v3-int8"));
    QCOMPARE(engines.speechState(), QStringLiteral("warm"));

    QSignalSpy changed(&engines, &QAbstractItemModel::dataChanged);
    link.notify(QStringLiteral("engine.state"), {{QStringLiteral("binary"), QStringLiteral("dettivo-engine-whisper")}, {QStringLiteral("state"), QStringLiteral("spawned")}, {QStringLiteral("model"), QJsonValue::Null}, {QStringLiteral("backend"), QJsonValue::Null}, {QStringLiteral("reason"), QJsonValue::Null}});
    QCOMPARE(engines.engines()[1].state, QStringLiteral("loading"));
    QVERIFY(engines.engines()[1].busy);
    link.notify(QStringLiteral("engine.state"), {{QStringLiteral("binary"), QStringLiteral("dettivo-engine-whisper")}, {QStringLiteral("state"), QStringLiteral("loaded")}, {QStringLiteral("model"), QStringLiteral("tiny.en")}, {QStringLiteral("backend"), QStringLiteral("cpu")}, {QStringLiteral("reason"), QJsonValue::Null}});
    QCOMPARE(engines.engines()[1].state, QStringLiteral("warm"));
    QCOMPARE(engines.engines()[1].name, QStringLiteral("whisper tiny.en"));
    QCOMPARE(engines.engines()[1].progress, 1.0);
    link.notify(QStringLiteral("model.download"), {{QStringLiteral("provider"), QStringLiteral("whisper")}, {QStringLiteral("model"), QStringLiteral("small")}, {QStringLiteral("state"), QStringLiteral("running")}, {QStringLiteral("bytes_done"), 50}, {QStringLiteral("bytes_total"), 200}, {QStringLiteral("error"), QJsonValue::Null}});
    QCOMPARE(engines.engines()[1].state, QStringLiteral("downloading"));
    QCOMPARE(engines.engines()[1].progress, 0.25);
    QVERIFY(changed.count() >= 3);
    const QModelIndex first = engines.index(0);
    QCOMPARE(engines.data(first, EnginesModel::StateRole).toString(), QStringLiteral("warm"));
    QCOMPARE(engines.roleNames().value(EnginesModel::ProgressRole), QByteArray("progress"));
}

void AppHostTest::historyModelPagesAndGroupsByDay()
{
    FakeLink link;
    QJsonArray page1;
    for (int i = 0; i < 3; ++i)
        page1.append(QJsonObject{{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), QStringLiteral("id-%1").arg(i)}, {QStringLiteral("kind"), QStringLiteral("dictation")}}},
                                 {QStringLiteral("title"), QStringLiteral("Item %1").arg(i)},
                                 {QStringLiteral("started_at"), i < 2 ? QStringLiteral("2026-02-13T1%1:00:00Z").arg(i) : QStringLiteral("2026-02-12T10:00:00Z")},
                                 {QStringLiteral("duration_seconds"), 4 + i},
                                 {QStringLiteral("status"), QStringLiteral("completed")}});
    link.answers.insert(QStringLiteral("transcripts.list"), {{QStringLiteral("items"), page1}, {QStringLiteral("next_cursor"), QStringLiteral("c2")}});
    HistoryModel history(&link);
    QVERIFY(!history.loaded());
    history.refresh();
    QVERIFY(history.loaded());
    QCOMPARE(history.rowCount(), 3);
    QCOMPARE(link.lastParams.value(QStringLiteral("transcripts.list")).value(QStringLiteral("limit")).toInt(), HistoryModel::kPageSize);
    QCOMPARE(history.data(history.index(0), HistoryModel::NewestDayRole).toBool(), true);
    QCOMPARE(history.data(history.index(2), HistoryModel::NewestDayRole).toBool(), false);
    QCOMPARE(history.data(history.index(0), HistoryModel::DurationRole).toString(), QStringLiteral("4 s"));
    QVERIFY(history.canFetchMore(QModelIndex()));

    NewestDayModel today(&history);
    QCOMPARE(today.rowCount(), 2);

    link.answers.insert(QStringLiteral("transcripts.list"), {{QStringLiteral("items"), QJsonArray{page1.at(2)}}, {QStringLiteral("next_cursor"), QJsonValue::Null}});
    history.fetchMore(QModelIndex());
    QCOMPARE(link.lastParams.value(QStringLiteral("transcripts.list")).value(QStringLiteral("cursor")).toString(), QStringLiteral("c2"));
    QCOMPARE(history.rowCount(), 4);
    QVERIFY(!history.canFetchMore(QModelIndex()));

    // A completed dictation reads the first page again.
    const auto before = link.calls.count(QStringLiteral("transcripts.list"));
    link.notify(QStringLiteral("dictation.state"), {{QStringLiteral("state"), QStringLiteral("idle")}, {QStringLiteral("previous_state"), QStringLiteral("inserting")}});
    QCOMPARE(link.calls.count(QStringLiteral("transcripts.list")), before + 1);
}

void AppHostTest::configBindingReadsAndWrites()
{
    FakeLink link;
    link.answers.insert(QStringLiteral("config.get"),
                        {{QStringLiteral("entries"), QJsonArray{QJsonObject{{QStringLiteral("key"), QStringLiteral("dictation.mode")}, {QStringLiteral("value"), QStringLiteral("raw")}, {QStringLiteral("source"), QStringLiteral("default")}},
                                                             QJsonObject{{QStringLiteral("key"), QStringLiteral("insert.terminal_app_ids")}, {QStringLiteral("value"), QJsonArray{QStringLiteral("foot"), QStringLiteral("kitty")}}, {QStringLiteral("source"), QStringLiteral("file")}}}}});
    link.answers.insert(QStringLiteral("config.set"), {{QStringLiteral("ok"), true}});
    ConfigBinding config(&link);
    QSignalSpy changed(&config, &ConfigBinding::changed);
    config.refresh();
    QCOMPARE(changed.count(), 1);
    QCOMPARE(config.value(QStringLiteral("dictation.mode")).toString(), QStringLiteral("raw"));
    QCOMPARE(config.source(QStringLiteral("dictation.mode")), QStringLiteral("default"));
    QCOMPARE(config.text(QStringLiteral("insert.terminal_app_ids")), QStringLiteral("foot, kitty"));
    QVERIFY(!config.value(QStringLiteral("nope")).isValid());
    config.set(QStringLiteral("dictation.mode"), QStringLiteral("polish"));
    QCOMPARE(link.lastParams.value(QStringLiteral("config.set")).value(QStringLiteral("value")).toString(), QStringLiteral("polish"));
    QCOMPARE(changed.count(), 2);
}

// qt-hosts/F9: one launch owns the instance; a second cannot take the
// lock, cannot listen, and leaves the owner's socket where it is.
void AppHostTest::instanceLockAdmitsOneOwner()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    const QString run = dir.filePath("run");
    InstanceLock owner;
    QString error;
    QVERIFY2(owner.acquire(run, &error), qPrintable(error));
    QVERIFY(QFile::exists(InstanceLock::pathFor(run)));
    InstanceLock second;
    QVERIFY(!second.acquire(run, &error));
    QVERIFY2(error.contains(QStringLiteral("another dettivo-app holds")), qPrintable(error));
    QVERIFY(!second.held());

    AppControl control;
    QVERIFY2(control.listen(run, owner, &error), qPrintable(error));
    AppControl loser;
    QVERIFY(!loser.listen(run, second, &error));
    QVERIFY2(error.contains(QStringLiteral("does not hold the instance lock")), qPrintable(error));
    QVERIFY(QFile::exists(control.socketPath()));
    QVERIFY(loser.socketPath().isEmpty());
    // The lock is released with its holder; the next launch takes it.
    {
        InstanceLock released;
        QVERIFY(!released.acquire(run, &error));
    }
    QVERIFY(owner.held());
}

void AppHostTest::instanceSocketAnswersOneLine()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    const QString run = dir.filePath("run");
    QJsonObject reply;
    QVERIFY(!AppControl::forward(run, {{QStringLiteral("cmd"), QStringLiteral("raise")}}, &reply, 100));

    InstanceLock lock;
    QString error;
    QVERIFY2(lock.acquire(run, &error), qPrintable(error));
    AppControl control;
    QVERIFY2(control.listen(run, lock, &error), qPrintable(error));
    QCOMPARE(control.socketPath(), run + QStringLiteral("/app.sock"));
    control.setStatusProvider([]() { return QJsonObject{{QStringLiteral("route"), QStringLiteral("home")}}; });
    QSignalSpy raised(&control, &AppControl::raiseRequested);
    QSignalSpy opened(&control, &AppControl::openRequested);

    // The forward is synchronous, as a second process's would be; the
    // server answers from this thread's loop, so the client runs on
    // another thread and the loop is pumped until it is done.
    QList<QJsonObject> answers;
    bool forwarded = true;
    std::thread client([&]() {
        for (const QJsonObject &command :
             {QJsonObject{{QStringLiteral("cmd"), QStringLiteral("open")}, {QStringLiteral("route"), QStringLiteral("agents")}},
              QJsonObject{{QStringLiteral("cmd"), QStringLiteral("open")}, {QStringLiteral("route"), QStringLiteral("garage")}},
              QJsonObject{{QStringLiteral("cmd"), QStringLiteral("status")}},
              QJsonObject{{QStringLiteral("cmd"), QStringLiteral("raise")}}}) {
            QJsonObject answer;
            forwarded = forwarded && AppControl::forward(run, command, &answer, 5000);
            answers.append(answer);
        }
    });
    QTRY_COMPARE_WITH_TIMEOUT(raised.count(), 2, 10000);
    client.join();
    QVERIFY(forwarded);
    QCOMPARE(answers.size(), 4);
    QCOMPARE(answers[0].value(QStringLiteral("ok")).toBool(), true);
    QCOMPARE(answers[0].value(QStringLiteral("route")).toString(), QStringLiteral("settings.agents"));
    QCOMPARE(answers[1].value(QStringLiteral("ok")).toBool(), false);
    QVERIFY(answers[1].value(QStringLiteral("error")).toString().contains(QStringLiteral("settings.hotkeys")));
    QCOMPARE(answers[2].value(QStringLiteral("route")).toString(), QStringLiteral("home"));
    QCOMPARE(answers[3].value(QStringLiteral("ok")).toBool(), true);
    QCOMPARE(opened.count(), 1);
    QCOMPARE(opened.first().at(0).toString(), QStringLiteral("settings.agents"));

    const QJsonObject unknown = control.applyCommand({{QStringLiteral("cmd"), QStringLiteral("dance")}});
    QCOMPARE(unknown.value(QStringLiteral("ok")).toBool(), false);
}

void AppHostTest::portalColourSchemeSignalFlipsThePalette()
{
    QTemporaryDir empty;
    QVERIFY(empty.isValid());
    ThemeBackend theme;
    theme.setThemeDirForTesting(empty.path());
    QCOMPARE(theme.source(), QStringLiteral("builtin-dark"));
    const QColor darkBackground = theme.colorBackground();

    // The portal's SettingChanged for org.freedesktop.appearance
    // color-scheme, delivered to the slot the bus would call.
    theme.setPortalColorSchemeForTesting(QStringLiteral("light"));
    QSignalSpy changed(&theme, &ThemeBackend::themeChanged);
    QVERIFY(QMetaObject::invokeMethod(&theme, "onPortalSettingChanged", Qt::DirectConnection,
                                      Q_ARG(QString, QStringLiteral("org.freedesktop.appearance")),
                                      Q_ARG(QString, QStringLiteral("color-scheme")),
                                      Q_ARG(QDBusVariant, QDBusVariant(QVariant(2u)))));
    QVERIFY(changed.count() >= 1);
    QCOMPARE(theme.source(), QStringLiteral("builtin-light"));
    QVERIFY(theme.colorBackground().lightness() > darkBackground.lightness());
}

QTEST_GUILESS_MAIN(AppHostTest)
#include "app_host_test.moc"
