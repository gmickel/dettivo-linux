#include "osd_model.h"

#include <QCoreApplication>
#include <QJsonArray>
#include <QLoggingCategory>

#include <algorithm>
#include <cmath>

namespace dettivo {

Q_LOGGING_CATEGORY(lcOsdModel, "dettivo.osd.model")

namespace {

OsdModel *gInstance = nullptr;

const QStringList kStates = {
    QStringLiteral("listening"), QStringLiteral("transcribing"), QStringLiteral("enhancing"),
    QStringLiteral("inserted"),  QStringLiteral("copied"),       QStringLiteral("error"),
    QStringLiteral("hidden"),
};

QString str(const QJsonObject &o, const char *key)
{
    return o.value(QLatin1String(key)).toString();
}

// "dettivo-engine-whisper" plus "tiny.en" reads as "whisper tiny.en".
QString engineLabel(const QString &binary, const QString &model)
{
    QString provider = binary;
    provider.remove(QStringLiteral("dettivo-engine-"));
    if (model.isEmpty())
        return provider;
    return provider + QLatin1Char(' ') + model;
}

}  // namespace

OsdModel::OsdModel(DaemonLink *link, const OsdSettings &settings, QObject *parent)
    : QObject(parent), m_link(link), m_settings(settings)
{
    m_elapsedTimer.setInterval(100);
    connect(&m_elapsedTimer, &QTimer::timeout, this, &OsdModel::tickElapsed);
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &OsdModel::handleNotification);
        connect(m_link, &DaemonLink::overflow, this, &OsdModel::handleOverflow);
        connect(m_link, &DaemonLink::connectedChanged, this, [this](bool connected) {
            emit daemonConnectedChanged();
            if (connected) {
                m_link->subscribe(topics());
                refreshFromStatus();
            } else if (m_state == QStringLiteral("listening") || m_state == QStringLiteral("transcribing")) {
                // A daemon that went away mid-session cannot finish it; the
                // pill hides rather than showing a stale state.
                hide();
            }
        });
    }
}

void OsdModel::setInstance(OsdModel *model)
{
    gInstance = model;
}

OsdModel *OsdModel::create(QQmlEngine *, QJSEngine *)
{
    Q_ASSERT_X(gInstance != nullptr, "OsdModel", "setInstance before the engine loads");
    QQmlEngine::setObjectOwnership(gInstance, QQmlEngine::CppOwnership);
    return gInstance;
}

QStringList OsdModel::states()
{
    return kStates;
}

QStringList OsdModel::topics()
{
    return {QStringLiteral("dictation.state"), QStringLiteral("audio.level"),
            QStringLiteral("job.progress"), QStringLiteral("engine.state"),
            QStringLiteral("meeting.state")};
}

void OsdModel::start()
{
    if (m_link == nullptr)
        return;
    m_link->subscribe(topics());
    if (m_link->connected())
        refreshFromStatus();
}

void OsdModel::show(const QString &state, const Texts &t)
{
    const bool stateChange = state != m_state;
    const bool textChange = t.title != m_title || t.hint != m_hint || t.engine != m_engine
        || t.elapsed != m_elapsed || t.words != m_words || t.target != m_target
        || t.reason != m_reason || t.action != m_action;
    m_title = t.title;
    m_hint = t.hint;
    m_engine = t.engine;
    m_elapsed = t.elapsed;
    m_words = t.words;
    m_target = t.target;
    m_reason = t.reason;
    m_action = t.action;
    // Transcribing counts the engine's time; Listening counts the
    // meeting's while one records.
    const bool timed = state == QStringLiteral("transcribing")
        || (state == QStringLiteral("listening") && m_meeting);
    if (timed) {
        if (!m_elapsedTimer.isActive()) {
            m_transcribingSince.start();
            m_elapsedTimer.start();
        }
    } else {
        m_elapsedTimer.stop();
    }
    if (textChange)
        emit textChanged();
    if (stateChange) {
        m_state = state;
        qCDebug(lcOsdModel) << "pill state" << state;
        emit stateChanged();
    }
}

void OsdModel::hide()
{
    ++m_take;
    m_elapsedTimer.stop();
    m_meeting = false;
    if (m_state == QStringLiteral("hidden"))
        return;
    m_state = QStringLiteral("hidden");
    emit stateChanged();
}

void OsdModel::pillHidden()
{
    hide();
}

