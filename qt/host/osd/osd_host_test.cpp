// The OSD host without a daemon or a compositor: the event stream maps to
// the pill's states (the completion transition carries the outcome and
// the first words), a daemon that goes away hides the pill, an overflow
// re-reads dictation.status, the control protocol answers one line, the
// [osd] settings parse with defaults and one warning per bad key, and
// the caret-avoidance rule flips only when the opposite edge is free. A
// meeting shows as Listening with its elapsed time (fn-28 R5).
#include "daemon_link.h"
#include "hyprland.h"
#include "osd_control.h"
#include "osd_model.h"
#include "osd_settings.h"

#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QLocalSocket>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

using namespace dettivo;

namespace {

class FakeLink : public DaemonLink {
public:
    bool connected() const override { return m_connected; }
    void call(const QString &method, const QJsonObject &, Reply reply) override
    {
        calls.append(method);
        if (method == QStringLiteral("dictation.status") && reply)
            reply(statusAnswer, {});
    }
    void subscribe(const QStringList &topics) override { subscriptions.append(topics); }
    void setConnected(bool on)
    {
        m_connected = on;
        emit connectedChanged(on);
    }
    void notify(const QString &topic, const QJsonObject &payload) { emit notification(topic, payload); }

    QStringList calls;
    QList<QStringList> subscriptions;
    QJsonObject statusAnswer{{QStringLiteral("is_active"), false}};

private:
    bool m_connected = true;
};

QJsonObject dictation(const char *state, const char *previous, const QJsonObject &extra = {})
{
    QJsonObject o{{QStringLiteral("job_id"), QStringLiteral("job_dict_1")},
                  {QStringLiteral("state"), QLatin1String(state)},
                  {QStringLiteral("previous_state"), QLatin1String(previous)},
                  {QStringLiteral("reason"), QJsonValue::Null}};
    for (auto it = extra.begin(); it != extra.end(); ++it)
        o.insert(it.key(), it.value());
    return o;
}

}  // namespace

class OsdHostTest : public QObject {
    Q_OBJECT

private slots:
    void streamMapsToStatesAndCompletionCarriesTheOutcome();
    void anOldTakesCompletionNeverOverwritesANewerTake();
    void failuresCancelsAndDaemonLossHideOrExplain();
    void meetingShowsListeningWithItsElapsedTime();
    void overflowResubscribesAndRereadsStatus();
    void controlCommandsAnswerOneObject();
    void settingsParseWithDefaultsAndWarnings();
    void caretAvoidanceFlipsOnlyWhenTheOtherEdgeIsFree();
    void noticeFileRoundTrips();
};

