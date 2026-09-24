#include "meeting_live_model.h"
#include "live_segments_model.h"
#include "meeting_format.h"
#include "status_format.h"

#include <QClipboard>
#include <QDateTime>
#include <QGuiApplication>
#include <QJsonArray>

namespace dettivo {

namespace {

constexpr int kClockMs = 250;

QString message(const QJsonObject &error)
{
    const QString text = error.value(QStringLiteral("message")).toString();
    return text.isEmpty() ? QObject::tr("the daemon did not answer") : text;
}

}  // namespace

MeetingLiveModel::MeetingLiveModel(DaemonLink *link, QObject *parent)
    : QObject(parent), m_link(link), m_segments(new LiveSegmentsModel(this)),
      m_saver(new NotesSaver(link, QStringLiteral("live"), this))
{
    m_clock.setInterval(kClockMs);
    connect(&m_clock, &QTimer::timeout, this, &MeetingLiveModel::tick);
    connect(m_saver, &NotesSaver::stateChanged, this, [this]() {
        m_notesState.clear();
        emit notesChanged();
    });
    connect(m_saver, &NotesSaver::failed, this, [this](const QString &reason) { emit failed(QStringLiteral("notes"), reason); });
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &MeetingLiveModel::handleNotification);
        connect(m_link, &DaemonLink::subscriptionReady, this, &MeetingLiveModel::reconcile);
        connect(m_link, &DaemonLink::overflow, this, [this](qint64) { reconcile(); });
        connect(m_link, &DaemonLink::connectedChanged, this, [this](bool connected) {
            ++m_reconcileGeneration;
            m_attachingId.clear();
            if (!connected)
                return;
            loadDisclosure();
            readDevices();
        });
    }
}

QObject *MeetingLiveModel::segmentsObject() const
{
    return m_segments;
}

bool MeetingLiveModel::finishing() const
{
    return m_stopRequested || m_state == QStringLiteral("stopping") || m_state == QStringLiteral("stopped")
        || m_state == QStringLiteral("transcribing");
}

QString MeetingLiveModel::elapsedText() const
{
    return meeting_format::elapsed(m_elapsedMs);
}

// The gate the daemon named (`meetingDisclosureRequired`,
// `sessionActive`, `engineWithoutTimestamps`) leads the reason, so the
// screen says what to clear rather than only that it was refused.
QString MeetingLiveModel::gateReason(const QJsonObject &error)
{
    const QJsonObject data = error.value(QStringLiteral("data")).toObject();
    const QJsonObject details = data.value(QStringLiteral("details")).toObject();
    const QString kind = details.value(QStringLiteral("kind")).toString();
    QString text = message(error);
    if (kind == QStringLiteral("sessionActive"))
        text = tr("A dictation or a meeting is already running; stop it first.");
    else if (kind == QStringLiteral("engineWithoutTimestamps"))
        text = tr("%1 cannot transcribe a meeting; pick a meeting-capable engine under Settings / Models.")
                   .arg(details.value(QStringLiteral("provider")).toString(tr("The selected engine")));
    else if (kind == QStringLiteral("meetingDisclosureRequired"))
        text = tr("The recording disclosure has to be acknowledged first.");
    return kind.isEmpty() ? text : QStringLiteral("%1 (%2)").arg(text, kind);
}

void MeetingLiveModel::start(const QString &title, bool systemAudio, const QString &provider, const QString &model,
                             int expectedSpeakers, const QVariant &analyze, const QVariant &diarize)
{
    m_error.clear();
    QJsonObject params{{QStringLiteral("capture"), QJsonObject{{QStringLiteral("microphone"), true}, {QStringLiteral("system_audio"), systemAudio}}}};
    // An explicit choice is a meeting-specific override; anything else
    // is left out so the configuration's defaults apply.
    if (analyze.typeId() == QMetaType::Bool)
        params.insert(QStringLiteral("analyze"), analyze.toBool());
    if (diarize.typeId() == QMetaType::Bool)
        params.insert(QStringLiteral("diarize"), diarize.toBool());
    if (!title.trimmed().isEmpty())
        params.insert(QStringLiteral("title"), title.trimmed());
    if (expectedSpeakers > 0)
        params.insert(QStringLiteral("expected_speakers"), expectedSpeakers);
    m_pending = params;
    m_systemAudio = systemAudio;
    m_title = title.trimmed();
    // The picker may be stale; the recorded meeting supplies the label.
    Q_UNUSED(provider)
    Q_UNUSED(model)
    m_engineLabel.clear();
    if (m_link == nullptr || !m_link->connected()) {
        m_error = tr("Daemon unavailable.");
        emit failed(QStringLiteral("start"), m_error);
        emit changed();
        return;
    }
    m_starting = true;
    emit changed();
    m_link->call(QStringLiteral("meetings.disclosure.get"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (m_pending.isEmpty())
            return;
        if (error.isEmpty())
            applyDisclosure(result);
        if (m_disclosureAcknowledged) {
            sendStart();
            return;
        }
        m_starting = false;
        emit changed();
        emit disclosureRequired(m_disclosureMessage);
    });
}

