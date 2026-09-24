// The post-processing stages of a meeting (fn-63, ADR 0061): the
// transcript, the speaker pass and the analysis as the strip over the
// detail draws them, from the facts `meetings.get` and the two topics
// already put on the model. A stage is `pending` while nothing has
// decided it, `queued` once the daemon planned it, `running`, `done`,
// `failed`, `unavailable` when the pass was wanted and the model was not
// there, or `skipped` when the meeting settled without it on purpose.
#include "meeting_detail_model.h"

#include <QVariantMap>

namespace dettivo {

namespace {

QVariantMap stageRow(const QString &key, const QString &label, const QString &state, const QString &detail, double progress = -1)
{
    return {{QStringLiteral("key"), key},       {QStringLiteral("label"), label},
            {QStringLiteral("state"), state},   {QStringLiteral("detail"), detail},
            {QStringLiteral("progress"), progress}};
}

bool settled(const QString &state)
{
    return state == QStringLiteral("done") || state == QStringLiteral("failed") || state == QStringLiteral("skipped")
        || state == QStringLiteral("unavailable");
}

}  // namespace

double MeetingDetailModel::stageProgress(const QString &stage) const
{
    return m_stage == stage && m_progress > 0 ? m_progress : -1;
}

// `job.progress` names no meeting, so the automatic pass's job is read
// from `meetings.status`, which answers this meeting's running
// diarization job; until it answers, the stage stays indeterminate and
// no other meeting's chunks are shown.
void MeetingDetailModel::bindPassJob()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    const QString id = m_id;
    const quint64 generation = m_generation;
    m_link->call(QStringLiteral("meetings.status"), {{QStringLiteral("meeting_id"), id}}, [this, id, generation](const QJsonObject &result, const QJsonObject &error) {
        if (id != m_id || generation != m_generation || !error.isEmpty() || !m_jobId.isEmpty())
            return;
        const QJsonObject job = result.value(QStringLiteral("job")).toObject();
        const QString jobId = job.value(QStringLiteral("job_id")).toString();
        if (!jobId.startsWith(QStringLiteral("job_diarize_")) || job.value(QStringLiteral("state")).toString() != QStringLiteral("running"))
            return;
        m_jobId = jobId;
        m_stage = QStringLiteral("diarizing");
        emit progressChanged();
    });
}

QVariantMap MeetingDetailModel::transcriptStage() const
{
    const QString label = tr("Transcript");
    const QString key = QStringLiteral("transcript");
    if (m_partial)
        return stageRow(key, label, QStringLiteral("failed"), partialLine());
    if (m_status == QStringLiteral("completed")) {
        const int n = int(m_segments.size());
        return stageRow(key, label, QStringLiteral("done"), n == 1 ? tr("1 segment") : tr("%1 segments").arg(n));
    }
    if (m_status == QStringLiteral("transcribing")) {
        const QString detail = m_chunksTotal > 0 ? tr("%1 of %2 chunks").arg(m_chunksCompleted).arg(m_chunksTotal) : tr("building the transcript");
        const double progress = m_chunksTotal > 0 ? double(m_chunksCompleted) / double(m_chunksTotal) : -1;
        return stageRow(key, label, QStringLiteral("running"), detail, progress);
    }
    if (m_status == QStringLiteral("failed"))
        return stageRow(key, label, QStringLiteral("failed"), tr("the transcription failed"));
    if (m_status == QStringLiteral("cancelled"))
        return stageRow(key, label, QStringLiteral("skipped"), tr("cancelled"));
    if (m_status == QStringLiteral("stopped"))
        return stageRow(key, label, QStringLiteral("queued"), tr("the recording ended"));
    return stageRow(key, label, QStringLiteral("queued"), tr("still recording"));
}

QVariantMap MeetingDetailModel::speakersStage() const
{
    const QString label = tr("Speakers");
    const QString key = QStringLiteral("speakers");
    const QString s = m_diarizationStatus;
    if (s == QStringLiteral("ready")) {
        // The pass assigns a speaker only under the coverage and share
        // rule (ADR 0035); the rest stays labelled by source, and the
        // count says how much of the transcript that is.
        const int n = int(m_speakers.size());
        QString detail = n == 1 ? tr("1 speaker") : tr("%1 speakers").arg(n);
        if (m_unassignedSegments == 1)
            detail = tr("%1 · 1 segment unassigned").arg(detail);
        else if (m_unassignedSegments > 1)
            detail = tr("%1 · %2 segments unassigned").arg(detail).arg(m_unassignedSegments);
        return stageRow(key, label, QStringLiteral("done"), detail);
    }
    if (s == QStringLiteral("running"))
        return stageRow(key, label, QStringLiteral("running"), tr("identifying who spoke"), stageProgress(QStringLiteral("diarizing")));
    if (s == QStringLiteral("queued"))
        return stageRow(key, label, QStringLiteral("queued"), tr("starts next"));
    if (s == QStringLiteral("failed"))
        return stageRow(key, label, QStringLiteral("failed"), m_diarizationError.isEmpty() ? tr("the speaker pass failed") : m_diarizationError);
    if (s == QStringLiteral("unavailable"))
        return stageRow(key, label, QStringLiteral("unavailable"), tr("no speaker model downloaded"));
    if (m_status == QStringLiteral("completed") && !m_partial)
        return stageRow(key, label, QStringLiteral("skipped"), tr("not run"));
    return stageRow(key, label, QStringLiteral("pending"), tr("after the transcript"));
}