void OsdHostTest::streamMapsToStatesAndCompletionCarriesTheOutcome()
{
    FakeLink link;
    OsdModel model(&link, OsdSettings());
    model.start();
    QCOMPARE(link.subscriptions.size(), 1);
    QVERIFY(link.subscriptions.first().contains(QStringLiteral("dictation.state")));
    QSignalSpy states(&model, &OsdModel::stateChanged);

    link.notify(QStringLiteral("dictation.state"), dictation("recording", "idle"));
    QCOMPARE(model.state(), QStringLiteral("listening"));
    QCOMPARE(model.hint(), QStringLiteral("release to insert"));
    QVERIFY(model.visible());
    link.notify(QStringLiteral("audio.level"), QJsonObject{{QStringLiteral("rms"), 0.1}, {QStringLiteral("peak"), 0.5}, {QStringLiteral("source"), QStringLiteral("microphone")}});
    QVERIFY(model.level() > 0.6 && model.level() < 0.8);

    link.notify(QStringLiteral("audio.level"), {{QStringLiteral("rms"), 0.01}});
    QVERIFY(model.level() > 0.25 && model.level() < 0.5);
    link.notify(QStringLiteral("audio.level"), {{QStringLiteral("rms"), 0.0001}});
    QCOMPARE(model.level(), 0.0);
    link.notify(QStringLiteral("audio.level"), {{QStringLiteral("rms"), 0.0}});
    QCOMPARE(model.level(), 0.0);

    link.notify(QStringLiteral("engine.state"), QJsonObject{{QStringLiteral("binary"), QStringLiteral("dettivo-engine-whisper")}, {QStringLiteral("state"), QStringLiteral("loaded")}, {QStringLiteral("model"), QStringLiteral("tiny.en")}});
    link.notify(QStringLiteral("dictation.state"), dictation("transcribing", "recording"));
    QCOMPARE(model.state(), QStringLiteral("transcribing"));
    QCOMPARE(model.engine(), QStringLiteral("whisper tiny.en"));
    QTRY_VERIFY(model.elapsed() != QStringLiteral("0.0 s"));
    link.notify(QStringLiteral("dictation.state"), dictation("inserting", "transcribing"));
    QCOMPARE(model.state(), QStringLiteral("transcribing"));

    const QJsonObject inserted{{QStringLiteral("outcome"), QStringLiteral("inserted")},
                               {QStringLiteral("method"), QStringLiteral("paste")},
                               {QStringLiteral("backend"), QStringLiteral("virtual_keyboard")},
                               {QStringLiteral("target_app"), QJsonObject{{QStringLiteral("bundle_id"), QStringLiteral("com.mitchellh.ghostty")}, {QStringLiteral("name"), QStringLiteral("ghostty")}}},
                               {QStringLiteral("reason"), QJsonValue::Null}};
    link.notify(QStringLiteral("dictation.state"),
                dictation("idle", "inserting", {{QStringLiteral("insertion"), inserted}, {QStringLiteral("first_words"), QStringLiteral("Add a regression test")}}));
    // The engine was quick: Transcribing holds for its beat, then the outcome shows.
    QCOMPARE(model.state(), QStringLiteral("transcribing"));
    QTRY_COMPARE_WITH_TIMEOUT(model.state(), QStringLiteral("inserted"), OsdModel::kMinTranscribingMs + 500);
    QCOMPARE(model.target(), QStringLiteral("ghostty"));
    QCOMPARE(model.words(), QStringLiteral("Add a regression test"));

    QJsonObject copied = inserted;
    copied.insert(QStringLiteral("outcome"), QStringLiteral("copied_to_clipboard"));
    copied.insert(QStringLiteral("reason"), QStringLiteral("no text input focused"));
    link.notify(QStringLiteral("dictation.state"), dictation("idle", "inserting", {{QStringLiteral("insertion"), copied}, {QStringLiteral("first_words"), QStringLiteral("x")}}));
    QCOMPARE(model.state(), QStringLiteral("copied"));
    QCOMPARE(model.reason(), QStringLiteral("no text input focused"));
    QCOMPARE(model.action(), QStringLiteral("Ctrl+V"));
    // A completion after a full beat shows at once.
    link.notify(QStringLiteral("dictation.state"), dictation("transcribing", "recording"));
    QTest::qWait(OsdModel::kMinTranscribingMs + 50);
    link.notify(QStringLiteral("dictation.state"), dictation("idle", "inserting", {{QStringLiteral("insertion"), inserted}}));
    QCOMPARE(model.state(), QStringLiteral("inserted"));

    QJsonObject failed = inserted;
    failed.insert(QStringLiteral("outcome"), QStringLiteral("failed"));
    failed.insert(QStringLiteral("reason"), QStringLiteral("clipboard unavailable"));
    link.notify(QStringLiteral("dictation.state"), dictation("idle", "inserting", {{QStringLiteral("insertion"), failed}}));
    QCOMPARE(model.state(), QStringLiteral("error"));
    QCOMPARE(model.title(), QStringLiteral("Not inserted"));
    QCOMPARE(model.reason(), QStringLiteral("clipboard unavailable"));

    // A silent take completes without an insertion.
    link.notify(QStringLiteral("dictation.state"), dictation("idle", "inserting"));
    QCOMPARE(model.title(), QStringLiteral("Nothing heard"));

    model.pillHidden();
    QCOMPARE(model.state(), QStringLiteral("hidden"));
    QVERIFY(states.count() >= 6);
}

