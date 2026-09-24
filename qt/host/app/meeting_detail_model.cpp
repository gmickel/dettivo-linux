#include "meeting_detail_model.h"
#include "meeting_format.h"
#include "status_format.h"

#include <QDateTime>
#include <QVariantMap>

namespace dettivo {

namespace {

QString message(const QJsonObject &error)
{
    const QString text = error.value(QStringLiteral("message")).toString();
    return text.isEmpty() ? QObject::tr("the daemon did not answer") : text;
}

QString clockOf(const QString &iso)
{
    const QDateTime when = QDateTime::fromString(iso, Qt::ISODateWithMs);
    return when.isValid() ? when.toLocalTime().toString(QStringLiteral("HH:mm")) : QString();
}

}  // namespace

MeetingDetailModel::MeetingDetailModel(DaemonLink *link, QObject *parent)
    : QObject(parent), m_link(link), m_saver(new NotesSaver(link, QStringLiteral("user"), this))
{
    connect(m_saver, &NotesSaver::stateChanged, this, [this]() {
        if (!m_saver->dirty() && m_saver->meetingId() == m_id) {
            m_notesSource = QStringLiteral("user");
            m_notesUpdatedAt = m_saver->updatedAt().isEmpty() ? m_notesUpdatedAt : m_saver->updatedAt();
        }
        emit notesChanged();
    });
    connect(m_saver, &NotesSaver::failed, this, [this](const QString &reason) { emit failed(QStringLiteral("notes"), reason); });
    connect(this, &MeetingDetailModel::changed, this, &MeetingDetailModel::stagesChanged);
    connect(this, &MeetingDetailModel::progressChanged, this, &MeetingDetailModel::stagesChanged);
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &MeetingDetailModel::handleNotification);
        connect(m_link, &DaemonLink::connectedChanged, this, [this](bool connected) {
            if (connected && !m_id.isEmpty())
                reload();
        });
    }
}

void MeetingDetailModel::load(const QString &id)
{
    if (id.isEmpty()) {
        clear();
        return;
    }
    if (id != m_id) {
        clear();
        m_id = id;
        // A draft the daemon still owes an answer for is the editor's text.
        const QString draft = m_saver->open(id);
        if (!draft.isNull()) {
            m_notes = draft;
            emit notesChanged();
        }
    }
    reload();
}

void MeetingDetailModel::reload()
{
    if (m_id.isEmpty() || m_link == nullptr || !m_link->connected())
        return;
    const QString id = m_id;
    const quint64 generation = ++m_generation;
    m_loading = true;
    m_error.clear();
    emit changed();
    m_link->call(QStringLiteral("meetings.get"), {{QStringLiteral("meeting_id"), id}}, [this, id, generation](const QJsonObject &result, const QJsonObject &error) {
        if (id != m_id || generation != m_generation)
            return;
        m_loading = false;
        if (!error.isEmpty()) {
            m_error = message(error);
            m_loaded = false;
            emit changed();
            return;
        }
        apply(id, result);
    });
}

void MeetingDetailModel::clear()
{
    ++m_generation;
    m_saver->leave();
    m_id.clear();
    m_error.clear();
    m_title.clear();
    m_status.clear();
    m_startedAt.clear();
    m_endedAt.clear();
    m_language.clear();
    m_provider.clear();
    m_model.clear();
    m_notes.clear();
    m_notesSource.clear();
    m_notesUpdatedAt.clear();
    m_analysisStatus.clear();
    m_summary.clear();
    m_analysisModel.clear();
    m_analysisAt.clear();
    m_analysisError.clear();
    m_diarizationStatus.clear();
    m_diarizationError.clear();
    m_diarizationEngine.clear();
    m_stage.clear();
    m_jobId.clear();
    m_decisions.clear();
    m_actionItems.clear();
    m_speakers.clear();
    m_segments.clear();
    m_durationMs = 0;
    m_progress = -1;
    m_chunksCompleted = m_chunksTotal = m_microphoneTakes = 0;
    m_unassignedSegments = 0;
    m_loaded = m_loading = m_partial = m_hasPolished = false;
    m_systemAudio = m_audioKept = false;
    emit changed();
    emit notesChanged();
    emit progressChanged();
}