QString OsdModel::meetingClock(qint64 ms)
{
    const qint64 seconds = ms / 1000;
    const qint64 hours = seconds / 3600;
    const qint64 minutes = (seconds / 60) % 60;
    const qint64 rest = seconds % 60;
    if (hours > 0)
        return QStringLiteral("%1:%2:%3").arg(hours).arg(minutes, 2, 10, QLatin1Char('0')).arg(rest, 2, 10, QLatin1Char('0'));
    return QStringLiteral("%1:%2").arg(minutes).arg(rest, 2, 10, QLatin1Char('0'));
}

void OsdModel::tickElapsed()
{
    if (m_meeting && m_state == QStringLiteral("listening")) {
        const QString clock = meetingClock(m_meetingBaseMs + m_meetingSince.elapsed());
        show(QStringLiteral("listening"), Texts{m_title, clock, {}, {}, {}, {}, {}, {}});
        return;
    }
    const double seconds = double(m_transcribingSince.elapsed()) / 1000.0;
    Texts t{m_title, m_hint, m_engineLabel, QStringLiteral("%1 s").arg(seconds, 0, 'f', 1), m_words, m_target, m_reason, m_action};
    show(QStringLiteral("transcribing"), t);
}

void OsdModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic == QStringLiteral("dictation.state")) {
        applyDictationState(payload);
    } else if (topic == QStringLiteral("audio.level")) {
        if (m_state != QStringLiteral("listening"))
            return;
        // Match the Omarchy pill's visual scale: -60 dBFS to 0 dBFS.
        // This changes the display only, never the capture gain.
        const qreal rms = std::clamp(payload.value(QStringLiteral("rms")).toDouble(), 0.0, 1.0);
        const qreal level = rms > 0.0 ? std::clamp((20.0 * std::log10(rms) + 60.0) / 60.0, 0.0, 1.0) : 0.0;
        if (!qFuzzyCompare(1.0 + level, 1.0 + m_level)) {
            m_level = level;
            emit levelChanged();
        }
    } else if (topic == QStringLiteral("engine.state")) {
        applyEngineState(payload);
    } else if (topic == QStringLiteral("meeting.state")) {
        applyMeetingState(payload);
    }
    // job.progress carries nothing the pill shows yet; the elapsed time
    // comes from the transcribing clock.
}

void OsdModel::applyEngineState(const QJsonObject &payload)
{
    const QString state = str(payload, "state");
    if (state == QStringLiteral("loaded") || state == QStringLiteral("spawned"))
        m_engineLabel = engineLabel(str(payload, "binary"), str(payload, "model"));
}

// A meeting shows as Listening with its elapsed time for as long as it
// records; the stop, the finalisation and the settled states take the
// pill away, and a failed meeting names the reason.
void OsdModel::applyMeetingState(const QJsonObject &payload)
{
    const QString state = str(payload, "state");
    if (state == QStringLiteral("recording")) {
        m_meeting = true;
        m_meetingBaseMs = qint64(payload.value(QStringLiteral("duration_ms")).toDouble(0.0));
        m_meetingSince.start();
        m_level = 0.0;
        emit levelChanged();
        show(QStringLiteral("listening"), Texts{{}, meetingClock(m_meetingBaseMs), {}, {}, {}, {}, {}, {}});
        return;
    }
    if (!m_meeting)
        return;
    if (state == QStringLiteral("failed")) {
        const QString reason = str(payload, "reason");
        m_meeting = false;
        show(QStringLiteral("error"),
             Texts{tr("Meeting failed"), {}, {}, {}, {}, {}, reason.isEmpty() ? tr("the capture ended") : reason, tr("run dettivo doctor")});
        return;
    }
    hide();
}

void OsdModel::applyDictationState(const QJsonObject &payload)
{
    const QString state = str(payload, "state");
    const QString previous = str(payload, "previous_state");
    if (state == QStringLiteral("recording")) {
        // A new take: a completion of the previous one still waiting out
        // its beat belongs to that take and is dropped when it fires.
        ++m_take;
        m_meeting = false;
        m_level = 0.0;
        emit levelChanged();
        show(QStringLiteral("listening"), Texts{{}, tr("release to insert"), {}, {}, {}, {}, {}, {}});
    } else if (state == QStringLiteral("transcribing")) {
        show(QStringLiteral("transcribing"), Texts{{}, {}, m_engineLabel, QStringLiteral("0.0 s"), {}, {}, {}, {}});
    } else if (state == QStringLiteral("inserting")) {
        // Still the engine's moment for the person watching.
    } else if (state == QStringLiteral("idle")) {
        if (previous == QStringLiteral("inserting"))
            completeAfterDwell(payload);
        else if (previous != QStringLiteral("failed") && previous != QStringLiteral("cancelled"))
            hide();
    } else if (state == QStringLiteral("failed")) {
        applyFailure(str(payload, "reason"));
    } else if (state == QStringLiteral("cancelled")) {
        hide();
    }
}