// qt-hosts/F10 (fn-43): take A completes inside its Transcribing beat
// and waits; take B starts and reaches Transcribing before the beat
// ends. A's completion is dropped: B's state and text stand, and B's own
// completion shows.
void OsdHostTest::anOldTakesCompletionNeverOverwritesANewerTake()
{
    FakeLink link;
    OsdModel model(&link, OsdSettings());
    model.start();
    const auto insertion = [](const char *words) {
        return QJsonObject{{QStringLiteral("insertion"),
                            QJsonObject{{QStringLiteral("outcome"), QStringLiteral("inserted")},
                                        {QStringLiteral("target_app"), QJsonObject{{QStringLiteral("name"), QStringLiteral("ghostty")}}},
                                        {QStringLiteral("reason"), QJsonValue::Null}}},
                           {QStringLiteral("first_words"), QLatin1String(words)}};
    };
    link.notify(QStringLiteral("dictation.state"), dictation("recording", "idle"));
    link.notify(QStringLiteral("dictation.state"), dictation("transcribing", "recording"));
    link.notify(QStringLiteral("dictation.state"), dictation("inserting", "transcribing"));
    link.notify(QStringLiteral("dictation.state"), dictation("idle", "inserting", insertion("first take")));
    QCOMPARE(model.state(), QStringLiteral("transcribing"));
    // Take B, inside A's beat.
    link.notify(QStringLiteral("dictation.state"), dictation("recording", "idle", {{QStringLiteral("job_id"), QStringLiteral("job_dict_2")}}));
    QCOMPARE(model.state(), QStringLiteral("listening"));
    link.notify(QStringLiteral("dictation.state"), dictation("transcribing", "recording", {{QStringLiteral("job_id"), QStringLiteral("job_dict_2")}}));
    QCOMPARE(model.state(), QStringLiteral("transcribing"));
    QTest::qWait(OsdModel::kMinTranscribingMs + 100);
    QCOMPARE(model.state(), QStringLiteral("transcribing"));
    QVERIFY2(model.words().isEmpty(), qPrintable(model.words()));
    link.notify(QStringLiteral("dictation.state"), dictation("inserting", "transcribing", {{QStringLiteral("job_id"), QStringLiteral("job_dict_2")}}));
    link.notify(QStringLiteral("dictation.state"), dictation("idle", "inserting", insertion("second take")));
    QTRY_COMPARE_WITH_TIMEOUT(model.state(), QStringLiteral("inserted"), OsdModel::kMinTranscribingMs + 500);
    QCOMPARE(model.words(), QStringLiteral("second take"));
    // A completion pending when the daemon goes away is dropped too.
    link.notify(QStringLiteral("dictation.state"), dictation("recording", "idle"));
    link.notify(QStringLiteral("dictation.state"), dictation("transcribing", "recording"));
    link.notify(QStringLiteral("dictation.state"), dictation("idle", "inserting", insertion("third take")));
    link.setConnected(false);
    QCOMPARE(model.state(), QStringLiteral("hidden"));
    QTest::qWait(OsdModel::kMinTranscribingMs + 100);
    QCOMPARE(model.state(), QStringLiteral("hidden"));
}

void OsdHostTest::failuresCancelsAndDaemonLossHideOrExplain()
{
    FakeLink link;
    OsdModel model(&link, OsdSettings());
    link.notify(QStringLiteral("dictation.state"), dictation("recording", "idle"));
    link.notify(QStringLiteral("dictation.state"), dictation("failed", "recording", {{QStringLiteral("reason"), QStringLiteral("device lost: Arctis Nova")}}));
    QCOMPARE(model.state(), QStringLiteral("error"));
    QCOMPARE(model.title(), QStringLiteral("No microphone"));
    QCOMPARE(model.reason(), QStringLiteral("Arctis Nova disconnected"));
    QCOMPARE(model.action(), QStringLiteral("choose input"));
    // The transient failed -> idle keeps the error on screen.
    link.notify(QStringLiteral("dictation.state"), dictation("idle", "failed", {{QStringLiteral("reason"), QStringLiteral("device lost: Arctis Nova")}}));
    QCOMPARE(model.state(), QStringLiteral("error"));

    link.notify(QStringLiteral("dictation.state"), dictation("recording", "idle"));
    link.notify(QStringLiteral("dictation.state"), dictation("cancelled", "recording"));
    QCOMPARE(model.state(), QStringLiteral("hidden"));

    link.notify(QStringLiteral("dictation.state"), dictation("recording", "idle"));
    link.setConnected(false);
    QCOMPARE(model.state(), QStringLiteral("hidden"));
    QVERIFY(!model.daemonConnected());
    // Reconnecting subscribes again and asks where the session stands.
    link.statusAnswer = QJsonObject{{QStringLiteral("is_active"), true},
                                    {QStringLiteral("job"), QJsonObject{{QStringLiteral("message"), QStringLiteral("transcribing with whisper/tiny.en")}}}};
    link.setConnected(true);
    QCOMPARE(link.subscriptions.size(), 1);
    QCOMPARE(model.state(), QStringLiteral("transcribing"));
    QCOMPARE(model.engine(), QStringLiteral("whisper tiny.en"));
}