void MeetingDetailModel::apply(const QString &id, const QJsonObject &result)
{
    m_id = id;
    m_loading = false;
    m_error.clear();
    m_title = result.value(QStringLiteral("title")).toString();
    m_status = result.value(QStringLiteral("status")).toString();
    m_partial = result.value(QStringLiteral("is_partial")).toBool(false) || m_status == QStringLiteral("partial");
    m_startedAt = result.value(QStringLiteral("started_at")).toString();
    m_endedAt = result.value(QStringLiteral("ended_at")).toString();
    m_language = result.value(QStringLiteral("language")).toString();
    m_provider = result.value(QStringLiteral("stt_provider_id")).toString();
    m_model = format::modelName(result.value(QStringLiteral("stt_model_id")).toString());
    m_durationMs = result.contains(QStringLiteral("duration_ms"))
        ? qint64(result.value(QStringLiteral("duration_ms")).toDouble())
        : qint64(result.value(QStringLiteral("duration_seconds")).toDouble() * 1000.0);
    m_chunksCompleted = result.value(QStringLiteral("chunks_completed")).toInt(0);
    m_chunksTotal = result.value(QStringLiteral("chunks_total")).toInt(0);
    m_microphoneTakes = result.value(QStringLiteral("microphone_takes")).toInt(1);
    m_systemAudio = result.value(QStringLiteral("system_audio")).toBool(false);
    m_audioKept = result.contains(QStringLiteral("audio_dir")) ? !result.value(QStringLiteral("audio_dir")).toString().isEmpty()
                                                              : result.value(QStringLiteral("audio_kept")).toBool(true);
    // A draft the daemon has not acknowledged outranks the row's notes.
    if (!m_saver->dirty())
        m_notes = result.value(QStringLiteral("notes")).toString();
    m_notesSource = result.value(QStringLiteral("notes_source")).toString();
    m_notesUpdatedAt = result.value(QStringLiteral("notes_updated_at")).toString();
    m_analysisStatus = result.value(QStringLiteral("analysis_status")).toString(QStringLiteral("none"));
    m_analysisError = result.value(QStringLiteral("analysis_error")).toString();
    m_analysisModel = format::modelName(result.value(QStringLiteral("analysis_model")).toString());
    m_analysisAt = result.value(QStringLiteral("analyzed_at")).toString(result.value(QStringLiteral("analysis_at")).toString());
    applyAnalysis(result.value(QStringLiteral("analysis")).toObject());
    const QJsonObject diarization = result.value(QStringLiteral("diarization")).toObject();
    m_diarizationStatus = diarization.value(QStringLiteral("status")).toString();
    m_diarizationError = diarization.value(QStringLiteral("error")).toString();
    m_diarizationEngine = diarization.value(QStringLiteral("engine")).toString();
    applySpeakers(result.value(QStringLiteral("speakers")).toArray());
    applySegments(result.value(QStringLiteral("segments")).toArray());
    if (m_speakers.isEmpty() && !m_segments.isEmpty())
        applySpeakers(speakersBySource(result.value(QStringLiteral("segments")).toArray()));
    m_loaded = true;
    if (m_progress >= 0 && m_analysisStatus != QStringLiteral("running") && m_diarizationStatus != QStringLiteral("running")) {
        m_progress = -1;
        m_stage.clear();
        m_jobId.clear();
        emit progressChanged();
    }
    if (m_jobId.isEmpty() && (m_diarizationStatus == QStringLiteral("running") || m_diarizationStatus == QStringLiteral("queued")))
        bindPassJob();
    emit changed();
    emit notesChanged();
}