void MeetingLiveModel::acknowledgeAndStart()
{
    if (m_pending.isEmpty())
        return;
    m_pending.insert(QStringLiteral("acknowledge_meeting_disclosure"), true);
    m_starting = true;
    emit changed();
    sendStart();
}

void MeetingLiveModel::dismissStart()
{
    m_pending = QJsonObject();
    m_starting = false;
    emit changed();
}

void MeetingLiveModel::sendStart()
{
    if (m_link == nullptr || !m_link->connected()) {
        m_starting = false;
        m_error = tr("Daemon unavailable.");
        emit failed(QStringLiteral("start"), m_error);
        emit changed();
        return;
    }
    const QJsonObject params = m_pending;
    m_pending = QJsonObject();
    m_link->call(QStringLiteral("meetings.start"), params, [this, params](const QJsonObject &result, const QJsonObject &error) {
        m_starting = false;
        if (!error.isEmpty()) {
            m_error = gateReason(error);
            emit failed(QStringLiteral("start"), m_error);
            emit changed();
            return;
        }
        reset();
        m_id = result.value(QStringLiteral("ref")).toObject().value(QStringLiteral("id")).toString();
        m_jobId = result.value(QStringLiteral("job")).toObject().value(QStringLiteral("job_id")).toString();
        m_startedAt = QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs);
        m_segments->setStartedAt(m_startedAt);
        // A start that carried the acknowledgement recorded it now.
        if (params.value(QStringLiteral("acknowledge_meeting_disclosure")).toBool(false) || m_disclosureAt.isEmpty())
            m_disclosureAt = QDateTime::currentDateTime().toString(QStringLiteral("HH:mm"));
        m_disclosureAcknowledged = true;
        setState(QStringLiteral("recording"));
        m_clock.start();
        emit disclosureChanged();
        emit changed();
        readMeetingFacts();
        emit started(m_id);
    });
}

void MeetingLiveModel::stop()
{
    if (m_id.isEmpty() || m_link == nullptr || !m_link->connected()) {
        emit failed(QStringLiteral("stop"), tr("No meeting is recording."));
        return;
    }
    flushNotes();
    const QString id = m_id;
    // The screen acknowledges the stop now: the square goes muted, the
    // clock freezes and Stop cannot be pressed twice. The daemon's
    // refusal, if any, releases it.
    m_error.clear();
    m_stopRequested = true;
    m_clock.stop();
    m_micLevel = m_micPeak = m_systemLevel = m_systemPeak = 0;
    emit levelsChanged();
    emit changed();
    m_link->call(QStringLiteral("meetings.stop"), {{QStringLiteral("meeting_id"), id}}, [this, id](const QJsonObject &result, const QJsonObject &error) {
        if (id != m_id)
            return;
        if (!error.isEmpty()) {
            m_stopRequested = false;
            if (m_state == QStringLiteral("recording"))
                m_clock.start();
            m_error = message(error);
            emit failed(QStringLiteral("stop"), m_error);
            emit changed();
            return;
        }
        m_jobId = result.value(QStringLiteral("job")).toObject().value(QStringLiteral("job_id")).toString(m_jobId);
        if (m_state == QStringLiteral("recording"))
            setState(QStringLiteral("stopping"));
    });
}