// A completion that lands inside the Transcribing beat waits for the beat
// to end; a newer take, a failure, a cancel or a hide in the meantime
// wins and the completion is dropped: the state name alone is not enough,
// since the next take can be transcribing by the time the beat ends.
void OsdModel::completeAfterDwell(const QJsonObject &payload)
{
    if (m_state != QStringLiteral("transcribing")) {
        applyCompletion(payload);
        return;
    }
    const qint64 shown = m_transcribingSince.isValid() ? m_transcribingSince.elapsed() : kMinTranscribingMs;
    if (shown >= kMinTranscribingMs) {
        applyCompletion(payload);
        return;
    }
    const QString job = str(payload, "job_id");
    const quint64 take = m_take;
    QTimer::singleShot(int(kMinTranscribingMs - shown), this, [this, payload, job, take]() {
        if (m_take == take && m_state == QStringLiteral("transcribing"))
            applyCompletion(payload);
        else
            qCDebug(lcOsdModel) << "completion of" << job << "superseded during the transcribing beat";
    });
}

void OsdModel::applyCompletion(const QJsonObject &payload)
{
    const QString words = str(payload, "first_words");
    const QJsonValue insertionValue = payload.value(QStringLiteral("insertion"));
    if (!insertionValue.isObject()) {
        show(QStringLiteral("error"),
             Texts{tr("Nothing heard"), {}, {}, {}, {}, {}, tr("the take was silent"), tr("try again")});
        return;
    }
    const QJsonObject insertion = insertionValue.toObject();
    const QString outcome = str(insertion, "outcome");
    const QJsonObject app = insertion.value(QStringLiteral("target_app")).toObject();
    QString target = str(app, "name");
    if (target.isEmpty())
        target = str(app, "bundle_id");
    const QString reason = str(insertion, "reason");
    if (outcome == QStringLiteral("inserted")) {
        show(QStringLiteral("inserted"), Texts{{}, {}, {}, {}, words, target == QStringLiteral("mock") ? QString() : target, {}, {}});
    } else if (outcome == QStringLiteral("copied_to_clipboard")) {
        show(QStringLiteral("copied"),
             Texts{{}, {}, {}, {}, words, {}, reason.isEmpty() ? tr("no text input focused") : reason, tr("Ctrl+V")});
    } else {
        show(QStringLiteral("error"),
             Texts{tr("Not inserted"), {}, {}, {}, words, {}, reason.isEmpty() ? tr("the insertion failed") : reason, tr("copy from history")});
    }
}

void OsdModel::applyFailure(const QString &reason)
{
    ++m_take;
    Texts t;
    if (reason.startsWith(QStringLiteral("device lost"))) {
        t.title = tr("No microphone");
        const auto colon = reason.indexOf(QLatin1Char(':'));
        t.reason = colon >= 0 ? reason.mid(colon + 1).trimmed() + tr(" disconnected") : reason;
        t.action = tr("choose input");
    } else if (reason.startsWith(QStringLiteral("engine"))) {
        t.title = tr("Engine failed");
        t.reason = reason;
        t.action = tr("run dettivo doctor");
    } else if (reason.startsWith(QStringLiteral("capture"))) {
        t.title = tr("No microphone");
        t.reason = reason;
        t.action = tr("choose input");
    } else {
        t.title = tr("Dictation failed");
        t.reason = reason;
        t.action = tr("try again");
    }
    show(QStringLiteral("error"), t);
}

void OsdModel::handleOverflow(qint64 dropped)
{
    qCInfo(lcOsdModel) << "event stream overflowed," << dropped << "dropped; re-reading dictation.status";
    if (m_link != nullptr)
        m_link->subscribe(topics());
    refreshFromStatus();
}

void OsdModel::refreshFromStatus()
{
    if (m_link == nullptr)
        return;
    m_link->call(QStringLiteral("dictation.status"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        const bool active = result.value(QStringLiteral("is_active")).toBool(false);
        if (!active) {
            if (m_state == QStringLiteral("listening") || m_state == QStringLiteral("transcribing"))
                hide();
            return;
        }
        const QString message = result.value(QStringLiteral("job")).toObject().value(QStringLiteral("message")).toString();
        const QString phase = message.section(QLatin1Char(' '), 0, 0);
        const QString engine = message.section(QStringLiteral(" with "), 1, 1).replace(QLatin1Char('/'), QLatin1Char(' '));
        if (!engine.isEmpty())
            m_engineLabel = engine;
        if (phase == QStringLiteral("listening"))
            show(QStringLiteral("listening"), Texts{{}, tr("release to insert"), {}, {}, {}, {}, {}, {}});
        else if (phase == QStringLiteral("transcribing") || phase == QStringLiteral("inserting"))
            show(QStringLiteral("transcribing"), Texts{{}, {}, m_engineLabel, m_elapsed, {}, {}, {}, {}});
    });
}