// Until the speaker pass ran, the bar shows the two sources with the
// time each side spoke, summed over its segments.
QJsonArray MeetingDetailModel::speakersBySource(const QJsonArray &segments)
{
    qint64 you = 0;
    qint64 remote = 0;
    for (const QJsonValue &v : segments) {
        const QJsonObject s = v.toObject();
        const qint64 span = qint64(s.value(QStringLiteral("end_ms")).toDouble() - s.value(QStringLiteral("start_ms")).toDouble());
        if (span <= 0)
            continue;
        if (s.value(QStringLiteral("source_type")).toString() == QStringLiteral("microphone"))
            you += span;
        else
            remote += span;
    }
    QJsonArray out;
    if (you > 0 || remote == 0)
        out.append(QJsonObject{{QStringLiteral("speaker_id"), QStringLiteral("you")}, {QStringLiteral("name"), tr("You")}, {QStringLiteral("color_index"), 0}, {QStringLiteral("talk_ms"), double(you)}});
    if (remote > 0)
        out.append(QJsonObject{{QStringLiteral("speaker_id"), QStringLiteral("remote")}, {QStringLiteral("name"), tr("Remote")}, {QStringLiteral("color_index"), 1}, {QStringLiteral("talk_ms"), double(remote)}});
    return out;
}

// The speakers come in swatch order with `You` first; a meeting whose
// pass has not run (or a room-audio meeting) shows its two sources.
void MeetingDetailModel::applySpeakers(const QJsonArray &speakers)
{
    m_speakers.clear();
    qint64 total = 0;
    for (const QJsonValue &v : speakers)
        total += qint64(v.toObject().value(QStringLiteral("talk_ms")).toDouble());
    for (const QJsonValue &v : speakers) {
        const QJsonObject s = v.toObject();
        const qint64 talk = qint64(s.value(QStringLiteral("talk_ms")).toDouble());
        const double share = total > 0 ? double(talk) / double(total) : 0.0;
        m_speakers.append(QVariantMap{{QStringLiteral("speakerId"), s.value(QStringLiteral("speaker_id")).toString()},
                                      {QStringLiteral("name"), s.value(QStringLiteral("name")).toString()},
                                      {QStringLiteral("colorIndex"), s.value(QStringLiteral("color_index")).toInt(0)},
                                      {QStringLiteral("talkMs"), talk},
                                      {QStringLiteral("talk"), meeting_format::talk(talk)},
                                      {QStringLiteral("share"), share},
                                      {QStringLiteral("shareText"), meeting_format::percent(share)}});
    }
}

void MeetingDetailModel::applySegments(const QJsonArray &segments)
{
    m_segments.clear();
    m_hasPolished = false;
    m_unassignedSegments = 0;
    QHash<QString, int> colors;
    for (const QVariant &v : m_speakers) {
        const QVariantMap s = v.toMap();
        colors.insert(s.value(QStringLiteral("speakerId")).toString(), s.value(QStringLiteral("colorIndex")).toInt());
    }
    for (const QJsonValue &v : segments) {
        const QJsonObject s = v.toObject();
        const QString source = s.value(QStringLiteral("source_type")).toString();
        const bool you = source == QStringLiteral("microphone");
        QString speakerId = s.value(QStringLiteral("speaker_id")).toString();
        QString speaker = s.value(QStringLiteral("speaker")).toString();
        if (speaker.isEmpty())
            speaker = you ? tr("You") : tr("Remote");
        if (speakerId.isEmpty()) {
            m_unassignedSegments += 1;
            speakerId = you ? QStringLiteral("you") : QStringLiteral("remote");
        }
        const int color = colors.contains(speakerId) ? colors.value(speakerId) : (you ? 0 : 1);
        const QString polished = s.value(QStringLiteral("polished_text")).toString();
        if (!polished.isEmpty())
            m_hasPolished = true;
        const qint64 start = qint64(s.value(QStringLiteral("start_ms")).toDouble());
        m_segments.append(QVariantMap{{QStringLiteral("time"), meeting_format::clockAt(m_startedAt, start)},
                                      {QStringLiteral("speaker"), speaker},
                                      {QStringLiteral("speakerId"), speakerId},
                                      {QStringLiteral("colorIndex"), color},
                                      {QStringLiteral("text"), s.value(QStringLiteral("text")).toString()},
                                      {QStringLiteral("polished"), polished.isEmpty() ? s.value(QStringLiteral("text")).toString() : polished},
                                      {QStringLiteral("gapBefore"), qint64(s.value(QStringLiteral("gap_before_ms")).toDouble(0))},
                                      {QStringLiteral("startMs"), start}});
    }
}