QVariantMap MeetingDetailModel::analysisStage() const
{
    const QString label = tr("Analysis");
    const QString key = QStringLiteral("analysis");
    const QString s = m_analysisStatus;
    if (s == QStringLiteral("ready"))
        return stageRow(key, label, QStringLiteral("done"), analysisMeta().isEmpty() ? tr("summary ready") : analysisMeta());
    if (s == QStringLiteral("running"))
        return stageRow(key, label, QStringLiteral("running"), tr("reading the transcript"), stageProgress(QStringLiteral("analyzing")));
    if (s == QStringLiteral("queued"))
        return stageRow(key, label, QStringLiteral("queued"), tr("after the speakers"));
    if (s == QStringLiteral("failed"))
        return stageRow(key, label, QStringLiteral("failed"), m_analysisError.isEmpty() ? tr("the analysis failed") : m_analysisError);
    if (m_status == QStringLiteral("completed") && !m_partial)
        return stageRow(key, label, QStringLiteral("skipped"), tr("not run"));
    return stageRow(key, label, QStringLiteral("pending"), tr("after the transcript"));
}

QVariantList MeetingDetailModel::stages() const
{
    if (!m_loaded)
        return {};
    return {transcriptStage(), speakersStage(), analysisStage()};
}

// `running` while a stage is still to come; once every stage settled,
// `failed` when one failed, `incomplete` when a wanted pass had no model
// to run on, `done` otherwise (a pass switched off on purpose is done).
QString MeetingDetailModel::processingState() const
{
    bool failed = false;
    bool unavailable = false;
    for (const QVariant &v : stages()) {
        const QString state = v.toMap().value(QStringLiteral("state")).toString();
        if (!settled(state))
            return QStringLiteral("running");
        failed = failed || state == QStringLiteral("failed");
        unavailable = unavailable || state == QStringLiteral("unavailable");
    }
    if (failed)
        return QStringLiteral("failed");
    return unavailable ? QStringLiteral("incomplete") : QStringLiteral("done");
}

// `Transcript ready · speakers running`, `Speakers ready · analysis
// queued`, `Processing complete`, `Processing complete · analysis not
// run`, `Processing incomplete · no speaker model downloaded`,
// `Processing finished · analysis failed`.
QString MeetingDetailModel::processingLine() const
{
    const QVariantList all = stages();
    if (all.isEmpty())
        return QString();
    QString ready;
    for (const QVariant &v : all) {
        const QVariantMap s = v.toMap();
        const QString state = s.value(QStringLiteral("state")).toString();
        const QString label = s.value(QStringLiteral("label")).toString();
        if (state == QStringLiteral("done")) {
            ready = tr("%1 ready").arg(label);
            continue;
        }
        if (settled(state))
            continue;
        QString next;
        if (state == QStringLiteral("running"))
            next = tr("%1 running").arg(label.toLower());
        else if (state == QStringLiteral("queued"))
            next = tr("%1 queued").arg(label.toLower());
        else
            next = tr("%1 waits for the transcript").arg(label.toLower());
        return ready.isEmpty() ? next.at(0).toUpper() + next.mid(1) : tr("%1 · %2").arg(ready, next);
    }
    QStringList failed, unavailable, skipped;
    for (const QVariant &v : all) {
        const QVariantMap s = v.toMap();
        const QString state = s.value(QStringLiteral("state")).toString();
        const QString label = s.value(QStringLiteral("label")).toString().toLower();
        if (state == QStringLiteral("failed"))
            failed.append(label);
        else if (state == QStringLiteral("unavailable"))
            unavailable.append(tr("%1 skipped, %2").arg(label, s.value(QStringLiteral("detail")).toString()));
        else if (state == QStringLiteral("skipped"))
            skipped.append(label);
    }
    if (!failed.isEmpty())
        return tr("Processing finished · %1 failed").arg(failed.join(tr(" and ")));
    if (!unavailable.isEmpty())
        return tr("Processing incomplete · %1").arg(unavailable.join(QStringLiteral(" · ")));
    if (!skipped.isEmpty())
        return tr("Processing complete · %1 not run").arg(skipped.join(tr(" and ")));
    return tr("Processing complete");
}

}  // namespace dettivo