void OsdModel::showSample(const QString &state)
{
    if (state == QStringLiteral("listening")) {
        m_level = 0.7;
        emit levelChanged();
        show(state, Texts{{}, tr("release to insert"), {}, {}, {}, {}, {}, {}});
    } else if (state == QStringLiteral("transcribing")) {
        show(state, Texts{{}, {}, QStringLiteral("parakeet v3"), QStringLiteral("0.4 s"), {}, {}, {}, {}});
        m_elapsedTimer.stop();
    } else if (state == QStringLiteral("enhancing")) {
        show(state, Texts{{}, {}, {}, {}, tr("add a regression test for the merger…"), {}, {}, {}});
    } else if (state == QStringLiteral("inserted")) {
        show(state, Texts{{}, {}, {}, {}, tr("Add a regression test for the merger…"), QStringLiteral("ghostty"), {}, {}});
    } else if (state == QStringLiteral("copied")) {
        show(state, Texts{{}, {}, {}, {}, {}, {}, tr("no text input focused"), QStringLiteral("Ctrl+V")});
    } else if (state == QStringLiteral("error")) {
        show(state, Texts{tr("No microphone"), {}, {}, {}, {}, {}, tr("Arctis Nova disconnected"), tr("choose input")});
    } else {
        hide();
    }
}

void OsdModel::setHostFacts(const QString &kind, const QString &position, const QString &monitor)
{
    m_hostKind = kind;
    m_hostPosition = position;
    m_hostMonitor = monitor;
}

void OsdModel::setThemeProvider(std::function<QJsonObject()> provider)
{
    m_themeProvider = std::move(provider);
}

QJsonObject OsdModel::status() const
{
    return QJsonObject{{QStringLiteral("ok"), true},
                       {QStringLiteral("host"), m_hostKind},
                       {QStringLiteral("position"), m_hostPosition},
                       {QStringLiteral("monitor"), m_hostMonitor},
                       {QStringLiteral("visible"), visible()},
                       {QStringLiteral("state"), m_state},
                       {QStringLiteral("daemon"), daemonConnected() ? QStringLiteral("connected") : QStringLiteral("reconnecting")},
                       {QStringLiteral("pid"), qint64(QCoreApplication::applicationPid())},
                       {QStringLiteral("theme"), m_themeProvider ? m_themeProvider() : QJsonObject()}};
}

QJsonObject OsdModel::applyCommand(const QJsonObject &command)
{
    const QString cmd = str(command, "cmd");
    auto refuse = [](const QString &why) {
        return QJsonObject{{QStringLiteral("ok"), false}, {QStringLiteral("error"), why}};
    };
    if (cmd == QStringLiteral("status"))
        return status();
    if (cmd == QStringLiteral("hide")) {
        hide();
        return status();
    }
    if (cmd != QStringLiteral("show"))
        return refuse(QStringLiteral("unknown command: %1 (show, hide or status)").arg(cmd));
    const QString state = str(command, "state");
    if (!kStates.contains(state))
        return refuse(QStringLiteral("unknown state: %1 (expected %2)").arg(state, kStates.join(QStringLiteral(", "))));
    if (state == QStringLiteral("hidden")) {
        hide();
        return status();
    }
    if (command.contains(QStringLiteral("level"))) {
        m_level = std::clamp(command.value(QStringLiteral("level")).toDouble(), 0.0, 1.0);
        emit levelChanged();
    }
    // Absent fields keep the state's own defaults; present ones replace them.
    const bool sample = !command.contains(QStringLiteral("title")) && !command.contains(QStringLiteral("hint"))
        && !command.contains(QStringLiteral("engine")) && !command.contains(QStringLiteral("words"))
        && !command.contains(QStringLiteral("target")) && !command.contains(QStringLiteral("reason"))
        && !command.contains(QStringLiteral("action"));
    if (sample) {
        showSample(state);
        return status();
    }
    Texts t{str(command, "title"),  str(command, "hint"),   str(command, "engine"), str(command, "elapsed"),
            str(command, "words"),  str(command, "target"), str(command, "reason"), str(command, "action")};
    show(state, t);
    if (state == QStringLiteral("transcribing") && command.contains(QStringLiteral("elapsed")))
        m_elapsedTimer.stop();
    return status();
}

}  // namespace dettivo