void MeetingDetailModel::applyAnalysis(const QJsonObject &analysis)
{
    m_summary = analysis.value(QStringLiteral("summary")).toString();
    m_decisions.clear();
    for (const QJsonValue &d : analysis.value(QStringLiteral("decisions")).toArray())
        m_decisions.append(d.toString());
    m_actionItems.clear();
    for (const QJsonValue &a : analysis.value(QStringLiteral("action_items")).toArray()) {
        const QJsonObject item = a.toObject();
        QString line = item.value(QStringLiteral("text")).toString();
        const QString owner = item.value(QStringLiteral("owner")).toString();
        const QString due = item.value(QStringLiteral("due")).toString();
        if (!owner.isEmpty())
            line = QStringLiteral("%1: %2").arg(owner, line);
        if (!due.isEmpty())
            line = tr("%1 · %2").arg(line, due);
        m_actionItems.append(line);
    }
}

QString MeetingDetailModel::factsLine() const
{
    QStringList parts;
    const QString day = meeting_format::weekdayDate(m_startedAt);
    if (!day.isEmpty())
        parts.append(day);
    const QString span = meeting_format::span(m_startedAt, m_endedAt);
    if (!span.isEmpty())
        parts.append(span);
    if (m_durationMs > 0)
        parts.append(meeting_format::minutes(m_durationMs));
    if (!m_provider.isEmpty())
        parts.append(m_model.isEmpty() ? m_provider : QStringLiteral("%1 %2").arg(m_provider, m_model));
    if (m_diarizationStatus == QStringLiteral("ready"))
        parts.append(tr("diarized"));
    else if (m_diarizationStatus == QStringLiteral("running"))
        parts.append(tr("diarizing"));
    if (m_analysisStatus == QStringLiteral("ready"))
        parts.append(tr("analysed"));
    if (m_status == QStringLiteral("transcribing"))
        parts.append(tr("transcribing"));
    else if (m_status == QStringLiteral("failed"))
        parts.append(tr("failed"));
    return parts.join(QStringLiteral(" · "));
}

QString MeetingDetailModel::partialLine() const
{
    if (!m_partial)
        return QString();
    if (m_chunksTotal > 0)
        return tr("Recovered after a crash · %1 of %2 chunks").arg(m_chunksCompleted).arg(m_chunksTotal);
    return tr("Recovered after a crash");
}

QString MeetingDetailModel::notesMeta() const
{
    if (m_notes.trimmed().isEmpty())
        return tr("none yet");
    int sections = 0;
    for (const QString &line : m_notes.split(QLatin1Char('\n')))
        sections += line.startsWith(QLatin1Char('#')) ? 1 : 0;
    QStringList parts;
    if (sections > 0)
        parts.append(sections == 1 ? tr("1 section") : tr("%1 sections").arg(sections));
    else
        parts.append(tr("%1 lines").arg(m_notes.split(QLatin1Char('\n'), Qt::SkipEmptyParts).size()));
    const QString at = clockOf(m_notesUpdatedAt);
    if (!at.isEmpty())
        parts.append(m_notesSource == QStringLiteral("live") ? tr("live · %1").arg(at) : tr("edited %1").arg(at));
    return parts.join(QStringLiteral(" · "));
}

QString MeetingDetailModel::analysisMeta() const
{
    if (m_analysisStatus != QStringLiteral("ready"))
        return QString();
    QStringList parts;
    if (!m_analysisModel.isEmpty())
        parts.append(m_analysisModel);
    const QString at = clockOf(m_analysisAt);
    if (!at.isEmpty())
        parts.append(at);
    return parts.join(QStringLiteral(" · "));
}

QString MeetingDetailModel::diarizationLine() const
{
    if (m_diarizationStatus == QStringLiteral("ready"))
        return m_diarizationEngine.isEmpty() ? tr("speakers assigned") : tr("speakers by %1").arg(m_diarizationEngine);
    if (m_diarizationStatus == QStringLiteral("running"))
        return tr("the speaker pass is running");
    if (m_diarizationStatus == QStringLiteral("queued"))
        return tr("the speaker pass starts next");
    if (m_diarizationStatus == QStringLiteral("unavailable"))
        return tr("no speaker model; run dettivo speech download --provider diarize --model diarization");
    if (m_diarizationStatus == QStringLiteral("failed"))
        return m_diarizationError.isEmpty() ? tr("the speaker pass failed") : tr("speaker pass failed: %1").arg(m_diarizationError);
    return tr("speakers by source until the pass runs");
}