void OsdHostTest::meetingShowsListeningWithItsElapsedTime()
{
    FakeLink link;
    OsdModel model(&link, OsdSettings());
    model.start();
    QVERIFY(link.subscriptions.first().contains(QStringLiteral("meeting.state")));
    QCOMPARE(OsdModel::meetingClock(61'000), QStringLiteral("1:01"));
    QCOMPARE(OsdModel::meetingClock(3'661'000), QStringLiteral("1:01:01"));
    const QJsonObject meeting{{QStringLiteral("kind"), QStringLiteral("meeting")},
                              {QStringLiteral("meeting_id"), QStringLiteral("m1")},
                              {QStringLiteral("live_segment_count"), 0},
                              {QStringLiteral("live_last_end_ms"), 0},
                              {QStringLiteral("is_finalizing"), false}};
    QJsonObject recording = meeting;
    recording.insert(QStringLiteral("state"), QStringLiteral("recording"));
    recording.insert(QStringLiteral("duration_ms"), 61'000);
    link.notify(QStringLiteral("meeting.state"), recording);
    QCOMPARE(model.state(), QStringLiteral("listening"));
    QCOMPARE(model.hint(), QStringLiteral("1:01"));
    // The clock runs on from the duration the event carried.
    QTRY_COMPARE_WITH_TIMEOUT(model.hint(), QStringLiteral("1:02"), 1500);
    link.notify(QStringLiteral("audio.level"), QJsonObject{{QStringLiteral("rms"), 0.1}, {QStringLiteral("peak"), 0.5}, {QStringLiteral("source"), QStringLiteral("system")}});
    QVERIFY(model.level() > 0.6 && model.level() < 0.8);
    QJsonObject stopping = meeting;
    stopping.insert(QStringLiteral("state"), QStringLiteral("stopping"));
    link.notify(QStringLiteral("meeting.state"), stopping);
    QCOMPARE(model.state(), QStringLiteral("hidden"));
    // The finalisation and the settled states are not the pill's.
    QJsonObject transcribing = meeting;
    transcribing.insert(QStringLiteral("state"), QStringLiteral("transcribing"));
    link.notify(QStringLiteral("meeting.state"), transcribing);
    QCOMPARE(model.state(), QStringLiteral("hidden"));
    // A failed meeting names the reason.
    link.notify(QStringLiteral("meeting.state"), recording);
    QJsonObject failed = meeting;
    failed.insert(QStringLiteral("state"), QStringLiteral("failed"));
    failed.insert(QStringLiteral("reason"), QStringLiteral("take unwritable: disk full"));
    link.notify(QStringLiteral("meeting.state"), failed);
    QCOMPARE(model.state(), QStringLiteral("error"));
    QCOMPARE(model.title(), QStringLiteral("Meeting failed"));
    QCOMPARE(model.reason(), QStringLiteral("take unwritable: disk full"));
}

void OsdHostTest::overflowResubscribesAndRereadsStatus()
{
    FakeLink link;
    OsdModel model(&link, OsdSettings());
    model.start();
    link.notify(QStringLiteral("dictation.state"), dictation("recording", "idle"));
    link.calls.clear();
    link.statusAnswer = QJsonObject{{QStringLiteral("is_active"), false}};
    emit link.overflow(12);
    QCOMPARE(link.subscriptions.size(), 2);
    QCOMPARE(link.calls, QStringList{QStringLiteral("dictation.status")});
    QCOMPARE(model.state(), QStringLiteral("hidden"));
}

void OsdHostTest::controlCommandsAnswerOneObject()
{
    OsdModel model(nullptr, OsdSettings());
    model.setHostFacts(QStringLiteral("window"), QStringLiteral("top"), QStringLiteral("DP-3"));
    QJsonObject status = model.applyCommand(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("status")}});
    QCOMPARE(status.value(QStringLiteral("ok")).toBool(), true);
    QCOMPARE(status.value(QStringLiteral("host")).toString(), QStringLiteral("window"));
    QCOMPARE(status.value(QStringLiteral("visible")).toBool(), false);
    QCOMPARE(status.value(QStringLiteral("daemon")).toString(), QStringLiteral("reconnecting"));

    QJsonObject shown = model.applyCommand(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("show")}, {QStringLiteral("state"), QStringLiteral("inserted")}, {QStringLiteral("target"), QStringLiteral("ghostty")}, {QStringLiteral("words"), QStringLiteral("Hello")}});
    QCOMPARE(shown.value(QStringLiteral("state")).toString(), QStringLiteral("inserted"));
    QCOMPARE(model.target(), QStringLiteral("ghostty"));
    QCOMPARE(model.words(), QStringLiteral("Hello"));
    // A bare show uses the state's sample text.
    model.applyCommand(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("show")}, {QStringLiteral("state"), QStringLiteral("error")}});
    QCOMPARE(model.title(), QStringLiteral("No microphone"));

    QJsonObject refused = model.applyCommand(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("show")}, {QStringLiteral("state"), QStringLiteral("sleeping")}});
    QCOMPARE(refused.value(QStringLiteral("ok")).toBool(), false);
    QVERIFY(refused.value(QStringLiteral("error")).toString().contains(QStringLiteral("unknown state: sleeping")));
    QCOMPARE(model.state(), QStringLiteral("error"));
    QVERIFY(!model.applyCommand(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("dance")}}).value(QStringLiteral("ok")).toBool());

    QJsonObject hidden = model.applyCommand(QJsonObject{{QStringLiteral("cmd"), QStringLiteral("hide")}});
    QCOMPARE(hidden.value(QStringLiteral("visible")).toBool(), false);

    // Over the socket: one line each way.
    QTemporaryDir dir;
    OsdControl control(&model);
    QString error;
    QVERIFY2(control.listen(dir.path(), &error), qPrintable(error));
    QLocalSocket client;
    client.connectToServer(control.socketPath());
    QVERIFY(client.waitForConnected(1000));
    client.write("{\"cmd\":\"show\",\"state\":\"listening\"}\n");
    // The server answers from this thread's event loop, so spin it.
    QTRY_VERIFY(client.canReadLine());
    const QJsonObject reply = QJsonDocument::fromJson(client.readLine()).object();
    QCOMPARE(reply.value(QStringLiteral("state")).toString(), QStringLiteral("listening"));
    QCOMPARE(model.state(), QStringLiteral("listening"));
}