void MeetingLiveModel::setNotes(const QString &markdown)
{
    if (markdown == m_notes || m_id.isEmpty())
        return;
    m_notes = markdown;
    m_notesState.clear();
    m_saver->edit(m_id, markdown);
    emit notesChanged();
}

void MeetingLiveModel::flushNotes()
{
    m_saver->flush();
}

void MeetingLiveModel::loadDisclosure()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    m_link->call(QStringLiteral("meetings.disclosure.get"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applyDisclosure(result);
    });
}

void MeetingLiveModel::applyDisclosure(const QJsonObject &result, const QDate &today)
{
    m_disclosureAcknowledged = result.value(QStringLiteral("acknowledged")).toBool(false);
    const QString at = result.value(QStringLiteral("acknowledged_at")).toString();
    const QDateTime when = QDateTime::fromString(at, Qt::ISODateWithMs);
    if (!when.isValid())
        m_disclosureAt.clear();
    else if (when.toLocalTime().date() == today)
        m_disclosureAt = when.toLocalTime().toString(QStringLiteral("HH:mm"));
    else
        m_disclosureAt = format::clock(when.toLocalTime());
    const QString text = result.value(QStringLiteral("message")).toString();
    if (!text.isEmpty())
        m_disclosureMessage = text;
    emit disclosureChanged();
}

void MeetingLiveModel::copyDisclosure()
{
    if (QClipboard *clipboard = QGuiApplication::clipboard(); clipboard != nullptr && !m_disclosureMessage.isEmpty())
        clipboard->setText(m_disclosureMessage);
}

void MeetingLiveModel::readDevices()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    m_link->call(QStringLiteral("audio.devices"), {}, [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applyDevices(result);
    });
}

void MeetingLiveModel::applyDevices(const QJsonObject &result)
{
    const QString pinned = result.value(QStringLiteral("pinned")).toString();
    const QString source = pinned.isEmpty() ? result.value(QStringLiteral("default_source")).toString() : pinned;
    const QString sink = result.value(QStringLiteral("default_sink")).toString();
    QString mic, system;
    for (const QJsonValue &d : result.value(QStringLiteral("devices")).toArray()) {
        const QJsonObject device = d.toObject();
        const QString name = device.value(QStringLiteral("name")).toString();
        if (name == source)
            mic = device.value(QStringLiteral("description")).toString();
        if (name == sink)
            system = device.value(QStringLiteral("description")).toString();
    }
    const bool pipewire = result.value(QStringLiteral("pipewire")).toBool(false);
    m_micDevice = mic.isEmpty() ? (pipewire ? tr("no input") : tr("no PipeWire")) : mic;
    m_systemDevice = system.isEmpty() ? (pipewire ? tr("no default sink") : tr("no PipeWire")) : tr("monitor of %1").arg(system);
    emit changed();
}

void MeetingLiveModel::reset()
{
    ++m_reconcileGeneration;
    m_attachingId.clear();
    m_clock.stop();
    m_saver->leave();
    m_id.clear();
    m_jobId.clear();
    m_startedAt.clear();
    m_engineLabel.clear();
    m_notes.clear();
    m_notesState.clear();
    m_elapsedMs = 0;
    m_chunksDone = 0;
    m_chunksTotal = 0;
    m_micLevel = m_micPeak = m_systemLevel = m_systemPeak = 0;
    m_segments->clear();
    m_segments->setStartedAt(QString());
    m_state = QStringLiteral("idle");
    m_stopRequested = false;
    m_sample = false;
    emit elapsedChanged();
    emit levelsChanged();
    emit progressChanged();
    emit notesChanged();
    emit changed();
}

void MeetingLiveModel::setState(const QString &state)
{
    if (state == m_state)
        return;
    m_state = state;
    if (state != QStringLiteral("recording")) {
        // The elapsed display keeps the recorded length once the
        // capture ended; a running clock would say the recording goes on.
        m_clock.stop();
        m_micLevel = m_micPeak = m_systemLevel = m_systemPeak = 0;
        emit levelsChanged();
    }
    emit changed();
}

void MeetingLiveModel::tick()
{
    if (m_sample)
        return;
    const QDateTime started = QDateTime::fromString(m_startedAt, Qt::ISODateWithMs);
    if (!started.isValid())
        return;
    const qint64 ms = started.msecsTo(QDateTime::currentDateTimeUtc());
    if (ms / 1000 == m_elapsedMs / 1000)
        return;
    m_elapsedMs = ms;
    emit elapsedChanged();
}

void MeetingLiveModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic == QStringLiteral("audio.level")) {
        if (!recording())
            return;
        const double rms = payload.value(QStringLiteral("rms")).toDouble();
        const double peak = payload.value(QStringLiteral("peak")).toDouble();
        const double level = qBound(0.0, rms * 3.0 + peak * 0.2, 1.0);
        if (payload.value(QStringLiteral("source")).toString() == QStringLiteral("system")) {
            m_systemLevel = level;
            m_systemPeak = qMax(peak, m_systemPeak * 0.92);
        } else {
            m_micLevel = level;
            m_micPeak = qMax(peak, m_micPeak * 0.92);
        }
        emit levelsChanged();
        return;
    }
    if (topic == QStringLiteral("meeting.segment")) {
        if (payload.value(QStringLiteral("meeting_id")).toString() == m_id)
            m_segments->apply(payload);
        return;
    }
    if (topic == QStringLiteral("job.progress")) {
        if (m_jobId.isEmpty() || payload.value(QStringLiteral("job_id")).toString() != m_jobId)
            return;
        m_chunksDone = payload.value(QStringLiteral("chunks_done")).toInt(m_chunksDone);
        m_chunksTotal = payload.value(QStringLiteral("chunks_total")).toInt(m_chunksTotal);
        emit progressChanged();
        return;
    }
    if (topic != QStringLiteral("meeting.state"))
        return;
    const QString id = payload.value(QStringLiteral("meeting_id")).toString();
    const QString state = payload.value(QStringLiteral("state")).toString();
    if (!id.isEmpty() && id == m_attachingId) {
        ++m_reconcileGeneration;
        m_attachingId.clear();
        if (id != m_id) {
            if (state == QStringLiteral("recording") || state == QStringLiteral("stopping")
                || state == QStringLiteral("stopped") || state == QStringLiteral("transcribing"))
                attach(id);
            return;
        }
    }
    if (state == QStringLiteral("recording") && !id.isEmpty() && id != m_id) {
        attach(id);
        return;
    }
    if (id.isEmpty() || id != m_id)
        return;
    ++m_reconcileGeneration;
    if (state == QStringLiteral("completed") || state == QStringLiteral("partial")) {
        const QString done = m_id;
        reset();
        emit completed(done);
        return;
    }
    if (state == QStringLiteral("cancelled")) {
        reset();
        return;
    }
    if (state == QStringLiteral("failed")) {
        m_error = payload.value(QStringLiteral("reason")).toString(tr("the meeting failed"));
        emit failed(QStringLiteral("meeting"), m_error);
        reset();
        return;
    }
    setState(state);
}

void MeetingLiveModel::applySample(const QJsonObject &facts, const QJsonArray &segments)
{
    reset();
    m_sample = true;
    m_id = facts.value(QStringLiteral("meeting_id")).toString();
    m_title = facts.value(QStringLiteral("title")).toString();
    m_startedAt = facts.value(QStringLiteral("started_at")).toString();
    m_engineLabel = facts.value(QStringLiteral("engine")).toString();
    m_micDevice = facts.value(QStringLiteral("mic_device")).toString();
    m_systemDevice = facts.value(QStringLiteral("system_device")).toString();
    m_systemAudio = true;
    m_elapsedMs = qint64(facts.value(QStringLiteral("elapsed_ms")).toDouble());
    m_micLevel = facts.value(QStringLiteral("mic_level")).toDouble();
    m_micPeak = facts.value(QStringLiteral("mic_peak")).toDouble();
    m_systemLevel = facts.value(QStringLiteral("system_level")).toDouble();
    m_systemPeak = facts.value(QStringLiteral("system_peak")).toDouble();
    m_notes = facts.value(QStringLiteral("notes")).toString();
    m_notesState = tr("Saved");
    m_state = facts.value(QStringLiteral("state")).toString(QStringLiteral("recording"));
    m_segments->setStartedAt(m_startedAt);
    m_segments->applyAll(segments);
    emit elapsedChanged();
    emit levelsChanged();
    emit notesChanged();
    emit changed();
}

}  // namespace dettivo