QString MeetingDetailModel::audioFacts() const
{
    QStringList parts;
    parts.append(m_systemAudio ? tr("2 tracks") : tr("1 track"));
    if (m_durationMs > 0)
        parts.append(meeting_format::minutes(m_durationMs));
    parts.append(m_audioKept ? tr("kept") : tr("removed"));
    return parts.join(QStringLiteral(" · "));
}

QString MeetingDetailModel::exportFacts() const
{
    return QStringLiteral("md · txt · srt · vtt · json");
}

QString MeetingDetailModel::lengthText() const
{
    return meeting_format::minutes(m_durationMs);
}

QVariantMap MeetingDetailModel::speakerAt(int index) const
{
    return index >= 0 && index < m_speakers.size() ? m_speakers.at(index).toMap() : QVariantMap();
}

void MeetingDetailModel::applyRename(const QString &speakerId, const QString &name)
{
    for (QVariant &v : m_speakers) {
        QVariantMap s = v.toMap();
        if (s.value(QStringLiteral("speakerId")).toString() == speakerId) {
            s.insert(QStringLiteral("name"), name);
            v = s;
        }
    }
    for (QVariant &v : m_segments) {
        QVariantMap s = v.toMap();
        if (s.value(QStringLiteral("speakerId")).toString() == speakerId) {
            s.insert(QStringLiteral("speaker"), name);
            v = s;
        }
    }
    emit changed();
}

void MeetingDetailModel::applyTitle(const QString &title)
{
    if (title == m_title)
        return;
    m_title = title;
    emit changed();
}

void MeetingDetailModel::setNotes(const QString &markdown)
{
    if (markdown == m_notes || m_id.isEmpty())
        return;
    m_notes = markdown;
    m_saver->edit(m_id, markdown);
    emit notesChanged();
}

void MeetingDetailModel::flushNotes()
{
    m_saver->flush();
}

void MeetingDetailModel::setLastTab(const QString &tab)
{
    const QString next = tab == QStringLiteral("notes") || tab == QStringLiteral("analysis") ? tab : QStringLiteral("transcript");
    if (next == m_lastTab)
        return;
    m_lastTab = next;
    emit lastTabChanged();
}

void MeetingDetailModel::trackJob(const QString &jobId, const QString &stage)
{
    m_jobId = jobId;
    m_stage = stage;
    m_progress = 0;
    if (stage == QStringLiteral("analyzing"))
        m_analysisStatus = QStringLiteral("running");
    else if (stage == QStringLiteral("diarizing"))
        m_diarizationStatus = QStringLiteral("running");
    emit progressChanged();
    emit changed();
}

void MeetingDetailModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (m_id.isEmpty())
        return;
    if (topic == QStringLiteral("job.progress")) {
        // Only the job this screen asked for or bound through
        // `meetings.status` is followed; another meeting's pass reports
        // under a job id this model never learned.
        if (m_jobId.isEmpty() || payload.value(QStringLiteral("job_id")).toString() != m_jobId)
            return;
        m_stage = payload.value(QStringLiteral("stage")).toString(m_stage);
        const bool ended = m_stage == QStringLiteral("done") || m_stage == QStringLiteral("failed")
            || m_stage == QStringLiteral("cancelled");
        m_progress = ended ? -1 : qBound(0.0, payload.value(QStringLiteral("progress")).toDouble(), 1.0);
        emit progressChanged();
        if (ended) {
            m_jobId.clear();
            reload();
        }
        return;
    }
    if (topic != QStringLiteral("meeting.state") || payload.value(QStringLiteral("meeting_id")).toString() != m_id)
        return;
    // Every transition of this meeting (the finalisation, the speaker
    // pass, the analysis) is read back whole from the row.
    reload();
}

}  // namespace dettivo