void OsdHostTest::settingsParseWithDefaultsAndWarnings()
{
    QString warning;
    OsdSettings defaults = OsdSettings::fromToml(QStringLiteral("[daemon]\nlog_level = \"info\"\n"), &warning);
    QCOMPARE(defaults.position, QStringLiteral("top"));
    QCOMPARE(defaults.hideAfterMs, 1800);
    QVERIFY(warning.isEmpty());

    OsdSettings parsed = OsdSettings::fromToml(QStringLiteral("[osd]\nenabled = false\nhost = \"window\"\nposition = \"bottom_right\"\nmargin = 8\nmonitor = \"DP-3\"\nhide_after_ms = 900\nerror_hide_after_ms = 2000\nshow_level = false\nmotion = \"reduced\"\n"), &warning);
    QVERIFY(!parsed.enabled);
    QCOMPARE(parsed.host, QStringLiteral("window"));
    QCOMPARE(parsed.position, QStringLiteral("bottom_right"));
    QVERIFY(!parsed.anchoredTop());
    QCOMPARE(parsed.margin, 8);
    QCOMPARE(parsed.monitor, QStringLiteral("DP-3"));
    QCOMPARE(parsed.hideAfterMs, 900);
    QCOMPARE(parsed.errorHideAfterMs, 2000);
    QVERIFY(!parsed.showLevel);
    QCOMPARE(parsed.motion, QStringLiteral("reduced"));
    QVERIFY(warning.isEmpty());

    OsdSettings bad = OsdSettings::fromToml(QStringLiteral("[osd]\nposition = \"middle\"\nmargin = -4\nhost = 3\n"), &warning);
    QCOMPARE(bad.position, QStringLiteral("top"));
    QCOMPARE(bad.margin, 24);
    QCOMPARE(warning.split(QLatin1Char('\n')).size(), 3);
    QVERIFY(warning.contains(QStringLiteral("osd.position")));
    QVERIFY(warning.contains(QStringLiteral("osd.margin")));
    QVERIFY(warning.contains(QStringLiteral("osd.host")));

    QProcessEnvironment env;
    env.insert(QStringLiteral("HOME"), QStringLiteral("/home/u"));
    QCOMPARE(OsdSettings::configPath(env), QStringLiteral("/home/u/.config/dettivo/config.toml"));
    env.insert(QStringLiteral("XDG_RUNTIME_DIR"), QStringLiteral("/run/user/7"));
    QCOMPARE(OsdSettings::socketDir(env), QStringLiteral("/run/user/7/dettivo"));
    QCOMPARE(OsdSettings::daemonSocket(env), QStringLiteral("/run/user/7/dettivo/dettivo.sock"));
    env.insert(QStringLiteral("DETTIVO_IPC_SOCKET"), QStringLiteral("/tmp/dq1/run/dettivo.sock"));
    QCOMPARE(OsdSettings::socketDir(env), QStringLiteral("/tmp/dq1/run"));
}

void OsdHostTest::caretAvoidanceFlipsOnlyWhenTheOtherEdgeIsFree()
{
    const HyprRect monitor{0, 0, 4096, 1152};
    const int band = 36 + 24;
    // A floating window hugging the top edge: the pill moves to the bottom.
    QVERIFY(Hyprland::shouldFlip(true, monitor, band, HyprRect{100, 10, 800, 400}));
    // The same window with the pill at the bottom: nothing to avoid.
    QVERIFY(!Hyprland::shouldFlip(false, monitor, band, HyprRect{100, 10, 800, 400}));
    // A tiled window that spans both bands: stay, both edges are covered.
    QVERIFY(!Hyprland::shouldFlip(true, monitor, band, HyprRect{0, 33, 2037, 1112}));
    // A window in the middle of the screen touches neither band.
    QVERIFY(!Hyprland::shouldFlip(true, monitor, band, HyprRect{500, 300, 800, 400}));
    // A window on another monitor does not count.
    QVERIFY(!Hyprland::shouldFlip(true, monitor, band, HyprRect{5000, 10, 800, 400}));

    const QJsonObject active = QJsonDocument::fromJson(R"({"at":[2052,33],"size":[2037,1112],"monitor":1,"class":"chromium"})").object();
    const auto window = Hyprland::parseActiveWindow(active);
    QVERIFY(window.has_value());
    QCOMPARE(window->rect.x, 2052);
    QCOMPARE(window->appClass, QStringLiteral("chromium"));
    const QJsonArray monitors = QJsonDocument::fromJson(R"([{"id":1,"name":"DP-3","x":0,"y":0,"width":5120,"height":1440,"scale":1.25,"focused":true}])").array();
    const auto list = Hyprland::parseMonitors(monitors);
    QCOMPARE(list.size(), 1);
    QCOMPARE(list.first().rect.width, 4096);
    QCOMPARE(list.first().rect.height, 1152);
    QVERIFY(list.first().focused);

    QProcessEnvironment none;
    QVERIFY(!Hyprland(none).available());
    QVERIFY(!Hyprland(none).activeWindow().has_value());
}

void OsdHostTest::noticeFileRoundTrips()
{
    QTemporaryDir dir;
    QVERIFY(OsdControl::writeNotice(dir.path(), QStringLiteral("disabled"), QStringLiteral("[osd] enabled = false")));
    QFile file(dir.path() + QStringLiteral("/osd.status.json"));
    QVERIFY(file.open(QIODevice::ReadOnly));
    const QJsonObject notice = QJsonDocument::fromJson(file.readAll()).object();
    QCOMPARE(notice.value(QStringLiteral("host")).toString(), QStringLiteral("disabled"));
    QCOMPARE(notice.value(QStringLiteral("notice")).toString(), QStringLiteral("[osd] enabled = false"));
    OsdControl::clearNotice(dir.path());
    QVERIFY(!QFile::exists(file.fileName()));
}

QTEST_GUILESS_MAIN(OsdHostTest)
#include "osd_host_test.moc"
